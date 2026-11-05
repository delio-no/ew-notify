use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    Multi(Vec<TrackedWindowInfo>),
}

#[derive(Clone)]
pub struct WindowFilterState {
    slots: Arc<RwLock<BTreeMap<u32, TrackedWindowInfo>>>,
    last_active_slot: Arc<AtomicU32>,
}

impl Default for WindowFilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowFilterState {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(RwLock::new(BTreeMap::new())),
            last_active_slot: Arc::new(AtomicU32::new(0)),
        }
    }

    #[inline]
    pub fn is_pid_allowed(&self, pid: u32) -> bool {
        let guard = self.slots.read().unwrap();
        if guard.is_empty() {
            true
        } else {
            guard.values().any(|w| w.pid == pid)
        }
    }

    pub fn get_mode(&self) -> WindowFilterMode {
        let guard = self.slots.read().unwrap();
        if guard.is_empty() {
            WindowFilterMode::All
        } else if guard.len() == 1 {
            WindowFilterMode::Single(guard.values().next().unwrap().clone())
        } else {
            WindowFilterMode::Multi(guard.values().cloned().collect())
        }
    }

    pub fn set_slot(&self, slot: u32, pid: u32, hwnd: isize, title: String) {
        let info = TrackedWindowInfo {
            slot,
            pid,
            hwnd,
            title,
        };
        self.last_active_slot.store(slot, Ordering::SeqCst);
        let mut guard = self.slots.write().unwrap();
        guard.insert(slot, info);
    }

    pub fn remove_slot(&self, slot: u32) {
        let mut guard = self.slots.write().unwrap();
        guard.remove(&slot);
    }

    pub fn reset_to_all(&self) {
        self.last_active_slot.store(0, Ordering::SeqCst);
        let mut guard = self.slots.write().unwrap();
        guard.clear();
    }

    pub fn get_all_slots(&self) -> Vec<TrackedWindowInfo> {
        let guard = self.slots.read().unwrap();
        guard.values().cloned().collect()
    }

    pub fn get_slot(&self, slot: u32) -> Option<TrackedWindowInfo> {
        let guard = self.slots.read().unwrap();
        guard.get(&slot).cloned()
    }

    pub fn get_slot_by_pid(&self, pid: u32) -> Option<u32> {
        let guard = self.slots.read().unwrap();
        guard.values().find(|w| w.pid == pid).map(|w| w.slot)
    }

    pub fn get_active_or_first_slot(&self) -> Option<TrackedWindowInfo> {
        let guard = self.slots.read().unwrap();
        let last = self.last_active_slot.load(Ordering::SeqCst);
        if let Some(w) = guard.get(&last) {
            Some(w.clone())
        } else {
            guard.values().next().cloned()
        }
    }

    pub fn set_single(&self, slot: u32, pid: u32, hwnd: isize, title: String) {
        self.set_slot(slot, pid, hwnd, title);
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
        assert!(filter.get_all_slots().is_empty());
    }

    #[test]
    fn test_window_filter_single_mode() {
        let filter = WindowFilterState::new();
        filter.set_slot(1, 1234, 5678, "EndlessWar".to_string());

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
        assert_eq!(filter.get_slot_by_pid(1234), Some(1));

        filter.reset_to_all();
        assert_eq!(filter.get_mode(), WindowFilterMode::All);
        assert!(filter.is_pid_allowed(9999));
    }

    #[test]
    fn test_window_filter_multi_mode() {
        let filter = WindowFilterState::new();
        filter.set_slot(1, 1111, 100, "Window 1".to_string());
        filter.set_slot(2, 2222, 200, "Window 2".to_string());

        assert_eq!(filter.get_all_slots().len(), 2);
        assert!(filter.is_pid_allowed(1111));
        assert!(filter.is_pid_allowed(2222));
        assert!(!filter.is_pid_allowed(3333));

        assert_eq!(filter.get_slot_by_pid(1111), Some(1));
        assert_eq!(filter.get_slot_by_pid(2222), Some(2));
        assert_eq!(filter.get_slot_by_pid(3333), None);

        match filter.get_mode() {
            WindowFilterMode::Multi(slots) => {
                assert_eq!(slots.len(), 2);
                assert_eq!(slots[0].slot, 1);
                assert_eq!(slots[1].slot, 2);
            }
            _ => panic!("Expected Multi mode"),
        }

        filter.remove_slot(1);
        assert_eq!(filter.get_all_slots().len(), 1);
        assert!(!filter.is_pid_allowed(1111));
        assert!(filter.is_pid_allowed(2222));

        match filter.get_mode() {
            WindowFilterMode::Single(info) => {
                assert_eq!(info.slot, 2);
            }
            _ => panic!("Expected Single mode after removing 1"),
        }
    }
}
