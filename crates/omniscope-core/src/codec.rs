use crate::error::ProtocolError;
use crate::protocol::{Frame, FrameHeader, HEADER_SIZE};
use bytes::{Buf, BufMut, BytesMut};
use crc32fast::Hasher;
use tokio_util::codec::{Decoder, Encoder};

/// Asynchronous stream codec for framing and serializing OmniScope binary protocol packets.
#[derive(Debug, Default, Clone)]
pub struct ProtocolCodec;

impl ProtocolCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Decoder for ProtocolCodec {
    type Item = Frame;
    type Error = ProtocolError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        let mut header_buf = [0u8; HEADER_SIZE];
        header_buf.copy_from_slice(&src[..HEADER_SIZE]);
        let header = FrameHeader::from_bytes(&header_buf)?;

        let total_frame_len = HEADER_SIZE + header.payload_len as usize;
        if src.len() < total_frame_len {
            // Buffer does not yet contain the complete frame payload
            src.reserve(total_frame_len - src.len());
            return Ok(None);
        }

        // Consume header
        src.advance(HEADER_SIZE);
        // Consume payload
        let payload = src.split_to(header.payload_len as usize).to_vec();

        // Verify CRC-32 checksum integrity
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let computed = hasher.finalize();

        if computed != header.crc32 {
            return Err(ProtocolError::CrcMismatch {
                expected: header.crc32,
                computed,
            });
        }

        Ok(Some(Frame { header, payload }))
    }
}

impl Encoder<Frame> for ProtocolCodec {
    type Error = ProtocolError;

    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(HEADER_SIZE + item.payload.len());
        dst.put_slice(&item.header.to_bytes());
        dst.put_slice(&item.payload);
        Ok(())
    }
}
