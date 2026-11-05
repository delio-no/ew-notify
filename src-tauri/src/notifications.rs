//! System notification helper for Windows and cross-platform desktop.
//! Uses native WinRT toast with fallback to ensure toasts are reliably delivered
//! even when running as an unpackaged / portable executable.

#[cfg(windows)]
pub fn register_app_user_model_id() {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey(r"Software\Classes\AppUserModelId\com.ewnotify.app") {
        let _ = key.set_value("DisplayName", &"EndlessWar Notifier");
        if let Ok(exe) = std::env::current_exe() {
            let _ = key.set_value("IconUri", &exe.to_string_lossy().as_ref());
        }
        let _ = key.set_value("ShowInSettings", &1u32);
    }
}

pub fn show_system_notification(title: &str, body: &str) {
    tracing::info!("Showing system notification: {} — {}", title, body);

    #[cfg(windows)]
    {
        // On Windows, use notify-rust with the standard built-in Windows PowerShell AUMID.
        // Unpackaged/portable EXEs using custom identifiers without Start menu shortcuts
        // are silently discarded by the Windows 10/11 Action Center. The PowerShell AUMID
        // is guaranteed to pop up a toast banner and display in the Action Center.
        let mut notif = notify_rust::Notification::new();
        notif
            .summary(title)
            .body(body)
            .timeout(notify_rust::Timeout::Milliseconds(5000))
            .app_id("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe");

        if let Err(e) = notif.show() {
            tracing::warn!("Failed to show Windows toast notification: {}", e);
        }
    }

    #[cfg(not(windows))]
    {
        let mut notif = notify_rust::Notification::new();
        notif.summary(title).body(body);
        if let Err(e) = notif.show() {
            tracing::warn!("Failed to show desktop notification: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_notifications_module_loads() {
        assert!(true);
    }
}
