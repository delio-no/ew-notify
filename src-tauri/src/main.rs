#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Неизвестная ошибка".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());

        let msg = format!(
            "Произошла ошибка при работе EndlessWar Notifier:\n\n{}\n\nМесто: {}\n\nПодробности записаны в файл panic.log",
            payload, location
        );
        let _ = std::fs::write("panic.log", format!("Error: {}\nLocation: {}\nInfo: {:?}", payload, location, info));
        #[cfg(windows)]
        unsafe {
            use windows::core::HSTRING;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            MessageBoxW(
                windows::Win32::Foundation::HWND(0),
                &HSTRING::from(&msg),
                &HSTRING::from("EndlessWar Notifier"),
                MB_OK | MB_ICONERROR,
            );
        }
    }));
    ew_notify_lib::run();
}
