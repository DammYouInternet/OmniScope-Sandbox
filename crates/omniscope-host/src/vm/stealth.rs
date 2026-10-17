use crate::vm::config::VmConfig;

/// Builds the comprehensive command-line argument list for launching QEMU
/// with hardware-assisted acceleration, anti-analysis stealth cloaking,
/// QMP control socket, and the OmniScope communication channel.
pub fn build_qemu_args(config: &VmConfig) -> Vec<String> {
    let mut args = Vec::new();

    // 1. Machine architecture and accelerator
    args.push("-machine".to_string());
    args.push("q35".to_string());

    if config.whpx_acceleration {
        args.push("-accel".to_string());
        args.push("whpx,kernel-irqchip=off".to_string());
    }

    // 2. Memory and CPU topology
    args.push("-m".to_string());
    args.push(format!("{}M", config.memory_mb));

    args.push("-smp".to_string());
    args.push(format!("cpus={}", config.cpu_cores));

    // 3. CPU Cloaking & Hypervisor Artifact Masking
    let mut cpu_features = config.stealth.cpu_model.clone();
    if config.stealth.mask_hypervisor_bit {
        cpu_features.push_str(",hypervisor=off");
    }
    if config.stealth.disable_kvm_brand {
        cpu_features.push_str(",kvm=off");
    }
    args.push("-cpu".to_string());
    args.push(cpu_features);

    // 4. SMBIOS Cloaking (OEM Hardware Identifiers)
    let smbios_arg = format!(
        "type=1,manufacturer={},product={},serial={}",
        config.stealth.smbios_manufacturer,
        config.stealth.smbios_product,
        config.stealth.smbios_serial
    );
    args.push("-smbios".to_string());
    args.push(smbios_arg);

    // 5. Storage with Cloaked Disk Serial
    let overlay_str = config.overlay_image_path.to_string_lossy().to_string();
    args.push("-drive".to_string());
    args.push(format!("file={},if=none,id=system_drive,format=qcow2", overlay_str));

    args.push("-device".to_string());
    args.push(format!(
        "nvme,serial={},drive=system_drive",
        config.stealth.disk_serial
    ));

    // 6. Network with Cloaked OEM MAC Address
    args.push("-netdev".to_string());
    args.push("user,id=net0".to_string());

    args.push("-device".to_string());
    args.push(format!(
        "e1000,netdev=net0,mac={}",
        config.stealth.oem_mac_address
    ));

    // 7. QMP (QEMU Machine Protocol) Control Port
    args.push("-qmp".to_string());
    args.push(format!("tcp:127.0.0.1:{},server,nowait", config.qmp_port));

    // 8. Guest-Host Communication Channel (Virtio Serial Socket)
    args.push("-device".to_string());
    args.push("virtio-serial".to_string());

    args.push("-chardev".to_string());
    args.push(format!(
        "socket,id=omniscope_ch,host=127.0.0.1,port={},server,nowait",
        config.vsock_port
    ));

    args.push("-device".to_string());
    args.push("virtserialport,chardev=omniscope_ch,name=omniscope.guest.0".to_string());

    // 9. Display
    if config.headless {
        args.push("-display".to_string());
        args.push("none".to_string());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::config::{StealthConfig, VmConfig};
    use std::path::PathBuf;

    #[test]
    fn test_build_qemu_args_stealth_and_topology() {
        let mut config = VmConfig::default();
        config.memory_mb = 4096;
        config.cpu_cores = 2;
        config.overlay_image_path = PathBuf::from("test_overlay.qcow2");
        config.stealth = StealthConfig {
            cpu_model: "host".to_string(),
            mask_hypervisor_bit: true,
            disable_kvm_brand: true,
            smbios_manufacturer: "Dell Inc.".to_string(),
            smbios_product: "OptiPlex 7090".to_string(),
            smbios_serial: "DELL-SERIAL-9988".to_string(),
            disk_serial: "SAMSUNG-MZVLB1T0HBLR-9988".to_string(),
            oem_mac_address: "00:14:22:01:23:45".to_string(),
        };
        config.qmp_port = 5555;
        config.vsock_port = 6666;
        config.headless = true;

        let args = build_qemu_args(&config);

        // Assert memory & topology
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"4096M".to_string()));
        assert!(args.contains(&"-smp".to_string()));
        assert!(args.contains(&"cpus=2".to_string()));

        // Assert CPU flags
        assert!(args.contains(&"-cpu".to_string()));
        let cpu_idx = args.iter().position(|a| a == "-cpu").unwrap();
        assert_eq!(args[cpu_idx + 1], "host,hypervisor=off,kvm=off");

        // Assert SMBIOS
        assert!(args.contains(&"-smbios".to_string()));
        let smbios_idx = args.iter().position(|a| a == "-smbios").unwrap();
        assert_eq!(
            args[smbios_idx + 1],
            "type=1,manufacturer=Dell Inc.,product=OptiPlex 7090,serial=DELL-SERIAL-9988"
        );

        // Assert Disk serial
        let drive_dev_idx = args.iter().position(|a| a.starts_with("nvme,serial=")).unwrap();
        assert_eq!(
            args[drive_dev_idx],
            "nvme,serial=SAMSUNG-MZVLB1T0HBLR-9988,drive=system_drive"
        );

        // Assert OEM MAC
        let net_dev_idx = args.iter().position(|a| a.starts_with("e1000,netdev=net0,mac=")).unwrap();
        assert_eq!(args[net_dev_idx], "e1000,netdev=net0,mac=00:14:22:01:23:45");

        // Assert QMP and guest channel
        assert!(args.contains(&"tcp:127.0.0.1:5555,server,nowait".to_string()));
        assert!(args.contains(&"socket,id=omniscope_ch,host=127.0.0.1,port=6666,server,nowait".to_string()));

        // Assert headless
        assert!(args.contains(&"-display".to_string()));
        assert!(args.contains(&"none".to_string()));
    }
}
