use crate::config::Pattern;
use crate::process_net::RemoteEndpoint;
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
            .map(|(name, (bytes, desc))| Pattern {
                name: name.clone(),
                bytes: bytes.iter().map(|b| format!("{:02X}", b)).collect(),
                description: desc.clone(),
            })
            .collect()
    }

    /// Extract transport payload (TCP or UDP) from raw IP packet.
    pub fn extract_transport_payload(packet: &[u8]) -> &[u8] {
        if packet.is_empty() {
            return &[];
        }

        match packet[0] >> 4 {
            4 => {
                if packet.len() < 20 {
                    return &[];
                }
                let ihl = (packet[0] & 0x0F) as usize;
                let ip_header_len = ihl * 4;
                if packet.len() < ip_header_len {
                    return &[];
                }
                let protocol = packet[9];
                let total_header = match protocol {
                    6 => {
                        if packet.len() <= ip_header_len + 4 {
                            return &[];
                        }
                        let tcp_header_len =
                            (((packet[ip_header_len + 12] >> 4) & 0xF) * 4) as usize;
                        ip_header_len + tcp_header_len
                    }
                    17 => ip_header_len + 8,
                    _ => return &[],
                };
                if packet.len() > total_header {
                    &packet[total_header..]
                } else {
                    &[]
                }
            }
            6 => {
                if packet.len() < 40 {
                    return &[];
                }
                let next_header = packet[6];
                let ipv6_header_len = 40;
                let total_header = match next_header {
                    6 => {
                        if packet.len() <= ipv6_header_len + 4 {
                            return &[];
                        }
                        let tcp_header_len =
                            (((packet[ipv6_header_len + 12] >> 4) & 0xF) * 4) as usize;
                        ipv6_header_len + tcp_header_len
                    }
                    17 => ipv6_header_len + 8,
                    _ => return &[],
                };
                if packet.len() > total_header {
                    &packet[total_header..]
                } else {
                    &[]
                }
            }
            _ => &[],
        }
    }

    pub fn analyze(&self, packet: &[u8]) -> Option<Pattern> {
        let payload = Self::extract_transport_payload(packet);
        self.match_payload(payload)
    }

    /// Search patterns inside a raw transport payload (or pasted hex bytes).
    pub fn match_payload(&self, payload: &[u8]) -> Option<Pattern> {
        if payload.is_empty() {
            return None;
        }

        for (name, (bytes, desc)) in &self.patterns {
            if bytes.is_empty() {
                continue;
            }
            if payload.windows(bytes.len()).any(|w| w == bytes.as_slice()) {
                return Some(Pattern {
                    name: name.clone(),
                    bytes: bytes.iter().map(|b| format!("{:02X}", b)).collect(),
                    description: desc.clone(),
                });
            }
        }
        None
    }
}

/// Identity of a TCP/UDP flow for per-client cooldown.
/// For inbound game traffic, `local_port` is the client-side port (unique per EndlessWar connection).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionKey {
    pub remote_ip: String,
    pub remote_port: u16,
    pub local_ip: String,
    pub local_port: u16,
}

impl ConnectionKey {
    pub fn cooldown_id(&self, pattern_name: &str) -> String {
        format!(
            "{}:{}->{}:{}|{}",
            self.remote_ip, self.remote_port, self.local_ip, self.local_port, pattern_name
        )
    }

    /// Short label for logs / notifications (local port distinguishes clients).
    pub fn short_label(&self) -> String {
        format!("{}:{}", self.local_ip, self.local_port)
    }
}

