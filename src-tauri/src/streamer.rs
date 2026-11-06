//! P2P WebRTC 1080p Window Streamer using OpenH264 & PeerJS signaling.

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tracing::{error, info, warn};

use openh264::encoder::Encoder;
use openh264::formats::YUVSource;

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

pub struct StreamSession {
    pub stream_id: String,
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub start_time: Instant,
    pub viewer_count: Arc<AtomicUsize>,
    pub force_keyframe: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
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

    let video_codec = create_h264_codec();

    // Create single static local video track
    let video_track = Arc::new(TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "ew-stream-video".to_string(),
            "ew-track-video".to_string(),
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
    )?);

    // Peer connections map (client_id -> Arc<dyn PeerConnection>)
    let peer_connections: Arc<RwLock<HashMap<String, Arc<dyn PeerConnection>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Outgoing WebSocket message sender channel
    let (ws_out_tx, ws_out_rx) = mpsc::channel::<String>(64);

    // Spawn WebSocket Signaling Task (connect to PeerJS Cloud)
    let sig_running = running.clone();
    let sig_stream_id = stream_id.clone();
    let sig_track = video_track.clone();
    let sig_pcs = peer_connections.clone();
    let sig_viewers = viewer_count.clone();
    let sig_force_keyframe = force_keyframe.clone();

    tokio::spawn(async move {
        run_signaling_loop(
            sig_stream_id,
            sig_running,
            sig_track,
            sig_pcs,
            sig_viewers,
            sig_force_keyframe,
            ws_out_tx,
            ws_out_rx,
        )
        .await;
    });

    // Spawn Screen Capture & H.264 Encoding Loop in dedicated OS thread
    let cap_running = running.clone();
    let cap_track = video_track.clone();
    let cap_viewers = viewer_count.clone();
    let cap_force_keyframe = force_keyframe.clone();

    std::thread::Builder::new()
        .name("ew-1080p-stream-capture".into())
        .spawn(move || {
            run_capture_encode_loop(hwnd, cap_running, cap_track, cap_viewers, cap_force_keyframe);
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
    running: Arc<AtomicBool>,
    video_track: Arc<TrackLocalStaticSample>,
    viewer_count: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
) {
    info!("Starting 1080p capture & encode loop for HWND: {}", hwnd_val);

    let mut encoder = match Encoder::new() {
        Ok(enc) => enc,
        Err(e) => {
            error!("Failed to initialize OpenH264 encoder: {}", e);
            return;
        }
    };

    let mut yuv_buffer = Yuv420Buffer::new(TARGET_WIDTH, TARGET_HEIGHT);
    let mut bgra_buffer = vec![0u8; TARGET_WIDTH * TARGET_HEIGHT * 4];

    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();

        // If no active viewers, sleep lightly to conserve CPU
        if viewer_count.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(150));
            continue;
        }

        // Capture 1080p frame from Windows window via GDI StretchBlt
        let captured = capture_window_1080p_into(hwnd_val, &mut bgra_buffer);
        if !captured {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Convert BGRA to planar YUV420p
        bgra_to_yuv420(&bgra_buffer, TARGET_WIDTH, TARGET_HEIGHT, &mut yuv_buffer);

        // Force keyframe if new viewer connected
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
            let writer = video_track.sample_writer(SSRC_VIDEO, PAYLOAD_TYPE_H264);
            let sample = Sample {
                data: bytes::Bytes::from(nalu_bytes),
                duration: Duration::from_millis(FRAME_DURATION_MS),
                ..Sample::new(Instant::now())
            };
            let _ = tauri::async_runtime::block_on(async {
                if let Err(e) = writer.write_sample(&sample).await {
                    warn!("WebRTC write_sample error: {}", e);
                }
            });
        }

        // Throttle to 30 FPS
        let elapsed = frame_start.elapsed();
        if elapsed < Duration::from_millis(FRAME_DURATION_MS) {
            std::thread::sleep(Duration::from_millis(FRAME_DURATION_MS) - elapsed);
        }
    }

    info!("1080p capture loop terminated for HWND: {}", hwnd_val);
}

