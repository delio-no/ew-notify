use anyhow::{anyhow, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub const GITHUB_REPO: &str = "delio-no/ew-notify";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: String,
    pub asset_name: String,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    #[allow(dead_code)]
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[allow(dead_code)]
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GithubReleaseAsset>,
}

/// Clean up leftover temporary or .old executable files from previous updates.
pub fn cleanup_old_version() {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let old_exe = exe_dir.join("ew-notify.exe.old");
            if old_exe.exists() {
                if let Err(e) = fs::remove_file(&old_exe) {
                    tracing::debug!("Could not delete old executable {}: {e}", old_exe.display());
                } else {
                    tracing::info!("Cleaned up old executable {}", old_exe.display());
                }
            }
            let tmp_update = exe_dir.join("ew-notify.update.tmp");
            if tmp_update.exists() {
                let _ = fs::remove_file(&tmp_update);
            }
            let tmp_zip = exe_dir.join("ew-notify.update.zip");
            if tmp_zip.exists() {
                let _ = fs::remove_file(&tmp_zip);
            }
            let unpack_dir = exe_dir.join("ew-notify.update.extracted");
            if unpack_dir.exists() {
                let _ = fs::remove_dir_all(&unpack_dir);
            }
        }
    }
}

/// Query GitHub Releases API to check if a newer version is available.
pub async fn check_update() -> Result<UpdateInfo> {
    let client = reqwest::Client::builder()
        .user_agent("ew-notify-updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    tracing::info!("Checking for updates at {url}");

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "GitHub API error: status {} {}",
            resp.status().as_u16(),
            resp.status().canonical_reason().unwrap_or("")
        ));
    }

    let release: GithubRelease = resp.json().await?;

    let latest_tag = release.tag_name.trim();
    let latest_version_str = latest_tag.trim_start_matches('v').trim();

    let current_ver = Version::parse(APP_VERSION)
        .map_err(|e| anyhow!("Failed to parse current version '{APP_VERSION}': {e}"))?;

    let latest_ver = Version::parse(latest_version_str)
        .map_err(|e| anyhow!("Failed to parse latest tag '{latest_tag}': {e}"))?;

    let has_update = latest_ver > current_ver;

    // Pick best asset: prefer ew-notify.zip (complete portable package), fallback to any .zip, fallback to .exe
    let best_asset = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("ew-notify.zip"))
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.ends_with(".zip"))
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("ew-notify.exe"))
        });

    let (download_url, asset_name) = match best_asset {
        Some(a) => (a.browser_download_url.clone(), a.name.clone()),
        None => (String::new(), String::new()),
    };

    Ok(UpdateInfo {
        current_version: format!("v{APP_VERSION}"),
        latest_version: format!("v{latest_version_str}"),
        has_update,
        release_notes: release.body.unwrap_or_default(),
        published_at: release.published_at.unwrap_or_default(),
        download_url,
        asset_name,
    })
}

