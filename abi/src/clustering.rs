// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RFC-0004 Phase 6.3.2: Service Registry Clustering
//!
//! This module implements distributed service registry with:
//! - Multi-node service clustering
//! - Eventual consistency with conflict resolution
//! - Health checking and automatic recovery
//! - Cross-node synchronization

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing;

/// Health status of a service node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Node is healthy and responding
    Healthy = 0,
    /// Node is unhealthy/unreachable
    Unhealthy = 1,
    /// Node status unknown
    Unknown = 2,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Unhealthy => write!(f, "Unhealthy"),
            HealthStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a single node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceNode {
    /// Unique node identifier
    pub node_id: String,
    /// Network address (IP or hostname)
    pub address: String,
    /// Port for service communication
    pub port: u16,
    /// Current health status
    pub health_status: HealthStatus,
    /// Last heartbeat timestamp (seconds since epoch)
    pub last_heartbeat: u64,
    /// Replication lag in milliseconds
    pub replication_lag_ms: u64,
}

impl ServiceNode {
    /// Create a new service node
    pub fn new(node_id: impl Into<String>, address: impl Into<String>, port: u16) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            node_id: node_id.into(),
            address: address.into(),
            port,
            health_status: HealthStatus::Unknown,
            last_heartbeat: now.as_secs(),
            replication_lag_ms: 0,
        }
    }

    /// Update heartbeat timestamp
    pub fn update_heartbeat(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        self.last_heartbeat = now.as_secs();
        self.health_status = HealthStatus::Healthy;
    }

    /// Check if node is considered unhealthy (no heartbeat in 30 seconds)
    pub fn is_timed_out(&self, timeout_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        (now.as_secs() - self.last_heartbeat) > timeout_secs
    }
}

/// Service information to be stored and replicated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterService {
    /// Unique service identifier
    pub service_id: String,
    /// Service name
    pub name: String,
    /// Service version
    pub version: String,
    /// Node where service is running
    pub node_id: String,
    /// Network address of service
    pub address: String,
    /// Service port
    pub port: u16,
    /// Metadata for the service
    pub metadata: HashMap<String, String>,
    /// Timestamp when registered (seconds since epoch)
    pub registered_at: u64,
    /// Version vector for conflict detection
    pub version_vector: HashMap<String, u64>,
}

impl ClusterService {
    /// Create a new cluster service
    pub fn new(
        service_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        node_id: impl Into<String>,
        address: impl Into<String>,
        port: u16,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            service_id: service_id.into(),
            name: name.into(),
            version: version.into(),
            node_id: node_id.into(),
            address: address.into(),
            port,
            metadata: HashMap::new(),
            registered_at: now.as_secs(),
            version_vector: HashMap::new(),
        }
    }

    /// Add metadata to the service
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Consensus type for cluster coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusType {
    /// Raft consensus (strong consistency)
    Raft = 0,
    /// Eventual consistency (last-write-wins)
    EventualConsistency = 1,
}

/// Service registry cluster for distributed deployments
pub struct ServiceCluster {
    /// Local node identifier
    pub local_node_id: String,
    /// All nodes in the cluster
    nodes: Arc<RwLock<HashMap<String, ServiceNode>>>,
    /// Registered services
    services: Arc<RwLock<HashMap<String, ClusterService>>>,
    /// Quorum size for consensus (if using Raft)
    pub quorum_size: usize,
    /// Consensus type
    pub consensus_type: ConsensusType,
    /// Health check timeout in seconds
    pub health_check_timeout_secs: u64,
}

impl ServiceCluster {
    /// Create a new service cluster
    pub fn new(
        local_node_id: impl Into<String>,
        consensus_type: ConsensusType,
        quorum_size: usize,
    ) -> Self {
        Self {
            local_node_id: local_node_id.into(),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(RwLock::new(HashMap::new())),
            quorum_size,
            consensus_type,
            health_check_timeout_secs: 30,
        }
    }

    /// Add a node to the cluster
    pub async fn add_node(&self, node: ServiceNode) -> Result<(), String> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    /// Register a service (distributed)
    pub async fn register_service(&self, service: ClusterService) -> Result<(), String> {
        // In production:
        // 1. Add to local service registry
        // 2. Serialize and broadcast to all nodes
        // 3. Apply version vector for conflict detection
        // 4. Handle consensus if using Raft

        let mut services = self.services.write().await;
        services.insert(service.service_id.clone(), service);
        Ok(())
    }

    /// Discover services by name
    pub async fn discover_services(&self, name: &str) -> Result<Vec<ClusterService>, String> {
        let services = self.services.read().await;
        let matching: Vec<ClusterService> = services
            .values()
            .filter(|s| s.name == name)
            .cloned()
            .collect();
        Ok(matching)
    }

