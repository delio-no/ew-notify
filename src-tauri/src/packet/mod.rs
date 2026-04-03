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
            anyhow::bail!("WinDivert is only available on Windows")
        }

        pub fn read_packet(&mut self) -> Result<Vec<u8>> {
            anyhow::bail!("WinDivert is only available on Windows")
        }
    }

    pub fn get_process_packets(_process_name: &str) -> Result<Vec<Vec<u8>>> {
        anyhow::bail!("WinDivert is only available on Windows")
    }
}

pub use win::PacketCapture;
