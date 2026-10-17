use thiserror::Error;

#[derive(Error, Debug)]
pub enum PerceptionError {
    #[error("Failed to capture screen: {0}")]
    CaptureFailed(String),

    #[error("GDI graphic subsystem error: {0}")]
    GdiError(String),

    #[error("UI accessibility tree traversal failed: {0}")]
    UiTreeError(String),

    #[error("Target window with HWND {0:#X} was not found or is closed")]
    WindowNotFound(u64),

    #[error("Accessibility tree recursion depth exceeded limit of {0}")]
    MaxDepthExceeded(u32),

    #[error("Frame encoding to target format failed: {0}")]
    EncodingError(String),
}
