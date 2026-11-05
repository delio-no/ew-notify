//! Background auto-clicker for target game windows.
//!
//! Supports multiple independent rules/keys, each with its own interval,
//! target window (captured, specific slot, or all), and pattern stop triggers.

use crate::config::{AppConfig, ClickerConfig, ClickerRule, ClickerTarget};
use crate::input_sender;
use crate::screen_capture;
use crate::window_filter::{WindowFilterMode, WindowFilterState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    active_rules: Arc<std::sync::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
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
            active_rules: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn get_status(&self, config: &AppConfig) -> ClickerStatus {
        let mut cfg = config.clicker.clone();
        // Ensure rules list is populated even if loaded from older config format
        if cfg.rules.is_empty() {
            cfg.rules = cfg.get_rules();
        }
        ClickerStatus {
            running: self.running.load(Ordering::SeqCst),
            count: self.count.load(Ordering::SeqCst),
            last_stop_reason: self.last_stop_reason.lock().unwrap().clone(),
            config: cfg,
        }
    }

    pub fn start(
        &self,
        app: &AppHandle,
        config_arc: &Arc<Mutex<AppConfig>>,
        filter: &WindowFilterState,
    ) {
        self.running.store(true, Ordering::SeqCst);

        if let Ok(mut reason_guard) = self.last_stop_reason.lock() {
            *reason_guard = None;
        }

        // Stop any previously running rule loops
        if let Ok(mut map) = self.active_rules.lock() {
            for (_, tx) in map.drain() {
                let _ = tx.send(true);
            }
        }

        let app_clone = app.clone();
        let config_arc_clone = config_arc.clone();
        let filter_clone = filter.clone();
        let self_clone = self.clone();

        tauri::async_runtime::spawn(async move {
            let (rules, proc_name) = {
                let cfg = config_arc_clone.lock().await;
                (cfg.clicker.get_rules(), cfg.target.process_name.clone())
            };

            let enabled_rules: Vec<ClickerRule> = rules.into_iter().filter(|r| r.enabled).collect();
            if enabled_rules.is_empty() {
                warn!("Clicker start requested, but no rules are enabled");
                self_clone.running.store(false, Ordering::SeqCst);
                let cfg = config_arc_clone.lock().await;
                let status = self_clone.get_status(&cfg);
                let _ = app_clone.emit("clicker-status-changed", status);
                return;
            }

            info!(
                "Starting clicker with {} active rule(s) for process '{}'",
                enabled_rules.len(),
                proc_name
            );

            for rule in enabled_rules {
                let (tx, mut rx) = tokio::sync::watch::channel(false);
                if let Ok(mut map) = self_clone.active_rules.lock() {
                    map.insert(rule.id.clone(), tx);
                }

                let rule_id = rule.id.clone();
                let rule_name = if rule.name.is_empty() {
                    format!("Клавиша {}", rule.key)
                } else {
                    rule.name.clone()
                };
                let key = rule.key.trim().to_uppercase();
                let interval_ms = rule.interval_ms.max(50);
                let target = rule.target.clone();

                let running_ref = self_clone.running.clone();
                let count_ref = self_clone.count.clone();
                let app_ref = app_clone.clone();
                let filter_ref = filter_clone.clone();
                let proc_ref = proc_name.clone();

                tauri::async_runtime::spawn(async move {
                    info!(
                        "Clicker rule '{}' started (Key: '{}', Interval: {}ms, Target: {:?})",
                        rule_name, key, interval_ms, target
                    );

                    loop {
                        if !running_ref.load(Ordering::SeqCst) || *rx.borrow() {
                            break;
                        }

                        // Resolve target HWND(s) according to this rule's independent setting
                        let target_hwnds: Vec<isize> = match &target {
                            ClickerTarget::Captured => {
                                match filter_ref.get_mode() {
                                    WindowFilterMode::Single(info) => {
                                        #[cfg(windows)]
                                        {
                                            use windows::Win32::Foundation::HWND;
                                            use windows::Win32::UI::WindowsAndMessaging::IsWindow;
                                            if unsafe { IsWindow(HWND(info.hwnd as _)).as_bool() } {
                                                vec![info.hwnd]
                                            } else {
                                                Vec::new()
                                            }
                                        }
                                        #[cfg(not(windows))]
                                        vec![info.hwnd]
                                    }
                                    WindowFilterMode::All => Vec::new(),
                                }
                            }
                            ClickerTarget::Slot(slot) => {
                                let current_mode = filter_ref.get_mode();
                                if let WindowFilterMode::Single(ref info) = current_mode {
                                    if info.slot == *slot {
                                        vec![info.hwnd]
                                    } else {
                                        let windows = screen_capture::find_target_windows(&proc_ref);
                                        let idx = (*slot as usize).saturating_sub(1);
                                        windows.get(idx).map(|w| vec![w.hwnd]).unwrap_or_default()
                                    }
                                } else {
                                    let windows = screen_capture::find_target_windows(&proc_ref);
                                    let idx = (*slot as usize).saturating_sub(1);
                                    windows.get(idx).map(|w| vec![w.hwnd]).unwrap_or_default()
                                }
                            }
                            ClickerTarget::All => {
                                let windows = screen_capture::find_target_windows(&proc_ref);
                                windows.into_iter().map(|w| w.hwnd).collect()
                            }
                        };

                        if !target_hwnds.is_empty() {
                            for &hwnd in &target_hwnds {
                                if !running_ref.load(Ordering::SeqCst) || *rx.borrow() {
                                    break;
                                }
                                match input_sender::send_key_background(hwnd, &key, 75).await {
                                    Ok(()) => {
                                        let new_count = count_ref.fetch_add(1, Ordering::SeqCst) + 1;
                                        let _ = app_ref.emit("clicker-tick", new_count);
                                    }
                                    Err(e) => {
                                        warn!("Rule '{}' failed to send key '{}': {}", rule_name, key, e);
                                    }
                                }
                            }
                        } else {
                            debug!("Rule '{}': no target window available", rule_name);
                        }

                        tokio::select! {
                            _ = rx.changed() => {
                                debug!("Rule '{}' loop received stop signal", rule_name);
                                break;
                            }
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)) => {}
                        }
                    }

                    info!("Rule '{}' (ID: {}) stopped", rule_name, rule_id);
                });
            }

            let cfg = config_arc_clone.lock().await;
            let status = self_clone.get_status(&cfg);
            let _ = app_clone.emit("clicker-status-changed", status);
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

        if let Ok(mut map) = self.active_rules.lock() {
            for (_, tx) in map.drain() {
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
        let rules = cfg.clicker.get_rules();
        let pattern_lower = pattern_name.to_lowercase();

        let mut stopped_rule_labels = Vec::new();

        if let Ok(mut map) = self.active_rules.lock() {
            for rule in &rules {
                if !rule.enabled || !rule.stop_on_pattern {
                    continue;
                }
                let matches = rule.stop_patterns.is_empty()
                    || rule.stop_patterns.iter().any(|p| p.to_lowercase() == pattern_lower);

                if matches {
                    if let Some(tx) = map.remove(&rule.id) {
                        let _ = tx.send(true);
                        stopped_rule_labels.push(format!("'{}' ({})", rule.name, rule.key));
                    }
                }
            }

            if map.is_empty() {
                self.running.store(false, Ordering::SeqCst);
            }
        }

        if !stopped_rule_labels.is_empty() {
            let reason = format!(
                "Остановлены [{}]: сработал паттерн '{}'",
                stopped_rule_labels.join(", "),
                pattern_name
            );
            info!("{}", reason);

            if let Ok(mut reason_guard) = self.last_stop_reason.lock() {
                *reason_guard = Some(reason.clone());
            }

            if cfg.clicker.notify_on_stop {
                use tauri_plugin_notification::NotificationExt;
                let _ = app
                    .notification()
                    .builder()
                    .title("EW Notify — Кликер")
                    .body(&reason)
                    .show();
            }

            let app_h = app.clone();
            let cfg_arc = config_arc.clone();
            let self_clone = self.clone();
            tauri::async_runtime::spawn(async move {
                let cfg = cfg_arc.lock().await;
                let status = self_clone.get_status(&cfg);
                let _ = app_h.emit("clicker-status-changed", status);
            });
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

    #[test]
    fn test_clicker_config_rules() {
        let cfg = ClickerConfig::default();
        let rules = cfg.get_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].key, "1");
        assert_eq!(rules[0].target, ClickerTarget::Captured);
    }
}
