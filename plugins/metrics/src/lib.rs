// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Metrics Plugin
//!
//! Collects Prometheus-style metrics:
//! - Counters: requests_total, errors_total, plugin_loads_total
//! - Gauges: memory_usage_bytes, cpu_percent, active_plugins
//! - Histograms: request_duration_ms, plugin_load_duration_ms
//!
//! Supports: Linux, macOS, Windows

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static ERRORS_TOTAL: AtomicU64 = AtomicU64::new(0);
static PLUGIN_LOADS_TOTAL: AtomicU64 = AtomicU64::new(0);
static MEMORY_USAGE_BYTES: AtomicU64 = AtomicU64::new(0);
static CPU_PERCENT: AtomicU64 = AtomicU64::new(0);
static ACTIVE_PLUGINS: AtomicU64 = AtomicU64::new(0);

static mut START_TIME: Option<Instant> = None;

#[derive(Debug, serde::Serialize)]
pub struct MetricsResponse {
    pub counters: Counters,
    pub gauges: Gauges,
    pub histograms: Histograms,
}

#[derive(Debug, serde::Serialize)]
pub struct Counters {
    #[serde(rename = "requests_total")]
    pub requests_total: u64,
    #[serde(rename = "errors_total")]
    pub errors_total: u64,
    #[serde(rename = "plugin_loads_total")]
    pub plugin_loads_total: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct Gauges {
    #[serde(rename = "memory_usage_bytes")]
    pub memory_usage_bytes: u64,
    #[serde(rename = "cpu_percent")]
    pub cpu_percent: u64,
    #[serde(rename = "active_plugins")]
    pub active_plugins: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct Histograms {
    #[serde(rename = "request_duration_ms")]
    pub request_duration_ms: HistogramData,
    #[serde(rename = "plugin_load_duration_ms")]
    pub plugin_load_duration_ms: HistogramData,
}

#[derive(Debug, serde::Serialize)]
pub struct HistogramData {
    pub count: u64,
    pub sum: u64,
    pub buckets: std::collections::HashMap<String, u64>,
}

#[derive(Debug, serde::Serialize)]
pub struct PrometheusMetrics {
    #[serde(skip)]
    pub counters: String,
    #[serde(skip)]
    pub gauges: String,
    #[serde(skip)]
    pub histograms: String,
}

pub fn increment_requests() {
    REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn increment_errors() {
    ERRORS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn increment_plugin_loads() {
    PLUGIN_LOADS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn set_memory_usage(bytes: u64) {
    MEMORY_USAGE_BYTES.store(bytes, Ordering::Relaxed);
}

pub fn set_cpu_percent(percent: u64) {
    CPU_PERCENT.store(percent, Ordering::Relaxed);
}

pub fn set_active_plugins(count: u64) {
    ACTIVE_PLUGINS.store(count, Ordering::Relaxed);
}

pub fn get_metrics() -> MetricsResponse {
    let counters = Counters {
        requests_total: REQUESTS_TOTAL.load(Ordering::Relaxed),
        errors_total: ERRORS_TOTAL.load(Ordering::Relaxed),
        plugin_loads_total: PLUGIN_LOADS_TOTAL.load(Ordering::Relaxed),
    };

    let gauges = Gauges {
        memory_usage_bytes: MEMORY_USAGE_BYTES.load(Ordering::Relaxed),
        cpu_percent: CPU_PERCENT.load(Ordering::Relaxed),
        active_plugins: ACTIVE_PLUGINS.load(Ordering::Relaxed),
    };

    let histograms = Histograms {
        request_duration_ms: HistogramData {
            count: 0,
            sum: 0,
            buckets: std::collections::HashMap::new(),
        },
        plugin_load_duration_ms: HistogramData {
            count: 0,
            sum: 0,
            buckets: std::collections::HashMap::new(),
        },
    };

    MetricsResponse {
        counters,
        gauges,
        histograms,
    }
}

pub fn export_prometheus_format() -> String {
    let metrics = get_metrics();
    let mut output = String::new();

    output.push_str("# HELP requests_total Total number of requests\n");
    output.push_str("# TYPE requests_total counter\n");
    output.push_str(&format!(
        "requests_total {}\n",
        metrics.counters.requests_total
    ));

    output.push_str("# HELP errors_total Total number of errors\n");
    output.push_str("# TYPE errors_total counter\n");
    output.push_str(&format!("errors_total {}\n", metrics.counters.errors_total));

    output.push_str("# HELP plugin_loads_total Total number of plugin loads\n");
    output.push_str("# TYPE plugin_loads_total counter\n");
    output.push_str(&format!(
        "plugin_loads_total {}\n",
        metrics.counters.plugin_loads_total
    ));

    output.push_str("# HELP memory_usage_bytes Current memory usage in bytes\n");
    output.push_str("# TYPE memory_usage_bytes gauge\n");
    output.push_str(&format!(
        "memory_usage_bytes {}\n",
        metrics.gauges.memory_usage_bytes
    ));

    output.push_str("# HELP cpu_percent Current CPU usage percentage\n");
    output.push_str("# TYPE cpu_percent gauge\n");
    output.push_str(&format!("cpu_percent {}\n", metrics.gauges.cpu_percent));

    output.push_str("# HELP active_plugins Number of currently active plugins\n");
    output.push_str("# TYPE active_plugins gauge\n");
    output.push_str(&format!(
        "active_plugins {}\n",
        metrics.gauges.active_plugins
    ));

    output.push_str("# HELP request_duration_ms Request duration in milliseconds\n");
    output.push_str("# TYPE request_duration_ms histogram\n");
    output.push_str(&format!(
        "request_duration_ms_count {}\n",
        metrics.histograms.request_duration_ms.count
    ));
    output.push_str(&format!(
        "request_duration_ms_sum {}\n",
        metrics.histograms.request_duration_ms.sum
    ));

    output.push_str("# HELP plugin_load_duration_ms Plugin load duration in milliseconds\n");
    output.push_str("# TYPE plugin_load_duration_ms histogram\n");
    output.push_str(&format!(
        "plugin_load_duration_ms_count {}\n",
        metrics.histograms.plugin_load_duration_ms.count
    ));
    output.push_str(&format!(
        "plugin_load_duration_ms_sum {}\n",
        metrics.histograms.plugin_load_duration_ms.sum
    ));

    output
}

#[cfg(target_os = "linux")]
fn collect_system_metrics() {
    use std::fs;

    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemAvailable:") {
                if let Some(available) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = available.parse::<u64>() {
                        set_memory_usage(kb * 1024);
                    }
                }
                break;
            }
        }
    }

    if let Ok(content) = fs::read_to_string("/proc/stat") {
        if let Some(cpu_line) = content.lines().next() {
            if cpu_line.starts_with("cpu ") {
                let parts: Vec<&str> = cpu_line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let total: u64 = parts[1..]
                        .iter()
                        .filter_map(|s| s.parse::<u64>().ok())
                        .sum();
                    let idle: u64 = parts.get(4).and_then(|s| s.parse::<u64>().unwrap_or(0));
                    if total > 0 {
                        let usage = ((total - idle) * 100) / total;
                        set_cpu_percent(usage);
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn collect_system_metrics() {
    use std::process::Command;

    if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        let mem = String::from_utf8_lossy(&output.stdout);
        if let Ok(bytes) = mem.trim().parse::<u64>() {
            set_memory_usage(bytes);
        }
    }

    if let Ok(output) = Command::new("top").args(["-l", "1", "-n", "0"]).output() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines() {
            if line.contains("CPU usage:") {
                if let Some(percent) = line.split("CPU usage:").nth(1) {
                    if let Some(first) = percent.split_whitespace().next() {
                        if let Ok(p) = first.trim_end_matches('%').parse::<u64>() {
                            set_cpu_percent(p);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn collect_system_metrics() {
    use std::process::Command;

    if let Ok(output) = Command::new("wmic")
        .args(["OS", "get", "FreePhysicalMemory", "/Value"])
        .output()
    {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines() {
            if line.starts_with("FreePhysicalMemory=") {
                if let Some(free) = line.split('=').nth(1) {
                    if let Ok(kb) = free.trim().parse::<u64>() {
                        set_memory_usage(kb * 1024);
                    }
                }
            }
        }
    }

    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "LoadPercentage"])
        .output()
    {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines().skip(1) {
            if let Ok(percent) = line.trim().parse::<u64>() {
                set_cpu_percent(percent);
                break;
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn collect_system_metrics() {}

fn update_metrics() {
    collect_system_metrics();
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        START_TIME = Some(Instant::now());
    }

    increment_plugin_loads();
    update_metrics();

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
