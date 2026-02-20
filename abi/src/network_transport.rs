// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Network Transport Abstraction (RFC-0068)
//!
//! Unified interface for overlay network plugins including WireGuard, Veilid, Tor, and libp2p.
//! This module defines the common traits and types for:
//! - Tunnel management (create, delete, list)
//! - Peer discovery and management
//! - Service advertisement over overlay networks
//! - Network metrics and health monitoring

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{c_char, c_void};

// ============================================================================
// Overlay Network Types
// ============================================================================

/// Supported overlay network transport types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub enum OverlayTransportType {
    /// libp2p-based P2P mesh network
    Libp2p,
    /// Tor hidden services
    Tor,
    /// I2P network
    I2p,
    /// WireGuard VPN tunnels
    WireGuard,
    /// Veilid distributed network
    Veilid,
    /// Custom/proprietary overlay
    Custom,
}

impl Default for OverlayTransportType {
    fn default() -> Self {
        Self::Libp2p
    }
}

/// Tunnel configuration for creating encrypted tunnels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    /// Unique tunnel identifier
    pub tunnel_id: String,
    /// Transport type for this tunnel
    pub transport: OverlayTransportType,
    /// Local address to bind (e.g., "0.0.0.0:8080")
    pub local_address: String,
    /// Remote address or peer ID to connect to
    pub remote_address: Option<String>,
    /// Virtual CIDR allocation for overlay network
    pub cidr: Option<String>,
    /// MTU for the tunnel interface
    pub mtu: Option<u16>,
    /// Keepalive interval in seconds
    pub keepalive_secs: Option<u32>,
    /// Additional transport-specific configuration
    pub extra_config: HashMap<String, String>,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            tunnel_id: uuid::Uuid::new_v4().to_string(),
            transport: OverlayTransportType::default(),
            local_address: "0.0.0.0:0".to_string(),
            remote_address: None,
            cidr: None,
            mtu: Some(1420), // Default WireGuard MTU
            keepalive_secs: Some(25),
            extra_config: HashMap::new(),
        }
    }
}

/// Information about an active tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
    /// Unique tunnel identifier
    pub tunnel_id: String,
    /// Transport type
    pub transport: OverlayTransportType,
    /// Current tunnel status
    pub status: TunnelStatus,
    /// Local endpoint address
    pub local_address: String,
    /// Remote endpoint address (if connected)
    pub remote_address: Option<String>,
    /// Allocated CIDR (if applicable)
    pub allocated_cidr: Option<String>,
    /// Bytes sent through tunnel
    pub bytes_sent: u64,
    /// Bytes received through tunnel
    pub bytes_received: u64,
    /// Tunnel creation timestamp (Unix epoch seconds)
    pub created_at: u64,
    /// Last activity timestamp (Unix epoch seconds)
    pub last_activity: Option<u64>,
}

/// Status of a tunnel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub enum TunnelStatus {
    /// Tunnel is being established
    Connecting,
    /// Tunnel is active and operational
    Active,
    /// Tunnel is being torn down
    Disconnecting,
    /// Tunnel is inactive/closed
    Inactive,
    /// Tunnel encountered an error
    Error,
}

/// Information about a peer in the overlay network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique peer identifier (transport-specific format)
    pub peer_id: String,
    /// Human-readable peer name (if available)
    pub name: Option<String>,
    /// Transport type through which peer is reachable
    pub transport: OverlayTransportType,
    /// List of addresses the peer is reachable at
    pub addresses: Vec<String>,
    /// Connection state
    pub connected: bool,
    /// Round-trip latency in milliseconds (if connected)
    pub latency_ms: Option<u64>,
    /// Protocols supported by the peer
    pub protocols: Vec<String>,
    /// Additional peer metadata
    pub metadata: HashMap<String, String>,
}

/// Service advertisement for overlay network discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAdvertisement {
    /// Service name
    pub service_name: String,
    /// Service type (e.g., "http", "grpc", "websocket")
    pub service_type: String,
    /// Transport type hosting the service
    pub transport: OverlayTransportType,
    /// Port the service is listening on
    pub port: u16,
    /// Overlay address (e.g., .onion address, peer ID)
    pub overlay_address: String,
    /// Service-specific metadata
    pub metadata: HashMap<String, String>,
    /// TTL for this advertisement in seconds (0 = permanent)
    pub ttl_secs: u32,
}

/// Network metrics for an overlay transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayMetrics {
    /// Transport type these metrics apply to
    pub transport: OverlayTransportType,
    /// Number of active tunnels
    pub active_tunnels: u32,
    /// Number of connected peers
    pub connected_peers: u32,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P99 latency in milliseconds
    pub p99_latency_ms: f64,
    /// Number of connection errors
    pub error_count: u64,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Transport-specific metrics
    pub extra_metrics: HashMap<String, f64>,
}

impl Default for OverlayMetrics {
    fn default() -> Self {
        Self {
            transport: OverlayTransportType::default(),
            active_tunnels: 0,
            connected_peers: 0,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            avg_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            error_count: 0,
            uptime_secs: 0,
            extra_metrics: HashMap::new(),
        }
    }
}

