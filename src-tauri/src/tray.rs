//! Tray helpers (menu/actions live in `lib.rs`).

use tracing::info;

/// Log-only helper kept for callers that want a consistent "notify" trace line.
pub fn show_notification(title: &str, body: &str) {
    info!("Notification: {} — {}", title, body);
}
