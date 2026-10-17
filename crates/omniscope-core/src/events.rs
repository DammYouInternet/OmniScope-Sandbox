use serde::{Deserialize, Serialize};

/// Identifies the subsystem generating telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetrySource {
    Process,
    FileSystem,
    Registry,
    Network,
    Ui,
}

/// Lifecycle events for guest processes and threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessEventType {
    Start,
    Exit,
    ThreadCreate,
    LoadDll,
}

/// File system operational primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOp {
    Create,
    Read,
    Write,
    Delete,
    Rename,
}

/// Windows Registry modification primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryOp {
    OpenKey,
    CreateKey,
    SetValue,
    DeleteValue,
}

/// Supported transport and application network protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetProtocol {
    Tcp,
    Udp,
    Dns,
}

/// Supported image formats for perception frame buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    WebP,
    Png,
    Jpeg,
}

/// Synthetic mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Synthetic mouse click actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonAction {
    Down,
    Up,
    Click,
    DoubleClick,
}

/// Micro-level memory introspection targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryQueryType {
    Strings,
    LoadedModules,
    OpenHandles,
}

/// Telemetry event for process lifecycle, threads, and dynamic module loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEvent {
    pub timestamp_ns: u64,
    pub event_type: ProcessEventType,
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub image_name: String,
    pub command_line: Option<String>,
    pub dll_path: Option<String>,
}

/// Telemetry event for file system reads, writes, and deletions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemEvent {
    pub timestamp_ns: u64,
    pub pid: u32,
    pub operation: FileOp,
    pub file_path: String,
    pub bytes_transferred: Option<u64>,
}

/// Telemetry event for registry operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEvent {
    pub timestamp_ns: u64,
    pub pid: u32,
    pub operation: RegistryOp,
    pub key_path: String,
    pub value_name: Option<String>,
    pub data_preview: Option<String>,
}

/// Telemetry event for TCP/UDP connections and DNS lookups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub timestamp_ns: u64,
    pub pid: u32,
    pub protocol: NetProtocol,
    pub local_addr: String,
    pub remote_addr: String,
    pub domain_query: Option<String>,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

/// Hierarchical UI Automation node describing accessible controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiElementNode {
    pub id: String,
    pub control_type: String, // e.g. "Button", "Edit", "Window", "ComboBox"
    pub name: String,
    pub bounds: [i32; 4],     // [left, top, width, height]
    pub is_enabled: bool,
    pub is_focused: bool,
    pub children: Vec<UiElementNode>,
}

/// Top-level polymorphic telemetry container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryEvent {
    Process(ProcessEvent),
    FileSystem(FileSystemEvent),
    Registry(RegistryEvent),
    Network(NetworkEvent),
}

impl TelemetryEvent {
    /// Returns the source category of the telemetry event.
    pub fn source(&self) -> TelemetrySource {
        match self {
            TelemetryEvent::Process(_) => TelemetrySource::Process,
            TelemetryEvent::FileSystem(_) => TelemetrySource::FileSystem,
            TelemetryEvent::Registry(_) => TelemetrySource::Registry,
            TelemetryEvent::Network(_) => TelemetrySource::Network,
        }
    }

    /// Returns the associated PID of the event.
    pub fn pid(&self) -> u32 {
        match self {
            TelemetryEvent::Process(e) => e.pid,
            TelemetryEvent::FileSystem(e) => e.pid,
            TelemetryEvent::Registry(e) => e.pid,
            TelemetryEvent::Network(e) => e.pid,
        }
    }

    /// Returns the timestamp in nanoseconds of the event.
    pub fn timestamp_ns(&self) -> u64 {
        match self {
            TelemetryEvent::Process(e) => e.timestamp_ns,
            TelemetryEvent::FileSystem(e) => e.timestamp_ns,
            TelemetryEvent::Registry(e) => e.timestamp_ns,
            TelemetryEvent::Network(e) => e.timestamp_ns,
        }
    }
}
