use tauri::{AppHandle, Manager};
use tracing::info;

pub fn show_notification(app: &AppHandle, title: &str, body: &str) {
    info!("Showing tray notification: {} - {}", title, body);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(
            "notification",
            serde_json::json!({
                "title": title,
                "body": body
            }),
        );
    }

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