/// Result of an overlay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayResult<T> {
    /// Whether the operation succeeded
    pub success: bool,
    /// Result data (if successful)
    pub data: Option<T>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl<T> OverlayResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

// ============================================================================
// FFI Types for C Interop
// ============================================================================

/// FFI version of TunnelInfo for C interop
#[repr(C)]
pub struct TunnelInfoFFI {
    pub tunnel_id: *const c_char,
    pub transport: OverlayTransportType,
    pub status: TunnelStatus,
    pub local_address: *const c_char,
    pub remote_address: *const c_char,
    pub allocated_cidr: *const c_char,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub created_at: u64,
    pub last_activity: u64,
}

/// FFI version of PeerInfo for C interop
#[repr(C)]
pub struct PeerInfoFFI {
    pub peer_id: *const c_char,
    pub name: *const c_char,
    pub transport: OverlayTransportType,
    pub addresses: *const *const c_char,
    pub address_count: usize,
    pub connected: i32,
    pub latency_ms: u64,
    pub protocols: *const *const c_char,
    pub protocol_count: usize,
}

/// FFI version of OverlayMetrics for C interop
#[repr(C)]
pub struct OverlayMetricsFFI {
    pub transport: OverlayTransportType,
    pub active_tunnels: u32,
    pub connected_peers: u32,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub error_count: u64,
    pub uptime_secs: u64,
}

/// FFI result type for operations returning strings
#[repr(C)]
pub struct OverlayStringResult {
    pub success: i32,
    pub data: *const c_char,
    pub error: *const c_char,
}

/// FFI result type for tunnel list operations
#[repr(C)]
pub struct OverlayTunnelListResult {
    pub success: i32,
    pub tunnels: *const TunnelInfoFFI,
    pub tunnel_count: usize,
    pub error: *const c_char,
}

/// FFI result type for peer list operations
#[repr(C)]
pub struct OverlayPeerListResult {
    pub success: i32,
    pub peers: *const PeerInfoFFI,
    pub peer_count: usize,
    pub error: *const c_char,
}

// ============================================================================
// OverlayNetwork Trait
// ============================================================================

/// Unified trait for overlay network implementations
///
/// This trait defines the common interface for all overlay network transports.
/// Implementations include Tor, libp2p, WireGuard, Veilid, and I2P.
pub trait OverlayNetwork: Send + Sync {
    /// Get the transport type for this implementation
    fn transport_type(&self) -> OverlayTransportType;

    /// Create a new tunnel with the given configuration
    fn create_tunnel(&self, config: TunnelConfig) -> impl std::future::Future<Output = OverlayResult<TunnelInfo>> + Send;

    /// Delete an existing tunnel
    fn delete_tunnel(&self, tunnel_id: &str) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Get information about a specific tunnel
    fn get_tunnel(&self, tunnel_id: &str) -> impl std::future::Future<Output = OverlayResult<TunnelInfo>> + Send;

    /// List all active tunnels
    fn list_tunnels(&self) -> impl std::future::Future<Output = OverlayResult<Vec<TunnelInfo>>> + Send;

    /// List known peers in the network
    fn list_peers(&self) -> impl std::future::Future<Output = OverlayResult<Vec<PeerInfo>>> + Send;

    /// Connect to a specific peer
    fn connect_peer(&self, peer_id: &str) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Disconnect from a specific peer
    fn disconnect_peer(&self, peer_id: &str) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Advertise a service on the overlay network
    fn advertise_service(&self, service: ServiceAdvertisement) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Remove a service advertisement
    fn unadvertise_service(&self, service_name: &str) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Get metrics for this overlay network
    fn get_metrics(&self) -> impl std::future::Future<Output = OverlayResult<OverlayMetrics>> + Send;

    /// Start the overlay network service
    fn start(&self) -> impl std::future::Future<Output = OverlayResult<()>> + Send;

    /// Stop the overlay network service
    fn stop(&self) -> impl std::future::Future<Output = OverlayResult<()>> + Send;
}

// ============================================================================
// OverlayNetworkV2 FFI Interface
// ============================================================================

/// FFI function table for OverlayNetworkV2 service
#[repr(C)]
pub struct OverlayNetworkV2 {
    /// User data pointer passed to all callbacks
    pub user_data: *mut c_void,

    /// Create a new tunnel
    pub create_tunnel: extern "C" fn(
        user_data: *mut c_void,
        config_json: *const c_char,
        result: *mut OverlayStringResult,
    ),

    /// Delete an existing tunnel
    pub delete_tunnel: extern "C" fn(
        user_data: *mut c_void,
        tunnel_id: *const c_char,
        result: *mut OverlayStringResult,
    ),

    /// List all tunnels
    pub list_tunnels: extern "C" fn(
        user_data: *mut c_void,
        result: *mut OverlayTunnelListResult,
    ),

    /// List all peers
    pub list_peers: extern "C" fn(
        user_data: *mut c_void,
        result: *mut OverlayPeerListResult,
    ),

    /// Connect to a peer
    pub connect_peer: extern "C" fn(
        user_data: *mut c_void,
        peer_id: *const c_char,
        result: *mut OverlayStringResult,
    ),

