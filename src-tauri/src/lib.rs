pub mod config;
pub mod crypto;
pub mod logging;
pub mod packet;
pub mod packet_log;
pub mod platform;
pub mod process_net;
pub mod telegram;
pub mod tray;

use config::{AppConfig, CaptureMode, Pattern};
use packet::{
    build_filter, build_filter_from_endpoints, extract_connection_key, parse_hex, PacketAnalyzer,
    PacketCapture,
};
use process_net::RemoteEndpoint;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio::sync::Mutex;
use tracing::info;

#[derive(Clone, Serialize)]
struct NotificationEvent {
    title: String,
    body: String,
}

#[derive(Clone, Serialize)]
pub struct StoredNotification {
    pub title: String,
    pub body: String,
    pub time: String,
}

#[derive(Clone, Serialize)]
pub struct MonitoringStatus {
    pub active: bool,
    pub process_name: String,
    pub tcp_port: Option<u16>,
    pub filter: String,
    pub packets_seen: u64,
    pub matches_seen: u64,
    pub dedup_suppressed: u64,
    pub process_filter_active: bool,
    pub bot_running: bool,
    pub last_error: String,
    pub is_admin: bool,
    pub config_path: String,
    pub log_path: String,
    pub capture_mode: String,
    pub discovered_endpoints: Vec<String>,
    pub packet_log_enabled: bool,
    pub packet_log_skipped: u64,
}

#[derive(Clone, Serialize)]
pub struct PatternMatchResult {
    pub matched: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub pattern_bytes: Option<String>,
}

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub capture: Arc<std::sync::Mutex<Option<Arc<PacketCapture>>>>,
    pub monitoring: Arc<AtomicBool>,
    pub bot_running: Arc<AtomicBool>,
    pub packets_seen: Arc<AtomicU64>,
    pub matches_seen: Arc<AtomicU64>,
    pub dedup_suppressed: Arc<AtomicU64>,
    pub active_filter: Arc<std::sync::Mutex<String>>,
    pub active_tcp_port: Arc<std::sync::Mutex<Option<u16>>>,
    pub active_process_name: Arc<std::sync::Mutex<String>>,
    pub active_capture_mode: Arc<std::sync::Mutex<CaptureMode>>,
    pub discovered_endpoints: Arc<std::sync::Mutex<Vec<RemoteEndpoint>>>,
    pub recent_notifications: Arc<std::sync::Mutex<Vec<StoredNotification>>>,
    pub last_error: Arc<std::sync::Mutex<String>>,
    pub log_buffer: logging::LogBuffer,
    pub packet_log: Arc<packet_log::PacketLog>,
    #[cfg(windows)]
    pub analyzer: Arc<std::sync::Mutex<PacketAnalyzer>>,
}

