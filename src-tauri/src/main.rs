#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "Произошла ошибка при работе EndlessWar Notifier:\n\n{}\n\nПодробности записаны в файл panic.log",
            info
        );
        let _ = std::fs::write("panic.log", format!("{:?}", info));
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
