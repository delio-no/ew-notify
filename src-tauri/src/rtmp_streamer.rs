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
const FRAME_DURATION: Duration = Duration::from_millis(1000 / TARGET_FPS);

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

    // Build destination RTMP URL
    let full_target = if rtmp_key.trim().is_empty() {
        rtmp_url.trim().to_string()
    } else if rtmp_key.starts_with("rtmp://") || rtmp_key.starts_with("rtmps://") {
        rtmp_key.trim().to_string()
    } else {
        format!("{}/{}", rtmp_url.trim_end_matches('/'), rtmp_key.trim())
    };

    info!("Spawning FFmpeg for RTMP stream to destination: {}...", rtmp_url);

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
        "-shortest",
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
            let mut last_log = Instant::now();

            while running_thread.load(Ordering::SeqCst) {
                let frame_start = Instant::now();

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
                if last_log.elapsed() >= Duration::from_secs(10) {
                    info!(
                        "RTMP stream active: {} frames piped (uptime: {}s)",
                        total_frames,
                        last_log.elapsed().as_secs()
                    );
                    last_log = Instant::now();
                }

                // Throttle to target FPS
                let elapsed = frame_start.elapsed();
                if elapsed < FRAME_DURATION {
                    std::thread::sleep(FRAME_DURATION - elapsed);
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
