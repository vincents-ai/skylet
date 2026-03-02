// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! System Monitor Plugin
//!
//! Monitors:
//! - CPU usage
//! - Memory usage
//! - Disk I/O
//! - Network I/O
//!
//! Supports: Linux, macOS, Windows

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static mut CURRENT_STATS: Option<String> = None;

#[derive(Debug, serde::Serialize)]
pub struct SystemStats {
    #[serde(rename = "cpu")]
    pub cpu: CpuStats,
    #[serde(rename = "memory")]
    pub memory: MemoryStats,
    #[serde(rename = "disk")]
    pub disk: DiskStats,
    #[serde(rename = "network")]
    pub network: NetworkStats,
    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CpuStats {
    #[serde(rename = "usagePercent")]
    pub usage_percent: f64,
    #[serde(rename = "coreCount")]
    pub core_count: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct MemoryStats {
    #[serde(rename = "totalBytes")]
    pub total_bytes: u64,
    #[serde(rename = "usedBytes")]
    pub used_bytes: u64,
    #[serde(rename = "availableBytes")]
    pub available_bytes: u64,
    #[serde(rename = "usagePercent")]
    pub usage_percent: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct DiskStats {
    #[serde(rename = "readBytes")]
    pub read_bytes: u64,
    #[serde(rename = "writeBytes")]
    pub write_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct NetworkStats {
    #[serde(rename = "rxBytes")]
    pub rx_bytes: u64,
    #[serde(rename = "txBytes")]
    pub tx_bytes: u64,
}

impl SystemStats {
    fn collect() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::collect_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::collect_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::collect_windows()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::collect_unknown()
        }
    }

    #[cfg(target_os = "linux")]
    fn collect_linux() -> Self {
        let cpu = Self::cpu_linux();
        let memory = Self::memory_linux();
        let disk = Self::disk_linux();
        let network = Self::network_linux();

        SystemStats {
            cpu,
            memory,
            disk,
            network,
            os: "linux".to_string(),
        }
    }

    #[cfg(target_os = "macos")]
    fn collect_macos() -> Self {
        let cpu = Self::cpu_macos();
        let memory = Self::memory_macos();
        let disk = Self::disk_macos();
        let network = Self::network_macos();

        SystemStats {
            cpu,
            memory,
            disk,
            network,
            os: "macos".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    fn collect_windows() -> Self {
        let cpu = Self::cpu_windows();
        let memory = Self::memory_windows();
        let disk = Self::disk_windows();
        let network = Self::network_windows();

        SystemStats {
            cpu,
            memory,
            disk,
            network,
            os: "windows".to_string(),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn collect_unknown() -> Self {
        SystemStats {
            cpu: CpuStats {
                usage_percent: 0.0,
                core_count: 0,
            },
            memory: MemoryStats {
                total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
            },
            disk: DiskStats {
                read_bytes: 0,
                write_bytes: 0,
            },
            network: NetworkStats {
                rx_bytes: 0,
                tx_bytes: 0,
            },
            os: "unknown".to_string(),
        }
    }

    #[cfg(target_os = "linux")]
    fn cpu_linux() -> CpuStats {
        use std::fs;

        let core_count = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1);

        let usage_percent = if let Ok(stat) = fs::read_to_string("/proc/stat") {
            let cpu_line = stat.lines().find(|l| l.starts_with("cpu "));
            if let Some(line) = cpu_line {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let user: u64 = parts[1].parse().unwrap_or(0);
                    let nice: u64 = parts[2].parse().unwrap_or(0);
                    let system: u64 = parts[3].parse().unwrap_or(0);
                    let idle: u64 = parts[4].parse().unwrap_or(0);
                    let total = user + nice + system + idle;
                    if total > 0 {
                        ((total - idle) as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        CpuStats {
            usage_percent,
            core_count,
        }
    }

    #[cfg(target_os = "macos")]
    fn cpu_macos() -> CpuStats {
        use std::process::Command;

        let core_count = std::thread::available_parallelization()
            .map(|p| p.get())
            .unwrap_or(1);

        let usage_percent =
            if let Ok(output) = Command::new("top").args(["-l", "1", "-n", "0"]).output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("CPU usage:") {
                        if let Some(usr) = line.find("user:") {
                            let rest = &line[usr..];
                            let parts: Vec<&str> = rest.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(val) = parts[1].trim_end_matches('%').parse::<f64>() {
                                    return CpuStats {
                                        usage_percent: val,
                                        core_count,
                                    };
                                }
                            }
                        }
                    }
                }
            };

        CpuStats {
            usage_percent: 0.0,
            core_count,
        }
    }

    #[cfg(target_os = "windows")]
    fn cpu_windows() -> CpuStats {
        use std::process::Command;

        let core_count = std::thread::available_parallelization()
            .map(|p| p.get())
            .unwrap_or(1);

        let usage_percent = if let Ok(output) = Command::new("wmic")
            .args(["cpu", "get", "loadpercentage"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                if let Ok(val) = line.trim().parse::<f64>() {
                    return CpuStats {
                        usage_percent: val,
                        core_count,
                    };
                }
            }
        };

        CpuStats {
            usage_percent: 0.0,
            core_count,
        }
    }

    #[cfg(target_os = "linux")]
    fn memory_linux() -> MemoryStats {
        use std::fs;

        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total: u64 = 0;
            let mut available: u64 = 0;
            let mut free: u64 = 0;
            let mut buffers: u64 = 0;
            let mut cached: u64 = 0;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    total = Self::parse_meminfo_value(line);
                } else if line.starts_with("MemAvailable:") {
                    available = Self::parse_meminfo_value(line);
                } else if line.starts_with("MemFree:") {
                    free = Self::parse_meminfo_value(line);
                } else if line.starts_with("Buffers:") {
                    buffers = Self::parse_meminfo_value(line);
                } else if line.starts_with("Cached:") {
                    cached = Self::parse_meminfo_value(line);
                }
            }

            if available == 0 {
                available = free + buffers + cached;
            }

            let used = total.saturating_sub(available);
            let usage_percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            MemoryStats {
                total_bytes: total * 1024,
                used_bytes: used * 1024,
                available_bytes: available * 1024,
                usage_percent,
            }
        } else {
            MemoryStats {
                total_bytes: 0,
                used_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn parse_meminfo_value(line: &str) -> u64 {
        line.split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    #[cfg(target_os = "macos")]
    fn memory_macos() -> MemoryStats {
        use std::process::Command;

        if let Ok(output) = Command::new("vm_stat").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut pages_free: u64 = 0;
            let mut pages_active: u64 = 0;
            let mut pages_inactive: u64 = 0;
            let mut pages_wire: u64 = 0;
            let mut page_size: u64 = 4096;

            if let Ok(out) = Command::new("pagesize").output() {
                if let Ok(ps) = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>() {
                    page_size = ps;
                }
            }

            for line in stdout.lines() {
                if line.contains("Pages free:") {
                    pages_free = Self::parse_macvm_value(line);
                } else if line.contains("Pages active:") {
                    pages_active = Self::parse_macvm_value(line);
                } else if line.contains("Pages inactive:") {
                    pages_inactive = Self::parse_macvm_value(line);
                } else if line.contains("Pages wired:") {
                    pages_wire = Self::parse_macvm_value(line);
                }
            }

            let total = (pages_free + pages_active + pages_inactive + pages_wire) * page_size;
            let available = pages_free * page_size;
            let used = (pages_active + pages_wire) * page_size;
            let usage_percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            return MemoryStats {
                total_bytes: total,
                used_bytes: used,
                available_bytes: available,
                usage_percent,
            };
        }

        MemoryStats {
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            usage_percent: 0.0,
        }
    }

    #[cfg(target_os = "macos")]
    fn parse_macvm_value(line: &str) -> u64 {
        line.split(':')
            .nth(1)
            .map(|v| v.trim().trim_end_matches('.').parse().unwrap_or(0))
            .unwrap_or(0)
    }

    #[cfg(target_os = "windows")]
    fn memory_windows() -> MemoryStats {
        use std::process::Command;

        if let Ok(output) = Command::new("wmic")
            .args([
                "OS",
                "get",
                "TotalVisibleMemorySize,FreePhysicalMemory",
                "/format:list",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut total: u64 = 0;
            let mut free: u64 = 0;

            for line in stdout.lines() {
                if line.starts_with("TotalVisibleMemorySize=") {
                    total = line
                        .split('=')
                        .nth(1)
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("FreePhysicalMemory=") {
                    free = line
                        .split('=')
                        .nth(1)
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                }
            }

            let available = free * 1024;
            let used = (total * 1024).saturating_sub(available);
            let usage_percent = if total > 0 {
                (used as f64 / (total * 1024) as f64) * 100.0
            } else {
                0.0
            };

            return MemoryStats {
                total_bytes: total * 1024,
                used_bytes: used,
                available_bytes: available,
                usage_percent,
            };
        }

        MemoryStats {
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            usage_percent: 0.0,
        }
    }

    #[cfg(target_os = "linux")]
    fn disk_linux() -> DiskStats {
        use std::fs;

        let mut read_bytes: u64 = 0;
        let mut write_bytes: u64 = 0;

        if let Ok(stat) = fs::read_to_string("/proc/diskstats") {
            for line in stat.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 14 {
                    if let Ok(r) = parts[5].parse::<u64>() {
                        read_bytes += r * 512;
                    }
                    if let Ok(w) = parts[9].parse::<u64>() {
                        write_bytes += w * 512;
                    }
                }
            }
        }

        DiskStats {
            read_bytes,
            write_bytes,
        }
    }

    #[cfg(target_os = "macos")]
    fn disk_macos() -> DiskStats {
        use std::process::Command;

        let mut read_bytes: u64 = 0;
        let mut write_bytes: u64 = 0;

        if let Ok(output) = Command::new("iostat").args(["-d", "-c", "2"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if i > 0 && !line.trim().is_empty() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let Ok(r) = parts[1].parse::<f64>() {
                            read_bytes += (r * 1024.0) as u64;
                        }
                        if let Ok(w) = parts[2].parse::<f64>() {
                            write_bytes += (w * 1024.0) as u64;
                        }
                    }
                }
            }
        }

        DiskStats {
            read_bytes,
            write_bytes,
        }
    }

    #[cfg(target_os = "windows")]
    fn disk_windows() -> DiskStats {
        use std::process::Command;

        let mut read_bytes: u64 = 0;
        let mut write_bytes: u64 = 0;

        if let Ok(output) = Command::new("wmic")
            .args(["logicaldisk", "get", "Size,FreeSpace", "/format:list"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("Size=") || line.starts_with("FreeSpace=") {
                    if let Some(val) = line.split('=').nth(1) {
                        if let Ok(v) = val.trim().parse::<u64>() {
                            if line.starts_with("Size=") {
                                read_bytes += v;
                            }
                        }
                    }
                }
            }
        }

        DiskStats {
            read_bytes,
            write_bytes,
        }
    }

    #[cfg(target_os = "linux")]
    fn network_linux() -> NetworkStats {
        use std::fs;

        let mut rx_bytes: u64 = 0;
        let mut tx_bytes: u64 = 0;

        if let Ok(net_dev) = fs::read_to_string("/proc/net/dev") {
            for line in net_dev.lines().skip(2) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 10 {
                    if let Ok(rx) = parts[1].parse::<u64>() {
                        rx_bytes += rx;
                    }
                    if let Ok(tx) = parts[9].parse::<u64>() {
                        tx_bytes += tx;
                    }
                }
            }
        }

        NetworkStats { rx_bytes, tx_bytes }
    }

    #[cfg(target_os = "macos")]
    fn network_macos() -> NetworkStats {
        use std::process::Command;

        let mut rx_bytes: u64 = 0;
        let mut tx_bytes: u64 = 0;

        if let Ok(output) = Command::new("netstat").args(["-ib"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    if let Ok(rx) = parts[6].parse::<u64>() {
                        rx_bytes += rx;
                    }
                    if let Ok(tx) = parts[9].parse::<u64>() {
                        tx_bytes += tx;
                    }
                }
            }
        }

        NetworkStats { rx_bytes, tx_bytes }
    }

    #[cfg(target_os = "windows")]
    fn network_windows() -> NetworkStats {
        use std::process::Command;

        let mut rx_bytes: u64 = 0;
        let mut tx_bytes: u64 = 0;

        if let Ok(output) = Command::new("powershell")
            .args([
                "-Command",
                "Get-NetAdapterStatistics | Select-Object -ExpandProperty ReceivedBytes",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(v) = line.trim().parse::<u64>() {
                    rx_bytes += v;
                }
            }
        }

        if let Ok(output) = Command::new("powershell")
            .args([
                "-Command",
                "Get-NetAdapterStatistics | Select-Object -ExpandProperty SentBytes",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(v) = line.trim().parse::<u64>() {
                    tx_bytes += v;
                }
            }
        }

        NetworkStats { rx_bytes, tx_bytes }
    }
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let stats = SystemStats::collect();
    let json = serde_json::to_string(&stats).unwrap_or_default();

    unsafe {
        CURRENT_STATS = Some(json);
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

#[no_mangle]
pub extern "C" fn plugin_invoke_v2(
    _context: *const PluginContextV2,
    method: *const std::os::raw::c_char,
) -> PluginResultV2 {
    if method.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let method_str = unsafe {
        std::ffi::CStr::from_ptr(method)
            .to_string_lossy()
            .into_owned()
    };

    if method_str == "get_stats" {
        let stats = SystemStats::collect();
        let json = serde_json::to_string(&stats).unwrap_or_default();

        unsafe {
            CURRENT_STATS = Some(json);
        }

        return PluginResultV2::Success;
    }

    if method_str == "refresh" {
        let stats = SystemStats::collect();
        let json = serde_json::to_string(&stats).unwrap_or_default();

        unsafe {
            CURRENT_STATS = Some(json);
        }

        return PluginResultV2::Success;
    }

    PluginResultV2::InvalidRequest
}
