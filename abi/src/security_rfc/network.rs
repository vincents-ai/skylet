// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Network Access Control - RFC-0008
//!
//! Whitelist-based network access control for plugins.
//! Validates host patterns and port restrictions.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::capabilities::CapabilityStatus;

/// A host pattern for network access control
#[derive(Debug, Clone)]
pub struct HostPattern {
    /// The host pattern (supports wildcards)
    /// Examples: "api.github.com", "*.google.com", "localhost"
    pub host: String,
    /// Allowed ports (empty = all)
    pub ports: Vec<u16>,
    /// Allowed protocols (empty = all)
    pub protocols: Vec<String>,
    /// Status of this permission
    pub status: CapabilityStatus,
}

/// Network access enforcer
///
/// Maintains a registry of approved network patterns per plugin
/// and validates connection requests against them.
#[derive(Debug)]
pub struct NetworkEnforcer {
    /// Map of plugin_id -> list of approved host patterns
    permissions: Arc<RwLock<HashMap<String, Vec<HostPattern>>>>,
    /// Global allowed hosts (e.g., localhost for health checks)
    global_allowed: Vec<HostPattern>,
    /// Blocked hosts (never allowed)
    blocked_hosts: Vec<String>,
}

impl NetworkEnforcer {
    /// Create a new network enforcer
    pub fn new() -> Self {
        Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
            global_allowed: vec![
                // Allow localhost for internal communication
                HostPattern {
                    host: "localhost".to_string(),
                    ports: vec![],
                    protocols: vec![],
                    status: CapabilityStatus::AutoApproved,
                },
                HostPattern {
                    host: "127.0.0.1".to_string(),
                    ports: vec![],
                    protocols: vec![],
                    status: CapabilityStatus::AutoApproved,
                },
            ],
            blocked_hosts: vec![
                // Block metadata endpoints that could leak cloud credentials
                "169.254.169.254".to_string(), // AWS/GCP/Azure metadata
                "metadata.google.internal".to_string(), // GCP metadata
                "metadata.azure.internal".to_string(), // Azure metadata
            ],
        }
    }

    /// Register host patterns for a plugin
    pub fn register_permissions(&self, plugin_id: &str, patterns: Vec<HostPattern>) {
        let mut perms = self.permissions.write().unwrap();
        perms.insert(plugin_id.to_string(), patterns);
    }

    /// Add a single host pattern for a plugin
    pub fn add_permission(&self, plugin_id: &str, pattern: HostPattern) {
        let mut perms = self.permissions.write().unwrap();
        perms
            .entry(plugin_id.to_string())
            .or_default()
            .push(pattern);
    }

    /// Remove all permissions for a plugin
    pub fn remove_plugin(&self, plugin_id: &str) {
        let mut perms = self.permissions.write().unwrap();
        perms.remove(plugin_id);
    }

    /// Check if a plugin can access a host:port
    pub fn check_access(
        &self,
        plugin_id: &str,
        host: &str,
        port: u16,
        protocol: &str,
    ) -> Result<(), NetworkAccessError> {
        // First, check blocked hosts
        for blocked in &self.blocked_hosts {
            if host_matches_pattern(host, blocked) {
                return Err(NetworkAccessError::BlockedHost(host.to_string()));
            }
        }

        // Check global allowed hosts
        for pattern in &self.global_allowed {
            if host_matches_pattern(host, &pattern.host)
                && (pattern.ports.is_empty() || pattern.ports.contains(&port))
                && (pattern.protocols.is_empty() || pattern.protocols.iter().any(|p| p == protocol))
            {
                return Ok(());
            }
        }

        // Check plugin-specific permissions
        let perms = self.permissions.read().unwrap();

        if let Some(plugin_perms) = perms.get(plugin_id) {
            for pattern in plugin_perms {
                // Skip non-approved patterns
                if pattern.status != CapabilityStatus::Approved
                    && pattern.status != CapabilityStatus::AutoApproved
                {
                    continue;
                }

                // Check host match
                if !host_matches_pattern(host, &pattern.host) {
                    continue;
                }

                // Check port restriction
                if !pattern.ports.is_empty() && !pattern.ports.contains(&port) {
                    continue;
                }

                // Check protocol restriction
                if !pattern.protocols.is_empty()
                    && !pattern
                        .protocols
                        .iter()
                        .any(|p| p.eq_ignore_ascii_case(protocol))
                {
                    continue;
                }

                return Ok(());
            }
        }

        Err(NetworkAccessError::PermissionDenied {
            plugin_id: plugin_id.to_string(),
            host: host.to_string(),
            port,
            protocol: protocol.to_string(),
        })
    }

    /// Get all permissions for a plugin
    pub fn get_permissions(&self, plugin_id: &str) -> Option<Vec<HostPattern>> {
        let perms = self.permissions.read().unwrap();
        perms.get(plugin_id).cloned()
    }

    /// Add a blocked host
    pub fn add_blocked_host(&mut self, host: &str) {
        self.blocked_hosts.push(host.to_string());
    }

    /// Add a global allowed host
    pub fn add_global_allowed(&mut self, pattern: HostPattern) {
        self.global_allowed.push(pattern);
    }

    /// List all blocked hosts
    pub fn list_blocked_hosts(&self) -> &[String] {
        &self.blocked_hosts
    }
}

