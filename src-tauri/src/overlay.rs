//! Visual overlay for highlighting the tracked game window (Windows).
//! Uses a native Win32 transparent, click-through, always-on-top layered window.

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayStatus {
    pub slot: u32,
    pub pid: u32,
    pub monitoring: bool,
    pub clicker_running: bool,
}

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub fn show_overlay(_hwnd: isize, _status: OverlayStatus) {}

#[cfg(not(windows))]
pub fn hide_overlay() {}

#[cfg(not(windows))]
pub fn update_overlay_from_state(
    _filter: &crate::window_filter::WindowFilterState,
    _monitoring: bool,
    _clicker_running: bool,
) {}

#[cfg(windows)]
mod win {
    use super::OverlayStatus;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Mutex, Once};
    use std::time::Duration;
    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
        FrameRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_CALCRECT, DT_LEFT,
        DT_SINGLELINE, DT_VCENTER, HBRUSH, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetWindowRect, IsIconic,
        IsWindow, PeekMessageW, PostQuitMessage, RegisterClassW, SetLayeredWindowAttributes,
        SetWindowPos, ShowWindow, TranslateMessage, HMENU, HWND_TOPMOST, LWA_COLORKEY, MSG,
        PM_REMOVE, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    enum OverlayCmd {
        Show {
            target_hwnd: isize,
            status: OverlayStatus,
        },
        Hide,
    }

    struct OverlayController {
        sender: Sender<OverlayCmd>,
    }

    static INIT: Once = Once::new();
    static CONTROLLER: Mutex<Option<OverlayController>> = Mutex::new(None);
    static IS_ACTIVE: AtomicBool = AtomicBool::new(false);

    #[inline]
    fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
        COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
    }

    // Shared state between worker and window proc
    struct WindowDrawState {
        status: OverlayStatus,
    }
    static DRAW_STATE: Mutex<Option<WindowDrawState>> = Mutex::new(None);

    unsafe extern "system" fn overlay_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        const WM_PAINT: u32 = 0x000F;
        const WM_DESTROY: u32 = 0x0002;

        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                // 1. Fill entire window with black (transparent key 0x00000000)
                // Note: The green outer border has been removed as requested by user.
                let black_brush = CreateSolidBrush(rgb(0, 0, 0));
                FillRect(hdc, &rc, black_brush);
                let _ = DeleteObject(black_brush);

                let status_opt = {
                    if let Ok(guard) = DRAW_STATE.lock() {
                        guard.as_ref().map(|s| s.status.clone())
                    } else {
                        None
                    }
                };

                // 2. Info badge in top-right corner showing EW Slot, Notification status, and Clicker status
                if let Some(status) = status_opt {
                    if (rc.right - rc.left) > 220 && (rc.bottom - rc.top) > 80 {
                        let font = CreateFontW(
                            14,
                            0,
                            0,
                            0,
                            700,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            w!("Segoe UI"),
                        );
                        let old_font = SelectObject(hdc, font);

                        let notif_text = if status.monitoring { "ВКЛ" } else { "ВЫКЛ" };
                        let notif_color = if status.monitoring {
                            rgb(74, 222, 128) // Emerald green #4ade80
                        } else {
                            rgb(248, 113, 113) // Coral red #f87171
                        };

                        let clicker_text = if status.clicker_running { "АКТИВЕН" } else { "ВЫКЛ" };
                        let clicker_color = if status.clicker_running {
                            rgb(74, 222, 128) // Emerald green #4ade80
                        } else {
                            rgb(148, 163, 184) // Slate-400 #94a3b8
                        };

                        let slot_str = format!("EW #{}", status.slot);
                        let segments: [(&str, COLORREF); 7] = [
                            (&slot_str, rgb(56, 189, 248)), // Sky blue #38bdf8
                            ("  |  ", rgb(71, 85, 105)),     // Slate-600 #475569
                            ("Уведомления: ", rgb(203, 213, 225)), // Slate-300
                            (notif_text, notif_color),
                            ("  |  ", rgb(71, 85, 105)),     // Slate-600 #475569
                            ("Кликер: ", rgb(203, 213, 225)), // Slate-300
                            (clicker_text, clicker_color),
                        ];

                        // Measure widths of each segment using GDI DT_CALCRECT
                        let mut wide_segments: Vec<(Vec<u16>, COLORREF, i32)> = Vec::with_capacity(7);
                        let mut total_content_w = 0i32;

                        for (text, col) in &segments {
                            let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                            let slice_len = wide.len().saturating_sub(1);
                            let mut calc_rc = RECT::default();
                            DrawTextW(hdc, &mut wide[..slice_len], &mut calc_rc, DT_CALCRECT | DT_SINGLELINE);
                            let w = calc_rc.right - calc_rc.left;
                            total_content_w += w;
                            wide_segments.push((wide, *col, w));
                        }

                        let pad_x = 12i32;
                        let badge_w = total_content_w + pad_x * 2;
                        let badge_h = 26i32;
                        let badge_margin_right = 10i32;
                        let badge_top = rc.top + 38; // Lowered below standard window title bar

                        let badge_rc = RECT {
                            left: (rc.right - badge_w - badge_margin_right).max(0),
                            top: badge_top,
                            right: (rc.right - badge_margin_right).max(0),
                            bottom: badge_top + badge_h,
                        };

                        // Dark charcoal pill background
                        let pill_brush = CreateSolidBrush(rgb(15, 23, 42)); // Slate-900
                        FillRect(hdc, &badge_rc, pill_brush);
                        let _ = DeleteObject(pill_brush);

                        // Subtle pill border
                        let pill_border = CreateSolidBrush(rgb(51, 65, 85)); // Slate-700
                        FrameRect(hdc, &badge_rc, pill_border);
                        let _ = DeleteObject(pill_border);

                        // Draw each text segment
                        let _ = SetBkMode(hdc, TRANSPARENT);
                        let mut cur_x = badge_rc.left + pad_x;

                        for (mut wide, col, w) in wide_segments {
                            SetTextColor(hdc, col);
                            let mut draw_rc = RECT {
                                left: cur_x,
                                top: badge_rc.top,
                                right: cur_x + w,
                                bottom: badge_rc.bottom,
                            };
                            let slice_len = wide.len().saturating_sub(1);
                            DrawTextW(
                                hdc,
                                &mut wide[..slice_len],
                                &mut draw_rc,
                                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                            );
                            cur_x += w;
                        }

                        SelectObject(hdc, old_font);
                        let _ = DeleteObject(font);
                    }
                }

                EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn init_overlay_thread() {
        INIT.call_once(|| {
            let (tx, rx) = channel::<OverlayCmd>();
            if let Ok(mut guard) = CONTROLLER.lock() {
                *guard = Some(OverlayController { sender: tx });
            }

            std::thread::Builder::new()
                .name("ew-overlay-thread".into())
                .spawn(move || {
                    run_overlay_loop(rx);
                })
                .expect("Failed to spawn overlay worker thread");
        });
    }

    fn run_overlay_loop(rx: Receiver<OverlayCmd>) {
        let class_name = w!("EW_OVERLAY_WINDOW_CLASS");
        let wnd_class = WNDCLASSW {
            style: Default::default(),
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: Default::default(),
            hIcon: Default::default(),
            hCursor: Default::default(),
            hbrBackground: HBRUSH(0),
            lpszMenuName: windows::core::PCWSTR::null(),
            lpszClassName: class_name,
        };

        unsafe {
            let _ = RegisterClassW(&wnd_class);

            let overlay_hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                class_name,
                w!("EW_Window_Overlay"),
                WS_POPUP,
                0,
                0,
                100,
                100,
                HWND(0),
                HMENU(0),
                HMODULE(0),
                None,
            );

            if overlay_hwnd.0 == 0 {
                tracing::error!("Failed to create overlay HWND");
                return;
            }

            // Key-color transparency: black (0x00000000) is 100% transparent and click-through
            let _ = SetLayeredWindowAttributes(
                overlay_hwnd,
                COLORREF(0x00000000),
                0,
                LWA_COLORKEY,
            );

            let mut active_target: Option<HWND> = None;
            let mut last_rect = RECT::default();
            let mut is_visible = false;

            loop {
                // 1. Process all pending commands
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        OverlayCmd::Show {
                            target_hwnd,
                            status,
                        } => {
                            let new_target = HWND(target_hwnd as _);
                            if active_target != Some(new_target) {
                                active_target = Some(new_target);
                                last_rect = RECT::default();
                            }

                            if let Ok(mut guard) = DRAW_STATE.lock() {
                                *guard = Some(WindowDrawState { status });
                            }
                            if is_visible {
                                let _ = InvalidateRect(overlay_hwnd, None, false);
                            }
                        }
                        OverlayCmd::Hide => {
                            active_target = None;
                            if is_visible {
                                let _ = ShowWindow(overlay_hwnd, SW_HIDE);
                                is_visible = false;
                            }
                            if let Ok(mut guard) = DRAW_STATE.lock() {
                                *guard = None;
                            }
                        }
                    }
                }

                // 2. Track target window position
                if let Some(target) = active_target {
                    if !IsWindow(target).as_bool() {
                        // Target window closed
                        active_target = None;
                        if is_visible {
                            let _ = ShowWindow(overlay_hwnd, SW_HIDE);
                            is_visible = false;
                        }
                    } else if IsIconic(target).as_bool() {
                        // Minimized
                        if is_visible {
                            let _ = ShowWindow(overlay_hwnd, SW_HIDE);
                            is_visible = false;
                        }
                    } else {
                        let mut target_rect = RECT::default();
                        if GetWindowRect(target, &mut target_rect).as_bool() {
                            let w = target_rect.right - target_rect.left;
                            let h = target_rect.bottom - target_rect.top;

                            if target_rect != last_rect {
                                last_rect = target_rect;
                                let _ = SetWindowPos(
                                    overlay_hwnd,
                                    HWND_TOPMOST,
                                    target_rect.left,
                                    target_rect.top,
                                    w,
                                    h,
                                    SWP_NOACTIVATE,
                                );
                                let _ = InvalidateRect(overlay_hwnd, None, false);
                            }

                            if !is_visible {
                                let _ = ShowWindow(overlay_hwnd, SW_SHOWNOACTIVATE);
                                is_visible = true;
                                let _ = InvalidateRect(overlay_hwnd, None, false);
                            }
                        }
                    }
                } else if is_visible {
                    let _ = ShowWindow(overlay_hwnd, SW_HIDE);
                    is_visible = false;
                }

                // 3. Dispatch standard Win32 messages
                let mut msg = MSG::default();
                while PeekMessageW(
                    &mut msg,
                    HWND(0),
                    0,
                    0,
                    PM_REMOVE,
                )
                .as_bool()
                {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    pub fn show_overlay(target_hwnd: isize, status: OverlayStatus) {
        init_overlay_thread();
        IS_ACTIVE.store(true, Ordering::SeqCst);
        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::Show {
                    target_hwnd,
                    status,
                });
            }
        }
    }

    pub fn hide_overlay() {
        if !IS_ACTIVE.swap(false, Ordering::SeqCst) {
            return;
        }
        init_overlay_thread();
        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::Hide);
            }
        }
    }

    pub fn update_overlay_from_state(
        filter: &crate::window_filter::WindowFilterState,
        monitoring: bool,
        clicker_running: bool,
    ) {
        if let crate::window_filter::WindowFilterMode::Single(ref win) = filter.get_mode() {
            show_overlay(
                win.hwnd,
                OverlayStatus {
                    slot: win.slot,
                    pid: win.pid,
                    monitoring,
                    clicker_running,
                },
            );
        } else {
            hide_overlay();
        }
    }
}