/// Download binary or archive, extract if needed, replace current executable, and relaunch.
pub async fn download_and_install_update(
    app: &AppHandle,
    download_url: &str,
    asset_name: &str,
) -> Result<()> {
    if download_url.is_empty() {
        return Err(anyhow!("Download URL is empty"));
    }

    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow!("Could not determine executable directory"))?;

    let client = reqwest::Client::builder()
        .user_agent("ew-notify-updater")
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    tracing::info!("Downloading update from {download_url}");
    let resp = client.get(download_url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "Failed to download update: HTTP status {}",
            resp.status()
        ));
    }

    let bytes = resp.bytes().await?;
    tracing::info!("Downloaded {} bytes", bytes.len());

    let is_zip = asset_name.ends_with(".zip") || download_url.ends_with(".zip");
    let new_exe_path = exe_dir.join("ew-notify.exe.new");

    if is_zip {
        // Extract ew-notify.exe from zip archive
        let zip_temp = exe_dir.join("ew-notify.update.zip");
        fs::write(&zip_temp, &bytes)?;

        let extracted_dir = exe_dir.join("ew-notify.update.extracted");
        if extracted_dir.exists() {
            let _ = fs::remove_dir_all(&extracted_dir);
        }
        fs::create_dir_all(&extracted_dir)?;

        // Try extracting via Windows tar.exe (available on Windows 10/11)
        let tar_status = std::process::Command::new("tar")
            .args(["-xf", zip_temp.to_str().unwrap(), "-C", extracted_dir.to_str().unwrap()])
            .output();

        let extracted_ok = match tar_status {
            Ok(output) if output.status.success() => true,
            _ => {
                // Fallback to powershell Expand-Archive
                let ps_cmd = format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    zip_temp.display(),
                    extracted_dir.display()
                );
                let ps_status = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &ps_cmd])
                    .status();
                matches!(ps_status, Ok(s) if s.success())
            }
        };

        let _ = fs::remove_file(&zip_temp);

        if !extracted_ok {
            let _ = fs::remove_dir_all(&extracted_dir);
            return Err(anyhow!("Failed to extract update zip archive"));
        }

        // Locate ew-notify.exe inside extracted directory (or root/subfolder)
        let found_exe = find_file_recursive(&extracted_dir, "ew-notify.exe")
            .ok_or_else(|| anyhow!("ew-notify.exe not found inside update archive"))?;

        fs::copy(&found_exe, &new_exe_path)?;

        // Ensure WinDivert DLL and drivers are present/updated in the application directory
        for dep in ["WinDivert.dll", "WinDivert64.sys", "WinDivert32.sys"] {
            if let Some(src_path) = find_file_recursive(&extracted_dir, dep) {
                let target_path = exe_dir.join(dep);
                let _ = fs::copy(&src_path, &target_path);
            }
        }

        let _ = fs::remove_dir_all(&extracted_dir);
    } else {
        // Direct .exe binary
        fs::write(&new_exe_path, &bytes)?;
    }

    if !new_exe_path.exists() {
        return Err(anyhow!("New executable was not created"));
    }

    // Windows in-place executable replacement:
    // 1. Rename running ew-notify.exe -> ew-notify.exe.old
    // 2. Rename ew-notify.exe.new -> ew-notify.exe
    let old_exe_path = exe_dir.join("ew-notify.exe.old");
    if old_exe_path.exists() {
        let _ = fs::remove_file(&old_exe_path);
    }

    tracing::info!(
        "Renaming current {} -> {}",
        current_exe.display(),
        old_exe_path.display()
    );
    fs::rename(&current_exe, &old_exe_path)?;

    tracing::info!(
        "Moving new {} -> {}",
        new_exe_path.display(),
        current_exe.display()
    );
    if let Err(e) = fs::rename(&new_exe_path, &current_exe) {
        // Rollback on error
        let _ = fs::rename(&old_exe_path, &current_exe);
        return Err(anyhow!("Failed to replace executable: {e}"));
    }

    tracing::info!("Update successfully installed! Relaunching application...");

    // Release the single instance lock in this process before spawning new one
    crate::platform::release_single_instance();

    // Spawn new process
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let exe_str = current_exe.to_string_lossy();
        let dir_str = exe_dir.to_string_lossy();

        // Use PowerShell with Start-Sleep and Start-Process:
        // 1. Waits 600ms for current process to release all handles and exit
        // 2. Uses Start-Process which invokes ShellExecuteEx (handling UAC/elevation manifest)
        // 3. Sets working directory properly
        let ps_cmd = format!(
            "Start-Sleep -Milliseconds 600; Start-Process -FilePath '{}' -WorkingDirectory '{}'",
            exe_str, dir_str
        );

        let ps_res = std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();

        if let Err(e) = ps_res {
            tracing::warn!("PowerShell relaunch failed: {e}, falling back to cmd start");
            let cmd_str = format!("ping 127.0.0.1 -n 2 >nul & start \"\" \"{}\"", exe_str);
            let _ = std::process::Command::new("cmd")
                .args(["/C", &cmd_str])
                .current_dir(exe_dir)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
    }

    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new(&current_exe)
            .current_dir(exe_dir)
            .spawn();
    }

    // Give background process a short moment to start, then exit
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    app.exit(0);

    Ok(())
}

pub(crate) fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_file_recursive(&path, filename) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.eq_ignore_ascii_case(filename))
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
    }
    None
}