fn stop_capture(state: &AppState) {
    state.monitoring.store(false, Ordering::SeqCst);
    // Take the slot so a dying thread cannot shutdown a newly opened capture.
    if let Ok(mut guard) = state.capture.lock() {
        if let Some(capture) = guard.take() {
            capture.shutdown();
        }
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn manual_filter(tcp_port: Option<u16>, udp_port: Option<u16>, remote_ip: &Option<String>) -> String {
    let remote_ip_ref = remote_ip.as_deref().filter(|s| !s.is_empty());
    build_filter(tcp_port, udp_port, remote_ip_ref)
}

async fn start_monitoring_inner(state: &AppState, app: &AppHandle) -> Result<String, String> {
    let had_capture = state
        .capture
        .lock()
        .ok()
        .map(|g| g.is_some())
        .unwrap_or(false);
    if state.monitoring.load(Ordering::SeqCst) || had_capture {
        stop_capture(state);
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    let config = state.config.lock().await;
    let process_name = config.target.process_name.clone();
    let capture_mode = config.target.capture_mode;
    let tcp_port = config.target.tcp_port;
    let udp_port = config.target.udp_port;
    let remote_ip = config.target.remote_ip.clone();
    let patterns = config.target.patterns.clone();
    drop(config);

    if patterns.is_empty() {
        tracing::warn!("No patterns loaded — matches will never fire");
    }

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            *analyzer = PacketAnalyzer::new(patterns.clone());
            info!(
                "Analyzer ready with {} pattern(s)",
                analyzer.get_patterns().len()
            );
        }
    }

    if let Ok(mut n) = state.active_process_name.lock() {
        *n = process_name.clone();
    }
    if let Ok(mut m) = state.active_capture_mode.lock() {
        *m = capture_mode;
    }
    if let Ok(mut d) = state.discovered_endpoints.lock() {
        d.clear();
    }
    if let Ok(mut f) = state.active_filter.lock() {
        *f = String::new();
    }
    if let Ok(mut p) = state.active_tcp_port.lock() {
        *p = tcp_port;
    }

    state.packets_seen.store(0, Ordering::SeqCst);
    state.matches_seen.store(0, Ordering::SeqCst);
    state.dedup_suppressed.store(0, Ordering::SeqCst);
    state.monitoring.store(true, Ordering::SeqCst);
    if let Ok(mut e) = state.last_error.lock() {
        *e = match capture_mode {
            CaptureMode::Auto => format!("waiting for process {process_name}…"),
            CaptureMode::Manual => String::new(),
        };
    }

    info!(
        "Starting monitoring mode={:?} process={} tcp_port={:?} remote_ip={:?}",
        capture_mode, process_name, tcp_port, remote_ip
    );

    let monitoring = state.monitoring.clone();
    let capture_slot = state.capture.clone();
    let config = state.config.clone();
    let packets_seen = state.packets_seen.clone();
    let matches_seen = state.matches_seen.clone();
    let dedup_suppressed = state.dedup_suppressed.clone();
    let recent_notifications = state.recent_notifications.clone();
    let last_error = state.last_error.clone();
    let active_filter = state.active_filter.clone();
    let active_tcp_port = state.active_tcp_port.clone();
    let discovered_endpoints = state.discovered_endpoints.clone();
    let packet_log = state.packet_log.clone();
    let rt = tokio::runtime::Handle::current();

    #[cfg(windows)]
    let analyzer = state.analyzer.clone();

    let app_notify = app.clone();
    let mode_label = match capture_mode {
        CaptureMode::Auto => "auto",
        CaptureMode::Manual => "manual",
    };

    std::thread::spawn(move || {
        info!("Monitoring supervisor started ({mode_label})");
        let mut last_fire: HashMap<String, Instant> = HashMap::new();
        let mut last_burst: HashMap<String, (Instant, String)> = HashMap::new();

        while monitoring.load(Ordering::SeqCst) {
            // Resolve filter for this capture session
            let (filter, endpoints, session_port) = match capture_mode {
                CaptureMode::Manual => {
                    let f = manual_filter(tcp_port, udp_port, &remote_ip);
                    (f, Vec::new(), tcp_port)
                }
                CaptureMode::Auto => {
                    let mut eps = process_net::discover_remote_endpoints(&process_name);
                    while eps.is_empty() && monitoring.load(Ordering::SeqCst) {
                        let pids = process_net::find_pids_by_name(&process_name);
                        let msg = if pids.is_empty() {
                            format!("waiting for process {process_name}…")
                        } else {
                            format!(
                                "process found (pid {}), waiting for ESTABLISHED TCP…",
                                pids[0]
                            )
                        };
                        if let Ok(mut err) = last_error.lock() {
                            *err = msg;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        eps = process_net::discover_remote_endpoints(&process_name);
                    }
                    if !monitoring.load(Ordering::SeqCst) {
                        break;
                    }
                    let f = build_filter_from_endpoints(&eps);
                    let port = eps.first().map(|e| e.port);
                    (f, eps, port)
                }
            };

            if let Ok(mut d) = discovered_endpoints.lock() {
                *d = endpoints.clone();
            }
            if let Ok(mut f) = active_filter.lock() {
                *f = filter.clone();
            }
            if let Ok(mut p) = active_tcp_port.lock() {
                *p = session_port;
            }
            if let Ok(mut err) = last_error.lock() {
                if err.starts_with("waiting for") {
                    err.clear();
                }
            }

            info!("Opening capture: {}", filter);
            let capture = match PacketCapture::open(&filter) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    let msg = format!("WinDivertOpen failed (run as Administrator?): {e}");
                    tracing::error!("{}", msg);
                    if let Ok(mut err) = last_error.lock() {
                        *err = msg;
                    }
                    // Retry after pause in auto; stop in manual
                    if capture_mode == CaptureMode::Manual {
                        monitoring.store(false, Ordering::SeqCst);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                }
            };

            {
                if let Ok(mut guard) = capture_slot.lock() {
                    *guard = Some(capture.clone());
                }
            }

            // Watcher: reopen when auto endpoints change
            let watch_monitoring = monitoring.clone();
            let watch_capture = capture.clone();
            let watch_process = process_name.clone();
            let watch_eps = endpoints.clone();
            let watch_discovered = discovered_endpoints.clone();
            let watcher = if capture_mode == CaptureMode::Auto {
                Some(std::thread::spawn(move || {
                    let current = watch_eps;
                    while watch_monitoring.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        if !watch_monitoring.load(Ordering::SeqCst) {
                            break;
                        }
                        let next = process_net::discover_remote_endpoints(&watch_process);
                        if next.is_empty() {
                            continue;
                        }
                        if next != current {
                            info!(
                                "Remote endpoints changed: {:?} → {:?} — reopening filter",
                                current, next
                            );
                            if let Ok(mut d) = watch_discovered.lock() {
                                *d = next;
                            }
                            watch_capture.shutdown();
                            break;
                        }
                    }
                }))
            } else {
                None
            };

            // Capture loop for this filter session
            let session_start_packets = packets_seen.load(Ordering::SeqCst);
            while monitoring.load(Ordering::SeqCst) {
                let raw_packet = match capture.read_packet() {
                    Ok(p) => p,
                    Err(e) => {
                        if monitoring.load(Ordering::SeqCst) {
                            tracing::info!("recv ended (reopen or stop): {}", e);
                        }
                        break;
                    }
                };

                let n = packets_seen.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 || n % 50 == 0 {
                    info!("Captured {} packets so far", n);
                }
                if n == session_start_packets + 1 {
                    if let Ok(mut err) = last_error.lock() {
                        if err.starts_with("waiting for packets") {
                            err.clear();
                        }
                    }
                }

                // Packet dump (full payload hex), rate-limited
                if packet_log.is_enabled() {
                    let conn = extract_connection_key(&raw_packet);
                    let payload = PacketAnalyzer::extract_transport_payload(&raw_packet);
                    let (remote, local) = match &conn {
                        Some(c) => (
                            format!("{}:{}", c.remote_ip, c.remote_port),
                            format!("{}:{}", c.local_ip, c.local_port),
                        ),
                        None => ("?".into(), "?".into()),
                    };
                    packet_log.try_push(remote, local, payload);
                }

                #[cfg(windows)]
                let matched = {
                    let Ok(analyzer) = analyzer.lock() else {
                        break;
                    };
                    analyzer.analyze(&raw_packet)
                };

                #[cfg(not(windows))]
                let matched: Option<Pattern> = {
                    let _ = &raw_packet;
                    None
                };

                let Some(pattern) = matched else {
                    continue;
                };

                let conn = extract_connection_key(&raw_packet);
                let cooldown_key = match &conn {
                    Some(c) => c.cooldown_id(&pattern.name),
                    None => pattern.name.clone(),
                };
                let client_label = conn
                    .as_ref()
                    .map(|c| c.short_label())
                    .unwrap_or_else(|| "unknown".into());

                let (cooldown_secs, tray_enabled, telegram_enabled, burst_window_ms) =
                    rt.block_on(async {
                        let cfg = config.lock().await;
                        (
                            cfg.notifications.cooldown_seconds,
                            cfg.notifications.tray_enabled,
                            cfg.notifications.telegram_enabled,
                            cfg.notifications.burst_dedup_ms,
                        )
                    });

                let now = Instant::now();

                if burst_window_ms > 0 {
                    if let Some(c) = &conn {
                        let burst_key =
                            format!("{}@{}:{}", pattern.name, c.remote_ip, c.remote_port);
                        if let Some((prev_t, prev_local_ip)) = last_burst.get(&burst_key) {
                            if now.duration_since(*prev_t).as_millis() < burst_window_ms as u128
                                && prev_local_ip != &c.local_ip
                            {
                                let n = dedup_suppressed.fetch_add(1, Ordering::SeqCst) + 1;
                                tracing::info!(
                                    "Dedup skip {} ({} vs {} within {}ms) #{}",
                                    pattern.name,
                                    prev_local_ip,
                                    c.local_ip,
                                    burst_window_ms,
                                    n
                                );
                                continue;
                            }
                        }
                        last_burst.insert(burst_key, (now, c.local_ip.clone()));
                    }
                }

                if let Some(prev) = last_fire.get(&cooldown_key) {
                    if now.duration_since(*prev).as_secs() < cooldown_secs {
                        continue;
                    }
                }
                last_fire.insert(cooldown_key, now);
                if last_fire.len() > 256 {
                    last_fire.retain(|_, t| {
                        now.duration_since(*t).as_secs()
                            < cooldown_secs.saturating_mul(4).max(60)
                    });
                }
                if last_burst.len() > 128 {
                    last_burst.retain(|_, (t, _)| now.duration_since(*t).as_millis() < 10_000);
                }

                let matches_n = matches_seen.fetch_add(1, Ordering::SeqCst) + 1;
                // User-facing: description only; pattern id without IP/client.
                let notify_title = pattern.description.clone();
                let notify_body = pattern.name.clone();

                tracing::info!(
                    "Pattern detected: {} ({}) client={} (match #{})",
                    pattern.name,
                    pattern.description,
                    client_label,
                    matches_n
                );

                let time = chrono::Local::now().format("%H:%M:%S").to_string();
                let stored = StoredNotification {
                    title: notify_title.clone(),
                    body: notify_body.clone(),
                    time,
                };
                if let Ok(mut list) = recent_notifications.lock() {
                    list.insert(0, stored);
                    list.truncate(50);
                }

                let event = NotificationEvent {
                    title: notify_title.clone(),
                    body: notify_body.clone(),
                };
                let _ = app_notify.emit("notification", event.clone());
                if let Some(win) = app_notify.get_webview_window("main") {
                    let _ = win.emit("notification", event);
                }

                if tray_enabled {
                    // Single toast via Tauri (avoid duplicate notify_rust).
                    use tauri_plugin_notification::NotificationExt;
                    if let Err(e) = app_notify
                        .notification()
                        .builder()
                        .title(&notify_title)
                        .body(&notify_body)
                        .show()
                    {
                        tracing::warn!("system notification failed: {}", e);
                    }
                }

                if telegram_enabled {
                    if let Err(e) = rt.block_on(telegram::send_notification(
                        &config,
                        &notify_title,
                        &notify_body,
                    )) {
                        let msg = format!("Telegram: {e}");
                        tracing::warn!("{}", msg);
                        if let Ok(mut err) = last_error.lock() {
                            *err = msg.clone();
                        }
                        let _ = app_notify.emit("bot-error", msg);
                    }
                }
            }

            // End of session: clear slot if ours
            if let Ok(mut guard) = capture_slot.lock() {
                if guard
                    .as_ref()
                    .map(|c| Arc::ptr_eq(c, &capture))
                    .unwrap_or(false)
                {
                    *guard = None;
                }
            }
            capture.shutdown();
            if let Some(w) = watcher {
                let _ = w.join();
            }

            if capture_mode == CaptureMode::Manual {
                break;
            }
            // Auto: short pause then re-resolve / reopen
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        monitoring.store(false, Ordering::SeqCst);
        info!(
            "Monitoring stopped (packets={}, matches={})",
            packets_seen.load(Ordering::SeqCst),
            matches_seen.load(Ordering::SeqCst)
        );
    });

    // Hint while waiting for first packet after filter is up
    let packets_watch = state.packets_seen.clone();
    let monitoring_watch = state.monitoring.clone();
    let last_error_watch = state.last_error.clone();
    let filter_watch = state.active_filter.clone();
    std::thread::spawn(move || {
        for i in 1..=12 {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if !monitoring_watch.load(Ordering::SeqCst) {
                break;
            }
            if packets_watch.load(Ordering::SeqCst) > 0 {
                break;
            }
            let has_filter = filter_watch
                .lock()
                .map(|f| !f.is_empty())
                .unwrap_or(false);
            if !has_filter {
                continue;
            }
            let msg = format!(
                "waiting for packets… {}s (filter ok, no inbound traffic yet)",
                i * 5
            );
            info!("{}", msg);
            if let Ok(mut err) = last_error_watch.lock() {
                if !err.starts_with("waiting for process")
                    && !err.starts_with("process found")
                {
                    *err = msg;
                }
            }
        }
    });

    Ok(format!("Monitoring started ({mode_label})"))
}

#[tauri::command]
async fn start_monitoring(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    start_monitoring_inner(&state, &app).await
}

#[tauri::command]
async fn stop_monitoring(state: State<'_, AppState>) -> Result<String, String> {
    info!("Stopping monitoring");
    stop_capture(&state);
    Ok("Monitoring stopped".to_string())
}

#[tauri::command]
async fn hide_to_tray(app: AppHandle) -> Result<String, String> {
    hide_main_window(&app);
    Ok("Hidden to tray".to_string())
}

#[tauri::command]
async fn get_monitoring_status(state: State<'_, AppState>) -> Result<MonitoringStatus, String> {
    let config = state.config.lock().await;
    let mode = state
        .active_capture_mode
        .lock()
        .map(|g| *g)
        .unwrap_or(config.target.capture_mode);
    let mode_str = match mode {
        CaptureMode::Auto => "auto",
        CaptureMode::Manual => "manual",
    };
    let discovered = state
        .discovered_endpoints
        .lock()
        .map(|g| g.iter().map(|e| e.label()).collect())
        .unwrap_or_default();
    Ok(MonitoringStatus {
        active: state.monitoring.load(Ordering::SeqCst),
        process_name: state
            .active_process_name
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| config.target.process_name.clone()),
        tcp_port: state
            .active_tcp_port
            .lock()
            .map(|g| *g)
            .unwrap_or(config.target.tcp_port),
        filter: state
            .active_filter
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default(),
        packets_seen: state.packets_seen.load(Ordering::SeqCst),
        matches_seen: state.matches_seen.load(Ordering::SeqCst),
        dedup_suppressed: state.dedup_suppressed.load(Ordering::SeqCst),
        process_filter_active: mode == CaptureMode::Auto,
        bot_running: state.bot_running.load(Ordering::SeqCst),
        last_error: state
            .last_error
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default(),
        is_admin: platform::is_elevated(),
        config_path: config::get_config_path().display().to_string(),
        log_path: logging::get_log_path().display().to_string(),
        capture_mode: mode_str.to_string(),
        discovered_endpoints: discovered,
        packet_log_enabled: state.packet_log.is_enabled(),
        packet_log_skipped: state.packet_log.skipped(),
    })
}

