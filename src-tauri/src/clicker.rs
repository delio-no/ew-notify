//! Background auto-clicker for target game windows.
//!
//! Supports multiple independent rules/keys, each with its own interval,
//! target window (captured, specific slot, or all), and pattern stop triggers.

use crate::config::{AppConfig, ClickerConfig, ClickerRule, ClickerTarget};
use crate::input_sender;
use crate::screen_capture;
use crate::window_filter::WindowFilterState;
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

fn rule_matches_window(
    target: &ClickerTarget,
    target_slot: Option<u32>,
    target_pid: Option<u32>,
    filter: &WindowFilterState,
) -> bool {
    // If packet has no slot and no pid information, allow it to match all rules
    if target_slot.is_none() && target_pid.is_none() {
        return true;
    }

    match target {
        ClickerTarget::All => true,
        ClickerTarget::Slot(s) => {
            if let Some(slot) = target_slot {
                if slot == *s {
                    return true;
                }
            }
            if let Some(pid) = target_pid {
                if let Some(slot_info) = filter.get_slot(*s) {
                    if slot_info.pid == pid {
                        return true;
                    }
                }
            }
            false
        }
        ClickerTarget::Captured => {
            if let Some(active_or_first) = filter.get_active_or_first_slot() {
                if let Some(slot) = target_slot {
                    if slot == active_or_first.slot {
                        return true;
                    }
                }
                if let Some(pid) = target_pid {
                    if pid == active_or_first.pid {
                        return true;
                    }
                }
                false
            } else {
                // No specific slot assigned (All mode)
                true
            }
        }
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
                self_clone.spawn_rule_task(&rule, &proc_name, &app_clone, &filter_clone);
            }

            let cfg = config_arc_clone.lock().await;
            let status = self_clone.get_status(&cfg);
            let _ = app_clone.emit("clicker-status-changed", status);
        });
    }

    pub fn spawn_rule_task(
        &self,
        rule: &ClickerRule,
        proc_name: &str,
        app: &AppHandle,
        filter: &WindowFilterState,
    ) {
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        if let Ok(mut map) = self.active_rules.lock() {
            if let Some(old_tx) = map.insert(rule.id.clone(), tx) {
                let _ = old_tx.send(true);
            }
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

        let running_ref = self.running.clone();
        let count_ref = self.count.clone();
        let app_ref = app.clone();
        let filter_ref = filter.clone();
        let proc_ref = proc_name.to_string();

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
                        if let Some(info) = filter_ref.get_active_or_first_slot() {
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
                        } else {
                            Vec::new()
                        }
                    }
                    ClickerTarget::Slot(slot) => {
                        if let Some(info) = filter_ref.get_slot(*slot) {
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

    pub fn trigger_pattern_event(
        &self,
        pattern_name: &str,
        target_slot: Option<u32>,
        target_pid: Option<u32>,
        app: &AppHandle,
        config_arc: &Arc<Mutex<AppConfig>>,
        filter: &WindowFilterState,
        rt: &tokio::runtime::Handle,
    ) {
        let cfg = rt.block_on(async { config_arc.lock().await.clone() });
        let rules = cfg.clicker.get_rules();
        let proc_name = cfg.target.process_name.clone();
        let pattern_lower = pattern_name.to_lowercase();

        let mut stopped_rule_labels = Vec::new();
        let mut started_rule_labels = Vec::new();

        // 1. Process stop triggers (for currently active rules that match window)
        if self.is_running() {
            if let Ok(mut map) = self.active_rules.lock() {
                for rule in &rules {
                    if !rule.enabled || !rule.stop_on_pattern {
                        continue;
                    }
                    if !rule_matches_window(&rule.target, target_slot, target_pid, filter) {
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
        }

        // 2. Process start triggers (for enabled rules configured to start on this pattern that match window)
        let rules_to_start: Vec<ClickerRule> = {
            let active_map = self.active_rules.lock().ok();
            rules
                .into_iter()
                .filter(|r| {
                    if !r.enabled || !r.start_on_pattern {
                        return false;
                    }
                    if !rule_matches_window(&r.target, target_slot, target_pid, filter) {
                        return false;
                    }
                    let matches = r.start_patterns.is_empty()
                        || r.start_patterns.iter().any(|p| p.to_lowercase() == pattern_lower);
                    if !matches {
                        return false;
                    }
                    if let Some(ref map) = active_map {
                        !map.contains_key(&r.id)
                    } else {
                        true
                    }
                })
                .collect()
        };

        if !rules_to_start.is_empty() {
            self.running.store(true, Ordering::SeqCst);
            for rule in &rules_to_start {
                self.spawn_rule_task(rule, &proc_name, app, filter);
                started_rule_labels.push(format!("'{}' ({})", rule.name, rule.key));
            }
        }

        // 3. Notifications & status emission
        if !stopped_rule_labels.is_empty() {
            let slot_prefix = target_slot
                .map(|s| format!("Слот #{}: ", s))
                .unwrap_or_default();
            let reason = format!(
                "{}Остановлены [{}]: сработал паттерн '{}'",
                slot_prefix,
                stopped_rule_labels.join(", "),
                pattern_name
            );
            info!("{}", reason);

            if let Ok(mut reason_guard) = self.last_stop_reason.lock() {
                *reason_guard = Some(reason.clone());
            }

            if cfg.clicker.is_notify_on_stop_for_slot(target_slot) {
                crate::notifications::show_system_notification("EW Notify — Кликер", &reason);
            }
        }

        if !started_rule_labels.is_empty() {
            let slot_prefix = target_slot
                .map(|s| format!("Слот #{}: ", s))
                .unwrap_or_default();
            let msg = format!(
                "{}Запущены [{}]: сработал паттерн '{}'",
                slot_prefix,
                started_rule_labels.join(", "),
                pattern_name
            );
            info!("{}", msg);

            if cfg.clicker.is_notify_on_start_for_slot(target_slot) {
                crate::notifications::show_system_notification("EW Notify — Кликер", &msg);
            }
        }

        if !stopped_rule_labels.is_empty() || !started_rule_labels.is_empty() {
            crate::overlay::update_overlay_from_state(filter, true, self.is_running());
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

    pub fn trigger_pattern_stop(
        &self,
        pattern_name: &str,
        app: &AppHandle,
        config_arc: &Arc<Mutex<AppConfig>>,
        filter: &WindowFilterState,
        rt: &tokio::runtime::Handle,
    ) {
        self.trigger_pattern_event(pattern_name, None, None, app, config_arc, filter, rt);
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
        assert_eq!(rules[0].target, ClickerTarget::Slot(1));
        // By default both stop_on_pattern and start_on_pattern must be false
        assert!(!rules[0].stop_on_pattern);
        assert!(!rules[0].start_on_pattern);
        assert!(rules[0].stop_patterns.is_empty());
        assert!(rules[0].start_patterns.is_empty());
        assert!(!cfg.notify_on_stop);
        assert!(!cfg.notify_on_start);
    }

    #[test]
    fn test_rule_matches_window() {
        let filter = WindowFilterState::new();
        filter.set_slot(1, 1001, 111, "Window 1".into());
        filter.set_slot(2, 1002, 222, "Window 2".into());

        // Target::All matches any slot
        assert!(rule_matches_window(&ClickerTarget::All, Some(1), Some(1001), &filter));
        assert!(rule_matches_window(&ClickerTarget::All, Some(2), Some(1002), &filter));

        // Target::Slot(1) matches slot 1, not slot 2
        assert!(rule_matches_window(&ClickerTarget::Slot(1), Some(1), Some(1001), &filter));
        assert!(!rule_matches_window(&ClickerTarget::Slot(1), Some(2), Some(1002), &filter));

        // Target::Slot(2) matches slot 2, not slot 1
        assert!(!rule_matches_window(&ClickerTarget::Slot(2), Some(1), Some(1001), &filter));
        assert!(rule_matches_window(&ClickerTarget::Slot(2), Some(2), Some(1002), &filter));

        // Unknown slot/pid matches everything
        assert!(rule_matches_window(&ClickerTarget::Slot(1), None, None, &filter));
    }
}
