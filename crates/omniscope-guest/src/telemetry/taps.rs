use omniscope_core::events::{
    FileSystemEvent, NetworkEvent, ProcessEvent, RegistryEvent,
};
use omniscope_core::ring_buffer::RingBuffer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Decoupled worker tap capturing process lifecycle, threads, and dynamic module loads.
#[derive(Debug)]
pub struct ProcessTap {
    enabled: AtomicBool,
    buffer: Arc<RingBuffer<ProcessEvent>>,
}

impl ProcessTap {
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            buffer: Arc::new(RingBuffer::new(capacity)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn record(&self, event: ProcessEvent) {
        if self.is_enabled() {
            self.buffer.push_overwrite(event);
        }
    }

    pub fn query(
        &self,
        pid_filter: Option<u32>,
        string_filter: Option<&str>,
        limit: usize,
    ) -> Vec<ProcessEvent> {
        let needle = string_filter.map(|s| s.to_lowercase());
        self.buffer.get_slice(
            |e| {
                if let Some(target_pid) = pid_filter {
                    if e.pid != target_pid {
                        return false;
                    }
                }
                if let Some(ref text) = needle {
                    let matches_name = e.image_name.to_lowercase().contains(text);
                    let matches_cmd = e
                        .command_line
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(text))
                        .unwrap_or(false);
                    let matches_dll = e
                        .dll_path
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(text))
                        .unwrap_or(false);
                    if !matches_name && !matches_cmd && !matches_dll {
                        return false;
                    }
                }
                true
            },
            limit,
        )
    }

    pub fn total_captured(&self) -> u64 {
        self.buffer.total_pushed()
    }

    pub fn dropped_count(&self) -> u64 {
        self.buffer.dropped_count()
    }
}

/// Decoupled worker tap capturing file creations, writes, deletions, and renames.
#[derive(Debug)]
pub struct FileSystemTap {
    enabled: AtomicBool,
    buffer: Arc<RingBuffer<FileSystemEvent>>,
}

impl FileSystemTap {
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            buffer: Arc::new(RingBuffer::new(capacity)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn record(&self, event: FileSystemEvent) {
        if self.is_enabled() {
            self.buffer.push_overwrite(event);
        }
    }

    pub fn query(
        &self,
        pid_filter: Option<u32>,
        string_filter: Option<&str>,
        limit: usize,
    ) -> Vec<FileSystemEvent> {
        let needle = string_filter.map(|s| s.to_lowercase());
        self.buffer.get_slice(
            |e| {
                if let Some(target_pid) = pid_filter {
                    if e.pid != target_pid {
                        return false;
                    }
                }
                if let Some(ref text) = needle {
                    if !e.file_path.to_lowercase().contains(text) {
                        return false;
                    }
                }
                true
            },
            limit,
        )
    }

    pub fn total_captured(&self) -> u64 {
        self.buffer.total_pushed()
    }

    pub fn dropped_count(&self) -> u64 {
        self.buffer.dropped_count()
    }
}

/// Decoupled worker tap capturing Windows Registry key and value operations.
#[derive(Debug)]
pub struct RegistryTap {
    enabled: AtomicBool,
    buffer: Arc<RingBuffer<RegistryEvent>>,
}

impl RegistryTap {
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            buffer: Arc::new(RingBuffer::new(capacity)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn record(&self, event: RegistryEvent) {
        if self.is_enabled() {
            self.buffer.push_overwrite(event);
        }
    }

    pub fn query(
        &self,
        pid_filter: Option<u32>,
        string_filter: Option<&str>,
        limit: usize,
    ) -> Vec<RegistryEvent> {
        let needle = string_filter.map(|s| s.to_lowercase());
        self.buffer.get_slice(
            |e| {
                if let Some(target_pid) = pid_filter {
                    if e.pid != target_pid {
                        return false;
                    }
                }
                if let Some(ref text) = needle {
                    let matches_key = e.key_path.to_lowercase().contains(text);
                    let matches_val = e
                        .value_name
                        .as_ref()
                        .map(|v| v.to_lowercase().contains(text))
                        .unwrap_or(false);
                    if !matches_key && !matches_val {
                        return false;
                    }
                }
                true
            },
            limit,
        )
    }

    pub fn total_captured(&self) -> u64 {
        self.buffer.total_pushed()
    }

    pub fn dropped_count(&self) -> u64 {
        self.buffer.dropped_count()
    }
}

/// Decoupled worker tap capturing TCP/UDP connections and DNS requests.
#[derive(Debug)]
pub struct NetworkTap {
    enabled: AtomicBool,
    buffer: Arc<RingBuffer<NetworkEvent>>,
}

impl NetworkTap {
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            buffer: Arc::new(RingBuffer::new(capacity)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn record(&self, event: NetworkEvent) {
        if self.is_enabled() {
            self.buffer.push_overwrite(event);
        }
    }

    pub fn query(
        &self,
        pid_filter: Option<u32>,
        string_filter: Option<&str>,
        limit: usize,
    ) -> Vec<NetworkEvent> {
        let needle = string_filter.map(|s| s.to_lowercase());
        self.buffer.get_slice(
            |e| {
                if let Some(target_pid) = pid_filter {
                    if e.pid != target_pid {
                        return false;
                    }
                }
                if let Some(ref text) = needle {
                    let matches_local = e.local_addr.to_lowercase().contains(text);
                    let matches_remote = e.remote_addr.to_lowercase().contains(text);
                    let matches_domain = e
                        .domain_query
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(text))
                        .unwrap_or(false);
                    if !matches_local && !matches_remote && !matches_domain {
                        return false;
                    }
                }
                true
            },
            limit,
        )
    }

    pub fn total_captured(&self) -> u64 {
        self.buffer.total_pushed()
    }

    pub fn dropped_count(&self) -> u64 {
        self.buffer.dropped_count()
    }
}
