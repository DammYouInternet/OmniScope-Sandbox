use crate::events::{
    ButtonAction, ImageFormat, MemoryQueryType, MouseButton, TelemetryEvent, TelemetrySource,
    UiElementNode,
};
use serde::{Deserialize, Serialize};

/// Commands sent from the Host to the Guest Agent over the IPC channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuestCommand {
    /// Ping / keep-alive heartbeat.
    Ping { timestamp_ms: u64 },

    /// Launch a target executable inside the guest environment.
    LaunchProcess {
        executable_path: String,
        arguments: Vec<String>,
        working_dir: Option<String>,
        suspend_on_entry: bool,
    },

    /// Terminate an active process by PID.
    TerminateProcess { pid: u32 },

    /// Capture desktop visual frame buffer.
    CaptureScreen {
        format: ImageFormat,
        quality: u8, // 1 to 100
        crop_active_window: bool,
    },

    /// Extract the Windows UI Automation accessibility tree.
    GetAccessibilityTree {
        max_depth: u32,
        root_hwnd: Option<u64>,
    },

    /// Inject synthetic mouse movement with optional human-mimicking Bézier curve.
    InjectMouseMove {
        target_x: i32,
        target_y: i32,
        duration_ms: u32,
        use_bezier_curve: bool,
    },

    /// Inject synthetic mouse button clicks.
    InjectMouseButton {
        button: MouseButton,
        action: ButtonAction,
    },

    /// Inject synthetic keystrokes with human-like timing jitter.
    InjectKeyboard {
        text: String,
        press_enter: bool,
        inter_key_jitter_ms: (u32, u32), // e.g. (60, 150)
    },

    /// Configure active telemetry taps and target PID tracking.
    ConfigureTaps {
        target_pids: Vec<u32>,
        enable_process: bool,
        enable_filesystem: bool,
        enable_registry: bool,
        enable_network: bool,
    },

    /// Query an on-demand slice from a specific telemetry tap ring buffer.
    QueryTelemetrySlice {
        source: TelemetrySource,
        pid_filter: Option<u32>,
        string_filter: Option<String>,
        limit: u32,
    },

    /// Micro-level memory and handle introspection on a specific process.
    InspectProcessMemory {
        pid: u32,
        query: MemoryQueryType,
    },
}

/// Responses returned from the Guest Agent to the Host over the IPC channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuestResponse {
    /// Pong heartbeat response.
    Pong { timestamp_ms: u64 },

    /// Confirmation of process creation.
    ProcessLaunched { pid: u32, main_thread_id: u32 },

    /// Confirmation of process termination.
    ProcessTerminated { pid: u32, exit_code: i32 },

    /// Visual screen frame buffer payload.
    ScreenFrame {
        width: u32,
        height: u32,
        format: ImageFormat,
        data: Vec<u8>,
    },

    /// Accessibility tree hierarchy.
    AccessibilityTree { root: UiElementNode },

    /// Confirmation of synthetic input actuation.
    ActuationCompleted { success: bool },

    /// On-demand telemetry event slice.
    TelemetrySlice {
        source: TelemetrySource,
        events: Vec<TelemetryEvent>,
        total_captured: u64,
        dropped_events: u64,
    },

    /// Process introspection details (JSON-formatted summary).
    MemoryInspectionResult {
        pid: u32,
        query: MemoryQueryType,
        details: String,
    },

    /// Explicit error response.
    Error { code: u32, message: String },
}
