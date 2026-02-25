// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Network Monitor Plugin
//!
//! Monitors:
//! - Network interfaces with traffic statistics (bytes sent/received)
//! - Active connections (ports, protocols)
//!
//! Supports: Linux, macOS, Windows

use serde::Serialize;
use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static mut NETWORK_STATS_JSON: Option<String> = None;

#[derive(Debug, Serialize)]
pub struct NetworkStats {
    #[serde(rename = "interfaces")]
    pub interfaces: Vec<NetworkInterface>,
    #[serde(rename = "connections")]
    pub connections: Vec<Connection>,
    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Debug, Serialize)]
pub struct NetworkInterface {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "bytesSent")]
    pub bytes_sent: u64,
    #[serde(rename = "bytesRecv")]
    pub bytes_recv: u64,
    #[serde(rename = "packetsSent")]
    pub packets_sent: u64,
    #[serde(rename = "packetsRecv")]
    pub packets_recv: u64,
    #[serde(rename = "errin")]
    pub err_in: u64,
    #[serde(rename = "errout")]
    pub err_out: u64,
    #[serde(rename = "dropin")]
    pub drop_in: u64,
    #[serde(rename = "dropout")]
    pub drop_out: u64,
}

#[derive(Debug, Serialize)]
pub struct Connection {
    #[serde(rename = "localAddr")]
    pub local_addr: String,
    #[serde(rename = "remoteAddr")]
    pub remote_addr: String,
    #[serde(rename = "state")]
    pub state: String,
    #[serde(rename = "protocol")]
    pub protocol: String,
    #[serde(rename = "pid")]
    pub pid: Option<u32>,
}

impl NetworkStats {
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
        use std::fs;

        let interfaces = collect_interfaces_linux();
        let connections = collect_connections_linux();

        NetworkStats {
            interfaces,
            connections,
            os: "linux".to_string(),
        }
    }

    #[cfg(target_os = "macos")]
    fn collect_macos() -> Self {
        use std::process::Command;

        let interfaces = collect_interfaces_macos();
        let connections = collect_connections_macos();

        NetworkStats {
            interfaces,
            connections,
            os: "macos".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    fn collect_windows() -> Self {
        use std::process::Command;

        let interfaces = collect_interfaces_windows();
        let connections = collect_connections_windows();

        NetworkStats {
            interfaces,
            connections,
            os: "windows".to_string(),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn collect_unknown() -> Self {
        NetworkStats {
            interfaces: vec![],
            connections: vec![],
            os: "unknown".to_string(),
        }
    }
}

// =============================================================================
// Linux Network Collection
// =============================================================================

#[cfg(target_os = "linux")]
fn collect_interfaces_linux() -> Vec<NetworkInterface> {
    use std::fs;

    let mut interfaces = Vec::new();

    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "lo" {
                continue;
            }

            let read_u64 = |path: &str| -> u64 {
                fs::read_to_string(path)
                    .ok()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0)
            };

            let base = format!("/sys/class/net/{}", name);
            let bytes_sent = read_u64(&format!("{}/statistics/tx_bytes", base));
            let bytes_recv = read_u64(&format!("{}/statistics/rx_bytes", base));
            let packets_sent = read_u64(&format!("{}/statistics/tx_packets", base));
            let packets_recv = read_u64(&format!("{}/statistics/rx_packets", base));
            let err_in = read_u64(&format!("{}/statistics/rx_errors", base));
            let err_out = read_u64(&format!("{}/statistics/tx_errors", base));
            let drop_in = read_u64(&format!("{}/statistics/rx_dropped", base));
            let drop_out = read_u64(&format!("{}/statistics/tx_dropped", base));

            if bytes_sent > 0 || bytes_recv > 0 || packets_sent > 0 || packets_recv > 0 {
                interfaces.push(NetworkInterface {
                    name,
                    bytes_sent,
                    bytes_recv,
                    packets_sent,
                    packets_recv,
                    errin: err_in,
                    errout: err_out,
                    dropin: drop_in,
                    dropout: drop_out,
                });
            }
        }
    }

    interfaces
}

#[cfg(target_os = "linux")]
fn collect_connections_linux() -> Vec<Connection> {
    use std::fs;

    let mut connections = Vec::new();

    let protocols = [
        ("tcp", "/proc/net/tcp"),
        ("udp", "/proc/net/udp"),
        ("tcp6", "/proc/net/tcp6"),
        ("udp6", "/proc/net/udp6"),
    ];

    for (protocol, path) in protocols {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 10 {
                    let local = parse_hex_address(parts[1]);
                    let remote = parse_hex_address(parts[2]);
                    let state = parse_hex_state(parts[3]);

                    if !local.contains(":0") {
                        connections.push(Connection {
                            local_addr: local,
                            remote_addr: remote,
                            state,
                            protocol: protocol.to_string(),
                            pid: None,
                        });
                    }
                }
            }
        }
    }

    connections
}

#[cfg(target_os = "linux")]
fn parse_hex_address(hex: &str) -> String {
    let parts: Vec<&str> = hex.split(':').collect();
    if parts.len() != 2 {
        return hex.to_string();
    }

    let ip_hex = u32::from_str_radix(parts[0], 16).ok();
    let port = u16::from_str_radix(parts[1], 16).ok();

    match (ip_hex, port) {
        (Some(ip), Some(port)) => {
            let octets = [
                (ip & 0xff) as u8,
                ((ip >> 8) & 0xff) as u8,
                ((ip >> 16) & 0xff) as u8,
                ((ip >> 24) & 0xff) as u8,
            ];
            format!(
                "{}.{}.{}.{}:{}",
                octets[0], octets[1], octets[2], octets[3], port
            )
        }
        _ => hex.to_string(),
    }
}

