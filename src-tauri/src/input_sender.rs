//! Background keystroke simulation for target game windows via Win32 PostMessage.

use anyhow::{anyhow, Result};
use std::time::Duration;

/// Parse human-readable key name to Win32 Virtual-Key (VK) code.
pub fn parse_key_to_vk(key: &str) -> Option<u16> {
    let clean = key.trim().to_uppercase();
    if clean.is_empty() {
        return None;
    }

    // Single character: letters A-Z and digits 0-9
    if clean.chars().count() == 1 {
        let ch = clean.chars().next()?;
        if ch.is_ascii_uppercase() {
            return Some(ch as u16);
        }
        if ch.is_ascii_digit() {
            return Some(ch as u16);
        }
    }

    // Function keys F1 - F24
    if let Some(num_str) = clean.strip_prefix('F') {
        if let Ok(num) = num_str.parse::<u16>() {
            if (1..=24).contains(&num) {
                return Some(0x70 + (num - 1));
            }
        }
    }

    // Named keys
    match clean.as_str() {
        "SPACE" | "SPACEBAR" => Some(0x20),
        "ENTER" | "RETURN" => Some(0x0D),
        "ESC" | "ESCAPE" => Some(0x1B),
        "TAB" => Some(0x09),
        "BACKSPACE" => Some(0x08),
        "INSERT" => Some(0x2D),
        "DELETE" | "DEL" => Some(0x2E),
        "HOME" => Some(0x24),
        "END" => Some(0x23),
        "PAGEUP" | "PGUP" => Some(0x21),
        "PAGEDOWN" | "PGDN" => Some(0x22),
        "UP" => Some(0x26),
        "DOWN" => Some(0x28),
        "LEFT" => Some(0x25),
        "RIGHT" => Some(0x27),
        "NUMPAD0" => Some(0x60),
        "NUMPAD1" => Some(0x61),
        "NUMPAD2" => Some(0x62),
        "NUMPAD3" => Some(0x63),
        "NUMPAD4" => Some(0x64),
        "NUMPAD5" => Some(0x65),
        "NUMPAD6" => Some(0x66),
        "NUMPAD7" => Some(0x67),
        "NUMPAD8" => Some(0x68),
        "NUMPAD9" => Some(0x69),
        _ => None,
    }
}

/// Send keystroke to target window asynchronously in the background without stealing focus.
pub async fn send_key_background(hwnd: isize, key: &str, hold_ms: u64) -> Result<()> {
    #[cfg(windows)]
    {
        win::send_key_background(hwnd, key, hold_ms).await
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, key, hold_ms);
        Err(anyhow!("Key sending is only supported on Windows"))
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, MAPVK_VK_TO_VSC,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindow, PostMessageW, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
    };

    pub async fn send_key_background(hwnd_val: isize, key: &str, hold_ms: u64) -> Result<()> {
        let hwnd = HWND(hwnd_val as _);
        unsafe {
            if !IsWindow(hwnd).as_bool() {
                return Err(anyhow!("Target window handle is invalid or closed: {}", hwnd_val));
            }
        }

        let vk = parse_key_to_vk(key).ok_or_else(|| anyhow!("Unknown or unsupported key: '{}'", key))?;

        let scan_code = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };

        // Bit 0-15: repeat count (1)
        // Bit 16-23: scan code
        // Bit 24: extended key flag
        // Bit 29: context code (0)
        // Bit 30: previous key state (0)
        // Bit 31: transition state (0 for down)
        let lparam_down = (1isize) | ((scan_code as isize) << 16);

        // For keyup:
        // Bit 30: previous key state (1)
        // Bit 31: transition state (1 for up)
        let lparam_up = (1isize) | ((scan_code as isize) << 16) | (1isize << 30) | (1isize << 31);

        let down_res = unsafe {
            PostMessageW(
                hwnd,
                WM_KEYDOWN,
                WPARAM(vk as usize),
                LPARAM(lparam_down),
            )
        };
        if !down_res.as_bool() {
            return Err(anyhow!("PostMessageW WM_KEYDOWN failed for HWND {}", hwnd_val));
        }

        // Also post WM_CHAR for printable characters (e.g. letters and digits)
        if (0x20..=0x7E).contains(&vk) {
            let _ = unsafe {
                PostMessageW(
                    hwnd,
                    WM_CHAR,
                    WPARAM(vk as usize),
                    LPARAM(lparam_down),
                )
            };
        }

        let sleep_duration = Duration::from_millis(if hold_ms == 0 { 75 } else { hold_ms });
        tokio::time::sleep(sleep_duration).await;

        let up_res = unsafe {
            PostMessageW(
                hwnd,
                WM_KEYUP,
                WPARAM(vk as usize),
                LPARAM(lparam_up),
            )
        };
        if !up_res.as_bool() {
            return Err(anyhow!("PostMessageW WM_KEYUP failed for HWND {}", hwnd_val));
        }

        tracing::info!(
            "PostMessage keystroke sent to HWND {}: key='{}' (VK 0x{:02X}, scan 0x{:02X}, hold {}ms)",
            hwnd_val,
            key,
            vk,
            scan_code,
            sleep_duration.as_millis()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_characters() {
        assert_eq!(parse_key_to_vk("z"), Some(0x5A));
        assert_eq!(parse_key_to_vk("Z"), Some(0x5A));
        assert_eq!(parse_key_to_vk("a"), Some(0x41));
        assert_eq!(parse_key_to_vk("1"), Some(0x31));
        assert_eq!(parse_key_to_vk("9"), Some(0x39));
    }

    #[test]
    fn test_parse_function_keys() {
        assert_eq!(parse_key_to_vk("f1"), Some(0x70));
        assert_eq!(parse_key_to_vk("F1"), Some(0x70));
        assert_eq!(parse_key_to_vk("F12"), Some(0x7B));
        assert_eq!(parse_key_to_vk("f24"), Some(0x87));
        assert_eq!(parse_key_to_vk("F25"), None);
    }

    #[test]
    fn test_parse_named_keys() {
        assert_eq!(parse_key_to_vk("space"), Some(0x20));
        assert_eq!(parse_key_to_vk("ENTER"), Some(0x0D));
        assert_eq!(parse_key_to_vk("esc"), Some(0x1B));
        assert_eq!(parse_key_to_vk("tab"), Some(0x09));
        assert_eq!(parse_key_to_vk("up"), Some(0x26));
        assert_eq!(parse_key_to_vk("numpad1"), Some(0x61));
    }

    #[test]
    fn test_parse_invalid_key() {
        assert_eq!(parse_key_to_vk(""), None);
        assert_eq!(parse_key_to_vk("   "), None);
        assert_eq!(parse_key_to_vk("NOT_A_KEY"), None);
    }
}
