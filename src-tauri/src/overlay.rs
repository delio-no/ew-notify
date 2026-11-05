//! Visual overlay for highlighting tracked game windows (Windows).
//! Uses native Win32 transparent, click-through, always-on-top layered windows.
//! Supports multiple simultaneous slots (e.g. Slot 1 on Window A, Slot 2 on Window B).

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
pub fn hide_all_overlays() {}

#[cfg(not(windows))]
pub fn sync_overlays_from_state(
    _filter: &crate::window_filter::WindowFilterState,
    _monitoring: bool,
    _clicker_running: bool,
) {}

#[cfg(not(windows))]
pub fn update_overlay_from_state(
    _filter: &crate::window_filter::WindowFilterState,
    _monitoring: bool,
    _clicker_running: bool,
) {}

#[cfg(windows)]
mod win {
    use super::OverlayStatus;
    use std::collections::HashMap;
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
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
        GetWindowRect, IsIconic, IsWindow, PeekMessageW, PostQuitMessage, RegisterClassW,
        SetLayeredWindowAttributes, SetWindowPos, ShowWindow, TranslateMessage, HMENU,
        HWND_TOPMOST, LWA_COLORKEY, MSG, PM_REMOVE, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE,
        WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP,
    };

    enum OverlayCmd {
        SyncSlots {
            slots: Vec<(u32, isize, OverlayStatus)>,
        },
        HideAll,
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

    // Map of overlay_hwnd -> OverlayStatus for painting
    static DRAW_STATES: Mutex<Option<HashMap<isize, OverlayStatus>>> = Mutex::new(None);

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

                // 1. Transparent key-color fill
                let black_brush = CreateSolidBrush(rgb(0, 0, 0));
                FillRect(hdc, &rc, black_brush);
                let _ = DeleteObject(black_brush);

                let status_opt = {
                    if let Ok(guard) = DRAW_STATES.lock() {
                        guard.as_ref().and_then(|map| map.get(&(hwnd.0 as isize)).cloned())
                    } else {
                        None
                    }
                };

                // 2. Info badge in top-right corner
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
                            rgb(74, 222, 128) // Emerald green
                        } else {
                            rgb(248, 113, 113) // Coral red
                        };

                        let clicker_text = if status.clicker_running { "АКТИВЕН" } else { "ВЫКЛ" };
                        let clicker_color = if status.clicker_running {
                            rgb(74, 222, 128) // Emerald green
                        } else {
                            rgb(148, 163, 184) // Slate-400
                        };

                        let slot_str = format!("EW #{}", status.slot);
                        let segments: [(&str, COLORREF); 7] = [
                            (&slot_str, rgb(56, 189, 248)), // Sky blue
                            ("  |  ", rgb(71, 85, 105)),
                            ("Уведомления: ", rgb(203, 213, 225)),
                            (notif_text, notif_color),
                            ("  |  ", rgb(71, 85, 105)),
                            ("Кликер: ", rgb(203, 213, 225)),
                            (clicker_text, clicker_color),
                        ];

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
                        let badge_top = rc.top + 38;

                        let badge_rc = RECT {
                            left: (rc.right - badge_w - badge_margin_right).max(0),
                            top: badge_top,
                            right: (rc.right - badge_margin_right).max(0),
                            bottom: badge_top + badge_h,
                        };

                        let pill_brush = CreateSolidBrush(rgb(15, 23, 42));
                        FillRect(hdc, &badge_rc, pill_brush);
                        let _ = DeleteObject(pill_brush);

                        let pill_border = CreateSolidBrush(rgb(51, 65, 85));
                        FrameRect(hdc, &badge_rc, pill_border);
                        let _ = DeleteObject(pill_border);

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

    struct SlotOverlay {
        overlay_hwnd: HWND,
        target_hwnd: HWND,
        last_rect: RECT,
        is_visible: bool,
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
        let class_name = w!("EW_MULTI_OVERLAY_WINDOW_CLASS");
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

            let mut active_overlays: HashMap<u32, SlotOverlay> = HashMap::new();

            loop {
                // 1. Process all pending commands
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        OverlayCmd::SyncSlots { slots } => {
                            let new_slot_nums: Vec<u32> = slots.iter().map(|(s, _, _)| *s).collect();

                            // Destroy overlays for slots that are no longer active
                            let removed_slots: Vec<u32> = active_overlays
                                .keys()
                                .filter(|s| !new_slot_nums.contains(s))
                                .copied()
                                .collect();

                            for s in removed_slots {
                                if let Some(entry) = active_overlays.remove(&s) {
                                    let _ = ShowWindow(entry.overlay_hwnd, SW_HIDE);
                                    let _ = DestroyWindow(entry.overlay_hwnd);
                                    if let Ok(mut guard) = DRAW_STATES.lock() {
                                        if let Some(map) = guard.as_mut() {
                                            map.remove(&(entry.overlay_hwnd.0 as isize));
                                        }
                                    }
                                }
                            }

                            // Create or update overlays for desired slots
                            for (slot, target_hwnd, status) in slots {
                                let new_target = HWND(target_hwnd as _);

                                if let Some(entry) = active_overlays.get_mut(&slot) {
                                    if entry.target_hwnd != new_target {
                                        entry.target_hwnd = new_target;
                                        entry.last_rect = RECT::default();
                                    }
                                    if let Ok(mut guard) = DRAW_STATES.lock() {
                                        let map = guard.get_or_insert_with(HashMap::new);
                                        map.insert(entry.overlay_hwnd.0 as isize, status);
                                    }
                                    if entry.is_visible {
                                        let _ = InvalidateRect(entry.overlay_hwnd, None, false);
                                    }
                                } else {
                                    // Create new overlay window for this slot
                                    let overlay_hwnd = CreateWindowExW(
                                        WS_EX_LAYERED
                                            | WS_EX_TRANSPARENT
                                            | WS_EX_TOPMOST
                                            | WS_EX_TOOLWINDOW
                                            | WS_EX_NOACTIVATE,
                                        class_name,
                                        w!("EW_Slot_Overlay"),
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

                                    if overlay_hwnd.0 != 0 {
                                        let _ = SetLayeredWindowAttributes(
                                            overlay_hwnd,
                                            COLORREF(0x00000000),
                                            0,
                                            LWA_COLORKEY,
                                        );

                                        if let Ok(mut guard) = DRAW_STATES.lock() {
                                            let map = guard.get_or_insert_with(HashMap::new);
                                            map.insert(overlay_hwnd.0 as isize, status);
                                        }

                                        active_overlays.insert(
                                            slot,
                                            SlotOverlay {
                                                overlay_hwnd,
                                                target_hwnd: new_target,
                                                last_rect: RECT::default(),
                                                is_visible: false,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        OverlayCmd::HideAll => {
                            for (_, entry) in active_overlays.drain() {
                                let _ = ShowWindow(entry.overlay_hwnd, SW_HIDE);
                                let _ = DestroyWindow(entry.overlay_hwnd);
                                if let Ok(mut guard) = DRAW_STATES.lock() {
                                    if let Some(map) = guard.as_mut() {
                                        map.remove(&(entry.overlay_hwnd.0 as isize));
                                    }
                                }
                            }
                        }
                    }
                }

                // 2. Track each slot's target window position & visibility
                for (_, entry) in active_overlays.iter_mut() {
                    if !IsWindow(entry.target_hwnd).as_bool() {
                        if entry.is_visible {
                            let _ = ShowWindow(entry.overlay_hwnd, SW_HIDE);
                            entry.is_visible = false;
                        }
                        continue;
                    }

                    if IsIconic(entry.target_hwnd).as_bool() {
                        if entry.is_visible {
                            let _ = ShowWindow(entry.overlay_hwnd, SW_HIDE);
                            entry.is_visible = false;
                        }
                        continue;
                    }

                    let mut target_rect = RECT::default();
                    if GetWindowRect(entry.target_hwnd, &mut target_rect).as_bool() {
                        let w = target_rect.right - target_rect.left;
                        let h = target_rect.bottom - target_rect.top;

                        if target_rect != entry.last_rect {
                            entry.last_rect = target_rect;
                            let _ = SetWindowPos(
                                entry.overlay_hwnd,
                                HWND_TOPMOST,
                                target_rect.left,
                                target_rect.top,
                                w,
                                h,
                                SWP_NOACTIVATE,
                            );
                            let _ = InvalidateRect(entry.overlay_hwnd, None, false);
                        }

                        if !entry.is_visible {
                            let _ = ShowWindow(entry.overlay_hwnd, SW_SHOWNOACTIVATE);
                            entry.is_visible = true;
                            let _ = InvalidateRect(entry.overlay_hwnd, None, false);
                        }
                    }
                }

                // 3. Dispatch Win32 messages
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    pub fn sync_overlays_from_state(
        filter: &crate::window_filter::WindowFilterState,
        monitoring: bool,
        clicker_running: bool,
    ) {
        init_overlay_thread();
        IS_ACTIVE.store(true, Ordering::SeqCst);
        let slots = filter.get_all_slots();

        if slots.is_empty() {
            hide_all_overlays();
            return;
        }

        let sync_data: Vec<(u32, isize, OverlayStatus)> = slots
            .into_iter()
            .map(|w| {
                (
                    w.slot,
                    w.hwnd,
                    OverlayStatus {
                        slot: w.slot,
                        pid: w.pid,
                        monitoring,
                        clicker_running,
                    },
                )
            })
            .collect();

        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::SyncSlots { slots: sync_data });
            }
        }
    }

    pub fn hide_all_overlays() {
        if !IS_ACTIVE.swap(false, Ordering::SeqCst) {
            return;
        }
        init_overlay_thread();
        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::HideAll);
            }
        }
    }

    // Legacy compatibility helpers
    pub fn show_overlay(target_hwnd: isize, status: OverlayStatus) {
        init_overlay_thread();
        IS_ACTIVE.store(true, Ordering::SeqCst);
        if let Ok(guard) = CONTROLLER.lock() {
            if let Some(ctrl) = guard.as_ref() {
                let _ = ctrl.sender.send(OverlayCmd::SyncSlots {
                    slots: vec![(status.slot, target_hwnd, status)],
                });
            }
        }
    }

    pub fn hide_overlay() {
        hide_all_overlays();
    }

    pub fn update_overlay_from_state(
        filter: &crate::window_filter::WindowFilterState,
        monitoring: bool,
        clicker_running: bool,
    ) {
        sync_overlays_from_state(filter, monitoring, clicker_running);
    }
}
