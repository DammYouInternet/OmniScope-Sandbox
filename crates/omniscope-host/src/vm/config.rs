use std::path::PathBuf;

/// Anti-sandbox stealth profile configuration for cloaking virtualization artifacts.
#[derive(Debug, Clone)]
pub struct StealthConfig {
    pub cpu_model: String,
    pub mask_hypervisor_bit: bool,
    pub disable_kvm_brand: bool,
    pub smbios_manufacturer: String,
    pub smbios_product: String,
    pub smbios_serial: String,
    pub disk_serial: String,
    pub oem_mac_address: String,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            cpu_model: "host".to_string(),
            mask_hypervisor_bit: true,
            disable_kvm_brand: true,
            smbios_manufacturer: "Dell Inc.".to_string(),
            smbios_product: "OptiPlex 7090".to_string(),
            smbios_serial: "7X89B22".to_string(),
            disk_serial: "SAMSUNG-MZVLB1T0HBLR-0000".to_string(),
            oem_mac_address: "00:1A:A0:5B:3F:82".to_string(), // Authentic Dell OUI
        }
    }
}

/// VM hypervisor execution and resource allocation configuration.
#[derive(Debug, Clone)]
pub struct VmConfig {
    pub qemu_executable: PathBuf,
    pub base_image_path: PathBuf,
    pub overlay_image_path: PathBuf,
    pub memory_mb: u32,
    pub cpu_cores: u32,
    pub whpx_acceleration: bool,
    pub qmp_port: u16,
    pub vsock_port: u16,
    pub stealth: StealthConfig,
    pub headless: bool,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            qemu_executable: PathBuf::from("qemu-system-x86_64"),
            base_image_path: PathBuf::from("images/golden_windows.qcow2"),
            overlay_image_path: PathBuf::from("images/overlay.qcow2"),
            memory_mb: 8192,
            cpu_cores: 4,
            whpx_acceleration: true,
            qmp_port: 4444,
            vsock_port: 5000,
            stealth: StealthConfig::default(),
            headless: true,
        }
    }
}
