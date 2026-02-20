// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Instance Management Abstraction - RFC-0100 (Phase 2.1)
//!
//! This module provides trait-based abstractions for instance identity and lifecycle
//! management, allowing the execution engine to work with different instance backends
//! without being tightly coupled to proprietary implementations.
//!
//! # Overview
//!
//! The `InstanceManager` trait defines a contract for:
//! - Instance identity and metadata
//! - Zone/cluster membership
//! - Peer discovery and communication
//! - Instance role and permissions
//!
//! # Default Implementation
//!
//! A `StandaloneInstanceManager` implementation is provided for single-instance
//! deployments without proprietary cluster or multi-instance management.
//!
//! # Feature-Gated Proprietary Support
//!
//! When the `proprietary` feature is enabled, implementations can be swapped to use
//! proprietary instance management systems like Skynet's zone-based hierarchy and
//! multi-instance orchestration.
//!
//! # Example
//!
//! ```no_run
//! # use skylet_abi::instance_management::InstanceManager;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let instance_manager = skylet_abi::instance_management::StandaloneInstanceManager::new("my-instance");
//!
//! let instance_id = instance_manager.instance_id();
//! println!("Instance: {}", instance_id);
//!
//! let zone = instance_manager.zone_id();
//! println!("Zone: {:?}", zone);
//!
//! let is_master = instance_manager.is_master();
//! println!("Is Master: {}", is_master);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Error type for instance management operations
#[derive(Error, Debug, Clone)]
pub enum InstanceManagementError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Zone not found: {0}")]
    ZoneNotFound(String),

    #[error("Operation not supported in standalone mode")]
    NotSupportedInStandaloneMode,

    #[error("Peer communication failed: {0}")]
    PeerCommunicationFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type InstanceManagementResult<T> = Result<T, InstanceManagementError>;

/// Information about a peer instance
#[derive(Debug, Clone)]
pub struct InstancePeerInfo {
    /// Unique identifier for this peer
    pub peer_id: String,

    /// Human-readable name for this peer
    pub peer_name: String,

    /// Zone this peer belongs to (if any)
    pub zone_id: Option<String>,

    /// Network address of the peer
    pub address: String,

    /// Port for peer communication
    pub port: u16,

    /// Whether this peer is currently reachable
    pub is_reachable: bool,

    /// Last seen timestamp (Unix timestamp)
    pub last_seen: i64,

    /// Custom metadata about the peer
    pub metadata: HashMap<String, String>,
}

/// Instance role determining permissions and capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    /// Master instance (can manage other instances)
    Master,

    /// Regular member instance
    Member,

    /// Observer instance (read-only)
    Observer,

    /// Replica instance (for failover)
    Replica,
}

/// Instance Manager trait for identity and metadata
///
/// Implementations of this trait manage instance identity, zone membership,
/// and peer discovery. Implementations must be thread-safe and typically
/// immutable after creation.
#[async_trait]
pub trait InstanceManager: Send + Sync {
    /// Get the unique identifier for this instance
    fn instance_id(&self) -> &str;

    /// Get the zone ID this instance belongs to (if any)
    ///
    /// Returns None for standalone instances or when not part of a zone
    fn zone_id(&self) -> Option<&str>;

    /// Get the region/region ID this instance belongs to (if any)
    fn region_id(&self) -> Option<&str>;

    /// Check if this is the master instance
    fn is_master(&self) -> bool;

    /// Get the role of this instance
    fn get_role(&self) -> InstanceRole;

    /// Get the human-readable name of this instance
    fn instance_name(&self) -> &str;

    /// Check if this instance is part of a cluster/zone
    fn is_clustered(&self) -> bool {
        self.zone_id().is_some()
    }

    /// Discover peers in the same zone or cluster
    ///
    /// # Returns
    /// A list of peer information for discoverable instances
    async fn discover_peers(&self) -> InstanceManagementResult<Vec<InstancePeerInfo>>;

    /// Get information about a specific peer
    ///
    /// # Arguments
    /// * `peer_id` - The ID of the peer to query
    async fn get_peer(&self, peer_id: &str) -> InstanceManagementResult<InstancePeerInfo>;

    /// Notify this instance that a peer has come online
    ///
    /// # Arguments
    /// * `peer_info` - Information about the peer that came online
    async fn peer_online(&self, peer_info: InstancePeerInfo) -> InstanceManagementResult<()>;

    /// Notify this instance that a peer has gone offline
    ///
    /// # Arguments
    /// * `peer_id` - ID of the peer that went offline
    async fn peer_offline(&self, peer_id: &str) -> InstanceManagementResult<()>;

    /// Get instance metadata
    fn get_metadata(&self) -> &HashMap<String, String>;

    /// Update instance metadata
    ///
    /// # Arguments
    /// * `key` - Metadata key
    /// * `value` - Metadata value
    fn set_metadata(&self, key: String, value: String) -> InstanceManagementResult<()>;