#[tauri::command]
async fn get_recent_notifications(
    state: State<'_, AppState>,
) -> Result<Vec<StoredNotification>, String> {
    state
        .recent_notifications
        .lock()
        .map(|g| g.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_logs(state: State<'_, AppState>) -> Result<Vec<logging::LogLine>, String> {
    Ok(state.log_buffer.snapshot())
}

#[tauri::command]
async fn clear_logs(state: State<'_, AppState>) -> Result<String, String> {
    state.log_buffer.clear();
    Ok("Logs cleared".to_string())
}

#[tauri::command]
async fn set_packet_log_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<String, String> {
    state.packet_log.set_enabled(enabled);
    Ok(if enabled {
        "Packet log enabled".into()
    } else {
        "Packet log disabled".into()
    })
}

#[tauri::command]
async fn get_recent_packets(
    state: State<'_, AppState>,
) -> Result<Vec<packet_log::PacketLogEntry>, String> {
    Ok(state.packet_log.snapshot())
}

#[tauri::command]
async fn clear_packet_log(state: State<'_, AppState>) -> Result<String, String> {
    state.packet_log.clear();
    Ok("Packet log cleared".into())
}

#[tauri::command]
async fn add_pattern(
    state: State<'_, AppState>,
    name: String,
    bytes: String,
    description: String,
) -> Result<String, String> {
    info!("Adding pattern: {}", name);
    let pattern = Pattern::new(name.clone(), bytes.clone(), description.clone())
        .map_err(|e| e.to_string())?;

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            analyzer.add_pattern(pattern.clone());
        }
    }

    let mut config = state.config.lock().await;
    config.target.patterns.push(pattern);
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern added".to_string())
}