    /// Sync services across nodes (eventual consistency)
    pub async fn sync_nodes(&self) -> Result<(), String> {
        // In production:
        // 1. For each node, get its current services
        // 2. Merge with local services using version vectors
        // 3. Detect conflicts (same service_id, different version vectors)
        // 4. Apply last-write-wins conflict resolution
        // 5. Broadcast merged state to all nodes

        let nodes = self.nodes.read().await;
        let conflicts: Vec<ServiceConflict> = Vec::new();

        for (node_id, _node) in nodes.iter() {
            // Placeholder: would sync with actual node
            // In production, use HTTP/gRPC to fetch remote services
            if node_id == &self.local_node_id {
                continue;
            }
        // TODO(#github-issue): Implement remote node sync via HTTP/gRPC
        // Currently only local node services are available. Remote sync requires:
        // - HTTP/gRPC client for fetching remote service registries
        // - Conflict resolution strategy (last-write-wins, CRDT, etc.)
        // - Authentication/authorization for cross-node communication
        }

        if !conflicts.is_empty() {
            tracing::error!("Detected {} service conflicts during sync", conflicts.len());
        }

        Ok(())
    }

    /// Detect conflicts between local and remote services
    pub async fn detect_conflicts(&self) -> Result<Vec<ServiceConflict>, String> {
        // Version vector comparison to find concurrent updates
        let _services = self.services.read().await;
        let conflicts: Vec<ServiceConflict> = Vec::new();

        // Placeholder: would compare version vectors across nodes
        // In production, this would track causal ordering of updates

        Ok(conflicts)
    }

    /// Check health of all nodes
    pub async fn health_check(&self) -> Result<(), String> {
        let mut nodes = self.nodes.write().await;

        for (_node_id, node) in nodes.iter_mut() {
            if node.is_timed_out(self.health_check_timeout_secs) {
                node.health_status = HealthStatus::Unhealthy;
            }
            // In production: would send actual heartbeat request to node
        }

        Ok(())
    }

    /// Get all healthy nodes
    pub async fn get_healthy_nodes(&self) -> Result<Vec<ServiceNode>, String> {
        let nodes = self.nodes.read().await;
        let healthy: Vec<ServiceNode> = nodes
            .values()
            .filter(|n| n.health_status == HealthStatus::Healthy)
            .cloned()
            .collect();
        Ok(healthy)
    }

    /// Get status of all nodes
    pub async fn get_cluster_status(&self) -> Result<ClusterStatus, String> {
        let nodes = self.nodes.read().await;
        let services = self.services.read().await;

        let healthy_count = nodes
            .values()
            .filter(|n| n.health_status == HealthStatus::Healthy)
            .count();

        Ok(ClusterStatus {
            total_nodes: nodes.len(),
            healthy_nodes: healthy_count,
            total_services: services.len(),
            consensus_type: self.consensus_type,
        })
    }

    /// Get a service by ID
    pub async fn get_service(&self, service_id: &str) -> Result<Option<ClusterService>, String> {
        let services = self.services.read().await;
        Ok(services.get(service_id).cloned())
    }

    /// List all services
    pub async fn list_services(&self) -> Result<Vec<ClusterService>, String> {
        let services = self.services.read().await;
        Ok(services.values().cloned().collect())
    }

    /// Deregister a service
    pub async fn deregister_service(&self, service_id: &str) -> Result<(), String> {
        let mut services = self.services.write().await;
        services.remove(service_id);
        Ok(())
    }

    /// Sync services from a remote node's service registry.
    ///
    /// This method merges remote services into the local registry using
    /// version vector comparison for conflict detection and last-write-wins
    /// for conflict resolution.
    pub async fn sync_from_remote_services(
        &self,
        _remote_node_id: &str,
        remote_services: Vec<ClusterService>,
    ) -> Result<(), String> {
        let mut services = self.services.write().await;

        for remote_service in remote_services {
            match services.get(&remote_service.service_id) {
                Some(local_service) => {
                    if Self::detect_version_conflict_static(
                        &local_service.version_vector,
                        &remote_service.version_vector,
                    )
                    .is_some()
                    {
                        let resolved = self.resolve_conflict_last_write_wins_internal(
                            local_service,
                            &remote_service,
                        );
                        services.insert(resolved.service_id.clone(), resolved);
                    }
                }
                None => {
                    services.insert(remote_service.service_id.clone(), remote_service);
                }
            }
        }

        Ok(())
    }

