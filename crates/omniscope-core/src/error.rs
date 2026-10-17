use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Invalid protocol magic: expected [0x4F, 0x4D, 0x4E, 0x49] ('OMNI'), found {0:?}")]
    InvalidMagic([u8; 4]),

    #[error("Unsupported protocol version: expected 1, found {0}")]
    UnsupportedVersion(u8),

    #[error("CRC-32 checksum mismatch: expected {expected:#010X}, computed {computed:#010X}")]
    CrcMismatch { expected: u32, computed: u32 },

    #[error("Payload length exceeds maximum allowable size: length {len} bytes, max {max} bytes")]
    PayloadTooLarge { len: u32, max: u32 },

    #[error("Incomplete frame buffer: needed at least {needed} bytes, but only {available} bytes available")]
    IncompleteFrame { needed: usize, available: usize },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("I/O error during protocol operation: {0}")]
    Io(#[from] std::io::Error),
}
