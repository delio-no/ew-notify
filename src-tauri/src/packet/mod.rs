use crate::config::Pattern;
use anyhow::Result;
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
    use windivert::{WinDivert, WinDivertAddress, WinDivertDirection, WinDivertPacket};

    pub struct PacketCapture {
        handle: WinDivert,
    }

    impl PacketCapture {
        pub fn new(_process_name: &str) -> Result<Self> {
            let filter = "tcp.SrcPort == 0".to_string();
            let handle = WinDivert::new(&filter, WinDivertDirection::Outbound, 0, true)?;
            Ok(Self { handle })
        }

        pub fn read_packet(&mut self) -> Result<Vec<u8>> {
            let mut packet_buf = [0u8; 65535];
            let mut addr = WinDivertAddress::default();
            let len = self.handle.recv(&mut packet_buf, &mut addr)?;
            Ok(packet_buf[..len].to_vec())
        }
    }

    pub fn get_process_packets(process_name: &str) -> Result<Vec<Vec<u8>>> {
        let filter = format!("process.name == \"{}\"", process_name);
        let handle = WinDivert::new(&filter, WinDivertDirection::Outbound, 0, true)?;
        let mut packets = Vec::new();
        let mut packet_buf = [0u8; 65535];
        let mut addr = WinDivertAddress::default();
        loop {
            match handle.recv(&mut packet_buf, &mut addr) {
                Ok(len) => packets.push(packet_buf[..len].to_vec()),
                Err(_) => break,
            }
        }
        Ok(packets)
    }
}

#[cfg(not(windows))]
pub mod win {
    use anyhow::Result;

    pub struct PacketCapture;

    impl PacketCapture {
        pub fn new(_process_name: &str) -> Result<Self> {
            Ok(Self)
        }

        pub fn read_packet(&mut self) -> Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    pub fn get_process_packets(_process_name: &str) -> Result<Vec<Vec<u8>>> {
        Ok(Vec::new())
    }
}

pub use win::PacketCapture;