fn ipv4_addr(bytes: &[u8]) -> String {
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

/// Parse remote/local endpoints from a raw IP packet (inbound: src=server, dst=local client).
pub fn extract_connection_key(packet: &[u8]) -> Option<ConnectionKey> {
    if packet.is_empty() {
        return None;
    }
    match packet[0] >> 4 {
        4 => {
            if packet.len() < 20 {
                return None;
            }
            let ihl = (packet[0] & 0x0F) as usize * 4;
            if packet.len() < ihl + 4 {
                return None;
            }
            let protocol = packet[9];
            if protocol != 6 && protocol != 17 {
                return None;
            }
            let remote_ip = ipv4_addr(&packet[12..16]);
            let local_ip = ipv4_addr(&packet[16..20]);
            let remote_port = read_u16(packet, ihl)?;
            let local_port = read_u16(packet, ihl + 2)?;
            Some(ConnectionKey {
                remote_ip,
                remote_port,
                local_ip,
                local_port,
            })
        }
        6 => {
            if packet.len() < 40 + 4 {
                return None;
            }
            let next = packet[6];
            if next != 6 && next != 17 {
                return None;
            }
            // Compress IPv6 for key (hex without separators is fine)
            let remote_ip = packet[8..24]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(":");
            let local_ip = packet[24..40]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(":");
            let remote_port = read_u16(packet, 40)?;
            let local_port = read_u16(packet, 42)?;
            Some(ConnectionKey {
                remote_ip,
                remote_port,
                local_ip,
                local_port,
            })
        }
        _ => None,
    }
}

/// Build WinDivert filter for traffic **from the game server**.
pub fn build_filter(
    tcp_port: Option<u16>,
    udp_port: Option<u16>,
    remote_ip: Option<&str>,
) -> String {
    let mut parts = vec!["inbound".to_string()];

    match (tcp_port, udp_port) {
        (Some(tcp), None) => parts.push(format!("tcp.SrcPort == {}", tcp)),
        (None, Some(udp)) => parts.push(format!("udp.SrcPort == {}", udp)),
        (Some(tcp), Some(udp)) => {
            parts.push(format!(
                "(tcp.SrcPort == {} or udp.SrcPort == {})",
                tcp, udp
            ));
        }
        (None, None) => {}
    }

    if let Some(ip) = remote_ip {
        if !ip.is_empty() {
            parts.push(format!("ip.SrcAddr == {}", ip));
        }
    }

    parts.join(" and ")
}

/// Filter matching any of the discovered remote endpoints (inbound from server).
pub fn build_filter_from_endpoints(endpoints: &[RemoteEndpoint]) -> String {
    if endpoints.is_empty() {
        return "inbound".to_string();
    }
    if endpoints.len() == 1 {
        let e = &endpoints[0];
        return format!(
            "inbound and tcp.SrcPort == {} and ip.SrcAddr == {}",
            e.port, e.ip
        );
    }
    let ors: Vec<String> = endpoints
        .iter()
        .map(|e| {
            format!(
                "(tcp.SrcPort == {} and ip.SrcAddr == {})",
                e.port, e.ip
            )
        })
        .collect();
    format!("inbound and ({})", ors.join(" or "))
}

#[cfg(windows)]
pub mod win {
    use super::build_filter;
    use anyhow::{anyhow, Result};
    use std::ffi::CString;
    use std::sync::Mutex;
    use windivert_sys::address::WINDIVERT_ADDRESS;
    use windivert_sys::{
        WinDivertClose, WinDivertFlags, WinDivertLayer, WinDivertOpen, WinDivertRecv,
        WinDivertShutdown, WinDivertShutdownMode,
    };
    use windows::Win32::Foundation::HANDLE;

    const PACKET_BUF_SIZE: usize = 0xFFFF;

    /// Sniff capture. `shutdown()` is safe to call from another thread while `read_packet` blocks.
    pub struct PacketCapture {
        handle: Mutex<HANDLE>,
        buffer: Mutex<Vec<u8>>,
    }

    impl PacketCapture {
        pub fn open(filter: &str) -> Result<Self> {
            tracing::info!("WinDivert sniff filter: {}", filter);

            let filter_c = CString::new(filter)?;
            let flags = WinDivertFlags::new().set_sniff();
            let handle =
                unsafe { WinDivertOpen(filter_c.as_ptr(), WinDivertLayer::Network, 0, flags) };

            if handle.is_invalid() {
                return Err(anyhow!(
                    "WinDivertOpen failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            Ok(Self {
                handle: Mutex::new(handle),
                buffer: Mutex::new(vec![0u8; PACKET_BUF_SIZE]),
            })
        }

        pub fn new(
            tcp_port: Option<u16>,
            udp_port: Option<u16>,
            remote_ip: Option<&str>,
        ) -> Result<Self> {
            Self::open(&build_filter(tcp_port, udp_port, remote_ip))
        }

        pub fn read_packet(&self) -> Result<Vec<u8>> {
            let handle = {
                let h = self
                    .handle
                    .lock()
                    .map_err(|_| anyhow!("handle lock poisoned"))?;
                *h
            };
            if handle.is_invalid() {
                return Err(anyhow!("WinDivert handle closed"));
            }

            let mut buffer = self
                .buffer
                .lock()
                .map_err(|_| anyhow!("packet buffer lock poisoned"))?;
            let mut packet_length: u32 = 0;
            let mut addr = WINDIVERT_ADDRESS::default();

            // Recv without holding handle mutex so shutdown() can run concurrently.
            let ok = unsafe {
                WinDivertRecv(
                    handle,
                    buffer.as_mut_ptr() as *mut _,
                    buffer.len() as u32,
                    &mut packet_length,
                    &mut addr,
                )
            };

            if !ok.as_bool() {
                return Err(anyhow!(
                    "WinDivertRecv failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            Ok(buffer[..packet_length as usize].to_vec())
        }

        /// Unblock a waiting recv (callable from another thread).
        pub fn shutdown(&self) {
            let Ok(mut handle) = self.handle.lock() else {
                return;
            };
            if handle.is_invalid() {
                return;
            }
            unsafe {
                let _ = WinDivertShutdown(*handle, WinDivertShutdownMode::Both);
                let _ = WinDivertClose(*handle);
            }
            *handle = HANDLE::default();
        }
    }

    impl Drop for PacketCapture {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
}

#[cfg(not(windows))]
pub mod win {
    use super::build_filter;
    use anyhow::Result;

    pub struct PacketCapture;

    impl PacketCapture {
        pub fn open(_filter: &str) -> Result<Self> {
            Ok(Self)
        }

        pub fn new(
            tcp_port: Option<u16>,
            udp_port: Option<u16>,
            remote_ip: Option<&str>,
        ) -> Result<Self> {
            let _ = build_filter(tcp_port, udp_port, remote_ip);
            Self::open("")
        }

        pub fn read_packet(&self) -> Result<Vec<u8>> {
            Ok(Vec::new())
        }

        pub fn shutdown(&self) {}
    }
}

pub use win::PacketCapture;

/// Build a minimal IPv4 + TCP packet wrapping `payload` (for tests / offline tools).
pub fn make_ipv4_tcp_packet(payload: &[u8]) -> Vec<u8> {
    make_ipv4_tcp_packet_ex(
        payload,
        [1, 2, 3, 4],
        7779,
        [10, 0, 0, 5],
        50000,
    )
}

/// IPv4 TCP packet with explicit endpoints (inbound: src=remote/server, dst=local/client).
pub fn make_ipv4_tcp_packet_ex(
    payload: &[u8],
    remote_ip: [u8; 4],
    remote_port: u16,
    local_ip: [u8; 4],
    local_port: u16,
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(40 + payload.len());
    let mut ip = [0u8; 20];
    ip[0] = 0x45;
    let total = (20 + 20 + payload.len()) as u16;
    ip[2] = (total >> 8) as u8;
    ip[3] = (total & 0xFF) as u8;
    ip[9] = 6; // TCP
    ip[12..16].copy_from_slice(&remote_ip);
    ip[16..20].copy_from_slice(&local_ip);
    packet.extend_from_slice(&ip);
    let mut tcp = [0u8; 20];
    tcp[0..2].copy_from_slice(&remote_port.to_be_bytes());
    tcp[2..4].copy_from_slice(&local_port.to_be_bytes());
    tcp[12] = 0x50;
    packet.extend_from_slice(&tcp);
    packet.extend_from_slice(payload);
    packet
}

/// Build a minimal IPv4 + UDP packet wrapping `payload`.
pub fn make_ipv4_udp_packet(payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(28 + payload.len());
    let mut ip = [0u8; 20];
    ip[0] = 0x45;
    let total = (20 + 8 + payload.len()) as u16;
    ip[2] = (total >> 8) as u8;
    ip[3] = (total & 0xFF) as u8;
    ip[9] = 17; // UDP
    packet.extend_from_slice(&ip);
    let mut udp = [0u8; 8];
    let len = (8 + payload.len()) as u16;
    udp[4] = (len >> 8) as u8;
    udp[5] = (len & 0xFF) as u8;
    packet.extend_from_slice(&udp);
    packet.extend_from_slice(payload);
    packet
}

/// Parse a hex string (spaces/newlines/`0x` prefixes allowed) into bytes.
pub fn parse_hex(input: &str) -> anyhow::Result<Vec<u8>> {
    // Drop optional 0x / 0X prefixes so the leading `0` is not kept as a digit.
    let without_prefix = input.replace("0x", "").replace("0X", "");
    let cleaned: String = without_prefix
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if cleaned.is_empty() {
        return Err(anyhow::anyhow!("empty hex input"));
    }
    if cleaned.len() % 2 != 0 {
        return Err(anyhow::anyhow!(
            "odd hex length {} after cleanup",
            cleaned.len()
        ));
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    for i in (0..cleaned.len()).step_by(2) {
        out.push(u8::from_str_radix(&cleaned[i..i + 2], 16)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, Pattern};

    fn death_pattern() -> Pattern {
        Pattern {
            name: "DEATH".into(),
            bytes: "00FE06032C010000".into(),
            description: "Персонаж умер".into(),
        }
    }

    fn stop_farm_pattern() -> Pattern {
        Pattern {
            name: "STOP_FARM".into(),
            bytes: "4E00FE11006100750074006F006600610072006D000000".into(),
            description: "Автофарм остановлен".into(),
        }
    }

    #[test]
    fn build_filter_tcp_only() {
        assert_eq!(
            build_filter(Some(7779), None, None),
            "inbound and tcp.SrcPort == 7779"
        );
    }

    #[test]
    fn build_filter_udp_only() {
        assert_eq!(
            build_filter(None, Some(9000), None),
            "inbound and udp.SrcPort == 9000"
        );
    }

    #[test]
    fn build_filter_tcp_udp_and_ip() {
        assert_eq!(
            build_filter(Some(7779), Some(9000), Some("1.2.3.4")),
            "inbound and (tcp.SrcPort == 7779 or udp.SrcPort == 9000) and ip.SrcAddr == 1.2.3.4"
        );
    }

    #[test]
    fn build_filter_from_one_endpoint() {
        let eps = vec![RemoteEndpoint {
            ip: "1.2.3.4".into(),
            port: 7777,
        }];
        assert_eq!(
            build_filter_from_endpoints(&eps),
            "inbound and tcp.SrcPort == 7777 and ip.SrcAddr == 1.2.3.4"
        );
    }

    #[test]
    fn build_filter_from_empty_endpoints() {
        assert_eq!(build_filter_from_endpoints(&[]), "inbound");
    }

    #[test]
    fn build_filter_from_multiple_endpoints_ors() {
        let eps = vec![
            RemoteEndpoint {
                ip: "1.2.3.4".into(),
                port: 7777,
            },
            RemoteEndpoint {
                ip: "5.6.7.8".into(),
                port: 7778,
            },
        ];
        assert_eq!(
            build_filter_from_endpoints(&eps),
            "inbound and ((tcp.SrcPort == 7777 and ip.SrcAddr == 1.2.3.4) or (tcp.SrcPort == 7778 and ip.SrcAddr == 5.6.7.8))"
        );
    }

    #[test]
    fn match_payload_direct_without_ip_wrap() {
        let death = parse_hex("00FE06032C010000").unwrap();
        let analyzer = PacketAnalyzer::new(vec![death_pattern()]);
        let hit = analyzer
            .match_payload(&death)
            .expect("DEATH should match raw payload");
        assert_eq!(hit.name, "DEATH");
        assert!(analyzer.match_payload(b"nope").is_none());
        assert!(analyzer.match_payload(&[]).is_none());
    }

    #[test]
    fn match_death_in_udp_packet() {
        let death = parse_hex("00FE06032C010000").unwrap();
        let analyzer = PacketAnalyzer::new(vec![death_pattern(), stop_farm_pattern()]);
        let pkt = make_ipv4_udp_packet(&death);
        let hit = analyzer.analyze(&pkt).expect("DEATH in UDP");
        assert_eq!(hit.name, "DEATH");
    }

    #[test]
    fn parse_hex_strips_noise() {
        assert_eq!(
            parse_hex("00 FE 06 03 2C 01 00 00").unwrap(),
            vec![0x00, 0xFE, 0x06, 0x03, 0x2C, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            parse_hex("0x00fe0603").unwrap(),
            vec![0x00, 0xFE, 0x06, 0x03]
        );
    }

    #[test]
    fn extract_tcp_payload() {
        let payload = b"hello-ew";
        let pkt = make_ipv4_tcp_packet(payload);
        assert_eq!(PacketAnalyzer::extract_transport_payload(&pkt), payload);
    }

    #[test]
    fn extract_udp_payload() {
        let payload = b"udp-payload";
        let pkt = make_ipv4_udp_packet(payload);
        assert_eq!(PacketAnalyzer::extract_transport_payload(&pkt), payload);
    }

    #[test]
    fn extract_empty_on_truncated() {
        assert!(PacketAnalyzer::extract_transport_payload(&[]).is_empty());
        assert!(PacketAnalyzer::extract_transport_payload(&[0x45, 0, 0]).is_empty());
        // TCP header only, no payload
        let mut pkt = make_ipv4_tcp_packet(&[]);
        assert!(PacketAnalyzer::extract_transport_payload(&pkt).is_empty());
        // corrupt version nibble
        pkt[0] = 0x00;
        assert!(PacketAnalyzer::extract_transport_payload(&pkt).is_empty());
    }

    #[test]
    fn match_death_in_tcp_packet() {
        let death = parse_hex("00FE06032C010000").unwrap();
        let mut payload = vec![0xAA, 0xBB];
        payload.extend_from_slice(&death);
        payload.extend_from_slice(&[0xCC]);

        let analyzer = PacketAnalyzer::new(vec![death_pattern(), stop_farm_pattern()]);
        let pkt = make_ipv4_tcp_packet(&payload);
        let hit = analyzer.analyze(&pkt).expect("DEATH should match");
        assert_eq!(hit.name, "DEATH");
    }

    #[test]
    fn match_stop_farm_in_tcp_packet() {
        let stop = parse_hex("4E00FE11006100750074006F006600610072006D000000").unwrap();
        let analyzer = PacketAnalyzer::new(vec![death_pattern(), stop_farm_pattern()]);
        let pkt = make_ipv4_tcp_packet(&stop);
        let hit = analyzer.analyze(&pkt).expect("STOP_FARM should match");
        assert_eq!(hit.name, "STOP_FARM");
    }

    #[test]
    fn no_match_on_unrelated_payload() {
        let analyzer = PacketAnalyzer::new(vec![death_pattern(), stop_farm_pattern()]);
        let pkt = make_ipv4_tcp_packet(b"nothing here");
        assert!(analyzer.analyze(&pkt).is_none());
    }

    #[test]
    fn default_config_patterns_match_known_hex() {
        let cfg = AppConfig::default();
        let analyzer = PacketAnalyzer::new(cfg.target.patterns.clone());
        assert_eq!(analyzer.get_patterns().len(), 2);

        let death_pkt = make_ipv4_tcp_packet(&parse_hex("00FE06032C010000").unwrap());
        assert_eq!(
            analyzer.analyze(&death_pkt).map(|p| p.name),
            Some("DEATH".into())
        );

        let farm_pkt = make_ipv4_tcp_packet(
            &parse_hex("4E00FE11006100750074006F006600610072006D000000").unwrap(),
        );
        assert_eq!(
            analyzer.analyze(&farm_pkt).map(|p| p.name),
            Some("STOP_FARM".into())
        );
    }

    #[test]
    fn invalid_pattern_hex_is_skipped() {
        let analyzer = PacketAnalyzer::new(vec![Pattern {
            name: "BAD".into(),
            bytes: "ZZ".into(),
            description: "broken".into(),
        }]);
        assert!(analyzer.get_patterns().is_empty());
    }

    #[test]
    fn add_remove_pattern() {
        let mut analyzer = PacketAnalyzer::new(vec![]);
        analyzer.add_pattern(death_pattern());
        assert_eq!(analyzer.get_patterns().len(), 1);
        analyzer.remove_pattern("DEATH");
        assert!(analyzer.get_patterns().is_empty());
    }

    #[test]
    fn connection_key_distinguishes_two_clients() {
        let payload = b"x";
        let a = make_ipv4_tcp_packet_ex(payload, [1, 1, 1, 1], 7779, [10, 0, 0, 2], 40001);
        let b = make_ipv4_tcp_packet_ex(payload, [1, 1, 1, 1], 7779, [10, 0, 0, 2], 40002);
        let ka = extract_connection_key(&a).unwrap();
        let kb = extract_connection_key(&b).unwrap();
        assert_eq!(ka.remote_port, 7779);
        assert_eq!(ka.local_port, 40001);
        assert_eq!(kb.local_port, 40002);
        assert_ne!(ka.cooldown_id("DEATH"), kb.cooldown_id("DEATH"));
        assert_eq!(ka.cooldown_id("DEATH"), ka.cooldown_id("DEATH"));
    }
}
