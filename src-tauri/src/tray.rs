use crate::config::AppConfig;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::info;

pub fn show_notification(config: &Arc<Mutex<AppConfig>>, title: &str, body: &str) {
    info!("Showing tray notification: {} - {}", title, body);

    #[cfg(windows)]
    {
        if let Err(e) = notify_rust::Notification::new()
            .title(title)
            .body(body)
            .show()
        {
            tracing::warn!("Failed to show notification: {}", e);
        }
    }
}