    /// Disconnect from a peer
    pub disconnect_peer: extern "C" fn(
        user_data: *mut c_void,
        peer_id: *const c_char,
        result: *mut OverlayStringResult,
    ),

    /// Advertise a service
    pub advertise_service: extern "C" fn(
        user_data: *mut c_void,
        service_json: *const c_char,
        result: *mut OverlayStringResult,
    ),

    /// Get overlay metrics
    pub get_metrics: extern "C" fn(
        user_data: *mut c_void,
        result: *mut OverlayMetricsFFI,
    ),

    /// Free a string returned by the overlay
    pub free_string: extern "C" fn(user_data: *mut c_void, ptr: *mut c_char),

    /// Free a tunnel list
    pub free_tunnel_list: extern "C" fn(user_data: *mut c_void, list: *mut OverlayTunnelListResult),

    /// Free a peer list
    pub free_peer_list: extern "C" fn(user_data: *mut c_void, list: *mut OverlayPeerListResult),
}

// ============================================================================
// Helper Functions
// ============================================================================

impl OverlayTransportType {
    /// Get a string representation of the transport type
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Libp2p => "libp2p",
            Self::Tor => "tor",
            Self::I2p => "i2p",
            Self::WireGuard => "wireguard",
            Self::Veilid => "veilid",
            Self::Custom => "custom",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "libp2p" => Some(Self::Libp2p),
            "tor" => Some(Self::Tor),
            "i2p" => Some(Self::I2p),
            "wireguard" | "wire guard" => Some(Self::WireGuard),
            "veilid" => Some(Self::Veilid),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

impl TunnelStatus {
    /// Get a string representation of the tunnel status
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Active => "active",
            Self::Disconnecting => "disconnecting",
            Self::Inactive => "inactive",
            Self::Error => "error",
        }
    }

    /// Check if the tunnel is operational
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl TunnelInfo {
    /// Calculate the total bytes transferred through this tunnel
    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }
}

impl OverlayMetrics {
    /// Calculate the throughput in bytes per second
    pub fn throughput_bps(&self) -> f64 {
        if self.uptime_secs == 0 {
            return 0.0;
        }
        (self.total_bytes_sent + self.total_bytes_received) as f64 / self.uptime_secs as f64
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_str_roundtrip() {
        let types = [
            OverlayTransportType::Libp2p,
            OverlayTransportType::Tor,
            OverlayTransportType::I2p,
            OverlayTransportType::WireGuard,
            OverlayTransportType::Veilid,
            OverlayTransportType::Custom,
        ];

        for t in types {
            let s = t.as_str();
            let parsed = OverlayTransportType::from_str(s);
            assert_eq!(parsed, Some(t));
        }
    }

    #[test]
    fn test_tunnel_config_defaults() {
        let config = TunnelConfig::default();
        assert!(!config.tunnel_id.is_empty());
        assert_eq!(config.transport, OverlayTransportType::Libp2p);
        assert_eq!(config.mtu, Some(1420));
        assert_eq!(config.keepalive_secs, Some(25));
    }

    #[test]
    fn test_tunnel_status_operational() {
        assert!(TunnelStatus::Active.is_operational());
        assert!(!TunnelStatus::Connecting.is_operational());
        assert!(!TunnelStatus::Error.is_operational());
    }

    #[test]
    fn test_overlay_result() {
        let ok_result: OverlayResult<i32> = OverlayResult::ok(42);
        assert!(ok_result.success);
        assert_eq!(ok_result.data, Some(42));
        assert!(ok_result.error.is_none());

        let err_result: OverlayResult<i32> = OverlayResult::err("test error");
        assert!(!err_result.success);
        assert!(err_result.data.is_none());
        assert_eq!(err_result.error, Some("test error".to_string()));
    }

    #[test]
    fn test_overlay_metrics_throughput() {
        let metrics = OverlayMetrics {
            total_bytes_sent: 1000,
            total_bytes_received: 1000,
            uptime_secs: 10,
            ..Default::default()
        };
        assert_eq!(metrics.throughput_bps(), 200.0);
    }

    #[test]
    fn test_tunnel_info_total_bytes() {
        let info = TunnelInfo {
            tunnel_id: "test".to_string(),
            transport: OverlayTransportType::Libp2p,
            status: TunnelStatus::Active,
            local_address: "0.0.0.0:8080".to_string(),
            remote_address: None,
            allocated_cidr: None,
            bytes_sent: 500,
            bytes_received: 300,
            created_at: 0,
            last_activity: None,
        };
        assert_eq!(info.total_bytes(), 800);
    }

    #[test]
    fn test_service_advertisement() {
        let ad = ServiceAdvertisement {
            service_name: "my-service".to_string(),
            service_type: "http".to_string(),
            transport: OverlayTransportType::Tor,
            port: 8080,
            overlay_address: "xyz123.onion".to_string(),
            metadata: HashMap::new(),
            ttl_secs: 3600,
        };

        assert_eq!(ad.service_name, "my-service");
        assert_eq!(ad.transport, OverlayTransportType::Tor);
        assert_eq!(ad.ttl_secs, 3600);
    }
}
