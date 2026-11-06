//! P2P WebRTC 1080p Window Streamer using OpenH264 & MQTT Signaling (broker.emqx.io / broker.hivemq.com).

use anyhow::{anyhow, Result};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, SpsPpsStrategy,
};
use openh264::formats::YUVSource;
use openh264::OpenH264API;

use rtc::media::Sample;
use rtc::media_stream::MediaStreamTrack;
use rtc::peer_connection::configuration::media_engine::{MIME_TYPE_H264, MediaEngine};
use rtc::peer_connection::configuration::RTCConfigurationBuilder;
use rtc::peer_connection::sdp::RTCSessionDescription;
use rtc::peer_connection::transport::RTCIceServer;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_local::TrackLocal;
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCIceGatheringState,
    RTCPeerConnectionState,
};

const TARGET_WIDTH: usize = 1920;
const TARGET_HEIGHT: usize = 1080;
const TARGET_FPS: u64 = 30;
const FRAME_DURATION_MS: u64 = 1000 / TARGET_FPS;
const SSRC_VIDEO: u32 = 5000;
const PAYLOAD_TYPE_H264: u8 = 102;

// Structure for YUV420p frame buffer feeding OpenH264
pub struct Yuv420Buffer {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl Yuv420Buffer {
    pub fn new(width: usize, height: usize) -> Self {
        let y_len = width * height;
        let uv_len = (width / 2) * (height / 2);
        Self {
            width,
            height,
            y: vec![0u8; y_len],
            u: vec![128u8; uv_len],
            v: vec![128u8; uv_len],
        }
    }
}

impl YUVSource for Yuv420Buffer {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn strides(&self) -> (usize, usize, usize) {
        (self.width, self.width / 2, self.width / 2)
    }

    fn y(&self) -> &[u8] {
        &self.y
    }

    fn u(&self) -> &[u8] {
        &self.u
    }

