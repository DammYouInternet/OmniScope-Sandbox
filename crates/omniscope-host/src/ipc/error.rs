use omniscope_core::error::ProtocolError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("Underlying protocol framing error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("IPC channel disconnected unexpectedly")]
    Disconnected,

    #[error("Command request timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Internal channel send error: {0}")]
    ChannelSend(String),

    #[error("Guest agent returned an error: [{code}] {message}")]
    GuestError { code: u32, message: String },
}
