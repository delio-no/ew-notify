//! Windows helpers: single-instance lock and admin check.

use anyhow::{anyhow, Result};
use std::sync::Mutex;

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
    use windows::Win32::UI::Shell::IsUserAnAdmin;

    pub struct InstanceLock {
        handle: HANDLE,
    }

    impl Drop for InstanceLock {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }

    /// Returns Ok(lock) if this is the only instance; Err if another is already running.
    pub fn acquire_single_instance(name: &str) -> Result<InstanceLock> {
        let wide: Vec<u16> = OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = CreateMutexW(None, true, PCWSTR(wide.as_ptr()))?;
            let already = windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS;
            if already {
                let _ = CloseHandle(handle);
                return Err(anyhow!(
                    "EndlessWar Notifier уже запущен. Закрой другой экземпляр или иконку в трее."
                ));
            }
            Ok(InstanceLock { handle })
        }
    }

    pub fn is_elevated() -> bool {
        unsafe { IsUserAnAdmin().as_bool() }
    }
}

#[cfg(not(windows))]
mod win {
    use super::*;

    pub struct InstanceLock;

    pub fn acquire_single_instance(_name: &str) -> Result<InstanceLock> {
        Ok(InstanceLock)
    }

    pub fn is_elevated() -> bool {
        true
    }
}

pub use win::{acquire_single_instance, is_elevated, InstanceLock};

/// Keep the mutex alive for the process lifetime.
static INSTANCE: Mutex<Option<InstanceLock>> = Mutex::new(None);

pub fn ensure_single_instance() -> Result<()> {
    let lock = acquire_single_instance("Local\\ew-notify_SingleInstance")?;
    *INSTANCE.lock().map_err(|e| anyhow!("{e}"))? = Some(lock);
    Ok(())
}
