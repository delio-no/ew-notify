//! Visual overlay for highlighting the tracked game window (Windows).
//! Uses a native Win32 transparent, click-through, always-on-top layered window.

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub fn show_overlay(_hwnd: isize, _label: &str, _flash: bool) {}

#[cfg(not(windows))]
pub fn hide_overlay() {}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Mutex, Once};
    use std::time::Duration;
    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
        FrameRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER,
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
            label: String,
            flash: bool,
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
        label: String,
        is_flash_bright: bool,
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

                // 1. Fill entire window with black (transparent key)
                let black_brush = CreateSolidBrush(rgb(0, 0, 0));
                FillRect(hdc, &rc, black_brush);
                let _ = DeleteObject(black_brush);

                let (label, flash_bright) = {
                    if let Ok(guard) = DRAW_STATE.lock() {
                        if let Some(state) = guard.as_ref() {
                            (state.label.clone(), state.is_flash_bright)
                        } else {
                            (String::new(), false)
                        }
                    } else {
                        (String::new(), false)
                    }
                };

                // 2. Draw 2px neon green border (or brighter on pulse)
                let border_col = if flash_bright {
                    rgb(255, 255, 255)
                } else {
                    rgb(34, 197, 94) // Neon green #22c55e
                };
                let border_brush = CreateSolidBrush(border_col);

                // Outer border line
                FrameRect(hdc, &rc, border_brush);
                // Inner border line for 2px thickness
                let inner_rc = RECT {
                    left: rc.left + 1,
                    top: rc.top + 1,
                    right: rc.right - 1,
                    bottom: rc.bottom - 1,
                };
                FrameRect(hdc, &inner_rc, border_brush);
                let _ = DeleteObject(border_brush);

                // 3. Top-Right Badge: [ 🟢 EW #1 (PID 7720) ]
                if !label.is_empty() && (rc.right - rc.left) > 200 {
                    let badge_w = 190;
                    let badge_h = 28;
                    let badge_margin = 8;
                    let badge_rc = RECT {
                        left: (rc.right - badge_w - badge_margin).max(0),
                        top: rc.top + badge_margin,
                        right: (rc.right - badge_margin).max(0),
                        bottom: rc.top + badge_margin + badge_h,
                    };

                    // Pill dark background (slate-900 / dark charcoal)
                    let pill_brush = CreateSolidBrush(rgb(15, 23, 42));
                    FillRect(hdc, &badge_rc, pill_brush);
                    let _ = DeleteObject(pill_brush);

                    // Pill border
                    let pill_border = CreateSolidBrush(border_col);
                    FrameRect(hdc, &badge_rc, pill_border);
                    let _ = DeleteObject(pill_border);

                    // Pill Text
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    let _ = SetTextColor(hdc, rgb(255, 255, 255));

                    let font = CreateFontW(
                        15,
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

                    let mut wide_label: Vec<u16> =
                        label.encode_utf16().chain(std::iter::once(0)).collect();
                    let mut text_rc = badge_rc;
                    let text_slice_len = wide_label.len().saturating_sub(1);
                    DrawTextW(
                        hdc,
                        &mut wide_label[..text_slice_len],
                        &mut text_rc,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );

                    SelectObject(hdc, old_font);
                    let _ = DeleteObject(font);
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
            let mut flash_ticks = 0u32;
            let mut is_visible = false;

            loop {
                // 1. Process all pending commands
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        OverlayCmd::Show {
                            target_hwnd,
                            label,
                            flash,
                        } => {
                            active_target = Some(HWND(target_hwnd as _));
                            flash_ticks = if flash { 6 } else { 0 };
                            last_rect = RECT::default();

                            if let Ok(mut guard) = DRAW_STATE.lock() {
                                *guard = Some(WindowDrawState {
                                    label,
                                    is_flash_bright: flash_ticks % 2 == 1,
                                });
                            }
                        }
                        OverlayCmd::Hide => {
                            active_target = None;
                            flash_ticks = 0;
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

                    // Handle flash animation (pulse 3 times over ~300ms)
                    if flash_ticks > 0 {
                        flash_ticks -= 1;
                        if let Ok(mut guard) = DRAW_STATE.lock() {
                            if let Some(state) = guard.as_mut() {
                                state.is_flash_bright = flash_ticks % 2 == 1;
                            }
                        }
                        let _ = InvalidateRect(overlay_hwnd, None, false);
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

    pub fn show_overlay(target_hwnd: isize, label: &str, flash: bool) {
        init_overlay_thread();
        IS_ACTIVE.store(true, Ordering::SeqCst);
        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::Show {
                    target_hwnd,
                    label: label.to_string(),
                    flash,
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
}
