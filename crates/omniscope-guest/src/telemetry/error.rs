use thiserror::Error;

#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("Failed to initialize telemetry collector for {subsystem}: {reason}")]
    InitFailed {
        subsystem: &'static str,
        reason: String,
    },

    #[error("Telemetry session start failed: {0}")]
    StartFailed(String),

    #[error("Telemetry worker stopped unexpectedly: {0}")]
    WorkerDied(String),

    #[error("ETW kernel session error: code {0}")]
    EtwError(u32),

    #[error("Buffer overflow in telemetry tap {0}")]
    BufferOverflow(String),
}