    /// Detect version conflict between local service and a remote service.
    ///
    /// Returns `Some(ServiceConflict)` if the version vectors are concurrent
    /// (neither dominates the other), indicating a true conflict.
    pub async fn detect_version_conflict(
        &self,
        remote_service: &ClusterService,
    ) -> Option<ServiceConflict> {
        let services = self.services.read().await;

        match services.get(&remote_service.service_id) {
            Some(local_service) => {
                Self::detect_version_conflict_static(
                    &local_service.version_vector,
                    &remote_service.version_vector,
                )
                .map(|(vector_a, vector_b)| ServiceConflict {
                    service_id: remote_service.service_id.clone(),
                    vector_a,
                    vector_b,
                })
            }
            None => None,
        }
    }

    fn detect_version_conflict_static(
        local_vector: &HashMap<String, u64>,
        remote_vector: &HashMap<String, u64>,
    ) -> Option<(HashMap<String, u64>, HashMap<String, u64>)> {
        let local_dominates = Self::version_vector_dominates(local_vector, remote_vector);
        let remote_dominates = Self::version_vector_dominates(remote_vector, local_vector);

        if !local_dominates && !remote_dominates && local_vector != remote_vector {
            Some((local_vector.clone(), remote_vector.clone()))
        } else {
            None
        }
    }

    /// Resolve conflict using last-write-wins strategy.
    ///
    /// Compares `registered_at` timestamps and returns the service with
    /// the later timestamp. If equal, remote service wins.
    pub fn resolve_conflict_last_write_wins(
        &self,
        local_service: &ClusterService,
        remote_service: &ClusterService,
        _conflict: &ServiceConflict,
    ) -> ClusterService {
        self.resolve_conflict_last_write_wins_internal(local_service, remote_service)
    }

    fn resolve_conflict_last_write_wins_internal(
        &self,
        local_service: &ClusterService,
        remote_service: &ClusterService,
    ) -> ClusterService {
        if remote_service.registered_at >= local_service.registered_at {
            remote_service.clone()
        } else {
            local_service.clone()
        }
    }

    /// Check if one version vector dominates another.
    ///
    /// Returns `true` if `vec_a` dominates `vec_b`, meaning:
    /// - For all keys, `vec_a[key] >= vec_b[key]`
    /// - For at least one key, `vec_a[key] > vec_b[key]`
    pub fn version_vector_dominates(
        vec_a: &HashMap<String, u64>,
        vec_b: &HashMap<String, u64>,
    ) -> bool {
        let all_keys: std::collections::HashSet<_> = vec_a.keys().chain(vec_b.keys()).collect();

        let mut a_greater_or_equal = true;
        let mut a_strictly_greater = false;

        for key in all_keys {
            let val_a = vec_a.get(key).copied().unwrap_or(0);
            let val_b = vec_b.get(key).copied().unwrap_or(0);

            if val_a < val_b {
                a_greater_or_equal = false;
                break;
            }
            if val_a > val_b {
                a_strictly_greater = true;
            }
        }

        a_greater_or_equal && a_strictly_greater
    }
}

/// Service conflict (version vector divergence)
#[derive(Debug, Clone)]
pub struct ServiceConflict {
    /// Service ID with conflict
    pub service_id: String,
    /// Version vector from node A
    pub vector_a: HashMap<String, u64>,
    /// Version vector from node B
    pub vector_b: HashMap<String, u64>,
}

