//! In-memory packet dump ring for the UI (not written to app.log).

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const MAX_ENTRIES: usize = 150;
const RATE_LIMIT_PER_SEC: u32 = 25;

#[derive(Clone, Serialize)]
pub struct PacketLogEntry {
    pub time: String,
    pub remote: String,
    pub local: String,
    pub len: usize,
    pub hex: String,
}

struct RateState {
    window_start: Instant,
    count: u32,
}

pub struct PacketLog {
    enabled: AtomicBool,
    skipped: AtomicU64,
    entries: Mutex<VecDeque<PacketLogEntry>>,
    rate: Mutex<RateState>,
}

impl PacketLog {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(false),
            skipped: AtomicU64::new(0),
            entries: Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)),
            rate: Mutex::new(RateState {
                window_start: Instant::now(),
                count: 0,
            }),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
        if !on {
            self.clear();
            self.skipped.store(0, Ordering::SeqCst);
        }
    }

    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::SeqCst)
    }

    pub fn clear(&self) {
        if let Ok(mut q) = self.entries.lock() {
            q.clear();
        }
    }

    pub fn snapshot(&self) -> Vec<PacketLogEntry> {
        self.entries
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Push one packet if logging enabled and under rate limit. Full hex, no truncation.
    pub fn try_push(&self, remote: String, local: String, payload: &[u8]) {
        if !self.is_enabled() {
            return;
        }

        let allow = {
            let Ok(mut rate) = self.rate.lock() else {
                return;
            };
            let now = Instant::now();
            if now.duration_since(rate.window_start).as_secs() >= 1 {
                rate.window_start = now;
                rate.count = 0;
            }
            if rate.count >= RATE_LIMIT_PER_SEC {
                false
            } else {
                rate.count += 1;
                true
            }
        };

        if !allow {
            self.skipped.fetch_add(1, Ordering::SeqCst);
            return;
        }

        let hex: String = payload.iter().map(|b| format!("{b:02X}")).collect();
        let entry = PacketLogEntry {
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            remote,
            local,
            len: payload.len(),
            hex,
        };
        if let Ok(mut q) = self.entries.lock() {
            q.push_back(entry);
            while q.len() > MAX_ENTRIES {
                q.pop_front();
            }
        }
    }

    /// Bypass rate-limit (tests only).
    #[cfg(test)]
    fn push_unchecked(&self, remote: String, local: String, payload: &[u8]) {
        let hex: String = payload.iter().map(|b| format!("{b:02X}")).collect();
        let entry = PacketLogEntry {
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            remote,
            local,
            len: payload.len(),
            hex,
        };
        if let Ok(mut q) = self.entries.lock() {
            q.push_back(entry);
            while q.len() > MAX_ENTRIES {
                q.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_writes_nothing() {
        let log = PacketLog::new();
        log.try_push("1.1.1.1:1".into(), "10.0.0.1:2".into(), &[0xAA, 0xBB]);
        assert!(log.snapshot().is_empty());
        assert_eq!(log.skipped(), 0);
    }

    #[test]
    fn enabled_stores_full_hex() {
        let log = PacketLog::new();
        log.set_enabled(true);
        let payload = [0x00, 0xFE, 0x06, 0x03];
        log.try_push("1.2.3.4:7777".into(), "10.0.0.5:40000".into(), &payload);
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].hex, "00FE0603");
        assert_eq!(snap[0].len, 4);
        assert_eq!(snap[0].remote, "1.2.3.4:7777");
    }

    #[test]
    fn ring_truncates_at_max_entries() {
        let log = PacketLog::new();
        log.set_enabled(true);
        // Bypass rate-limit so we can fill past MAX_ENTRIES in one go.
        for i in 0..(MAX_ENTRIES + 20) {
            log.push_unchecked(format!("r:{i}"), format!("l:{i}"), &[i as u8]);
        }
        let snap = log.snapshot();
        assert_eq!(snap.len(), MAX_ENTRIES);
        assert_eq!(snap[0].remote, format!("r:{}", 20));
        assert_eq!(
            snap[MAX_ENTRIES - 1].remote,
            format!("r:{}", MAX_ENTRIES + 19)
        );
    }

    #[test]
    fn rate_limit_increments_skipped() {
        let log = PacketLog::new();
        log.set_enabled(true);
        for i in 0..(RATE_LIMIT_PER_SEC as usize + 10) {
            log.try_push("r".into(), "l".into(), &[i as u8]);
        }
        assert_eq!(log.snapshot().len(), RATE_LIMIT_PER_SEC as usize);
        assert_eq!(log.skipped(), 10);
    }

    #[test]
    fn disable_clears_buffer_and_skipped() {
        let log = PacketLog::new();
        log.set_enabled(true);
        log.try_push("r".into(), "l".into(), &[1]);
        for _ in 0..40 {
            log.try_push("r".into(), "l".into(), &[2]);
        }
        assert!(!log.snapshot().is_empty());
        log.set_enabled(false);
        assert!(log.snapshot().is_empty());
        assert_eq!(log.skipped(), 0);
        assert!(!log.is_enabled());
    }
}
