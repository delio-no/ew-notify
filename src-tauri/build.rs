use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Single Windows MANIFEST via Tauri (do NOT also use embed-resource —
    // that causes CVT1100 duplicate MANIFEST / LNK1123).
    let mut windows = tauri_build::WindowsAttributes::new();
    windows = windows.app_manifest(include_str!("windows.manifest"));
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    if let Err(e) = tauri_build::try_build(attrs) {
        // bundle.resources copies WinDivert*.sys next to the exe. If the kernel
        // driver is still loaded from a previous run, Windows returns
        // ERROR_SHARING_VIOLATION (32) and tauri-build fails even though codegen
        // already finished.
        if is_sharing_violation(&e) {
            println!(
                "cargo:warning=tauri-build could not refresh a locked file: {e}"
            );
            println!(
                "cargo:warning=WinDivert driver likely still loaded — quit the app or run `sc stop WinDivert` as Admin if resources are stale"
            );
        } else {
            panic!("failed to run tauri-build: {e}");
        }
    }

    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rerun-if-changed=windows.manifest");

    let windivert_dir = manifest_dir.join("windivert");
    let target_dir = resolve_target_dir();
    println!("cargo:rerun-if-changed={}", windivert_dir.display());

    // A locked/interrupted cdylib link can leave a 0-byte DLL next to the exe.
    // Windows then fails process start with STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139).
    remove_empty_dll(&target_dir.join("ew_notify_lib.dll"));

    // Official WinDivert 2.2 files must sit next to the exe (DLL looks for *.sys beside it).
    for name in ["WinDivert.dll", "WinDivert64.sys", "WinDivert32.sys"] {
        let src = windivert_dir.join(name);
        if !src.exists() {
            println!(
                "cargo:warning=Missing {} — place WinDivert 2.2 driver files in src-tauri/windivert/",
                src.display()
            );
            continue;
        }
        let dest = target_dir.join(name);
        // Skip rewrite if already present and identical size (avoids ERROR_SHARING_VIOLATION
        // when a previous run still holds the loaded .sys/.dll).
        if dest.exists() {
            let same = fs::metadata(&src)
                .ok()
                .zip(fs::metadata(&dest).ok())
                .map(|(a, b)| a.len() == b.len())
                .unwrap_or(false);
            if same {
                continue;
            }
        }
        match fs::copy(&src, &dest) {
            Ok(_) => println!("cargo:warning=Copied {} -> {}", name, dest.display()),
            Err(e) if dest.exists() => {
                println!(
                    "cargo:warning=Could not refresh {} (in use: {}); keeping existing file",
                    name, e
                );
            }
            Err(e) => println!("cargo:warning=Failed to copy {}: {}", name, e),
        }
    }
}

fn is_sharing_violation(err: &dyn std::fmt::Display) -> bool {
    let msg = err.to_string();
    msg.contains("os error 32")
        || msg.contains("(os error 32)")
        || msg.contains("being used by another process")
        || msg.contains("занят другим процессом")
}

fn remove_empty_dll(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() == 0 {
        match fs::remove_file(path) {
            Ok(()) => println!(
                "cargo:warning=Removed empty {} (prevents STATUS_ENTRYPOINT_NOT_FOUND)",
                path.display()
            ),
            Err(e) => println!(
                "cargo:warning=Empty {} could not be removed: {}",
                path.display(),
                e
            ),
        }
    }
}

fn resolve_target_dir() -> PathBuf {
    // PROFILE=debug|release → target/<profile>/ where binaries land
    if let Ok(dir) = env::var("CARGO_TARGET_DIR") {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".into());
        return PathBuf::from(dir).join(profile);
    }

    // Fallback: walk up from OUT_DIR (.../target/<profile>/build/<crate>/out)
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_dir
        .ancestors()
        .nth(3)
        .unwrap_or(Path::new("."))
        .to_path_buf()
}
