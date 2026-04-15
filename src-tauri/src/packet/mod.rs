use crate::config::Pattern;
use std::collections::HashMap;

#[derive(Clone)]
pub struct PacketAnalyzer {
    patterns: HashMap<String, (Vec<u8>, String)>,
}

impl PacketAnalyzer {
    pub fn new(patterns: Vec<Pattern>) -> Self {
        let mut map = HashMap::new();
        for p in patterns {
            if let Ok(bytes) = p.to_bytes() {
                map.insert(p.name.clone(), (bytes, p.description));
            }
        }
        Self { patterns: map }
    }

    pub fn add_pattern(&mut self, pattern: Pattern) {
        if let Ok(bytes) = pattern.to_bytes() {
            self.patterns
                .insert(pattern.name, (bytes, pattern.description));
        }
    }

    pub fn remove_pattern(&mut self, name: &str) {
        self.patterns.remove(name);
    }

    pub fn get_patterns(&self) -> Vec<Pattern> {
        self.patterns
            .iter()
            .map(|(name, (_, desc))| Pattern {
                name: name.clone(),
                bytes: String::new(),
                description: desc.clone(),
            })
            .collect()
    }

    pub fn analyze(&self, packet: &[u8]) -> Option<Pattern> {
        for (name, (bytes, desc)) in &self.patterns {
            if packet.windows(bytes.len()).any(|w| w == bytes) {
                return Some(Pattern {
                    name: name.clone(),
                    bytes: String::new(),
                    description: desc.clone(),
                });
            }
        }
        None
    }
}

#[cfg(windows)]
pub mod win {
    use anyhow::Result;
    use windivert::prelude::*;

    pub struct PacketCapture {
        handle: WinDivert<NetworkLayer>,
    }

    impl PacketCapture {
        pub fn new(_process_name: &str) -> Result<Self> {
            let filter = "tcp.SrcPort == 0".to_string();
            let handle = WinDivert::network(&filter, 0, WinDivertFlags::default())?;
            Ok(Self { handle })
        }

        pub fn read_packet(&mut self) -> Result<Vec<u8>> {
            let packet = self.handle.recv(None)?;
            Ok(packet.data.to_vec())
        }
    }

    pub fn get_process_packets(process_name: &str) -> Result<Vec<Vec<u8>>> {
        let filter = format!("process.name == \"{}\"", process_name);
        let handle = WinDivert::network(&filter, 0, WinDivertFlags::default())?;
        let mut packets = Vec::new();
        loop {
            match handle.recv(None) {
                Ok(packet) => packets.push(packet.data.to_vec()),
                Err(_) => break,
            }
        }
        Ok(packets)
    }
}

#[cfg(not(windows))]
pub mod linux {
    use anyhow::{Context, Result};
    use pcap::{Active, Device};
    use std::fs;

    pub struct PacketCapture {
        capture: Option<pcap::Capture<Active>>,
        process_ip: Option<String>,
    }

    impl PacketCapture {
        pub fn new(process_name: &str) -> Result<Self> {
            let devices = Device::list().context("Failed to list network devices")?;
            let device = devices
                .into_iter()
                .find(|d| d.name != "lo")
                .context("No suitable network device found")?;

            let mut capture = device
                .open()
                .context("Failed to open device")?
                .setnonblock()?;

            let process_ip = get_process_ip(process_name).ok();

            if let Some(ref ip) = process_ip {
                let _ = capture.filter(&format!("tcp and host {}", ip), true);
            }

            Ok(Self {
                capture: Some(capture),
                process_ip,
            })
        }

        pub fn read_packet(&mut self) -> Result<Vec<u8>> {
            if let Some(ref mut capture) = self.capture {
                match capture.next_packet() {
                    Ok(packet) => Ok(packet.data.to_vec()),
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("No packets")
                            || err_str.contains("timed out")
                            || err_str.contains("non-blocking")
                        {
                            Ok(Vec::new())
                        } else {
                            Err(e.into())
                        }
                    }
                }
            } else {
                Ok(Vec::new())
            }
        }
    }

    fn get_process_ip(process_name: &str) -> Result<String> {
        let pids = find_pids_by_name(process_name)?;

        for pid in pids {
            if let Some(ip) = find_local_ip_for_pid(pid)? {
                return Ok(ip);
            }
        }

        Err(anyhow::anyhow!("No IP found for process"))
    }

    fn find_pids_by_name(name: &str) -> Result<Vec<u32>> {
        let mut pids = Vec::new();

        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let path = entry.path();

            if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                let cmdline_path = path.join("cmdline");
                if let Ok(content) = fs::read_to_string(&cmdline_path) {
                    if content.contains(name) {
                        pids.push(pid);
                    }
                }
            }
        }

        Ok(pids)
    }

    fn find_local_ip_for_pid(pid: u32) -> Result<Option<String>> {
        let tcp_path = format!("/proc/{}/fd", pid);

        if !std::path::Path::new(&tcp_path).exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&tcp_path)? {
            let entry = entry?;
            let link = entry.path();

            let target = fs::read_link(&link)?;
            let target_str = target.to_string_lossy().to_string();
            if target_str.starts_with("socket:") {
                let inode_str = target_str
                    .trim_start_matches("socket:[")
                    .trim_end_matches(']');
                let inode = inode_str.parse::<u64>().ok();

                if let Some(inode) = inode {
                    if let Some(ip) = find_ip_by_inode(inode, "/proc/net/tcp") {
                        return Ok(Some(ip));
                    }
                    if let Some(ip) = find_ip_by_inode(inode, "/proc/net/tcp6") {
                        return Ok(Some(ip));
                    }
                }
            }
        }

        Ok(None)
    }

    fn find_ip_by_inode(_inode: u64, path: &str) -> Option<String> {
        let content = fs::read_to_string(path).ok()?;

        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let local_address = parts[1];
                let parts_addr: Vec<&str> = local_address.split(':').collect();
                if parts_addr.len() == 2 {
                    let hex_port = parts_addr[1];
                    if hex_port != "0000" {
                        let ip_hex = parts_addr[0];
                        let ip_chars: Vec<char> = ip_hex.chars().collect();

                        if ip_chars.len() >= 8 {
                            let octets: Vec<u8> = (0..4)
                                .map(|i| {
                                    let idx = i * 2;
                                    let s = format!("{}{}", ip_chars[idx], ip_chars[idx + 1]);
                                    u8::from_str_radix(&s, 16).unwrap_or(0)
                                })
                                .collect();

                            if octets.len() == 4 && !octets.iter().all(|&o| o == 0) {
                                return Some(format!(
                                    "{}.{}.{}.{}",
                                    octets[0], octets[1], octets[2], octets[3]
                                ));
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_process_packets(_process_name: &str) -> Result<Vec<Vec<u8>>> {
        let devices = Device::list().context("Failed to list network devices")?;
        let device = devices
            .into_iter()
            .find(|d| d.name != "lo")
            .context("No suitable network device found")?;

        let mut capture = device.open().context("Failed to open device")?;
        let _ = capture.filter("tcp", true);

        let mut packets = Vec::new();
        for _ in 0..10 {
            match capture.next_packet() {
                Ok(packet) => packets.push(packet.data.to_vec()),
                Err(_) => break,
            }
        }

        Ok(packets)
    }
}

#[cfg(windows)]
pub use win::PacketCapture;

#[cfg(not(windows))]
pub use linux::PacketCapture;
