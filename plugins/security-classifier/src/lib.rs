// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Security Classification Plugin
//!
//! Classifies device security level based on platform detection:
//! - Trusted: Bare metal + Secure Boot + TPM
//! - High: Bare metal OR Virtualized + Secure Boot + TPM
//! - Moderate: Virtualized + No TPM, or Container + Secure Boot
//! - Low: Container + No TPM, or No Secure Boot + No TPM

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static mut CLASSIFICATION: Option<String> = None;

#[derive(Debug, serde::Serialize, Clone)]
pub struct SecurityClassification {
    pub level: String,
    pub score: u8,
    pub factors: Vec<SecurityFactor>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct SecurityFactor {
    pub name: String,
    pub value: String,
    pub impact: String,
}

impl SecurityClassification {
    fn classify(platform: &PlatformInput) -> Self {
        let mut score: u8 = 0;
        let mut factors = Vec::new();
        let mut recommendations = Vec::new();

        // Platform type scoring
        match platform.platform_type.as_str() {
            "bare-metal" => {
                score += 40;
                factors.push(SecurityFactor {
                    name: "platform_type".to_string(),
                    value: "bare-metal".to_string(),
                    impact: "positive".to_string(),
                });
            }
            "virtualized" => {
                score += 20;
                factors.push(SecurityFactor {
                    name: "platform_type".to_string(),
                    value: "virtualized".to_string(),
                    impact: "neutral".to_string(),
                });
                recommendations.push("Consider bare-metal for sensitive workloads".to_string());
            }
            _ => {
                score += 10;
                factors.push(SecurityFactor {
                    name: "platform_type".to_string(),
                    value: "unknown".to_string(),
                    impact: "negative".to_string(),
                });
            }
        }

        // Container scoring
        if platform.is_container {
            score = score.saturating_sub(20);
            factors.push(SecurityFactor {
                name: "container".to_string(),
                value: platform.container_runtime.clone().unwrap_or_default(),
                impact: "negative".to_string(),
            });
            recommendations
                .push("Container isolation - ensure proper network sandboxing".to_string());
        }

        // Hypervisor scoring
        if let Some(hypervisor) = &platform.hypervisor {
            score += 10;
            factors.push(SecurityFactor {
                name: "hypervisor".to_string(),
                value: hypervisor.clone(),
                impact: "neutral".to_string(),
            });
        }

        // Secure boot scoring
        if platform.secure_boot {
            score += 30;
            factors.push(SecurityFactor {
                name: "secure_boot".to_string(),
                value: "enabled".to_string(),
                impact: "positive".to_string(),
            });
        } else {
            factors.push(SecurityFactor {
                name: "secure_boot".to_string(),
                value: "disabled".to_string(),
                impact: "negative".to_string(),
            });
            recommendations.push("Enable Secure Boot in UEFI/BIOS for boot integrity".to_string());
        }

        // TPM scoring
        if platform.tpm_present {
            score += 20;
            factors.push(SecurityFactor {
                name: "tpm".to_string(),
                value: "present".to_string(),
                impact: "positive".to_string(),
            });
        } else {
            factors.push(SecurityFactor {
                name: "tpm".to_string(),
                value: "not present".to_string(),
                impact: "negative".to_string(),
            });
            recommendations
                .push("Consider adding TPM 2.0 for hardware-backed key storage".to_string());
        }

        // Determine level
        let level = match score {
            80..=100 => "trusted".to_string(),
            60..=79 => "high".to_string(),
            40..=59 => "moderate".to_string(),
            20..=39 => "low".to_string(),
            _ => "minimal".to_string(),
        };

        // Add specific recommendations based on score
        if score < 60 {
            recommendations.push("Review and strengthen security configuration".to_string());
        }

        if platform.is_container && !platform.secure_boot {
            recommendations
                .push("Container without secure boot - consider rootless containers".to_string());
        }

        if platform.platform_type == "virtualized" && !platform.tpm_present {
            recommendations
                .push("Virtualized without TPM - ensure hypervisor security".to_string());
        }

        SecurityClassification {
            level,
            score,
            factors,
            recommendations,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct PlatformInput {
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
}

impl Default for PlatformInput {
    fn default() -> Self {
        PlatformInput {
            platform_type: "unknown".to_string(),
            hypervisor: None,
            is_container: false,
            container_runtime: None,
            secure_boot: false,
            tpm_present: false,
        }
    }
}

#[cfg(target_os = "linux")]
fn get_platform_info() -> PlatformInput {
    use std::fs;

    let mut platform = PlatformInput::default();

    // Detect platform type
    if let Ok(content) = fs::read_to_string("/sys/class/dmi/id/product_name") {
        let product = content.trim().to_lowercase();
        if product.contains("vmware")
            || product.contains("kvm")
            || product.contains("qemu")
            || product.contains("xen")
            || product.contains("hyper")
            || product.contains("virtualbox")
        {
            platform.platform_type = "virtualized".to_string();
            platform.hypervisor = Some(product);
        } else {
            platform.platform_type = "bare-metal".to_string();
        }
    }

    // Detect container
    if fs::read_to_string("/proc/1/cgroup")
        .map(|c| c.contains("docker") || c.contains("containerd") || c.contains("podman"))
        .unwrap_or(false)
        || fs::metadata("/.dockerenv").is_ok()
    {
        platform.is_container = true;
        platform.container_runtime = Some("docker".to_string());
    }

    // Detect secure boot (simplified)
    platform.secure_boot = std::process::Command::new("bootctl")
        .arg("status")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Secure Boot: enabled"))
        .unwrap_or(false);

    // Detect TPM
    platform.tpm_present = fs::read_dir("/dev/tpm").is_ok()
        || fs::read_dir("/dev/tpm0").is_ok()
        || fs::read_dir("/dev/tpmrm0").is_ok();

    platform
}

#[cfg(not(target_os = "linux"))]
fn get_platform_info() -> PlatformInput {
    // For non-Linux, return default (would need platform-specific impl)
    PlatformInput::default()
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let platform = get_platform_info();
    let classification = SecurityClassification::classify(&platform);
    let json = serde_json::to_string(&classification).unwrap_or_default();

    unsafe {
        CLASSIFICATION = Some(json);
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
