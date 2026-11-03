//! Global Hotkey listener for switching tracked game windows (Ctrl+1..6, Ctrl+0, Numpad).

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
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
        GetWindowThreadProcessId, PeekMessageW, TranslateMessage, WindowFromPoint,
        MSG, PM_NOREMOVE,
    };

    pub fn play_beep() {}

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
            // 1. Force creation of thread message queue before registering hotkeys
            let mut init_msg = MSG::default();
            let _ = PeekMessageW(&mut init_msg, HWND(0), 0, 0, PM_NOREMOVE);

            let reg_hotkey = |id: i32, vk: u32| -> bool {
                let ok = RegisterHotKey(HWND(0), id, MOD_CONTROL | MOD_NOREPEAT, vk).as_bool();
                if ok {
                    true
                } else {
                    RegisterHotKey(HWND(0), id, MOD_CONTROL, vk).as_bool()
                }
            };

            let mut registered_count = 0;

            // Register Ctrl + 0 (ID 10: Reset to All)
            if reg_hotkey(10, 0x30) {
                registered_count += 1;
            } else {
                tracing::warn!("Failed to register hotkey Ctrl+0");
            }

            // Register Ctrl + Num0 (ID 20: Reset to All via Numpad)
            let _ = reg_hotkey(20, 0x60);

            // Register Ctrl + 1..6 (ID 1..6) & Ctrl + Num1..6 (ID 21..26)
            for slot in 1..=6 {
                let vk = 0x30 + slot;
                if reg_hotkey(slot as i32, vk) {
                    registered_count += 1;
                } else {
                    tracing::warn!("Failed to register hotkey Ctrl+{}", slot);
                }

                let vk_num = 0x60 + slot;
                let _ = reg_hotkey((20 + slot) as i32, vk_num);
            }

            tracing::info!(
                "Global window hotkeys registered ({} main slots active): Ctrl+1..6, Ctrl+0 (and Numpad)",
                registered_count
            );

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                if msg.message == WM_HOTKEY {
                    let raw_id = msg.wParam.0 as u32;
                    let hotkey_id = if (21..=26).contains(&raw_id) {
                        raw_id - 20
                    } else if raw_id == 20 {
                        10
                    } else {
                        raw_id
                    };

                    tracing::info!("WM_HOTKEY received: raw_id={}, mapped_id={}", raw_id, hotkey_id);
                    handle_hotkey(hotkey_id, &app, &filter, &config);
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Cleanup on thread exit
            for id in 1..=6 {
                let _ = UnregisterHotKey(HWND(0), id);
                let _ = UnregisterHotKey(HWND(0), 20 + id);
            }
            let _ = UnregisterHotKey(HWND(0), 10);
            let _ = UnregisterHotKey(HWND(0), 20);
        }
    }

    fn handle_hotkey(
        hotkey_id: u32,
        app: &AppHandle,
        filter: &WindowFilterState,
        config: &Arc<Mutex<AppConfig>>,
    ) {
        if hotkey_id == 10 {
            // Ctrl + 0: Reset to All Windows
            filter.reset_to_all();
            overlay::hide_overlay();
            let _ = app.emit("window-filter-changed", WindowFilterMode::All);
            tracing::info!("Window filter reset to All windows via Ctrl+0");
            return;
        }

        let slot = hotkey_id;
        if !(1..=6).contains(&slot) {
            return;
        }

        let current_mode = filter.get_mode();
        if let WindowFilterMode::Single(ref info) = current_mode {
            // If pressing the same slot again, toggle back to All
            if info.slot == slot {
                filter.reset_to_all();
                overlay::hide_overlay();
                let _ = app.emit("window-filter-changed", WindowFilterMode::All);
                tracing::info!("Window filter toggled off via Ctrl+{} -> All windows", slot);
                return;
            }
        }

        let process_name = match config.try_lock() {
            Ok(guard) => guard.target.process_name.clone(),
            Err(_) => config.blocking_lock().target.process_name.clone(),
        };
        let process_name = if process_name.is_empty() {
            "EndlessWar.exe".to_string()
        } else {
            process_name
        };

        let target_windows = screen_capture::find_target_windows(&process_name);
        if target_windows.is_empty() {
            tracing::warn!("Hotkey Ctrl+{}: no windows found for {}", slot, process_name);
            return;
        }

        // Priority 1: Check window under mouse cursor
        let mut chosen_window: Option<screen_capture::TargetWindow> = None;
        unsafe {
            let mut cursor_pt = POINT::default();
            if GetCursorPos(&mut cursor_pt).as_bool() {
                let cursor_hwnd = WindowFromPoint(cursor_pt);
                if cursor_hwnd.0 != 0 {
                    let mut cursor_pid = 0u32;
                    GetWindowThreadProcessId(cursor_hwnd, Some(&mut cursor_pid));
                    if cursor_pid != 0 {
                        if let Some(w) = target_windows.iter().find(|tw| tw.pid == cursor_pid) {
                            chosen_window = Some(w.clone());
                        }
                    }
                }
            }

            // Priority 2: Check foreground window
            if chosen_window.is_none() {
                let fg_hwnd = GetForegroundWindow();
                if fg_hwnd.0 != 0 {
                    let mut fg_pid = 0u32;
                    GetWindowThreadProcessId(fg_hwnd, Some(&mut fg_pid));
                    if let Some(w) = target_windows
                        .iter()
                        .find(|tw| tw.pid == fg_pid || tw.hwnd == (fg_hwnd.0 as isize))
                    {
                        chosen_window = Some(w.clone());
                    }
                }
            }
        }

        // Priority 3: Fallback by slot index (1 -> 0, 2 -> 1, ...)
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
            filter.set_single(slot, target.pid, target.hwnd, target.title.clone());

            overlay::show_overlay(target.hwnd, &label, false);

            let new_mode = filter.get_mode();
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
