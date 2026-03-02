// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Platform Detection Plugin
//!
//! Detects:
//! - Bare metal vs virtualized vs container
//! - Secure boot status
//! - TPM availability
//!
//! Supports: Linux, macOS, Windows

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static mut DETECTED_PLATFORM: Option<String> = None;

#[derive(Debug, serde::Serialize)]
pub struct PlatformInfo {
    #[serde(rename = "platformType")]
    pub platform_type: String,
    #[serde(rename = "hypervisor")]
    pub hypervisor: Option<String>,
    #[serde(rename = "isContainer")]
    pub is_container: bool,
    #[serde(rename = "containerRuntime")]
    pub container_runtime: Option<String>,
    #[serde(rename = "secureBoot")]
    pub secure_boot: bool,
    #[serde(rename = "tpmPresent")]
    pub tpm_present: bool,
    #[serde(rename = "os")]
    pub os: String,
}

impl PlatformInfo {
    fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::detect_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::detect_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::detect_windows()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::detect_unknown()
        }
    }

    #[cfg(target_os = "linux")]
    fn detect_linux() -> Self {
        let (platform_type, hypervisor) = detect_virtualization_linux();
        let (is_container, container_runtime) = detect_container_linux();
        let secure_boot = detect_secure_boot_linux();
        let tpm_present = detect_tpm_linux();

        PlatformInfo {
            platform_type,
            hypervisor,
            is_container,
            container_runtime,
            secure_boot,
            tpm_present,
            os: "linux".to_string(),
        }
    }

    #[cfg(target_os = "macos")]
    fn detect_macos() -> Self {
        use std::process::Command;

        let (platform_type, hypervisor) = detect_virtualization_macos();
        let (is_container, container_runtime) = detect_container_macos();
        let secure_boot = detect_secure_boot_macos();
        let tpm_present = detect_tpm_macos();

        PlatformInfo {
            platform_type,
            hypervisor,
            is_container,
            container_runtime,
            secure_boot,
            tpm_present,
            os: "macos".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    fn detect_windows() -> Self {
        use std::process::Command;

        let (platform_type, hypervisor) = detect_virtualization_windows();
        let (is_container, container_runtime) = detect_container_windows();
        let secure_boot = detect_secure_boot_windows();
        let tpm_present = detect_tpm_windows();

        PlatformInfo {
            platform_type,
            hypervisor,
            is_container,
            container_runtime,
            secure_boot,
            tpm_present,
            os: "windows".to_string(),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn detect_unknown() -> Self {
        PlatformInfo {
            platform_type: "unknown".to_string(),
            hypervisor: None,
            is_container: false,
            container_runtime: None,
            secure_boot: false,
            tpm_present: false,
            os: "unknown".to_string(),
        }
    }
}

// =============================================================================
// Linux Detection
// =============================================================================

#[cfg(target_os = "linux")]
fn detect_virtualization_linux() -> (String, Option<String>) {
    use std::fs;

    if let Ok(content) = fs::read_to_string("/sys/class/dmi/id/product_name") {
        let product = content.trim().to_lowercase();

        let hypervisors = [
            ("vmware", "VMware"),
            ("kvm", "KVM"),
            ("qemu", "QEMU"),
            ("xen", "Xen"),
            ("hyper-v", "Hyper-V"),
            ("virtualbox", "VirtualBox"),
            ("parallels", "Parallels"),
            ("google", "Google Cloud"),
            ("amazon", "AWS"),
            ("microsoft corporation", "Azure"),
        ];

        for (pattern, name) in hypervisors.iter() {
            if product.contains(pattern) {
                return ("virtualized".to_string(), Some(name.to_string()));
            }
        }
    }

    if fs::read_to_string("/proc/xen/xsd_port").is_ok() {
        return ("virtualized".to_string(), Some("Xen".to_string()));
    }

    ("bare-metal".to_string(), None)
}

#[cfg(target_os = "linux")]
fn detect_container_linux() -> (bool, Option<String>) {
    use std::fs;

    if fs::read_to_string("/proc/1/cgroup")
        .map(|c| c.contains("docker"))
        .unwrap_or(false)
    {
        return (true, Some("docker".to_string()));
    }

    if fs::read_to_string("/proc/1/cgroup")
        .map(|c| c.contains("containerd"))
        .unwrap_or(false)
    {
        return (true, Some("containerd".to_string()));
    }

    if fs::read_to_string("/proc/1/cgroup")
        .map(|c| c.contains("podman"))
        .unwrap_or(false)
    {
        return (true, Some("podman".to_string()));
    }

    if fs::metadata("/.dockerenv").is_ok() {
        return (true, Some("docker".to_string()));
    }

    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return (true, Some("kubernetes".to_string()));
    }

    (false, None)
}

#[cfg(target_os = "linux")]
fn detect_secure_boot_linux() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("bootctl").arg("status").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Secure Boot: enabled") {
            return true;
        }
    }

    false
}

