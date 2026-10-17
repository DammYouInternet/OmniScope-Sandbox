use crate::commands::{GuestCommand, GuestResponse};
use crate::error::ProtocolError;
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

/// Magic bytes identifying an OmniScope frame ('OMNI').
pub const PROTOCOL_MAGIC: [u8; 4] = [0x4F, 0x4D, 0x4E, 0x49];

/// Current protocol wire version.
pub const PROTOCOL_VERSION: u8 = 1;

/// Fixed length of the binary frame header in bytes.
pub const HEADER_SIZE: usize = 32;

/// Maximum payload size allowed (16 MB to support high-res screenshots).
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

pub const FLAG_COMPRESSED: u8 = 0x01;
pub const FLAG_URGENT: u8 = 0x02;
pub const FLAG_ERROR: u8 = 0x04;

/// Fixed-size 32-byte binary frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub flags: u8,
    pub reserved: u16,
    pub sequence_id: u64,
    pub trace_id: u64,
    pub payload_len: u32,
    pub crc32: u32,
}

impl FrameHeader {
    /// Creates a new frame header for the given payload and metadata.
    pub fn new(sequence_id: u64, trace_id: u64, flags: u8, payload: &[u8]) -> Result<Self, ProtocolError> {
        let payload_len = payload.len() as u32;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                len: payload_len,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        let mut hasher = Hasher::new();
        hasher.update(payload);
        let crc32 = hasher.finalize();

        Ok(Self {
            magic: PROTOCOL_MAGIC,
            version: PROTOCOL_VERSION,
            flags,
            reserved: 0,
            sequence_id,
            trace_id,
            payload_len,
            crc32,
        })
    }

    /// Serializes the header into a 32-byte fixed buffer.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4] = self.version;
        buf[5] = self.flags;
        buf[6..8].copy_from_slice(&self.reserved.to_le_bytes());
        buf[8..16].copy_from_slice(&self.sequence_id.to_le_bytes());
        buf[16..24].copy_from_slice(&self.trace_id.to_le_bytes());
        buf[24..28].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[28..32].copy_from_slice(&self.crc32.to_le_bytes());
        buf
    }

    /// Parses a 32-byte slice into a verified `FrameHeader`.
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Result<Self, ProtocolError> {
        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != PROTOCOL_MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        let version = bytes[4];
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let flags = bytes[5];
        let reserved = u16::from_le_bytes([bytes[6], bytes[7]]);
        let sequence_id = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11],
            bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        let trace_id = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19],
            bytes[20], bytes[21], bytes[22], bytes[23],
        ]);
        let payload_len = u32::from_le_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27],
        ]);
        let crc32 = u32::from_le_bytes([
            bytes[28], bytes[29], bytes[30], bytes[31],
        ]);

        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                len: payload_len,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        Ok(Self {
            magic,
            version,
            flags,
            reserved,
            sequence_id,
            trace_id,
            payload_len,
            crc32,
        })
    }
}

/// A complete protocol frame containing a 32-byte header and a verified payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Encodes a `GuestCommand` into a new `Frame`.
    pub fn new_command(
        sequence_id: u64,
        trace_id: u64,
        command: &GuestCommand,
    ) -> Result<Self, ProtocolError> {
        let payload = bincode::serialize(command)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?;
        let header = FrameHeader::new(sequence_id, trace_id, 0, &payload)?;
        Ok(Self { header, payload })
    }

    /// Encodes a `GuestResponse` into a new `Frame`.
    pub fn new_response(
        sequence_id: u64,
        trace_id: u64,
        response: &GuestResponse,
    ) -> Result<Self, ProtocolError> {
        let mut flags = 0;
        if matches!(response, GuestResponse::Error { .. }) {
            flags |= FLAG_ERROR;
        }

        let payload = bincode::serialize(response)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?;
        let header = FrameHeader::new(sequence_id, trace_id, flags, &payload)?;
        Ok(Self { header, payload })
    }

    /// Deserializes the payload into a `GuestCommand`.
    pub fn decode_command(&self) -> Result<GuestCommand, ProtocolError> {
        bincode::deserialize(&self.payload)
            .map_err(|e| ProtocolError::Deserialization(e.to_string()))
    }

    /// Deserializes the payload into a `GuestResponse`.
    pub fn decode_response(&self) -> Result<GuestResponse, ProtocolError> {
        bincode::deserialize(&self.payload)
            .map_err(|e| ProtocolError::Deserialization(e.to_string()))
    }

    /// Serializes the entire frame (header + payload) into a contiguous byte vector.
    pub fn encode_to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        bytes.extend_from_slice(&self.header.to_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Decodes a complete frame from a byte slice, verifying header magic and payload CRC32.
    pub fn decode_from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let (frame, consumed) = Self::decode_from_slice(bytes)?;
        if consumed != bytes.len() {
            // Unconsumed extra trailing bytes in fixed decode
            return Ok(frame);
        }
        Ok(frame)
    }

    /// Decodes a frame from a stream slice, returning the parsed frame and the count of bytes consumed.
    pub fn decode_from_slice(bytes: &[u8]) -> Result<(Self, usize), ProtocolError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ProtocolError::IncompleteFrame {
                needed: HEADER_SIZE,
                available: bytes.len(),
            });
        }

        let mut header_buf = [0u8; HEADER_SIZE];
        header_buf.copy_from_slice(&bytes[..HEADER_SIZE]);
        let header = FrameHeader::from_bytes(&header_buf)?;

        let total_frame_len = HEADER_SIZE + header.payload_len as usize;
        if bytes.len() < total_frame_len {
            return Err(ProtocolError::IncompleteFrame {
                needed: total_frame_len,
                available: bytes.len(),
            });
        }

        let payload = bytes[HEADER_SIZE..total_frame_len].to_vec();

        // Verify CRC-32 integrity
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let computed_crc = hasher.finalize();

        if computed_crc != header.crc32 {
            return Err(ProtocolError::CrcMismatch {
                expected: header.crc32,
                computed: computed_crc,
            });
        }

        Ok((Self { header, payload }, total_frame_len))
    }
}
