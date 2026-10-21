//! Discover remote TCP endpoints owned by a named process (Windows).

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize)]
pub struct RemoteEndpoint {
    pub ip: String,
    pub port: u16,
}

impl RemoteEndpoint {
    pub fn label(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// Find PIDs whose image name matches `process_name` (case-insensitive; `.exe` optional).
pub fn find_pids_by_name(process_name: &str) -> Vec<u32> {
    #[cfg(windows)]
    {
        win::find_pids_by_name(process_name)
    }
    #[cfg(not(windows))]
    {
        let _ = process_name;
        Vec::new()
    }
}

/// ESTABLISHED remote endpoints for the given process name (deduped, filtered).
pub fn discover_remote_endpoints(process_name: &str) -> Vec<RemoteEndpoint> {
    #[cfg(windows)]
    {
        win::discover_remote_endpoints(process_name)
    }
    #[cfg(not(windows))]
    {
        let _ = process_name;
        Vec::new()
    }
}

/// Prefer game-like remotes: drop loopback; if any non-80/443 remain, drop 80/443.
pub fn filter_game_endpoints(endpoints: Vec<RemoteEndpoint>) -> Vec<RemoteEndpoint> {
    let mut set: BTreeSet<RemoteEndpoint> = endpoints
        .into_iter()
        .filter(|e| e.ip != "127.0.0.1" && e.ip != "0.0.0.0" && e.port != 0)
        .collect();

    let has_non_web = set.iter().any(|e| e.port != 80 && e.port != 443);
    if has_non_web {
        set.retain(|e| e.port != 80 && e.port != 443);
    }

    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(ip: &str, port: u16) -> RemoteEndpoint {
        RemoteEndpoint {
            ip: ip.into(),
            port,
        }
    }

    #[test]
    fn filter_drops_loopback_and_zero() {
        let out = filter_game_endpoints(vec![
            ep("127.0.0.1", 7777),
            ep("0.0.0.0", 7777),
            ep("1.2.3.4", 0),
            ep("1.2.3.4", 7777),
        ]);
        assert_eq!(out, vec![ep("1.2.3.4", 7777)]);
    }

    #[test]
    fn filter_drops_web_when_game_ports_present() {
        let out = filter_game_endpoints(vec![
            ep("1.2.3.4", 80),
            ep("1.2.3.4", 443),
            ep("1.2.3.4", 7777),
        ]);
        assert_eq!(out, vec![ep("1.2.3.4", 7777)]);
    }

    #[test]
    fn filter_keeps_web_if_only_web() {
        let out = filter_game_endpoints(vec![ep("1.2.3.4", 80), ep("1.2.3.4", 443)]);
        assert_eq!(out.len(), 2);
        assert!(out.contains(&ep("1.2.3.4", 80)));
        assert!(out.contains(&ep("1.2.3.4", 443)));
    }

    #[test]
    fn filter_dedups() {
        let out = filter_game_endpoints(vec![
            ep("1.2.3.4", 7777),
            ep("1.2.3.4", 7777),
            ep("1.2.3.4", 7777),
        ]);
        assert_eq!(out, vec![ep("1.2.3.4", 7777)]);
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_CONNECTIONS,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    const MIB_TCP_STATE_ESTAB: u32 = 5;
    const AF_INET: u32 = 2;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    fn norm_name(name: &str) -> String {
        let lower = name.trim().to_ascii_lowercase();
        if lower.ends_with(".exe") {
            lower
        } else {
            format!("{lower}.exe")
        }
    }

    pub fn find_pids_by_name(process_name: &str) -> Vec<u32> {
        let want = norm_name(process_name);
        let mut pids = Vec::new();

        unsafe {
            let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return pids,
            };

            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snap, &mut entry).as_bool() {
                loop {
                    let end = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
                    if exe == want {
                        pids.push(entry.th32ProcessID);
                    }
                    if !Process32NextW(snap, &mut entry).as_bool() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
        pids
    }

    fn ipv4_string(addr_be: u32) -> String {
        let b = addr_be.to_be_bytes();
        format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
    }

    pub fn discover_remote_endpoints(process_name: &str) -> Vec<RemoteEndpoint> {
        let pids = find_pids_by_name(process_name);
        if pids.is_empty() {
            return Vec::new();
        }
        let pid_set: BTreeSet<u32> = pids.into_iter().collect();

        let mut found = Vec::new();
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(
                None,
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            );
            if size == 0 {
                return Vec::new();
            }

            let mut buf = vec![0u8; size as usize];
            let mut status = GetExtendedTcpTable(
                Some(buf.as_mut_ptr() as *mut _),
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            );
            if status == ERROR_INSUFFICIENT_BUFFER {
                buf.resize(size as usize, 0);
                status = GetExtendedTcpTable(
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut size,
                    false,
                    AF_INET,
                    TCP_TABLE_OWNER_PID_CONNECTIONS,
                    0,
                );
            }
            if status != 0 {
                return Vec::new();
            }

            if buf.len() < size_of::<u32>() {
                return Vec::new();
            }
            let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
            let count = table.dwNumEntries as usize;
            let rows_ptr = table.table.as_ptr();
            for i in 0..count {
                let row = &*rows_ptr.add(i);
                if row.dwState != MIB_TCP_STATE_ESTAB {
                    continue;
                }
                if !pid_set.contains(&row.dwOwningPid) {
                    continue;
                }
                let ip = ipv4_string(row.dwRemoteAddr);
                let port = u16::from_be(row.dwRemotePort as u16);
                found.push(RemoteEndpoint { ip, port });
            }
        }

        filter_game_endpoints(found)
    }
}