#[cfg(target_os = "linux")]
fn parse_hex_state(hex: &str) -> String {
    match u8::from_str_radix(hex, 16).ok() {
        Some(0x01) => "ESTABLISHED".to_string(),
        Some(0x02) => "SYN_SENT".to_string(),
        Some(0x03) => "SYN_RECV".to_string(),
        Some(0x04) => "FIN_WAIT1".to_string(),
        Some(0x05) => "FIN_WAIT2".to_string(),
        Some(0x06) => "TIME_WAIT".to_string(),
        Some(0x07) => "CLOSE".to_string(),
        Some(0x08) => "CLOSE_WAIT".to_string(),
        Some(0x09) => "LAST_ACK".to_string(),
        Some(0x0a) => "LISTEN".to_string(),
        Some(0x0b) => "CLOSING".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}

// =============================================================================
// macOS Network Collection
// =============================================================================

#[cfg(target_os = "macos")]
fn collect_interfaces_macos() -> Vec<NetworkInterface> {
    use std::process::Command;

    let mut interfaces = Vec::new();

    if let Ok(output) = Command::new("netstat", ["-ib"]).output() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let name = parts[0].to_string();
                if name == "lo0" {
                    continue;
                }
                if let (Some(bytes_recv), Some(bytes_sent)) = (
                    parts.get(6).and_then(|s| s.parse().ok()),
                    parts.get(8).and_then(|s| s.parse().ok()),
                ) {
                    if bytes_recv > 0 || bytes_sent > 0 {
                        interfaces.push(NetworkInterface {
                            name,
                            bytes_sent,
                            bytes_recv,
                            packets_sent: 0,
                            packets_recv: 0,
                            errin: 0,
                            errout: 0,
                            dropin: 0,
                            dropout: 0,
                        });
                    }
                }
            }
        }
    }

    interfaces
}

#[cfg(target_os = "macos")]
fn collect_connections_macos() -> Vec<Connection> {
    use std::process::Command;

    let mut connections = Vec::new();

    if let Ok(output) = Command::new("netstat", ["-an"]).output() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines() {
            if line.contains("tcp4")
                || line.contains("tcp6")
                || line.contains("udp4")
                || line.contains("udp6")
            {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let protocol = if line.contains("tcp") { "tcp" } else { "udp" };
                    let local_addr = parts[3].to_string();
                    let state = if protocol == "tcp" && parts.len() >= 6 {
                        parts.get(5).unwrap_or(&"").to_string()
                    } else {
                        "LISTEN".to_string()
                    };

                    if !local_addr.contains(":0") && local_addr != "*.*" {
                        connections.push(Connection {
                            local_addr: local_addr.clone(),
                            remote_addr: "*.*".to_string(),
                            state,
                            protocol: protocol.to_string(),
                            pid: None,
                        });
                    }
                }
            }
        }
    }

    connections
}

// =============================================================================
// Windows Network Collection
// =============================================================================

#[cfg(target_os = "windows")]
fn collect_interfaces_windows() -> Vec<NetworkInterface> {
    use std::process::Command;

    let mut interfaces = Vec::new();

    if let Ok(output) = Command::new("netstat", ["-e"]).output() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                if let (
                    Some(bytes_recv),
                    Some(bytes_sent),
                    Some(packets_recv),
                    Some(packets_sent),
                ) = (
                    parts.get(1).and_then(|s| s.parse().ok()),
                    parts.get(2).and_then(|s| s.parse().ok()),
                    parts.get(3).and_then(|s| s.parse().ok()),
                    parts.get(4).and_then(|s| s.parse().ok()),
                ) {
                    interfaces.push(NetworkInterface {
                        name: "default".to_string(),
                        bytes_sent,
                        bytes_recv,
                        packets_sent,
                        packets_recv,
                        errin: 0,
                        errout: 0,
                        dropin: 0,
                        dropout: 0,
                    });
                }
            }
        }
    }

    interfaces
}

#[cfg(target_os = "windows")]
fn collect_connections_windows() -> Vec<Connection> {
    use std::process::Command;

    let mut connections = Vec::new();

    if let Ok(output) = Command::new("netstat", ["-ano"]).output() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines().skip(4) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let protocol = parts[0].to_lowercase();
                if protocol != "tcp" && protocol != "udp" {
                    continue;
                }

                let local_addr = parts[1].to_string();
                let remote_addr = parts[2].to_string();
                let state = if protocol == "tcp" && parts.len() >= 5 {
                    parts[3].to_string()
                } else {
                    "LISTEN".to_string()
                };
                let pid = parts.last().and_then(|s| s.parse().ok());

                if !local_addr.contains(":0") && local_addr != "*:*" {
                    connections.push(Connection {
                        local_addr,
                        remote_addr,
                        state,
                        protocol,
                        pid,
                    });
                }
            }
        }
    }

    connections
}

// =============================================================================
// Plugin Entry Points
// =============================================================================

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let stats = NetworkStats::collect();
    let json = serde_json::to_string(&stats).unwrap_or_default();

    unsafe {
        NETWORK_STATS_JSON = Some(json);
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
