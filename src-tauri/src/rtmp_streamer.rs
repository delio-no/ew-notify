//! RTMP Video Streamer for Telegram Channel / Group Live Video Broadcasts via FFmpeg.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

const TARGET_WIDTH: usize = 1280;
const TARGET_HEIGHT: usize = 720;
const TARGET_FPS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRtmpInfo {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub rtmp_url: String,
    pub uptime_secs: u64,
}

struct RtmpSession {
    hwnd: isize,
    pid: u32,
    title: String,
    rtmp_url: String,
    start_time: Instant,
    running: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    last_errors: Arc<std::sync::Mutex<VecDeque<String>>>,
}

pub struct GlobalRtmpManager {
    current_session: Mutex<Option<RtmpSession>>,
}

static MANAGER: std::sync::OnceLock<GlobalRtmpManager> = std::sync::OnceLock::new();
static RTMP_RUNNING_FLAG: AtomicBool = AtomicBool::new(false);

fn get_manager() -> &'static GlobalRtmpManager {
    MANAGER.get_or_init(|| GlobalRtmpManager {
        current_session: Mutex::new(None),
    })
}

/// Locate FFmpeg executable on the system
pub fn find_ffmpeg() -> Option<PathBuf> {
    // 1. Next to current running executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("ffmpeg.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 2. Portable installation directory
    let portable = PathBuf::from(r"D:\ew-notify-portable\ffmpeg.exe");
    if portable.is_file() {
        return Some(portable);
    }

    // 3. Search PATH via where.exe
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = Command::new("where.exe")
            .arg("ffmpeg.exe")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = PathBuf::from(first_line.trim());
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FFmpegInfo {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

pub fn get_ffmpeg_version(path: &std::path::Path) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = Command::new(path)
            .arg("-version")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    return Some(first_line.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn check_ffmpeg_info() -> FFmpegInfo {
    if let Some(path) = find_ffmpeg() {
        let version = get_ffmpeg_version(&path);
        FFmpegInfo {
            installed: true,
            path: Some(path.to_string_lossy().to_string()),
            version,
        }
    } else {
        FFmpegInfo {
            installed: false,
            path: None,
            version: None,
        }
    }
}

/// Check if RTMP streaming is currently active
pub fn is_rtmp_running() -> bool {
    RTMP_RUNNING_FLAG.load(Ordering::Relaxed)
}

/// Get snapshot info of active RTMP stream
pub async fn get_rtmp_info() -> Option<ActiveRtmpInfo> {
    let mgr = get_manager();
    let guard = mgr.current_session.lock().await;
    guard.as_ref().map(|s| ActiveRtmpInfo {
        hwnd: s.hwnd,
        pid: s.pid,
        title: s.title.clone(),
        rtmp_url: s.rtmp_url.clone(),
        uptime_secs: s.start_time.elapsed().as_secs(),
    })
}

/// Start RTMP streaming of a target game window to Telegram Channel / Group
pub async fn start_rtmp_stream(
    hwnd: isize,
    pid: u32,
    title: String,
    rtmp_url: String,
    rtmp_key: String,
) -> Result<ActiveRtmpInfo> {
    let mgr = get_manager();
    let mut guard = mgr.current_session.lock().await;

    // If an existing RTMP session is running, stop it first
    if let Some(existing) = guard.take() {
        info!("Stopping previous RTMP session before starting new one...");
        existing.running.store(false, Ordering::SeqCst);
        let mut child_guard = existing.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    RTMP_RUNNING_FLAG.store(false, Ordering::SeqCst);

    let ffmpeg_path = find_ffmpeg()
        .ok_or_else(|| anyhow!("FFmpeg не найден на компьютере! Поместите ffmpeg.exe в папку программы."))?;
    info!("Using FFmpeg binary at: {:?}", ffmpeg_path);

    let clean_key = rtmp_key.trim();
    if clean_key.is_empty() {
        return Err(anyhow!("Ключ трансляции (Stream Key) не заполнен."));
    }

    let full_target = if clean_key.starts_with("rtmp://") || clean_key.starts_with("rtmps://") {
        clean_key.to_string()
    } else {
        let mut server = rtmp_url.trim().to_string();
        if server.is_empty() {
            server = "rtmps://live.me.t.me/live".to_string();
        }
        if !server.starts_with("rtmp://") && !server.starts_with("rtmps://") {
            server = format!("rtmps://{}", server);
        }
        let clean_server = server.trim_end_matches('/');
        format!("{}/{}", clean_server, clean_key)
    };

    let masked_target = if full_target.len() > 15 {
        format!("{}...****", &full_target[..full_target.len().saturating_sub(6)])
    } else {
        "***".to_string()
    };
    info!("Spawning FFmpeg for RTMP stream to target: {}", masked_target);

    let mut cmd = Command::new(ffmpeg_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd.args([
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "bgra",
        "-s",
        &format!("{}x{}", TARGET_WIDTH, TARGET_HEIGHT),
        "-r",
        &TARGET_FPS.to_string(),
        "-i",
        "-",
        "-f",
        "lavfi",
        "-i",
        "anullsrc=channel_layout=stereo:sample_rate=44100",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-tune",
        "zerolatency",
        "-b:v",
        "2500k",
        "-maxrate",
        "3000k",
        "-bufsize",
        "6000k",
        "-pix_fmt",
        "yuv420p",
        "-g",
        "60",
        "-keyint_min",
        "60",
        "-sc_threshold",
        "0",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-ar",
        "44100",
        "-flvflags",
        "no_duration_filesize",
        "-f",
        "flv",
        &full_target,
    ]);

    cmd.stdin(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow!("Не удалось запустить процесс FFmpeg: {e}"))?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Не удалось открыть stdin для процесса FFmpeg"))?;

    let child_stderr = child.stderr.take();

    let running = Arc::new(AtomicBool::new(true));
    let cap_running = running.clone();
    let last_errors = Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(30)));
    let err_buf = last_errors.clone();

    // Spawn thread to drain stderr and store error messages
    if let Some(stderr) = child_stderr {
        std::thread::Builder::new()
            .name("ew-rtmp-stderr".into())
            .spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if line.contains("Error")
                        || line.contains("error")
                        || line.contains("Connection refused")
                        || line.contains("Handshake failed")
                        || line.contains("Failed to")
                    {
                        warn!("FFmpeg stderr: {}", line);
                        if let Ok(mut buf) = err_buf.lock() {
                            if buf.len() >= 25 {
                                buf.pop_front();
                            }
                            buf.push_back(line);
                        }
                    }
                }
            })
            .ok();
    }

    // Spawn 720p 30fps screen capture & pipe thread
    let running_thread = cap_running.clone();
    std::thread::Builder::new()
        .name("ew-rtmp-capture-pipe".into())
        .spawn(move || {
            info!("Starting RTMP capture loop for HWND: {}, PID: {}", hwnd, pid);
            let mut bgra_buffer = vec![0u8; TARGET_WIDTH * TARGET_HEIGHT * 4];
            let mut current_hwnd = hwnd;
            let mut last_hwnd_check = Instant::now();
            let mut total_frames = 0u64;
            let stream_start = Instant::now();
            let mut last_log = Instant::now();

            while running_thread.load(Ordering::SeqCst) {
                // Validate window handle; reconnect if process restarted or changed HWND
                #[cfg(windows)]
                {
                    use windows::Win32::Foundation::HWND;
                    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
                    if unsafe { !IsWindow(HWND(current_hwnd)).as_bool() } {
                        if last_hwnd_check.elapsed() > Duration::from_secs(1) {
                            last_hwnd_check = Instant::now();
                            let windows = crate::screen_capture::find_target_windows("EndlessWar.exe");
                            if let Some(target) = windows.into_iter().find(|w| w.pid == pid) {
                                info!(
                                    "RTMP Stream HWND updated from {} to new HWND {} for PID {}",
                                    current_hwnd, target.hwnd, pid
                                );
                                current_hwnd = target.hwnd;
                            }
                        }
                    }
                }

                // Capture window frame scaled directly to 720p BGRA
                let captured = crate::streamer::capture_window_720p_into(current_hwnd, &mut bgra_buffer);
                if !captured {
                    // Window might be minimized or occluded temporarily, maintain stream with blank/black frame
                    bgra_buffer.fill(0);
                }

                // Pipe raw BGRA frame to FFmpeg stdin
                if let Err(e) = child_stdin.write_all(&bgra_buffer) {
                    warn!("FFmpeg stdin pipe broken ({}), terminating RTMP capture loop", e);
                    break;
                }
                total_frames += 1;

                // Wall-clock synchronization:
                // If capture took longer than 1/30s, emit duplicate frame(s) to guarantee exactly 30.0 FPS.
                // This prevents audio/video PTS desynchronization in RTMP and ensures the Telegram player never starves.
                let elapsed_secs = stream_start.elapsed().as_secs_f64();
                let target_frames = (elapsed_secs * TARGET_FPS as f64).floor() as u64;
                while total_frames < target_frames {
                    if let Err(_) = child_stdin.write_all(&bgra_buffer) {
                        break;
                    }
                    total_frames += 1;
                }
                let _ = child_stdin.flush();

                if last_log.elapsed() >= Duration::from_secs(10) {
                    let total_elapsed = stream_start.elapsed().as_secs_f64();
                    let actual_fps = total_frames as f64 / total_elapsed.max(0.001);
                    info!(
                        "RTMP stream active: {} frames piped (fps: {:.1}, uptime: {}s)",
                        total_frames,
                        actual_fps,
                        total_elapsed as u64
                    );
                    last_log = Instant::now();
                }

                // Sleep until the exact schedule for the next frame
                let next_frame_micros = total_frames * 1_000_000 / TARGET_FPS as u64;
                let next_frame_time = stream_start + Duration::from_micros(next_frame_micros);
                let now = Instant::now();
                if next_frame_time > now {
                    std::thread::sleep(next_frame_time - now);
                }
            }

            info!("RTMP capture loop finished (total frames: {})", total_frames);
            running_thread.store(false, Ordering::SeqCst);
            RTMP_RUNNING_FLAG.store(false, Ordering::SeqCst);
        })
        .map_err(|e| anyhow!("Не удалось запустить поток захвата экрана: {e}"))?;

    RTMP_RUNNING_FLAG.store(true, Ordering::SeqCst);

    let session = RtmpSession {
        hwnd,
        pid,
        title: title.clone(),
        rtmp_url: rtmp_url.clone(),
        start_time: Instant::now(),
        running,
        child: Arc::new(Mutex::new(Some(child))),
        last_errors,
    };

    let info = ActiveRtmpInfo {
        hwnd,
        pid,
        title,
        rtmp_url,
        uptime_secs: 0,
    };

    *guard = Some(session);
    Ok(info)
}