    /// Get instance capabilities/permissions
    fn get_capabilities(&self) -> Vec<String>;
}

/// Standalone instance manager for single-instance deployments
///
/// This implementation provides basic instance management for standalone
/// deployments without zone membership or peer discovery.
pub struct StandaloneInstanceManager {
    instance_id: String,
    instance_name: String,
    role: InstanceRole,
    metadata: std::sync::Arc<parking_lot::RwLock<HashMap<String, String>>>,
}

impl StandaloneInstanceManager {
    /// Create a new standalone instance manager
    ///
    /// # Arguments
    /// * `instance_id` - Unique identifier for this instance
    pub fn new(instance_id: impl Into<String>) -> Self {
        let id = instance_id.into();
        Self {
            instance_id: id.clone(),
            instance_name: format!("Instance {}", id),
            role: InstanceRole::Master,
            metadata: std::sync::Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Create a new standalone instance manager with a custom name
    pub fn with_name(instance_id: impl Into<String>, name: impl Into<String>) -> Self {
        let mut manager = Self::new(instance_id);
        manager.instance_name = name.into();
        manager
    }

    /// Set the role of this instance
    pub fn with_role(mut self, role: InstanceRole) -> Self {
        self.role = role;
        self
    }
}

#[async_trait]
impl InstanceManager for StandaloneInstanceManager {
    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn zone_id(&self) -> Option<&str> {
        None
    }

    fn region_id(&self) -> Option<&str> {
        None
    }

    fn is_master(&self) -> bool {
        matches!(self.role, InstanceRole::Master)
    }

    fn get_role(&self) -> InstanceRole {
        self.role
    }

    fn instance_name(&self) -> &str {
        &self.instance_name
    }

    async fn discover_peers(&self) -> InstanceManagementResult<Vec<InstancePeerInfo>> {
        // No peers in standalone mode
        Ok(vec![])
    }

    async fn get_peer(&self, peer_id: &str) -> InstanceManagementResult<InstancePeerInfo> {
        Err(InstanceManagementError::InstanceNotFound(
            peer_id.to_string(),
        ))
    }

    async fn peer_online(&self, _peer_info: InstancePeerInfo) -> InstanceManagementResult<()> {
        // No-op in standalone mode
        Ok(())
    }

    async fn peer_offline(&self, _peer_id: &str) -> InstanceManagementResult<()> {
        // No-op in standalone mode
        Ok(())
    }

    fn get_metadata(&self) -> &HashMap<String, String> {
        // This is a bit of a compromise - we return a reference to the locked data
        // In practice, callers should use set_metadata for updates
        unsafe { &*(&*self.metadata.read() as *const HashMap<String, String>) }
    }

    fn set_metadata(&self, key: String, value: String) -> InstanceManagementResult<()> {
        self.metadata.write().insert(key, value);
        Ok(())
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec![
            "plugin.load".to_string(),
            "plugin.unload".to_string(),
            "plugin.configure".to_string(),
            "plugin.query".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standalone_instance_manager() {
        let manager = StandaloneInstanceManager::new("test-instance");

        assert_eq!(manager.instance_id(), "test-instance");
        assert_eq!(manager.zone_id(), None);
        assert_eq!(manager.region_id(), None);
        assert!(manager.is_master());
        assert_eq!(manager.get_role(), InstanceRole::Master);
    }

    #[test]
    fn test_instance_manager_with_name() {
        let manager = StandaloneInstanceManager::with_name("test-id", "My Instance");
        assert_eq!(manager.instance_name(), "My Instance");
    }

    #[test]
    fn test_instance_manager_with_role() {
        let manager = StandaloneInstanceManager::new("test-id").with_role(InstanceRole::Observer);
        assert!(!manager.is_master());
        assert_eq!(manager.get_role(), InstanceRole::Observer);
    }

    #[tokio::test]
    async fn test_discover_peers_returns_empty() {
        let manager = StandaloneInstanceManager::new("test-id");
        let peers = manager.discover_peers().await.unwrap();
        assert!(peers.is_empty());
    }

    #[tokio::test]
    async fn test_get_peer_fails() {
        let manager = StandaloneInstanceManager::new("test-id");
        let result = manager.get_peer("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata_operations() {
        let manager = StandaloneInstanceManager::new("test-id");

        manager
            .set_metadata("key1".to_string(), "value1".to_string())
            .unwrap();
        assert_eq!(
            manager.get_metadata().get("key1"),
            Some(&"value1".to_string())
        );
    }

    #[test]
    fn test_capabilities() {
        let manager = StandaloneInstanceManager::new("test-id");
        let capabilities = manager.get_capabilities();

        assert!(!capabilities.is_empty());
        assert!(capabilities.contains(&"plugin.load".to_string()));
    }
}
