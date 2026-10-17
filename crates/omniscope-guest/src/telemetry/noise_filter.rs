use omniscope_core::events::TelemetryEvent;
use std::collections::HashSet;

/// Configurable rule engine for suppressing high-volume background OS noise.
#[derive(Debug, Clone)]
pub struct NoiseFilter {
    noise_processes: HashSet<String>,
    noise_path_prefixes: Vec<String>,
    noise_registry_prefixes: Vec<String>,
}

impl Default for NoiseFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl NoiseFilter {
    /// Creates a new `NoiseFilter` with standard Windows OS background daemon suppression rules.
    pub fn new() -> Self {
        let mut noise_processes = HashSet::new();
        for proc in &[
            "dwm.exe",
            "explorer.exe",
            "searchapp.exe",
            "searchhost.exe",
            "shellexperiencehost.exe",
            "startmenuexperiencehost.exe",
            "svchost.exe",
            "runtimebroker.exe",
            "sihost.exe",
            "fontdrvhost.exe",
            "audiodg.exe",
            "taskhostw.exe",
            "system",
            "msmpeng.exe", // Windows Defender
        ] {
            noise_processes.insert(proc.to_lowercase());
        }

        let noise_path_prefixes = vec![
            "c:\\windows\\fonts\\".to_string(),
            "c:\\windows\\winsxs\\".to_string(),
            "c:\\windows\\system32\\catroot".to_string(),
            "c:\\windows\\system32\\wbem".to_string(),
            "c:\\windows\\assembly\\".to_string(),
        ];

        let noise_registry_prefixes = vec![
            "hkcu\\software\\microsoft\\windows\\currentversion\\explorer\\sessioninfo".to_string(),
            "hkcu\\software\\classes\\locall\\".to_string(),
            "hklm\\system\\currentcontrolset\\services\\bam".to_string(),
        ];

        Self {
            noise_processes,
            noise_path_prefixes,
            noise_registry_prefixes,
        }
    }

    /// Returns `true` if the given image name is identified as an operating system background daemon.
    pub fn is_noise_process(&self, image_name: &str) -> bool {
        let clean = image_name
            .rsplit('\\')
            .next()
            .unwrap_or(image_name)
            .to_lowercase();
        self.noise_processes.contains(&clean)
    }

    /// Returns `true` if the given file system path is an internal OS system path.
    pub fn is_noise_path(&self, file_path: &str) -> bool {
        let lower = file_path.to_lowercase();
        self.noise_path_prefixes
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    }

    /// Returns `true` if the registry key is an OS internal heartbeat/cache key.
    pub fn is_noise_registry(&self, key_path: &str) -> bool {
        let lower = key_path.to_lowercase();
        self.noise_registry_prefixes
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    }

    /// Evaluates whether an event should be filtered out as background noise.
    pub fn is_noise(&self, event: &TelemetryEvent) -> bool {
        match event {
            TelemetryEvent::Process(e) => self.is_noise_process(&e.image_name),
            TelemetryEvent::FileSystem(e) => self.is_noise_path(&e.file_path),
            TelemetryEvent::Registry(e) => self.is_noise_registry(&e.key_path),
            TelemetryEvent::Network(_) => false, // Network traffic is rarely filtered unless explicitly blocked
        }
    }
}