/// Stop active RTMP stream and kill FFmpeg process
pub async fn stop_rtmp_stream() -> Result<()> {
    let mgr = get_manager();
    let mut guard = mgr.current_session.lock().await;

    if let Some(session) = guard.take() {
        info!("Stopping active RTMP stream...");
        session.running.store(false, Ordering::SeqCst);
        RTMP_RUNNING_FLAG.store(false, Ordering::SeqCst);

        let mut child_guard = session.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        info!("RTMP stream successfully stopped");
    } else {
        RTMP_RUNNING_FLAG.store(false, Ordering::SeqCst);
    }

    Ok(())
}

/// Get the most recent error lines from FFmpeg if stream exited or failed
pub async fn get_last_rtmp_errors() -> Vec<String> {
    let mgr = get_manager();
    let guard = mgr.current_session.lock().await;
    if let Some(s) = guard.as_ref() {
        if let Ok(buf) = s.last_errors.lock() {
            return buf.iter().cloned().collect();
        }
    }
    Vec::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FFmpegDownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f32,
    pub stage: String,
    pub message: String,
}

/// Download official Gyan Essentials FFmpeg release archive, extract ffmpeg.exe, and place in app directory
pub async fn download_and_install_ffmpeg(app: &tauri::AppHandle) -> Result<String> {
    use tauri::Emitter;

    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("Не удалось определить папку программы"))?
        .to_path_buf();

    let target_ffmpeg = exe_dir.join("ffmpeg.exe");

    // Official Gyan FFmpeg release essentials build hosted on GitHub CDN
    let download_url = "https://github.com/GyanD/codexffmpeg/releases/download/7.1/ffmpeg-7.1-essentials_build.zip";
    info!("Starting FFmpeg download from {}...", download_url);

    let client = reqwest::Client::builder()
        .user_agent("ew-notify-downloader")
        .timeout(Duration::from_secs(600))
        .build()?;

    let mut resp = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| anyhow!("Ошибка подключения к серверу загрузки FFmpeg: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("Сервер вернул статус ошибки: {}", resp.status()));
    }

    let total_bytes = resp.content_length();
    let temp_zip = exe_dir.join("ffmpeg.download.tmp.zip");
    let mut file = std::fs::File::create(&temp_zip)
        .map_err(|e| anyhow!("Не удалось создать временный файл: {e}"))?;

    let mut downloaded_bytes: u64 = 0;
    let mut last_emit = Instant::now();

    let _ = app.emit(
        "ffmpeg-download-progress",
        &FFmpegDownloadProgress {
            downloaded_bytes: 0,
            total_bytes,
            percent: 0.0,
            stage: "downloading".to_string(),
            message: "Начало загрузки архива FFmpeg...".to_string(),
        },
    );

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| anyhow!("Ошибка скачивания данных: {e}"))?
    {
        file.write_all(&chunk)
            .map_err(|e| anyhow!("Ошибка записи во временный файл: {e}"))?;
        downloaded_bytes += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(150) {
            let percent = if let Some(total) = total_bytes {
                if total > 0 {
                    (downloaded_bytes as f32 / total as f32) * 100.0
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let _ = app.emit(
                "ffmpeg-download-progress",
                &FFmpegDownloadProgress {
                    downloaded_bytes,
                    total_bytes,
                    percent,
                    stage: "downloading".to_string(),
                    message: format!(
                        "Скачивание: {:.1} МБ / {:.1} МБ ({:.0}%)",
                        downloaded_bytes as f64 / 1_048_576.0,
                        total_bytes.unwrap_or(0) as f64 / 1_048_576.0,
                        percent
                    ),
                },
            );
            last_emit = Instant::now();
        }
    }

    drop(file);

    let _ = app.emit(
        "ffmpeg-download-progress",
        &FFmpegDownloadProgress {
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
            stage: "extracting".to_string(),
            message: "Распаковка ffmpeg.exe из архива...".to_string(),
        },
    );

    let extract_dir = exe_dir.join("ffmpeg.extract.tmp");
    if extract_dir.exists() {
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    std::fs::create_dir_all(&extract_dir)?;

    let tar_status = std::process::Command::new("tar")
        .args([
            "-xf",
            temp_zip.to_str().unwrap(),
            "-C",
            extract_dir.to_str().unwrap(),
        ])
        .output();

    let extracted_ok = match tar_status {
        Ok(out) if out.status.success() => true,
        _ => {
            let ps_cmd = format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                temp_zip.display(),
                extract_dir.display()
            );
            let ps_status = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_cmd])
                .status();
            matches!(ps_status, Ok(s) if s.success())
        }
    };

    let _ = std::fs::remove_file(&temp_zip);

    if !extracted_ok {
        let _ = std::fs::remove_dir_all(&extract_dir);
        return Err(anyhow!("Не удалось распаковать архив FFmpeg (tar/powershell)"));
    }

    let found_ffmpeg = crate::updater::find_file_recursive(&extract_dir, "ffmpeg.exe")
        .ok_or_else(|| anyhow!("ffmpeg.exe не найден внутри архива"))?;

    std::fs::copy(&found_ffmpeg, &target_ffmpeg)
        .map_err(|e| anyhow!("Не удалось сохранить ffmpeg.exe в папку программы: {e}"))?;

    let _ = std::fs::remove_dir_all(&extract_dir);

    info!("FFmpeg successfully installed to {:?}", target_ffmpeg);

    let _ = app.emit(
        "ffmpeg-download-progress",
        &FFmpegDownloadProgress {
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
            stage: "done".to_string(),
            message: "FFmpeg успешно установлен!".to_string(),
        },
    );

    Ok(format!("FFmpeg успешно установлен в {:?}", target_ffmpeg))
}

/// Delete ffmpeg.exe binary from application folder to free disk space
pub fn delete_ffmpeg_binary() -> Result<String> {
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("Не удалось определить папку программы"))?
        .to_path_buf();

    let target = exe_dir.join("ffmpeg.exe");
    if target.is_file() {
        std::fs::remove_file(&target)
            .map_err(|e| anyhow!("Не удалось удалить ffmpeg.exe: {e}"))?;
        Ok("Файл ffmpeg.exe удален, место на диске освобождено.".to_string())
    } else {
        Ok("Файл ffmpeg.exe не найден в папке программы.".to_string())
    }
}