/// Overall cluster health status
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    /// Total number of nodes
    pub total_nodes: usize,
    /// Number of healthy nodes
    pub healthy_nodes: usize,
    /// Total registered services
    pub total_services: usize,
    /// Consensus type being used
    pub consensus_type: ConsensusType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cluster_initialization() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        assert_eq!(cluster.local_node_id, "node-1");
        assert_eq!(cluster.quorum_size, 2);
    }

    #[tokio::test]
    async fn test_add_node() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let node = ServiceNode::new("node-2", "192.168.1.2", 8080);

        assert!(cluster.add_node(node).await.is_ok());
    }

    #[tokio::test]
    async fn test_register_service() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let service = ClusterService::new(
            "svc-1",
            "my-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );

        assert!(cluster.register_service(service).await.is_ok());
    }

    #[tokio::test]
    async fn test_discover_services() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let service = ClusterService::new(
            "svc-1",
            "my-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );

        cluster.register_service(service).await.unwrap();

        let found = cluster.discover_services("my-service").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].service_id, "svc-1");
    }

    #[tokio::test]
    async fn test_health_check() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let mut node = ServiceNode::new("node-2", "192.168.1.2", 8080);
        node.health_status = HealthStatus::Healthy;

        cluster.add_node(node).await.unwrap();
        assert!(cluster.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cluster_status() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let node = ServiceNode::new("node-2", "192.168.1.2", 8080);

        cluster.add_node(node).await.unwrap();

        let status = cluster.get_cluster_status().await.unwrap();
        assert_eq!(status.total_nodes, 1);
        assert_eq!(status.total_services, 0);
    }

    #[tokio::test]
    async fn test_service_deregistration() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let service = ClusterService::new(
            "svc-1",
            "my-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );

        cluster.register_service(service).await.unwrap();
        cluster.deregister_service("svc-1").await.unwrap();

        let services = cluster.list_services().await.unwrap();
        assert_eq!(services.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_remote_node_services() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);
        let remote_node = ServiceNode::new("node-2", "192.168.1.2", 8080);
        cluster.add_node(remote_node).await.unwrap();

        let mut remote_service = ClusterService::new(
            "svc-remote-1",
            "remote-service",
            "1.0.0",
            "node-2",
            "192.168.1.2",
            8080,
        );
        remote_service.version_vector.insert("node-2".to_string(), 1);

        let remote_services = vec![remote_service.clone()];

        let result = cluster.sync_from_remote_services("node-2", remote_services).await;
        assert!(result.is_ok());

        let services = cluster.list_services().await.unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].service_id, "svc-remote-1");
    }

    #[tokio::test]
    async fn test_detect_version_conflicts() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        let mut local_service = ClusterService::new(
            "svc-conflict",
            "conflict-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );
        local_service.version_vector.insert("node-1".to_string(), 2);
        local_service.version_vector.insert("node-2".to_string(), 1);

        let mut remote_service = local_service.clone();
        remote_service.node_id = "node-2".to_string();
        remote_service.version_vector.insert("node-1".to_string(), 1);
        remote_service.version_vector.insert("node-2".to_string(), 2);

        cluster.register_service(local_service).await.unwrap();

        let conflict = cluster.detect_version_conflict(&remote_service).await;
        assert!(conflict.is_some());
        let conflict = conflict.unwrap();
        assert_eq!(conflict.service_id, "svc-conflict");
    }

    #[tokio::test]
    async fn test_resolve_conflict_last_write_wins() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        let mut local_service = ClusterService::new(
            "svc-lww",
            "lww-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );
        local_service.registered_at = 1000;
        local_service.version_vector.insert("node-1".to_string(), 1);

        let mut remote_service = ClusterService::new(
            "svc-lww",
            "lww-service",
            "2.0.0",
            "node-2",
            "192.168.1.2",
            8080,
        );
        remote_service.registered_at = 2000;
        remote_service.version_vector.insert("node-2".to_string(), 1);

        let conflict = ServiceConflict {
            service_id: "svc-lww".to_string(),
            vector_a: local_service.version_vector.clone(),
            vector_b: remote_service.version_vector.clone(),
        };

        let resolved = cluster.resolve_conflict_last_write_wins(&local_service, &remote_service, &conflict);
        assert_eq!(resolved.version, "2.0.0");
        assert_eq!(resolved.node_id, "node-2");
    }

    #[tokio::test]
    async fn test_sync_with_conflict_resolution() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        let mut local_service = ClusterService::new(
            "svc-sync",
            "sync-service",
            "1.0.0",
            "node-1",
            "192.168.1.1",
            8080,
        );
        local_service.registered_at = 1000;
        local_service.version_vector.insert("node-1".to_string(), 1);

        cluster.register_service(local_service.clone()).await.unwrap();

        let mut remote_service = ClusterService::new(
            "svc-sync",
            "sync-service",
            "2.0.0",
            "node-2",
            "192.168.1.2",
            8080,
        );
        remote_service.registered_at = 2000;
        remote_service.version_vector.insert("node-2".to_string(), 1);

        let result = cluster.sync_from_remote_services("node-2", vec![remote_service]).await;
        assert!(result.is_ok());

        let service = cluster.get_service("svc-sync").await.unwrap().unwrap();
        assert_eq!(service.version, "2.0.0");
    }

    #[tokio::test]
    async fn test_version_vector_comparison() {
        let mut vec_a: HashMap<String, u64> = HashMap::new();
        vec_a.insert("node-1".to_string(), 2);
        vec_a.insert("node-2".to_string(), 1);

        let mut vec_b: HashMap<String, u64> = HashMap::new();
        vec_b.insert("node-1".to_string(), 1);
        vec_b.insert("node-2".to_string(), 2);

        assert!(!ServiceCluster::version_vector_dominates(&vec_a, &vec_b));
        assert!(!ServiceCluster::version_vector_dominates(&vec_b, &vec_a));

        let mut vec_c: HashMap<String, u64> = HashMap::new();
        vec_c.insert("node-1".to_string(), 3);
        vec_c.insert("node-2".to_string(), 2);

        assert!(ServiceCluster::version_vector_dominates(&vec_c, &vec_a));
    }
}
