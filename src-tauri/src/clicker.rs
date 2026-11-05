//! Background auto-clicker for target game windows.
//!
//! Sends periodic keystrokes to target windows and automatically stops
//! upon detecting specific packet patterns or hotkeys.

use crate::config::{AppConfig, ClickerConfig};
use crate::input_sender;
use crate::screen_capture;
use crate::window_filter::{WindowFilterMode, WindowFilterState};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickerStatus {
    pub running: bool,
    pub count: u64,
    pub last_stop_reason: Option<String>,
    pub config: ClickerConfig,
}

#[derive(Clone)]
pub struct ClickerState {
    pub running: Arc<AtomicBool>,
    pub count: Arc<AtomicU64>,
    pub last_stop_reason: Arc<std::sync::Mutex<Option<String>>>,
    stop_tx: Arc<std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}

impl Default for ClickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickerState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            count: Arc::new(AtomicU64::new(0)),
            last_stop_reason: Arc::new(std::sync::Mutex::new(None)),
            stop_tx: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn get_status(&self, config: &AppConfig) -> ClickerStatus {
        ClickerStatus {
            running: self.running.load(Ordering::SeqCst),
            count: self.count.load(Ordering::SeqCst),
            last_stop_reason: self.last_stop_reason.lock().unwrap().clone(),
            config: config.clicker.clone(),
        }
    }

    pub fn start(
        &self,
        app: &AppHandle,
        config_arc: &Arc<Mutex<AppConfig>>,
        filter: &WindowFilterState,
    ) {
        if self.running.swap(true, Ordering::SeqCst) {
            info!("Clicker start requested, but it is already running");
            return;
        }

        if let Ok(mut reason_guard) = self.last_stop_reason.lock() {
            *reason_guard = None;
        }

        let (tx, mut rx) = tokio::sync::watch::channel(false);
        if let Ok(mut stop_guard) = self.stop_tx.lock() {
            *stop_guard = Some(tx);
        }

        let running_clone = self.running.clone();
        let count_clone = self.count.clone();
        let app_clone = app.clone();
        let config_arc_clone = config_arc.clone();
        let filter_clone = filter.clone();

        {
            let app_h = app.clone();
            let cfg_arc = config_arc.clone();
            let self_clone = self.clone();
            tauri::async_runtime::spawn(async move {
                let cfg = cfg_arc.lock().await;
                let status = self_clone.get_status(&cfg);
                let _ = app_h.emit("clicker-status-changed", status);
            });
        }

        tauri::async_runtime::spawn(async move {
            info!("Clicker background loop started");

            loop {
                if !running_clone.load(Ordering::SeqCst) {
                    break;
                }

                let (key, interval_ms, proc_name) = {
                    let cfg = config_arc_clone.lock().await;
                    (
                        cfg.clicker.key.clone(),
                        cfg.clicker.interval_ms.max(50),
                        cfg.target.process_name.clone(),
                    )
                };

                let target_hwnd = match filter_clone.get_mode() {
                    WindowFilterMode::Single(info) => {
                        #[cfg(windows)]
                        {
                            use windows::Win32::Foundation::HWND;
                            use windows::Win32::UI::WindowsAndMessaging::IsWindow;
                            if unsafe { IsWindow(HWND(info.hwnd as _)).as_bool() } {
                                Some(info.hwnd)
                            } else {
                                None
                            }
                        }
                        #[cfg(not(windows))]
                        Some(info.hwnd)
                    }
                    WindowFilterMode::All => {
                        let windows = screen_capture::find_target_windows(&proc_name);
                        windows.first().map(|w| w.hwnd)
                    }
                };

                if let Some(hwnd) = target_hwnd {
                    match input_sender::send_key_background(hwnd, &key, 75).await {
                        Ok(()) => {
                            let new_count = count_clone.fetch_add(1, Ordering::SeqCst) + 1;
                            let _ = app_clone.emit("clicker-tick", new_count);
                        }
                        Err(e) => {
                            warn!("Clicker failed to send key '{}': {}", key, e);
                        }
                    }
                } else {
                    debug!("Clicker: target window for '{}' not found", proc_name);
                }

                tokio::select! {
                    _ = rx.changed() => {
                        debug!("Clicker sleep aborted by stop signal");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)) => {}
                }
            }

            running_clone.store(false, Ordering::SeqCst);
            info!("Clicker background loop finished");
        });
    }

    pub fn stop(
        &self,
        app: &AppHandle,
        reason: Option<String>,
        config_arc: &Arc<Mutex<AppConfig>>,
    ) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return;
        }

        if let Ok(mut reason_guard) = self.last_stop_reason.lock() {
            *reason_guard = reason.clone();
        }

        if let Ok(mut stop_guard) = self.stop_tx.lock() {
            if let Some(tx) = stop_guard.take() {
                let _ = tx.send(true);
            }
        }

        info!("Clicker stopped: reason={:?}", reason);

        let app_h = app.clone();
        let cfg_arc = config_arc.clone();
        let self_clone = self.clone();
        tauri::async_runtime::spawn(async move {
            let cfg = cfg_arc.lock().await;
            let status = self_clone.get_status(&cfg);
            let _ = app_h.emit("clicker-status-changed", status);
        });
    }

    pub fn trigger_pattern_stop(
        &self,
        pattern_name: &str,
        app: &AppHandle,
        config_arc: &Arc<Mutex<AppConfig>>,
        rt: &tokio::runtime::Handle,
    ) {
        if !self.is_running() {
            return;
        }

        let cfg = rt.block_on(async { config_arc.lock().await.clone() });

        if !cfg.clicker.stop_on_pattern {
            return;
        }

        if !cfg.clicker.stop_patterns.is_empty() {
            let pattern_lower = pattern_name.to_lowercase();
            let matches = cfg
                .clicker
                .stop_patterns
                .iter()
                .any(|p| p.to_lowercase() == pattern_lower);
            if !matches {
                return;
            }
        }

        let reason = format!("Сработал паттерн '{}'", pattern_name);
        self.stop(app, Some(reason.clone()), config_arc);

        if cfg.clicker.notify_on_stop {
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("EW Notify — Кликер остановлен")
                .body(&reason)
                .show();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clicker_state_default() {
        let state = ClickerState::new();
        assert!(!state.is_running());
        assert_eq!(state.count.load(Ordering::SeqCst), 0);
    }
}
