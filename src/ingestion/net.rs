use crate::metrics;
use std::net::UdpSocket;
use std::sync::atomic::Ordering;

/// VITA-49 Packet Header Structure
pub struct Vita49Header {
    pub packet_type: u8,
    pub seq_num: u8,
    pub packet_size_words: u16,
    pub stream_id: u32,
    pub timestamp_seconds: u32,
    pub timestamp_fraction: u64,
}

impl Vita49Header {
    /// Decode a 20-byte VITA-49.0 standard header
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 20 {
            return Err("Packet too short for VITA-49 header");
        }

        let word0 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let packet_type = ((word0 >> 28) & 0x0F) as u8;
        let seq_num = ((word0 >> 16) & 0x0F) as u8;
        let packet_size_words = (word0 & 0xFFFF) as u16;

        let stream_id = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let timestamp_seconds = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let timestamp_fraction = u64::from_be_bytes([
            bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
        ]);

        Ok(Self {
            packet_type,
            seq_num,
            packet_size_words,
            stream_id,
            timestamp_seconds,
            timestamp_fraction,
        })
    }
}

/// UDP Listener for VITA-49 data streams
pub struct UdpIngestReceiver {
    socket: UdpSocket,
    expected_seq: u8,
}

impl UdpIngestReceiver {
    /// Bind to a local port
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self {
            socket,
            expected_seq: 0,
        })
    }

    /// Read a single packet, parse it, and fill the provided SampleBlock with its payload
    pub fn recv_packet(
        &mut self,
        block: &mut crate::buffer::SampleBlock,
    ) -> Result<(), &'static str> {
        // Buffer: 20 bytes VITA-49 header + 512 bytes payload = 532 bytes
        let mut buf = [0u8; 532];
        let (amt, _) = self
            .socket
            .recv_from(&mut buf)
            .map_err(|_| "Socket read error")?;

        if amt < 532 {
            return Err("Received packet size smaller than expected 532 bytes");
        }

        // Parse VITA-49 Header
        let header = Vita49Header::parse(&buf[0..20])?;

        // Detect network drops using sequence gaps (modulo 16)
        let diff = header.seq_num.wrapping_sub(self.expected_seq) & 0x0F;
        if diff > 0 {
            // Log sequence drop metric
            metrics::METRIC_BLOCKS_DROPPED.store(
                metrics::METRIC_BLOCKS_DROPPED.load(Ordering::Relaxed) + diff as u32,
                Ordering::Relaxed,
            );
        }
        self.expected_seq = (header.seq_num + 1) & 0x0F;

        // Extract 512-byte sample payload and copy to the provided block
        block.samples.copy_from_slice(&buf[20..532]);
        block.timestamp_ticks = header.timestamp_fraction;
        block.block_index = header.timestamp_seconds; // Use seconds as block index for net tracking

        Ok(())
    }
}