    fn v(&self) -> &[u8] {
        &self.v
    }
}

/// Convert BGRA (1920x1080) to YUV420 planar buffer
pub fn bgra_to_yuv420(bgra: &[u8], width: usize, height: usize, yuv: &mut Yuv420Buffer) {
    let half_w = width / 2;
    for y in 0..height {
        let is_even_y = (y % 2) == 0;
        let y_row_idx = y * width;
        let uv_row_idx = (y / 2) * half_w;

        for x in 0..width {
            let px_idx = (y_row_idx + x) * 4;
            let b = bgra[px_idx] as i32;
            let g = bgra[px_idx + 1] as i32;
            let r = bgra[px_idx + 2] as i32;

            // Standard BT.601 Color Conversion
            let y_val = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
            yuv.y[y_row_idx + x] = y_val.clamp(0, 255) as u8;

            if is_even_y && (x % 2 == 0) {
                let u_val = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                let v_val = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
                let uv_idx = uv_row_idx + (x / 2);
                yuv.u[uv_idx] = u_val.clamp(0, 255) as u8;
                yuv.v[uv_idx] = v_val.clamp(0, 255) as u8;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStreamInfo {
    pub stream_id: String,
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub viewer_count: usize,
    pub player_url: String,
    pub uptime_secs: u64,
}

#[derive(Clone)]
pub struct ActiveViewer {
    pub track: Arc<TrackLocalStaticSample>,
    pub payload_type: u8,
    pub pc: Arc<dyn PeerConnection>,
    pub is_connected: Arc<AtomicBool>,
}

pub struct StreamSession {
    pub stream_id: String,
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub start_time: Instant,
    pub viewer_count: Arc<AtomicUsize>,
    pub force_keyframe: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub active_viewers: Arc<std::sync::RwLock<HashMap<String, ActiveViewer>>>,
}

static ACTIVE_STREAM_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static ACTIVE_STREAM_VIEWERS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_STREAM_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn get_sync_viewer_count(hwnd: isize) -> Option<usize> {
    if ACTIVE_STREAM_RUNNING.load(Ordering::Relaxed) && ACTIVE_STREAM_HWND.load(Ordering::Relaxed) == hwnd {
        Some(ACTIVE_STREAM_VIEWERS.load(Ordering::Relaxed))
    } else {
        None
    }
}

pub struct GlobalStreamManager {
    current_session: Mutex<Option<StreamSession>>,
}

static MANAGER: std::sync::OnceLock<GlobalStreamManager> = std::sync::OnceLock::new();

fn get_manager() -> &'static GlobalStreamManager {
    MANAGER.get_or_init(|| GlobalStreamManager {
        current_session: Mutex::new(None),
    })
}

pub async fn get_stream_info() -> Option<ActiveStreamInfo> {
    let mgr = get_manager();
    let guard = mgr.current_session.lock().await;
    guard.as_ref().map(|s| ActiveStreamInfo {
        stream_id: s.stream_id.clone(),
        hwnd: s.hwnd,
        pid: s.pid,
        title: s.title.clone(),
        viewer_count: s.viewer_count.load(Ordering::SeqCst),
        player_url: format!("https://delio-no.github.io/ew-notify/?s={}", s.stream_id),
        uptime_secs: s.start_time.elapsed().as_secs(),
    })
}

pub async fn stop_stream() {
    ACTIVE_STREAM_RUNNING.store(false, Ordering::SeqCst);
    ACTIVE_STREAM_HWND.store(0, Ordering::SeqCst);
    ACTIVE_STREAM_VIEWERS.store(0, Ordering::SeqCst);

    let mgr = get_manager();
    let mut guard = mgr.current_session.lock().await;
    if let Some(session) = guard.take() {
        info!("Stopping active stream: {}", session.stream_id);
        session.running.store(false, Ordering::SeqCst);
        let viewers = session.active_viewers.write().unwrap().drain().collect::<Vec<_>>();
        for (_, viewer) in viewers {
            let pc = viewer.pc;
            tokio::spawn(async move {
                let _ = pc.close().await;
            });
        }
    }
}

pub async fn is_streaming_hwnd(hwnd: isize) -> bool {
    let mgr = get_manager();
    let guard = mgr.current_session.lock().await;
    if let Some(s) = guard.as_ref() {
        s.hwnd == hwnd && s.running.load(Ordering::SeqCst)
    } else {
        false
    }
}

pub async fn get_hwnd_viewer_count(hwnd: isize) -> Option<usize> {
    let mgr = get_manager();
    let guard = mgr.current_session.lock().await;
    if let Some(s) = guard.as_ref() {
        if s.hwnd == hwnd && s.running.load(Ordering::SeqCst) {
            return Some(s.viewer_count.load(Ordering::SeqCst));
        }
    }
    None
}

/// Create H.264 video codec description
fn create_h264_codec() -> RTCRtpCodecParameters {
    RTCRtpCodecParameters {
        rtp_codec: RTCRtpCodec {
            mime_type: MIME_TYPE_H264.to_owned(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                .to_owned(),
            rtcp_feedback: vec![],
        },
        payload_type: PAYLOAD_TYPE_H264,
        ..Default::default()
    }
}

/// Start a new 1080p stream for the given game window
pub async fn start_stream(hwnd: isize, pid: u32, title: String) -> Result<ActiveStreamInfo> {
    let mgr = get_manager();
    let mut guard = mgr.current_session.lock().await;

    // Stop existing session if any
    if let Some(old) = guard.take() {
        info!("Stopping previous stream: {}", old.stream_id);
        old.running.store(false, Ordering::SeqCst);
        let viewers = old.active_viewers.write().unwrap().drain().collect::<Vec<_>>();
        for (_, viewer) in viewers {
            let pc = viewer.pc;
            tokio::spawn(async move {
                let _ = pc.close().await;
            });
        }
    }

    // Generate random stream ID e.g. ew-a7b2c9
    let rand_hex: String = (0..6)
        .map(|_| {
            let b = (rand::random::<u8>() % 16) as u8;
            format!("{:x}", b)
        })
        .collect();
    let stream_id = format!("ew-{}", rand_hex);

    let running = Arc::new(AtomicBool::new(true));
    let viewer_count = Arc::new(AtomicUsize::new(0));
    let force_keyframe = Arc::new(AtomicBool::new(true));
    let active_viewers = Arc::new(std::sync::RwLock::new(HashMap::new()));

    // Spawn MQTT Signaling Task (connects to EMQX / HiveMQ with failover)
    let sig_stream_id = stream_id.clone();
    let sig_running = running.clone();
    let sig_viewers_map = active_viewers.clone();
    let sig_viewer_count = viewer_count.clone();
    let sig_force_keyframe = force_keyframe.clone();

    tokio::spawn(async move {
        run_mqtt_signaling_loop(
            sig_stream_id,
            sig_running,
            sig_viewers_map,
            sig_viewer_count,
            sig_force_keyframe,
        )
        .await;
    });

    // Spawn Screen Capture & H.264 Encoding Loop in dedicated OS thread
    let cap_running = running.clone();
    let cap_viewers_map = active_viewers.clone();
    let cap_viewer_count = viewer_count.clone();
    let cap_force_keyframe = force_keyframe.clone();

    std::thread::Builder::new()
        .name("ew-1080p-stream-capture".into())
        .spawn(move || {
            run_capture_encode_loop(
                hwnd,
                pid,
                cap_running,
                cap_viewers_map,
                cap_viewer_count,
                cap_force_keyframe,
            );
        })
        .map_err(|e| anyhow!("Failed to spawn stream capture thread: {e}"))?;

    ACTIVE_STREAM_HWND.store(hwnd, Ordering::SeqCst);
    ACTIVE_STREAM_VIEWERS.store(0, Ordering::SeqCst);
    ACTIVE_STREAM_RUNNING.store(true, Ordering::SeqCst);

    let session = StreamSession {
        stream_id: stream_id.clone(),
        hwnd,
        pid,
        title: title.clone(),
        start_time: Instant::now(),
        viewer_count: viewer_count.clone(),
        force_keyframe,
        running,
        active_viewers,
    };

    let player_url = format!("https://delio-no.github.io/ew-notify/?s={}", stream_id);
    let info = ActiveStreamInfo {
        stream_id,
        hwnd,
        pid,
        title,
        viewer_count: 0,
        player_url,
        uptime_secs: 0,
    };

    *guard = Some(session);
    Ok(info)
}

/// Screen capture and H.264 encoding loop at 1080p, 30fps (runs in dedicated OS thread)
fn run_capture_encode_loop(
    hwnd_val: isize,
    pid: u32,
    running: Arc<AtomicBool>,
    active_viewers: Arc<std::sync::RwLock<HashMap<String, ActiveViewer>>>,
    _viewer_count: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
) {
    info!("Starting 1080p capture & encode loop for HWND: {}, PID: {}", hwnd_val, pid);

    #[cfg(windows)]
    unsafe {
        use windows::core::w;
        use windows::Win32::System::StationsAndDesktops::{
            CloseDesktop, OpenDesktopW, SetThreadDesktop, DESKTOP_CONTROL_FLAGS,
        };
        if let Ok(h_desk) = OpenDesktopW(w!("default"), DESKTOP_CONTROL_FLAGS(0), false, 0x01FF) {
            let _ = SetThreadDesktop(h_desk);
            let _ = CloseDesktop(h_desk);
        }
    }

    let api = OpenH264API::from_source();
    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(3_000_000))
        .max_frame_rate(FrameRate::from_hz(TARGET_FPS as f32))
        .intra_frame_period(IntraFramePeriod::from_num_frames(30))
        .sps_pps_strategy(SpsPpsStrategy::IncreasingId);

    let mut encoder = match Encoder::with_api_config(api, config) {
        Ok(enc) => enc,
        Err(e) => {
            error!("Failed to initialize OpenH264 encoder with config: {}. Retrying default...", e);
            match Encoder::new() {
                Ok(enc) => enc,
                Err(e2) => {
                    error!("Failed to initialize OpenH264 default encoder: {}", e2);
                    return;
                }
            }
        }
    };

    let mut yuv_buffer = Yuv420Buffer::new(TARGET_WIDTH, TARGET_HEIGHT);
    let mut bgra_buffer = vec![0u8; TARGET_WIDTH * TARGET_HEIGHT * 4];
    let mut current_hwnd = hwnd_val;
    let mut last_hwnd_check = Instant::now();
    let mut total_encoded_frames = 0u64;
    let mut last_stat_log = Instant::now();

    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();

        // Get snapshot of CONNECTED active viewers only (DTLS/SRTP established)
        let viewers = {
            let guard = active_viewers.read().unwrap();
            guard
                .iter()
                .filter(|(_, v)| v.is_connected.load(Ordering::SeqCst))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        // If no active connected viewers, sleep lightly to conserve CPU
        if viewers.is_empty() {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Validate window handle; if closed/recreated, scan for new HWND for process pid
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
                            "Stream HWND updated from {} to new HWND {} for PID {}",
                            current_hwnd, target.hwnd, pid
                        );
                        current_hwnd = target.hwnd;
                        ACTIVE_STREAM_HWND.store(current_hwnd, Ordering::SeqCst);
                    } else {
                        std::thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }
        }

        // Capture 1080p frame from Windows window via GDI StretchBlt
        let captured = capture_window_1080p_into(current_hwnd, &mut bgra_buffer);
        if !captured {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Convert BGRA to planar YUV420p
        bgra_to_yuv420(&bgra_buffer, TARGET_WIDTH, TARGET_HEIGHT, &mut yuv_buffer);

        // Force keyframe if requested (new viewer connected)
        if force_keyframe.swap(false, Ordering::SeqCst) {
            encoder.force_intra_frame();
        }

        // Encode to H.264 and copy bytes immediately to free C FFI bitstream
        let nalu_opt = match encoder.encode(&yuv_buffer) {
            Ok(bitstream) => {
                let bytes = bitstream.to_vec();
                if !bytes.is_empty() {
                    Some(bytes)
                } else {
                    None
                }
            }
            Err(e) => {
                warn!("OpenH264 encoding error: {}", e);
                None
            }
        };

        if let Some(nalu_bytes) = nalu_opt {
            let sample_bytes = bytes::Bytes::from(nalu_bytes);
            total_encoded_frames += 1;

            for (vid, viewer) in &viewers {
                let sample = Sample {
                    data: sample_bytes.clone(),
                    duration: Duration::from_millis(FRAME_DURATION_MS),
                    ..Sample::new(Instant::now())
                };
                let writer = viewer.track.sample_writer(SSRC_VIDEO, viewer.payload_type);
                let _ = tauri::async_runtime::block_on(async {
                    if let Err(e) = writer.write_sample(&sample).await {
                        warn!("WebRTC write_sample error for viewer '{}': {}", vid, e);
                    }
                });
            }

            if last_stat_log.elapsed() >= Duration::from_secs(5) {
                info!(
                    "Streaming active: encoded {} frames (1080p 30fps), active viewers: {}",
                    total_encoded_frames, viewers.len()
                );
                last_stat_log = Instant::now();
            }
        }

        // Throttle to 30 FPS
        let elapsed = frame_start.elapsed();
        if elapsed < Duration::from_millis(FRAME_DURATION_MS) {
            std::thread::sleep(Duration::from_millis(FRAME_DURATION_MS) - elapsed);
        }
    }

    info!("1080p capture loop terminated for HWND: {}", current_hwnd);
}

/// Capture game window and scale directly to 1920x1080 BGRA
#[cfg(windows)]
fn capture_window_1080p_into(hwnd_val: isize, out_bgra: &mut [u8]) -> bool {
    use std::mem::{size_of, zeroed};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BitBlt, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HALFTONE, HDC, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, IsIconic, ShowWindow, SW_SHOWNOACTIVATE,
    };

    let hwnd = HWND(hwnd_val);

    unsafe {
        if IsIconic(hwnd).as_bool() {
            // If window is minimized to taskbar, restore it without taking input focus
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            std::thread::sleep(Duration::from_millis(30));
        }

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);
        let src_w = rect.right - rect.left;
        let src_h = rect.bottom - rect.top;

        if src_w <= 10 || src_h <= 10 {
            return false;
        }

        let hdc_screen = GetDC(HWND(0));
        if hdc_screen.is_invalid() {
            return false;
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbmp_1080p = CreateCompatibleBitmap(
            hdc_screen,
            TARGET_WIDTH as i32,
            TARGET_HEIGHT as i32,
        );

        if hdc_mem.is_invalid() || hbmp_1080p.is_invalid() {
            if !hbmp_1080p.is_invalid() {
                DeleteObject(hbmp_1080p);
            }
            if !hdc_mem.is_invalid() {
                DeleteDC(hdc_mem);
            }
            ReleaseDC(HWND(0), hdc_screen);
            return false;
        }

        let old_mem = SelectObject(hdc_mem, hbmp_1080p);

        // Capture source window into memory DC
        let hdc_src = CreateCompatibleDC(hdc_screen);
        let hbmp_src = CreateCompatibleBitmap(hdc_screen, src_w, src_h);
        let old_src = SelectObject(hdc_src, hbmp_src);

        #[link(name = "user32")]
        extern "system" {
            fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> windows::Win32::Foundation::BOOL;
        }

        // 1st attempt: PrintWindow PW_RENDERFULLCONTENT = 2
        let printed = PrintWindow(hwnd, HDC(hdc_src.0), 2);
        if !printed.as_bool() {
            // Fallback to window DC BitBlt
            let hdc_win = GetDC(hwnd);
            if !hdc_win.is_invalid() {
                let _ = BitBlt(hdc_src, 0, 0, src_w, src_h, hdc_win, 0, 0, SRCCOPY);
                ReleaseDC(hwnd, hdc_win);
            } else {
                let _ = BitBlt(hdc_src, 0, 0, src_w, src_h, hdc_screen, rect.left, rect.top, SRCCOPY);
            }
        }

        // StretchBlt from original resolution to 1920x1080
        SetStretchBltMode(hdc_mem, HALFTONE);
        let _ = StretchBlt(
            hdc_mem,
            0,
            0,
            TARGET_WIDTH as i32,
            TARGET_HEIGHT as i32,
            hdc_src,
            0,
            0,
            src_w,
            src_h,
            SRCCOPY,
        );

        // Retrieve raw BGRA pixels
        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = TARGET_WIDTH as i32;
        bmi.bmiHeader.biHeight = -(TARGET_HEIGHT as i32); // Top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0 as u32;

        let lines = GetDIBits(
            hdc_mem,
            hbmp_1080p,
            0,
            TARGET_HEIGHT as u32,
            Some(out_bgra.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Check if frame has content; if all black and PrintWindow was used, fallback to screen DC BitBlt
        let sample_has_content = out_bgra.iter().take(2000).any(|&b| b > 10);
        if lines > 0 && !sample_has_content && printed.as_bool() {
            let _ = BitBlt(hdc_src, 0, 0, src_w, src_h, hdc_screen, rect.left, rect.top, SRCCOPY);
            let _ = StretchBlt(
                hdc_mem, 0, 0, TARGET_WIDTH as i32, TARGET_HEIGHT as i32,
                hdc_src, 0, 0, src_w, src_h, SRCCOPY,
            );
            let _ = GetDIBits(
                hdc_mem, hbmp_1080p, 0, TARGET_HEIGHT as u32,
                Some(out_bgra.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS,
            );
        }

        // Cleanup GDI
        SelectObject(hdc_src, old_src);
        DeleteObject(hbmp_src);
        DeleteDC(hdc_src);

        SelectObject(hdc_mem, old_mem);
        DeleteObject(hbmp_1080p);
        DeleteDC(hdc_mem);
        ReleaseDC(HWND(0), hdc_screen);

        lines > 0
    }
}

#[cfg(not(windows))]
fn capture_window_1080p_into(_hwnd: isize, _out: &mut [u8]) -> bool {
    false
}

/// PeerConnection event handler for tracking viewer connection state & ICE
struct ViewerHandler {
    gather_complete_tx: mpsc::Sender<()>,
    viewers: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
    viewer_id: String,
    active_viewers: Arc<std::sync::RwLock<HashMap<String, ActiveViewer>>>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for ViewerHandler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            let _ = self.gather_complete_tx.try_send(());
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        match state {
            RTCPeerConnectionState::Connected => {
                if let Some(viewer) = self.active_viewers.read().unwrap().get(&self.viewer_id) {
                    viewer.is_connected.store(true, Ordering::SeqCst);
                }
                let current = self.viewers.fetch_add(1, Ordering::SeqCst) + 1;
                ACTIVE_STREAM_VIEWERS.store(current, Ordering::SeqCst);
                self.force_keyframe.store(true, Ordering::SeqCst);
                info!(
                    "Viewer '{}' CONNECTED and ready for RTP streaming! Active viewers: {}",
                    self.viewer_id, current
                );
            }
            RTCPeerConnectionState::Disconnected
            | RTCPeerConnectionState::Failed
            | RTCPeerConnectionState::Closed => {
                {
                    let mut map = self.active_viewers.write().unwrap();
                    map.remove(&self.viewer_id);
                }
                let prev = self.viewers.load(Ordering::SeqCst);
                if prev > 0 {
                    let current = self.viewers.fetch_sub(1, Ordering::SeqCst) - 1;
                    ACTIVE_STREAM_VIEWERS.store(current, Ordering::SeqCst);
                    info!(
                        "Viewer '{}' disconnected. Active viewers: {}",
                        self.viewer_id, current
                    );
                }
            }
            _ => {}
        }
    }
}

#[derive(Deserialize)]
struct OfferMessage {
    viewer_id: String,
    sdp: String,
}

/// MQTT signaling loop with automatic failover between EMQX and HiveMQ
async fn run_mqtt_signaling_loop(
    stream_id: String,
    running: Arc<AtomicBool>,
    active_viewers: Arc<std::sync::RwLock<HashMap<String, ActiveViewer>>>,
    viewer_count: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
) {
    let brokers = [
        ("broker.emqx.io", 1883),
        ("broker.hivemq.com", 1883),
    ];

    let mut broker_idx = 0;

    while running.load(Ordering::SeqCst) {
        let (host, port) = brokers[broker_idx % brokers.len()];
        let rand_client_id: String = (0..8)
            .map(|_| format!("{:x}", rand::random::<u8>() % 16))
            .collect();
        let client_id = format!("ew-stream-{}-{}", stream_id, rand_client_id);

        info!(
            "Connecting to MQTT signaling broker {}:{} for stream '{}'...",
            host, port, stream_id
        );

        let mut mqttoptions = MqttOptions::new(&client_id, host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(15));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 64);
        let offer_topic = format!("ew-notify/{}/offer", stream_id);

        if let Err(e) = client.subscribe(&offer_topic, QoS::AtLeastOnce).await {
            warn!(
                "Failed to subscribe to MQTT topic {} on {}: {}. Retrying with next broker...",
                offer_topic, host, e
            );
            broker_idx += 1;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        info!(
            "Registered stream ID '{}' with MQTT broker {}! Listening on topic '{}'",
            stream_id, host, offer_topic
        );

        while running.load(Ordering::SeqCst) {
            match eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p))) => {
                    if p.topic == offer_topic {
                        let payload_bytes = p.payload.to_vec();
                        let active_viewers_clone = active_viewers.clone();
                        let viewer_count_clone = viewer_count.clone();
                        let force_keyframe_clone = force_keyframe.clone();
                        let stream_id_clone = stream_id.clone();
                        let client_clone = client.clone();

                        tokio::spawn(async move {
                            handle_offer_message(
                                &payload_bytes,
                                &stream_id_clone,
                                active_viewers_clone,
                                viewer_count_clone,
                                force_keyframe_clone,
                                client_clone,
                            )
                            .await;
                        });
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("MQTT eventloop error on {}: {}. Reconnecting...", host, e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    break;
                }
            }
        }

        broker_idx += 1;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    info!("MQTT signaling loop finished for stream: {}", stream_id);
}

/// Handle incoming WebRTC OFFER message from viewer
async fn handle_offer_message(
    payload: &[u8],
    stream_id: &str,
    active_viewers: Arc<std::sync::RwLock<HashMap<String, ActiveViewer>>>,
    viewer_count: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
    client: AsyncClient,
) {
    let msg: OfferMessage = match serde_json::from_slice(payload) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid offer payload received via MQTT: {}", e);
            return;
        }
    };

    let viewer_id = msg.viewer_id;
    let sdp_str = msg.sdp;

    info!("Received WebRTC OFFER from viewer '{}'", viewer_id);

    // Create unique static local video track for this viewer
    let video_codec = create_h264_codec();
    let video_track = match TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "ew-stream-video".to_string(),
            format!("ew-track-{}", viewer_id),
            "EndlessWar Video Stream".to_string(),
            RtpCodecKind::Video,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(SSRC_VIDEO),
                    ..Default::default()
                },
                codec: video_codec.rtp_codec.clone(),
                ..Default::default()
            }],
        ),
    ) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            error!("Failed to create TrackLocalStaticSample: {}", e);
            return;
        }
    };

    let mut media_engine = MediaEngine::default();
    if let Err(e) = media_engine.register_codec(video_codec, RtpCodecKind::Video) {
        error!("Failed to register video codec: {}", e);
        return;
    }

    // Configure WebRTC PeerConnection with STUN
    let rtc_config = RTCConfigurationBuilder::new()
        .with_ice_servers(vec![
            RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            },
            RTCIceServer {
                urls: vec!["stun:global.stun.twilio.com:3478".to_string()],
                ..Default::default()
            },
        ])
        .build();

    let (gather_complete_tx, mut gather_complete_rx) = mpsc::channel::<()>(1);
    let handler = Arc::new(ViewerHandler {
        gather_complete_tx,
        viewers: viewer_count.clone(),
        force_keyframe: force_keyframe.clone(),
        viewer_id: viewer_id.clone(),
        active_viewers: active_viewers.clone(),
    });

    let pc = match PeerConnectionBuilder::new()
        .with_configuration(rtc_config)
        .with_media_engine(media_engine)
        .with_handler(handler)
        .with_udp_addrs(vec!["0.0.0.0:0"])
        .build()
        .await
    {
        Ok(p) => Arc::new(p) as Arc<dyn PeerConnection>,
        Err(e) => {
            error!("Failed to build PeerConnection: {}", e);
            return;
        }
    };

    // Add local H.264 video track
    let track_dyn = Arc::clone(&video_track) as Arc<dyn TrackLocal>;
    let rtp_sender = match pc.add_track(track_dyn).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to add track to PeerConnection: {}", e);
            return;
        }
    };

    // Parse remote Offer SDP and set remote description
    let offer_desc = match RTCSessionDescription::offer(sdp_str) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to parse Offer SDP: {}", e);
            return;
        }
    };

    if let Err(e) = pc.set_remote_description(offer_desc).await {
        error!("Failed to set_remote_description: {}", e);
        return;
    }

    // Generate Answer SDP
    let answer = match pc.create_answer(None).await {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to create_answer: {}", e);
            return;
        }
    };

    if let Err(e) = pc.set_local_description(answer).await {
        error!("Failed to set_local_description: {}", e);
        return;
    }

    // Read dynamically negotiated H.264 payload type from RTP sender parameters
    let mut negotiated_pt = PAYLOAD_TYPE_H264;
    if let Ok(params) = rtp_sender.get_parameters().await {
        for codec in params.rtp_parameters.codecs {
            if codec.rtp_codec.mime_type.eq_ignore_ascii_case(MIME_TYPE_H264) {
                negotiated_pt = codec.payload_type;
                info!(
                    "Viewer '{}' negotiated H.264 payload type: {}",
                    viewer_id, negotiated_pt
                );
                break;
            }
        }
    }

    // Wait briefly for ICE candidates to gather into local_description
    let _ = tokio::time::timeout(Duration::from_millis(700), gather_complete_rx.recv()).await;

    if let Some(local_desc) = pc.local_description().await {
        // Register viewer in active viewers map (waiting for DTLS Connected state)
        let is_connected = Arc::new(AtomicBool::new(false));
        {
            let mut map = active_viewers.write().unwrap();
            map.insert(
                viewer_id.clone(),
                ActiveViewer {
                    track: video_track,
                    payload_type: negotiated_pt,
                    pc: pc.clone(),
                    is_connected,
                },
            );
        }

        force_keyframe.store(true, Ordering::SeqCst);

        // Send ANSWER back to viewer through MQTT topic
        let answer_topic = format!("ew-notify/{}/answer/{}", stream_id, viewer_id);
        let answer_msg = serde_json::json!({
            "sdp": local_desc.sdp,
        });

        if let Err(e) = client
            .publish(
                &answer_topic,
                QoS::AtLeastOnce,
                false,
                answer_msg.to_string().into_bytes(),
            )
            .await
        {
            warn!("Failed to publish WebRTC ANSWER via MQTT: {}", e);
        } else {
            info!(
                "Successfully sent WebRTC ANSWER to viewer '{}' on MQTT topic '{}'",
                viewer_id, answer_topic
            );
        }
    }
}