impl Default for NetworkEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a host matches a pattern
///
/// Supports:
/// - Exact match: "api.github.com"
/// - Wildcard subdomain: "*.google.com"
/// - Full wildcard: "*"
pub fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    // Normalize to lowercase for comparison
    let host = host.to_lowercase();
    let pattern = pattern.to_lowercase();

    // Exact match
    if host == pattern {
        return true;
    }

    // Wildcard subdomain match (*.example.com)
    if let Some(suffix) = pattern.strip_prefix('*') {
        // suffix is ".example.com", pattern[2..] would be "example.com"
        let base_domain = &pattern[2..];
        return host.ends_with(suffix) || host == base_domain; // matches sub.example.com or example.com
    }

    false
}

/// Network access errors
#[derive(Debug, Clone)]
pub enum NetworkAccessError {
    /// Permission denied for plugin
    PermissionDenied {
        plugin_id: String,
        host: String,
        port: u16,
        protocol: String,
    },
    /// Host is blocked
    BlockedHost(String),
    /// Invalid host pattern
    InvalidPattern(String),
}

impl std::fmt::Display for NetworkAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkAccessError::PermissionDenied {
                plugin_id,
                host,
                port,
                protocol,
            } => {
                write!(
                    f,
                    "Permission denied for plugin {} to connect to {}://{}:{}",
                    plugin_id, protocol, host, port
                )
            }
            NetworkAccessError::BlockedHost(host) => write!(f, "Host is blocked: {}", host),
            NetworkAccessError::InvalidPattern(pattern) => {
                write!(f, "Invalid host pattern: {}", pattern)
            }
        }
    }
}

impl std::error::Error for NetworkAccessError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_enforcer_new() {
        let enforcer = NetworkEnforcer::new();
        assert!(!enforcer.blocked_hosts.is_empty());
        assert!(!enforcer.global_allowed.is_empty());
    }

    #[test]
    fn test_host_matches_pattern() {
        // Exact match
        assert!(host_matches_pattern("api.github.com", "api.github.com"));
        assert!(!host_matches_pattern("api.github.com", "github.com"));

        // Wildcard match
        assert!(host_matches_pattern("www.google.com", "*.google.com"));
        assert!(host_matches_pattern("mail.google.com", "*.google.com"));
        assert!(host_matches_pattern("google.com", "*.google.com"));
        assert!(!host_matches_pattern("google.evil.com", "*.google.com"));

        // Full wildcard
        assert!(host_matches_pattern("any.host.com", "*"));
    }

    #[test]
    fn test_check_access_localhost() {
        let enforcer = NetworkEnforcer::new();

        // Localhost should be globally allowed
        assert!(enforcer
            .check_access("any-plugin", "localhost", 8080, "tcp")
            .is_ok());
        assert!(enforcer
            .check_access("any-plugin", "127.0.0.1", 3000, "tcp")
            .is_ok());
    }

    #[test]
    fn test_check_access_blocked() {
        let enforcer = NetworkEnforcer::new();

        // Metadata endpoints should be blocked
        assert!(enforcer
            .check_access("any-plugin", "169.254.169.254", 80, "tcp")
            .is_err());
        assert!(enforcer
            .check_access("any-plugin", "metadata.google.internal", 80, "tcp")
            .is_err());
    }

    #[test]
    fn test_check_access_with_permission() {
        let enforcer = NetworkEnforcer::new();

        enforcer.add_permission(
            "github-plugin",
            HostPattern {
                host: "api.github.com".to_string(),
                ports: vec![443],
                protocols: vec!["https".to_string()],
                status: CapabilityStatus::Approved,
            },
        );

        // Should allow approved access
        assert!(enforcer
            .check_access("github-plugin", "api.github.com", 443, "https")
            .is_ok());

        // Should deny wrong port
        assert!(enforcer
            .check_access("github-plugin", "api.github.com", 80, "https")
            .is_err());

        // Should deny wrong host
        assert!(enforcer
            .check_access("github-plugin", "github.com", 443, "https")
            .is_err());
    }

    #[test]
    fn test_check_access_wildcard() {
        let enforcer = NetworkEnforcer::new();

        enforcer.add_permission(
            "google-plugin",
            HostPattern {
                host: "*.google.com".to_string(),
                ports: vec![443],
                protocols: vec!["https".to_string()],
                status: CapabilityStatus::Approved,
            },
        );

        // Should allow any subdomain
        assert!(enforcer
            .check_access("google-plugin", "www.google.com", 443, "https")
            .is_ok());
        assert!(enforcer
            .check_access("google-plugin", "api.google.com", 443, "https")
            .is_ok());
        assert!(enforcer
            .check_access("google-plugin", "google.com", 443, "https")
            .is_ok());
    }
}