#[tauri::command]
async fn remove_pattern(state: State<'_, AppState>, name: String) -> Result<String, String> {
    info!("Removing pattern: {}", name);

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            analyzer.remove_pattern(&name);
        }
    }

    let mut config = state.config.lock().await;
    config.target.patterns.retain(|p| p.name != name);
    config::save_config(&config).map_err(|e| e.to_string())?;

    Ok("Pattern removed".to_string())
}

/// Restore built-in DEATH / STOP_FARM patterns only (other settings untouched).
#[tauri::command]
async fn reset_patterns_to_defaults(
    state: State<'_, AppState>,
) -> Result<Vec<Pattern>, String> {
    info!("Resetting patterns to defaults");
    let patterns = config::default_patterns();

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            *analyzer = PacketAnalyzer::new(patterns.clone());
        }
    }

    let mut config = state.config.lock().await;
    config.target.patterns = patterns.clone();
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(patterns)
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    // Prefer in-memory (bot may have newer users); merge disk only if memory empty of users?
    // Always return memory — bot updates memory + disk together.
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
async fn reload_config_from_disk(state: State<'_, AppState>) -> Result<AppConfig, String> {
    match config::load_config() {
        Ok(new_config) => {
            let mut config = state.config.lock().await;
            // Keep in-memory registered_users if disk is older/empty race
            let mem_users = config.telegram.registered_users.clone();
            *config = new_config;
            if config.telegram.registered_users.is_empty() && !mem_users.is_empty() {
                config.telegram.registered_users = mem_users;
            }
            Ok(config.clone())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn update_config(
    state: State<'_, AppState>,
    app: AppHandle,
    new_config: AppConfig,
) -> Result<String, String> {
    info!("Updating config");

    let was_monitoring = state.monitoring.load(Ordering::SeqCst);
    let old_port = {
        let current = state.config.lock().await;
        current.target.tcp_port
    };
    let old_remote_ip = {
        let current = state.config.lock().await;
        current.target.remote_ip.clone()
    };
    let old_mode = {
        let current = state.config.lock().await;
        current.target.capture_mode
    };
    let old_process = {
        let current = state.config.lock().await;
        current.target.process_name.clone()
    };
    let filter_changed = was_monitoring
        && (old_port != new_config.target.tcp_port
            || old_remote_ip.as_deref().unwrap_or("")
                != new_config.target.remote_ip.as_deref().unwrap_or("")
            || old_mode != new_config.target.capture_mode
            || old_process != new_config.target.process_name);

    let mut new_config = new_config;
    // Don't let a settings save wipe patterns if the UI sent an empty list
    {
        let current = state.config.lock().await;
        if new_config.target.patterns.is_empty() && !current.target.patterns.is_empty() {
            new_config.target.patterns = current.target.patterns.clone();
        }
    }
    config::ensure_default_patterns(&mut new_config);

    // Normalize empty remote_ip to None
    if new_config
        .target
        .remote_ip
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(false)
    {
        new_config.target.remote_ip = None;
    }

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            *analyzer = PacketAnalyzer::new(new_config.target.patterns.clone());
        }
    }

    let mut config = state.config.lock().await;
    *config = new_config.clone();
    config::save_config(&config).map_err(|e| e.to_string())?;
    drop(config);

    if filter_changed {
        match start_monitoring_inner(&state, &app).await {
            Ok(msg) => {
                return Ok(format!("Config updated; monitoring restarted — {msg}"));
            }
            Err(e) => {
                return Ok(format!(
                    "Config updated; monitoring restart failed: {e}"
                ));
            }
        }
    }

    Ok("Config updated".to_string())
}

/// Replace in-memory + on-disk config with built-in defaults (DEATH/STOP_FARM, port 7777, etc.).
#[tauri::command]
async fn reset_config_to_defaults(state: State<'_, AppState>) -> Result<AppConfig, String> {
    info!("Resetting config to defaults");
    let default = AppConfig::default();

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            *analyzer = PacketAnalyzer::new(default.target.patterns.clone());
        }
    }

    let mut config = state.config.lock().await;
    *config = default.clone();
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
async fn get_patterns(state: State<'_, AppState>) -> Result<Vec<Pattern>, String> {
    let config = state.config.lock().await;
    Ok(config.target.patterns.clone())
}