#[cfg(target_os = "linux")]
fn detect_tpm_linux() -> bool {
    use std::fs;
    fs::read_dir("/dev/tpm").is_ok()
        || fs::read_dir("/dev/tpm0").is_ok()
        || fs::read_dir("/dev/tpmrm0").is_ok()
}

// =============================================================================
// macOS Detection
// =============================================================================

#[cfg(target_os = "macos")]
fn detect_virtualization_macos() -> (String, Option<String>) {
    use std::process::Command;

    if let Ok(output) = Command::new("sysctl")
        .arg("machdep.cpu.brand_string")
        .output()
    {
        let output = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if output.contains("vmware") {
            return ("virtualized".to_string(), Some("VMware".to_string()));
        }
        if output.contains("virtualbox") {
            return ("virtualized".to_string(), Some("VirtualBox".to_string()));
        }
        if output.contains("parallels") {
            return ("virtualized".to_string(), Some("Parallels".to_string()));
        }
    }

    if std::path::Path::new("/System/Library/Frameworks/Virtualization.framework").exists() {
        return (
            "virtualized".to_string(),
            Some("Apple Virtualization".to_string()),
        );
    }

    ("bare-metal".to_string(), None)
}

#[cfg(target_os = "macos")]
fn detect_container_macos() -> (bool, Option<String>) {
    use std::process::Command;

    if Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return (true, Some("docker".to_string()));
    }

    if Command::new("podman")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return (true, Some("podman".to_string()));
    }

    (false, None)
}

#[cfg(target_os = "macos")]
fn detect_secure_boot_macos() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("csrutil").arg("status").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.contains("enabled");
    }

    false
}

#[cfg(target_os = "macos")]
fn detect_tpm_macos() -> bool {
    std::path::Path::new("/System/Library/PrivateFrameworks/SealedDisk.enforcement").exists()
}

// =============================================================================
// Windows Detection
// =============================================================================

#[cfg(target_os = "windows")]
fn detect_virtualization_windows() -> (String, Option<String>) {
    use std::process::Command;

    if let Ok(output) = Command::new("systeminfo").output() {
        let output = String::from_utf8_lossy(&output.stdout).to_lowercase();

        if output.contains("vmware") {
            return ("virtualized".to_string(), Some("VMware".to_string()));
        }
        if output.contains("virtualbox") {
            return ("virtualized".to_string(), Some("VirtualBox".to_string()));
        }
        if output.contains("hyper-v") || output.contains("microsoft corporation") {
            return ("virtualized".to_string(), Some("Hyper-V".to_string()));
        }
        if output.contains("kvm") || output.contains("qemu") {
            return ("virtualized".to_string(), Some("QEMU/KVM".to_string()));
        }
        if output.contains("amazon") {
            return ("virtualized".to_string(), Some("AWS".to_string()));
        }
        if output.contains("google") {
            return ("virtualized".to_string(), Some("Google Cloud".to_string()));
        }
    }

    ("bare-metal".to_string(), None)
}

#[cfg(target_os = "windows")]
fn detect_container_windows() -> (bool, Option<String>) {
    use std::process::Command;

    if Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return (true, Some("docker".to_string()));
    }

    if std::env::var("COMPUTERNAME")
        .map(|c| c.ends_with("-"))
        .unwrap_or(false)
    {
        return (true, Some("windows-container".to_string()));
    }

    (false, None)
}

#[cfg(target_os = "windows")]
fn detect_secure_boot_windows() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("bcdedit").arg("/enum").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("secureboot") && stdout.contains("enabled") {
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn detect_tpm_windows() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("powershell")
        .args(["-Command", "Get-Tpm"])
        .output()
    {
        let output = String::from_utf8_lossy(&output.stdout);
        return output.contains("TpmPresent") && output.contains("True");
    }

    false
}

// =============================================================================
// Plugin Entry Points
// =============================================================================

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let platform = PlatformInfo::detect();
    let json = serde_json::to_string(&platform).unwrap_or_default();

    unsafe {
        DETECTED_PLATFORM = Some(json);
    }

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    std::ptr::null()
}
