use crate::telemetry::noise_filter::NoiseFilter;
use crate::telemetry::taps::{FileSystemTap, NetworkTap, ProcessTap, RegistryTap};
use omniscope_core::events::{TelemetryEvent, TelemetrySource};
use std::collections::HashSet;
use std::sync::RwLock;

/// Central coordinator managing decoupled telemetry workers, noise suppression, and target PID filters.
#[derive(Debug)]
pub struct TapManager {
    pub process_tap: ProcessTap,
    pub fs_tap: FileSystemTap,
    pub reg_tap: RegistryTap,
    pub net_tap: NetworkTap,
    pub noise_filter: NoiseFilter,
    target_pids: RwLock<HashSet<u32>>,
}

impl Default for TapManager {
    fn default() -> Self {
        Self::new(1000, 2000, 2000, 1000)
    }
}

impl TapManager {
    /// Creates a new `TapManager` with specified ring buffer capacities.
    pub fn new(proc_cap: usize, fs_cap: usize, reg_cap: usize, net_cap: usize) -> Self {
        Self {
            process_tap: ProcessTap::new(proc_cap),
            fs_tap: FileSystemTap::new(fs_cap),
            reg_tap: RegistryTap::new(reg_cap),
            net_tap: NetworkTap::new(net_cap),
            noise_filter: NoiseFilter::new(),
            target_pids: RwLock::new(HashSet::new()),
        }
    }

    /// Dynamically configures active taps and target PID tracking.
    pub fn configure(
        &self,
        target_pids: Vec<u32>,
        enable_process: bool,
        enable_filesystem: bool,
        enable_registry: bool,
        enable_network: bool,
    ) {
        self.process_tap.set_enabled(enable_process);
        self.fs_tap.set_enabled(enable_filesystem);
        self.reg_tap.set_enabled(enable_registry);
        self.net_tap.set_enabled(enable_network);

        let mut guard = self.target_pids.write().unwrap();
        guard.clear();
        for pid in target_pids {
            guard.insert(pid);
        }
    }

    /// Adds a newly spawned child PID to the target tracking set.
    pub fn add_target_pid(&self, pid: u32) {
        self.target_pids.write().unwrap().insert(pid);
    }

    /// Records an arbitrary telemetry event, applying noise suppression and target PID checks.
    pub fn record_event(&self, event: TelemetryEvent) {
        // Skip background operating system noise
        if self.noise_filter.is_noise(&event) {
            return;
        }

        // Auto-adopt child processes if parent is currently tracked
        if let TelemetryEvent::Process(ref e) = event {
            if let Some(parent) = e.parent_pid {
                let targets = self.target_pids.read().unwrap();
                if targets.contains(&parent) {
                    drop(targets);
                    self.add_target_pid(e.pid);
                }
            }
        }

        // If specific target PIDs are configured, filter out unmonitored processes
        let targets = self.target_pids.read().unwrap();
        if !targets.is_empty() && !targets.contains(&event.pid()) {
            return;
        }
        drop(targets);

        match event {
            TelemetryEvent::Process(e) => self.process_tap.record(e),
            TelemetryEvent::FileSystem(e) => self.fs_tap.record(e),
            TelemetryEvent::Registry(e) => self.reg_tap.record(e),
            TelemetryEvent::Network(e) => self.net_tap.record(e),
        }
    }

    /// Queries an on-demand slice from a specific telemetry subsystem.
    ///
    /// Returns a tuple of `(events, total_captured, dropped_events)`.
    pub fn query_slice(
        &self,
        source: TelemetrySource,
        pid_filter: Option<u32>,
        string_filter: Option<&str>,
        limit: usize,
    ) -> (Vec<TelemetryEvent>, u64, u64) {
        match source {
            TelemetrySource::Process => {
                let events: Vec<TelemetryEvent> = self
                    .process_tap
                    .query(pid_filter, string_filter, limit)
                    .into_iter()
                    .map(TelemetryEvent::Process)
                    .collect();
                (
                    events,
                    self.process_tap.total_captured(),
                    self.process_tap.dropped_count(),
                )
            }
            TelemetrySource::FileSystem => {
                let events: Vec<TelemetryEvent> = self
                    .fs_tap
                    .query(pid_filter, string_filter, limit)
                    .into_iter()
                    .map(TelemetryEvent::FileSystem)
                    .collect();
                (
                    events,
                    self.fs_tap.total_captured(),
                    self.fs_tap.dropped_count(),
                )
            }
            TelemetrySource::Registry => {
                let events: Vec<TelemetryEvent> = self
                    .reg_tap
                    .query(pid_filter, string_filter, limit)
                    .into_iter()
                    .map(TelemetryEvent::Registry)
                    .collect();
                (
                    events,
                    self.reg_tap.total_captured(),
                    self.reg_tap.dropped_count(),
                )
            }
            TelemetrySource::Network => {
                let events: Vec<TelemetryEvent> = self
                    .net_tap
                    .query(pid_filter, string_filter, limit)
                    .into_iter()
                    .map(TelemetryEvent::Network)
                    .collect();
                (
                    events,
                    self.net_tap.total_captured(),
                    self.net_tap.dropped_count(),
                )
            }
            TelemetrySource::Ui => (Vec::new(), 0, 0),
        }
    }
}
