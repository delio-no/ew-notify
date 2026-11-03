use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackedWindowInfo {
    pub slot: u32,
    pub pid: u32,
    pub hwnd: isize,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "data")]
pub enum WindowFilterMode {
    All,
    Single(TrackedWindowInfo),
}

#[derive(Clone)]
pub struct WindowFilterState {
    mode: Arc<RwLock<WindowFilterMode>>,
    /// Fast atomic PID check for hot packet path:
    /// 0 = All windows, >0 = only this specific PID allowed.
    active_pid: Arc<AtomicU32>,
}

impl Default for WindowFilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowFilterState {
    pub fn new() -> Self {
        Self {
            mode: Arc::new(RwLock::new(WindowFilterMode::All)),
            active_pid: Arc::new(AtomicU32::new(0)),
        }
    }

    #[inline]
    pub fn is_pid_allowed(&self, pid: u32) -> bool {
        let active = self.active_pid.load(Ordering::Relaxed);
        active == 0 || active == pid
    }

    pub fn get_mode(&self) -> WindowFilterMode {
        self.mode.read().unwrap().clone()
    }

    pub fn set_single(&self, slot: u32, pid: u32, hwnd: isize, title: String) {
        let info = TrackedWindowInfo {
            slot,
            pid,
            hwnd,
            title,
        };
        self.active_pid.store(pid, Ordering::SeqCst);
        let mut guard = self.mode.write().unwrap();
        *guard = WindowFilterMode::Single(info);
    }

    pub fn reset_to_all(&self) {
        self.active_pid.store(0, Ordering::SeqCst);
        let mut guard = self.mode.write().unwrap();
        *guard = WindowFilterMode::All;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_filter_default_all() {
        let filter = WindowFilterState::new();
        assert_eq!(filter.get_mode(), WindowFilterMode::All);
        assert!(filter.is_pid_allowed(100));
        assert!(filter.is_pid_allowed(200));
    }

    #[test]
    fn test_window_filter_single_mode() {
        let filter = WindowFilterState::new();
        filter.set_single(1, 1234, 5678, "EndlessWar".to_string());

        assert_eq!(
            filter.get_mode(),
            WindowFilterMode::Single(TrackedWindowInfo {
                slot: 1,
                pid: 1234,
                hwnd: 5678,
                title: "EndlessWar".to_string(),
            })
        );

        assert!(filter.is_pid_allowed(1234));
        assert!(!filter.is_pid_allowed(9999));

        filter.reset_to_all();
        assert_eq!(filter.get_mode(), WindowFilterMode::All);
        assert!(filter.is_pid_allowed(9999));
    }
}
