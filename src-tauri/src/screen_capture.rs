//! Window discovery and screenshot capture for target game processes (Windows).

use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TargetWindow {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub is_minimized: bool,
    pub width: i32,
    pub height: i32,
}

pub fn find_target_windows(process_name: &str) -> Vec<TargetWindow> {
    #[cfg(windows)]
    {
        win::find_target_windows(process_name)
    }
    #[cfg(not(windows))]
    {
        let _ = process_name;
        Vec::new()
    }
}

pub fn capture_window(target: &TargetWindow, force_restore: bool) -> Result<Vec<u8>> {
    #[cfg(windows)]
    {
        win::capture_window(target, force_restore)
    }
    #[cfg(not(windows))]
    {
        let _ = (target, force_restore);
        Err(anyhow!("Screen capture is only supported on Windows"))
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows::core::w;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HDC, SRCCOPY,
    };
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, EnumDesktopWindows, OpenDesktopW, SetThreadDesktop, DESKTOP_CONTROL_FLAGS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, ShowWindow, SHOW_WINDOW_CMD,
        SW_MINIMIZE, SW_SHOWNOACTIVATE,
    };

    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> BOOL;
    }

    const PW_RENDERFULLCONTENT: u32 = 2;

    struct EnumContext {
        pids: Vec<u32>,
        windows: Vec<TargetWindow>,
    }

    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut EnumContext);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if !ctx.pids.contains(&pid) {
            return BOOL(1);
        }

        let is_visible = IsWindowVisible(hwnd).as_bool();
        let is_minimized = IsIconic(hwnd).as_bool();

        // If not visible and not minimized, ignore
        if !is_visible && !is_minimized {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        let title_len = GetWindowTextLengthW(hwnd);
        let mut title = String::new();
        if title_len > 0 {
            let mut buf = vec![0u16; (title_len + 1) as usize];
            let read = GetWindowTextW(hwnd, &mut buf);
            if read > 0 {
                title = String::from_utf16_lossy(&buf[..read as usize]);
            }
        }

        // Filter out helper IME/DirectInput background windows
        if title == "Default IME" || title == "MSCTFIME UI" || title == "DIEmWin" {
            return BOOL(1);
        }

        if !is_minimized && (width <= 100 || height <= 100) {
            return BOOL(1);
        }

        if title.is_empty() {
            title = format!("EndlessWar (PID {pid})");
        }

        ctx.windows.push(TargetWindow {
            hwnd: hwnd.0,
            pid,
            title,
            is_minimized,
            width,
            height,
        });

        BOOL(1)
    }

    pub fn find_target_windows(process_name: &str) -> Vec<TargetWindow> {
        let pids = crate::process_net::find_pids_by_name(process_name);
        if pids.is_empty() {
            return Vec::new();
        }

        let mut ctx = EnumContext {
            pids,
            windows: Vec::new(),
        };

        unsafe {
            if let Ok(h_desk) = OpenDesktopW(w!("default"), DESKTOP_CONTROL_FLAGS(0), false, 0x0001 | 0x0040) {
                let _ = EnumDesktopWindows(
                    h_desk,
                    Some(enum_windows_callback),
                    LPARAM(&mut ctx as *mut _ as isize),
                );
                let _ = CloseDesktop(h_desk);
            } else {
                let _ = EnumWindows(
                    Some(enum_windows_callback),
                    LPARAM(&mut ctx as *mut _ as isize),
                );
            }
        }

        ctx.windows.sort_by_key(|w| w.pid);
        ctx.windows.dedup_by_key(|w| w.hwnd);
        ctx.windows
    }

    pub fn capture_window(target: &TargetWindow, force_restore: bool) -> Result<Vec<u8>> {
        let hwnd = HWND(target.hwnd);

        unsafe {
            if let Ok(h_desk) = OpenDesktopW(w!("default"), DESKTOP_CONTROL_FLAGS(0), false, 0x01FF) {
                let _ = SetThreadDesktop(h_desk);
                let _ = CloseDesktop(h_desk);
            }
        }

        let was_minimized = unsafe { IsIconic(hwnd).as_bool() };
        if was_minimized {
            if !force_restore {
                return Err(anyhow!("Window is minimized"));
            }
            // Temporarily restore without stealing keyboard focus
            unsafe {
                ShowWindow(hwnd, SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0 as u32));
            }
            // Sleep brief moment so DWM & DirectX render the backbuffer
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        let res = capture_window_raw(hwnd, target.width, target.height);

        // If it was minimized and we forced restore, minimize it back
        if was_minimized && force_restore {
            unsafe {
                ShowWindow(hwnd, SHOW_WINDOW_CMD(SW_MINIMIZE.0 as u32));
            }
        }

        res
    }

    fn capture_window_raw(hwnd: HWND, fallback_w: i32, fallback_h: i32) -> Result<Vec<u8>> {
        unsafe {
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect);
            let mut width = rect.right - rect.left;
            let mut height = rect.bottom - rect.top;

            if width <= 0 || height <= 0 {
                width = fallback_w;
                height = fallback_h;
            }

            if width <= 0 || height <= 0 {
                return Err(anyhow!("Invalid window size: {width}x{height}"));
            }

            let hdc_screen = GetDC(HWND(0));
            if hdc_screen.is_invalid() {
                return Err(anyhow!("Failed to get screen DC"));
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_invalid() {
                ReleaseDC(HWND(0), hdc_screen);
                return Err(anyhow!("Failed to create compatible memory DC"));
            }

            let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            if hbitmap.is_invalid() {
                DeleteDC(hdc_mem);
                ReleaseDC(HWND(0), hdc_screen);
                return Err(anyhow!("Failed to create compatible bitmap"));
            }

            let old_obj = SelectObject(hdc_mem, hbitmap);

            // First attempt: PrintWindow with PW_RENDERFULLCONTENT (captures even occluded windows via DWM)
            let printed = PrintWindow(hwnd, HDC(hdc_mem.0), PW_RENDERFULLCONTENT);

            let mut bmi: BITMAPINFO = zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            bmi.bmiHeader.biHeight = -height; // Top-down scanlines
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0 as u32;

            let mut bgra_buffer = vec![0u8; (width * height * 4) as usize];
            let lines = GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                Some(bgra_buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Check if PrintWindow failed or resulted in all black/transparent
            let sample_has_content = bgra_buffer.iter().take(4000).any(|&b| b > 10);

            if !printed.as_bool() || lines == 0 || !sample_has_content {
                // Fallback attempt: BitBlt directly from window DC or screen DC
                let hdc_win = GetDC(hwnd);
                if !hdc_win.is_invalid() {
                    let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_win, 0, 0, SRCCOPY);
                    ReleaseDC(hwnd, hdc_win);

                    let _ = GetDIBits(
                        hdc_mem,
                        hbitmap,
                        0,
                        height as u32,
                        Some(bgra_buffer.as_mut_ptr() as *mut _),
                        &mut bmi,
                        DIB_RGB_COLORS,
                    );
                }
            }

            // Cleanup GDI objects
            SelectObject(hdc_mem, old_obj);
            DeleteObject(hbitmap);
            DeleteDC(hdc_mem);
            ReleaseDC(HWND(0), hdc_screen);

            // Convert BGRA to RGB for JPEG encoding
            let mut rgb_buffer = Vec::with_capacity((width * height * 3) as usize);
            for chunk in bgra_buffer.chunks_exact(4) {
                rgb_buffer.push(chunk[2]); // R
                rgb_buffer.push(chunk[1]); // G
                rgb_buffer.push(chunk[0]); // B
            }

            let mut jpeg_bytes = Vec::new();
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 82);
            encoder.encode(
                &rgb_buffer,
                width as u32,
                height as u32,
                image::ColorType::Rgb8,
            )?;

            Ok(jpeg_bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_windows_does_not_panic() {
        let pids = crate::process_net::find_pids_by_name("EndlessWar.exe");
        println!("Target PIDs: {:?}", pids);
        let windows = find_target_windows("EndlessWar.exe");
        println!("Found {} target windows: {:?}", windows.len(), windows);
    }

    #[test]
    fn test_capture_live_window() {
        let windows = find_target_windows("EndlessWar.exe");
        if let Some(win) = windows.first() {
            let res = capture_window(win, false);
            assert!(res.is_ok(), "Capture should succeed: {:?}", res.err());
            let jpeg = res.unwrap();
            println!("Captured JPEG for {:?}: {} bytes", win.title, jpeg.len());
            assert!(jpeg.len() > 1000);
        }
    }
}
