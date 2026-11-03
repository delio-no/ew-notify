//! Global Hotkey listener for switching tracked game windows (Ctrl+1, Ctrl+2, ... Ctrl+0).

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub fn start_hotkey_listener(
    _app: tauri::AppHandle,
    _filter: crate::window_filter::WindowFilterState,
    _config: std::sync::Arc<tokio::sync::Mutex<crate::config::AppConfig>>,
) {}

#[cfg(windows)]
mod win {
    use crate::config::AppConfig;
    use crate::overlay;
    use crate::screen_capture;
    use crate::window_filter::{WindowFilterMode, WindowFilterState};
    use std::sync::Arc;
    use tauri::{AppHandle, Emitter};
    use tokio::sync::Mutex;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetForegroundWindow, GetMessageW, TranslateMessage, MSG,
    };

    #[link(name = "user32")]
    extern "system" {
        fn MessageBeep(uType: u32) -> i32;
    }

    pub fn play_beep() {
        unsafe {
            MessageBeep(0);
        }
    }

    pub fn start_hotkey_listener(
        app: AppHandle,
        filter: WindowFilterState,
        config: Arc<Mutex<AppConfig>>,
    ) {
        std::thread::Builder::new()
            .name("ew-hotkeys-thread".into())
            .spawn(move || {
                run_hotkey_loop(app, filter, config);
            })
            .expect("Failed to spawn hotkey listener thread");
    }

    fn run_hotkey_loop(
        app: AppHandle,
        filter: WindowFilterState,
        config: Arc<Mutex<AppConfig>>,
    ) {
        const WM_HOTKEY: u32 = 0x0312;

        unsafe {
            // Register Ctrl + 0 (ID 10: Reset to All)
            let _ = RegisterHotKey(
                HWND(0),
                10,
                MOD_CONTROL | MOD_NOREPEAT,
                0x30, // '0'
            );

            // Register Ctrl + 1 .. Ctrl + 6 (ID 1 .. 6)
            for slot in 1..=6 {
                let vk = 0x30 + slot; // '1' .. '6'
                let _ = RegisterHotKey(
                    HWND(0),
                    slot as i32,
                    MOD_CONTROL | MOD_NOREPEAT,
                    vk,
                );
            }

            tracing::info!("Global window hotkeys registered: Ctrl+1..6 (Select slot), Ctrl+0 (All windows)");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                if msg.message == WM_HOTKEY {
                    let hotkey_id = msg.wParam.0 as u32;
                    handle_hotkey(hotkey_id, &app, &filter, &config);
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Cleanup on thread exit
            for id in 1..=6 {
                let _ = UnregisterHotKey(HWND(0), id);
            }
            let _ = UnregisterHotKey(HWND(0), 10);
        }
    }

    fn handle_hotkey(
        hotkey_id: u32,
        app: &AppHandle,
        filter: &WindowFilterState,
        config: &Arc<Mutex<AppConfig>>,
    ) {
        let rt = tokio::runtime::Handle::current();

        if hotkey_id == 10 {
            // Ctrl + 0: Reset to All Windows
            rt.block_on(async {
                filter.reset_to_all().await;
            });
            overlay::hide_overlay();
            play_beep();
            let _ = app.emit("window-filter-changed", WindowFilterMode::All);
            tracing::info!("Window filter reset to All windows via Ctrl+0");
            return;
        }

        let slot = hotkey_id;
        if !(1..=6).contains(&slot) {
            return;
        }

        let current_mode = rt.block_on(filter.get_mode());
        if let WindowFilterMode::Single(ref info) = current_mode {
            // If pressing the same slot again, toggle back to All!
            if info.slot == slot {
                rt.block_on(async {
                    filter.reset_to_all().await;
                });
                overlay::hide_overlay();
                play_beep();
                let _ = app.emit("window-filter-changed", WindowFilterMode::All);
                tracing::info!("Window filter toggled off via Ctrl+{} -> All windows", slot);
                return;
            }
        }

        let process_name = rt.block_on(async {
            let guard = config.lock().await;
            guard.target.process_name.clone()
        });

        let target_windows = screen_capture::find_target_windows(&process_name);
        if target_windows.is_empty() {
            tracing::warn!("Hotkey Ctrl+{}: no windows found for {}", slot, process_name);
            return;
        }

        // Check if current foreground window is one of our target game windows
        let fg_hwnd = unsafe { GetForegroundWindow() };
        let mut chosen_window = None;

        if fg_hwnd.0 != 0 {
            if let Some(w) = target_windows.iter().find(|tw| tw.hwnd == (fg_hwnd.0 as isize)) {
                chosen_window = Some(w.clone());
            }
        }

        // If foreground window wasn't a game window, pick by slot index (1 -> 0, 2 -> 1, ...)
        if chosen_window.is_none() {
            let idx = (slot - 1) as usize;
            if idx < target_windows.len() {
                chosen_window = Some(target_windows[idx].clone());
            } else {
                chosen_window = Some(target_windows[0].clone());
            }
        }

        if let Some(target) = chosen_window {
            let label = format!("🟢 EW #{} [PID {}]", slot, target.pid);
            rt.block_on(async {
                filter
                    .set_single(slot, target.pid, target.hwnd, target.title.clone())
                    .await;
            });

            overlay::show_overlay(target.hwnd, &label, true);
            play_beep();

            let new_mode = rt.block_on(filter.get_mode());
            let _ = app.emit("window-filter-changed", new_mode);
            tracing::info!(
                "Window filter set via Ctrl+{} to window #{} [PID {}] (HWND {})",
                slot,
                slot,
                target.pid,
                target.hwnd
            );
        }
    }
}