/// Capture game window and scale directly to 1920x1080 BGRA
#[cfg(windows)]
fn capture_window_1080p_into(hwnd_val: isize, out_bgra: &mut [u8]) -> bool {
    use std::mem::{size_of, zeroed};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HALFTONE, HDC, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic};

    let hwnd = HWND(hwnd_val);

    unsafe {
        if IsIconic(hwnd).as_bool() {
            return false;
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

        // Capture source window
        let hdc_src = CreateCompatibleDC(hdc_screen);
        let hbmp_src = CreateCompatibleBitmap(hdc_screen, src_w, src_h);
        let old_src = SelectObject(hdc_src, hbmp_src);

        #[link(name = "user32")]
        extern "system" {
            fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> windows::Win32::Foundation::BOOL;
        }

        // PW_RENDERFULLCONTENT = 2
        let printed = PrintWindow(hwnd, HDC(hdc_src.0), 2);
        if !printed.as_bool() {
            // Fallback to window DC BitBlt
            let hdc_win = GetDC(hwnd);
            if !hdc_win.is_invalid() {
                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_src, 0, 0, src_w, src_h, hdc_win, 0, 0, SRCCOPY,
                );
                ReleaseDC(hwnd, hdc_win);
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
                let current = self.viewers.fetch_add(1, Ordering::SeqCst) + 1;
                ACTIVE_STREAM_VIEWERS.store(current, Ordering::SeqCst);
                self.force_keyframe.store(true, Ordering::SeqCst);
                info!(
                    "Viewer '{}' CONNECTED! Active viewers: {}",
                    self.viewer_id, current
                );
            }
            RTCPeerConnectionState::Disconnected
            | RTCPeerConnectionState::Failed
            | RTCPeerConnectionState::Closed => {
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

/// WebSocket signaling connection to PeerJS Cloud (0.peerjs.com)
async fn run_signaling_loop(
    stream_id: String,
    running: Arc<AtomicBool>,
    video_track: Arc<TrackLocalStaticSample>,
    peer_connections: Arc<RwLock<HashMap<String, Arc<dyn PeerConnection>>>>,
    viewer_count: Arc<AtomicUsize>,
    force_keyframe: Arc<AtomicBool>,
    ws_tx: mpsc::Sender<String>,
    mut ws_rx: mpsc::Receiver<String>,
) {
    let token = format!("{:x}", rand::random::<u64>());
    let ws_url = format!(
        "wss://0.peerjs.com/peerjs?key=peerjs&id={}&token={}",
        stream_id, token
    );

    info!("Connecting to PeerJS signaling server: {}", ws_url);

    let (ws_stream, _) = match connect_async(&ws_url).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to PeerJS broker: {}", e);
            return;
        }
    };

    let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();
    info!("Registered stream ID '{}' with PeerJS broker!", stream_id);

    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            _ = heartbeat_interval.tick() => {
                let hb = serde_json::json!({ "type": "HEARTBEAT" }).to_string();
                if let Err(e) = ws_sink.send(WsMessage::Text(hb)).await {
                    warn!("PeerJS heartbeat send failed: {}", e);
                    break;
                }
            }
            Some(out_msg) = ws_rx.recv() => {
                if let Err(e) = ws_sink.send(WsMessage::Text(out_msg)).await {
                    warn!("Failed to send signaling message: {}", e);
                    break;
                }
            }
            msg_opt = ws_stream_rx.next() => {
                match msg_opt {
                    Some(Ok(WsMessage::Text(text))) => {
                        handle_peerjs_message(
                            &text,
                            &stream_id,
                            &video_track,
                            &peer_connections,
                            &viewer_count,
                            &force_keyframe,
                            &ws_tx,
                        ).await;
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        let _ = ws_sink.send(WsMessage::Pong(data)).await;
                    }
                    Some(Ok(WsMessage::Close(_))) | None => {
                        info!("PeerJS signaling connection closed");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("PeerJS signaling error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    info!("Signaling loop finished for stream: {}", stream_id);
}

/// Handle messages from PeerJS signaling broker
async fn handle_peerjs_message(
    text: &str,
    my_stream_id: &str,
    video_track: &Arc<TrackLocalStaticSample>,
    peer_connections: &Arc<RwLock<HashMap<String, Arc<dyn PeerConnection>>>>,
    viewer_count: &Arc<AtomicUsize>,
    force_keyframe: &Arc<AtomicBool>,
    ws_tx: &mpsc::Sender<String>,
) {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let src = json.get("src").and_then(|s| s.as_str()).unwrap_or("");

    match msg_type {
        "OFFER" => {
            let payload = match json.get("payload") {
                Some(p) => p,
                None => return,
            };
            let sdp_val = payload.get("sdp").and_then(|s| s.get("sdp")).and_then(|s| s.as_str());
            let conn_id = payload.get("connectionId").and_then(|c| c.as_str()).unwrap_or("mc_0");

            if let Some(sdp_str) = sdp_val {
                info!("Received WebRTC OFFER from viewer: {}", src);

                let video_codec = create_h264_codec();
                let mut media_engine = MediaEngine::default();
                if let Err(e) = media_engine.register_codec(video_codec, RtpCodecKind::Video) {
                    error!("Failed to register video codec: {}", e);
                    return;
                }

                // Configure WebRTC PeerConnection with Google STUN
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
                    viewer_id: src.to_string(),
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
                let track_dyn = Arc::clone(video_track) as Arc<dyn TrackLocal>;
                if let Err(e) = pc.add_track(track_dyn).await {
                    error!("Failed to add track to PeerConnection: {}", e);
                    return;
                }

                // Parse Offer SDP and set remote description
                let offer_desc = match RTCSessionDescription::offer(sdp_str.to_string()) {
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

                // Wait briefly for ICE candidates to gather into local_description
                let _ = tokio::time::timeout(Duration::from_millis(600), gather_complete_rx.recv()).await;

                if let Some(local_desc) = pc.local_description().await {
                    // Send ANSWER back to viewer through PeerJS broker
                    let answer_msg = serde_json::json!({
                        "type": "ANSWER",
                        "src": my_stream_id,
                        "dst": src,
                        "payload": {
                            "sdp": {
                                "type": "answer",
                                "sdp": local_desc.sdp
                            },
                            "type": "media",
                            "connectionId": conn_id
                        }
                    });

                    let _ = ws_tx.send(answer_msg.to_string()).await;
                    force_keyframe.store(true, Ordering::SeqCst);
                }

                // Store PeerConnection
                {
                    let mut map = peer_connections.write().await;
                    map.insert(src.to_string(), pc);
                }
            }
        }
        "OPEN" => {
            info!("PeerJS registered successfully: {}", my_stream_id);
        }
        _ => {}
    }
}
