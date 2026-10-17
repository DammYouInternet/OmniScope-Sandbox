use thiserror::Error;

#[derive(Error, Debug)]
pub enum ActuationError {
    #[error("Target coordinates ({x}, {y}) out of screen bounds: max ({max_x}, {max_y})")]
    OutOfBounds {
        x: i32,
        y: i32,
        max_x: i32,
        max_y: i32,
    },

    #[error("Failed to inject input event via Windows SendInput: error code {0}")]
    SendInputFailed(u32),

    #[error("Target control or window is occluded or not interactive: {0}")]
    TargetOccluded(String),

    #[error("Actuation execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Internal driver error: {0}")]
    Driver(String),
}