#[tauri::command]
async fn test_telegram(token: String) -> Result<String, String> {
    telegram::test_connection(&token)
        .await
        .map_err(|e| e.to_string())?;
    Ok("Telegram connected successfully!".to_string())
}

#[tauri::command]
async fn start_telegram_bot(
    state: State<'_, AppState>,
    app: AppHandle,
    token: String,
) -> Result<String, String> {
    if token.is_empty() {
        return Err("Token is empty".to_string());
    }

    // Stop previous bot loop if any
    if state.bot_running.load(Ordering::SeqCst) {
        state.bot_running.store(false, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    }

    let config = state.config.clone();
    let running = state.bot_running.clone();
    // Persist token
    {
        let mut cfg = state.config.lock().await;
        cfg.telegram.bot_token = token.clone();
        let _ = config::save_config(&cfg);
    }

    tokio::spawn(async move {
        if let Err(e) = telegram::start_bot(&token, config, running, app).await {
            tracing::error!("Failed to start telegram bot: {}", e);
        }
    });

    // Give the loop a moment to set running=true
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok("Telegram bot started!".to_string())
}

#[tauri::command]
async fn stop_telegram_bot(state: State<'_, AppState>) -> Result<String, String> {
    state.bot_running.store(false, Ordering::SeqCst);
    Ok("Telegram bot stop requested".to_string())
}

#[tauri::command]
async fn get_registered_users(
    state: State<'_, AppState>,
) -> Result<Vec<config::RegisteredUser>, String> {
    let config = state.config.lock().await;
    Ok(config.telegram.registered_users.clone())
}

#[tauri::command]
async fn set_user_enabled(
    state: State<'_, AppState>,
    chat_id: String,
    enabled: bool,
) -> Result<String, String> {
    let mut config = state.config.lock().await;
    config::set_user_enabled(&mut config, &chat_id, enabled);
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(format!(
        "User {} {}",
        chat_id,
        if enabled { "enabled" } else { "disabled" }
    ))
}

#[tauri::command]
async fn set_notify_all_users(state: State<'_, AppState>, enabled: bool) -> Result<String, String> {
    let mut config = state.config.lock().await;
    config.telegram.notify_all_users = enabled;
    config::save_config(&config).map_err(|e| e.to_string())?;
    Ok(format!("Notify all users: {}", enabled))
}

#[tauri::command]
async fn export_patterns(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.lock().await;
    serde_json::to_string_pretty(&config.target.patterns).map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_patterns(
    state: State<'_, AppState>,
    json: String,
    replace: bool,
) -> Result<Vec<Pattern>, String> {
    let mut incoming: Vec<Pattern> =
        serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {e}"))?;
    for p in &incoming {
        Pattern::new(p.name.clone(), p.bytes.clone(), p.description.clone())
            .map_err(|e| format!("{}: {e}", p.name))?;
    }

    let mut config = state.config.lock().await;
    if replace {
        config.target.patterns = incoming;
    } else {
        for p in incoming.drain(..) {
            if let Some(existing) = config
                .target
                .patterns
                .iter_mut()
                .find(|x| x.name == p.name)
            {
                *existing = p;
            } else {
                config.target.patterns.push(p);
            }
        }
    }
    config::ensure_default_patterns(&mut config);
    let patterns = config.target.patterns.clone();
    config::save_config(&config).map_err(|e| e.to_string())?;
    drop(config);

    #[cfg(windows)]
    {
        if let Ok(mut analyzer) = state.analyzer.lock() {
            *analyzer = PacketAnalyzer::new(patterns.clone());
        }
    }

    Ok(patterns)
}

#[tauri::command]
async fn test_pattern_hex(
    state: State<'_, AppState>,
    hex: String,
) -> Result<PatternMatchResult, String> {
    let bytes = parse_hex(&hex).map_err(|e| e.to_string())?;
    let config = state.config.lock().await;
    let analyzer = PacketAnalyzer::new(config.target.patterns.clone());
    drop(config);

    match analyzer.match_payload(&bytes) {
        Some(p) => Ok(PatternMatchResult {
            matched: true,
            name: Some(p.name),
            description: Some(p.description),
            pattern_bytes: Some(p.bytes),
        }),
        None => Ok(PatternMatchResult {
            matched: false,
            name: None,
            description: None,
            pattern_bytes: None,
        }),
    }
}

pub fn run() {
    if let Err(e) = platform::ensure_single_instance() {
        eprintln!("{e}");
        std::process::exit(1);
    }

    let log_buffer = logging::LogBuffer::new();
    logging::init(log_buffer.clone());

    info!("Starting EndlessWar Notifier");
    info!(
        "Admin elevated: {} | config: {}",
        platform::is_elevated(),
        config::get_config_path().display()
    );

    let config = config::load_config().unwrap_or_else(|_| {
        let default = AppConfig::default();
        let _ = config::save_config(&default);
        default
    });
    info!(
        "Config loaded: tcp_port={:?} remote_ip={:?} patterns={}",
        config.target.tcp_port,
        config.target.remote_ip,
        config.target.patterns.len()
    );

    let app_state = AppState {
        config: Arc::new(Mutex::new(config.clone())),
        capture: Arc::new(std::sync::Mutex::new(None)),
        monitoring: Arc::new(AtomicBool::new(false)),
        bot_running: Arc::new(AtomicBool::new(false)),
        packets_seen: Arc::new(AtomicU64::new(0)),
        matches_seen: Arc::new(AtomicU64::new(0)),
        dedup_suppressed: Arc::new(AtomicU64::new(0)),
        active_filter: Arc::new(std::sync::Mutex::new(String::new())),
        active_tcp_port: Arc::new(std::sync::Mutex::new(None)),
        active_process_name: Arc::new(std::sync::Mutex::new(String::new())),
        active_capture_mode: Arc::new(std::sync::Mutex::new(config.target.capture_mode)),
        discovered_endpoints: Arc::new(std::sync::Mutex::new(Vec::new())),
        recent_notifications: Arc::new(std::sync::Mutex::new(Vec::new())),
        last_error: Arc::new(std::sync::Mutex::new(String::new())),
        log_buffer,
        packet_log: packet_log::PacketLog::new(),
        #[cfg(windows)]
        analyzer: Arc::new(std::sync::Mutex::new(PacketAnalyzer::new(
            config.target.patterns.clone(),
        ))),
    };

    let auto_start_token = if config.telegram.auto_start && !config.telegram.bot_token.is_empty() {
        Some(config.telegram.bot_token.clone())
    } else {
        None
    };
    let auto_start_monitoring = config.startup.auto_start_monitoring;
    let start_minimized = config.startup.start_minimized_to_tray;

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            info!("Setup started");
            let start = MenuItem::with_id(app, "start", "Старт мониторинга", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, "stop", "Стоп мониторинга", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Показать окно", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Свернуть в трей", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&start, &stop, &show, &hide, &quit])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("EndlessWar Notifier")
                .show_menu_on_left_click(false);

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "start" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            match start_monitoring_inner(&state, &app).await {
                                Ok(msg) => info!("{}", msg),
                                Err(e) => tracing::error!("tray start monitoring failed: {}", e),
                            }
                            let _ = app.emit("tray-start", ());
                        });
                    }
                    "stop" => {
                        let state = app.state::<AppState>();
                        stop_capture(&state);
                        info!("Monitoring stopped from tray");
                        let _ = app.emit("tray-stop", ());
                    }
                    "show" => show_main_window(app),
                    "hide" => hide_main_window(app),
                    "quit" => {
                        // Allow real exit: stop capture then exit
                        let state = app.state::<AppState>();
                        stop_capture(&state);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let state = app_handle.state::<AppState>();
                        let close_to_tray = tauri::async_runtime::block_on(async {
                            state.config.lock().await.startup.close_to_tray
                        });
                        if close_to_tray {
                            api.prevent_close();
                            hide_main_window(&app_handle);
                            info!("Window hidden to tray (close_to_tray)");
                        }
                    }
                });

                if start_minimized {
                    info!("Starting minimized to tray");
                    let _ = window.hide();
                } else {
                    info!("Showing main window");
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            if let Some(token) = auto_start_token.clone() {
                info!("Auto-starting Telegram bot...");
                let handle = app.handle().clone();
                let state = app.state::<AppState>();
                let config = state.config.clone();
                let running = state.bot_running.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = telegram::start_bot(&token, config, running, handle).await {
                        tracing::error!("Failed to start telegram bot: {}", e);
                    }
                });
            }

            if auto_start_monitoring {
                info!("Auto-starting monitoring...");
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Small delay so WinDivert/driver is ready after process start
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    let state = handle.state::<AppState>();
                    match start_monitoring_inner(&state, &handle).await {
                        Ok(msg) => info!("Auto monitoring: {}", msg),
                        Err(e) => {
                            tracing::error!("Auto-start monitoring failed: {}", e);
                            if let Ok(mut err) = state.last_error.lock() {
                                *err = format!("auto-start: {e}");
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_monitoring,
            stop_monitoring,
            hide_to_tray,
            get_monitoring_status,
            get_recent_notifications,
            get_recent_logs,
            clear_logs,
            set_packet_log_enabled,
            get_recent_packets,
            clear_packet_log,
            add_pattern,
            remove_pattern,
            reset_patterns_to_defaults,
            export_patterns,
            import_patterns,
            test_pattern_hex,
            get_config,
            reload_config_from_disk,
            update_config,
            reset_config_to_defaults,
            get_patterns,
            test_telegram,
            start_telegram_bot,
            stop_telegram_bot,
            get_registered_users,
            set_user_enabled,
            set_notify_all_users,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
