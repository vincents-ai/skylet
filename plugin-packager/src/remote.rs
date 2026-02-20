// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Remote registry integration for plugin-packager
//!
//! This module provides integration with the marketplace registry system,
//! allowing plugins to be discovered and managed from remote registries
//! while maintaining compatibility with the local registry.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::registry::{LocalRegistry, PluginRegistryEntry};

/// Remote registry client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRegistryConfig {
    /// Registry URL (e.g., "https://registry.example.com")
    pub url: String,

    /// Authentication token (optional)
    pub token: Option<String>,

    /// Request timeout duration
    #[serde(default = "RemoteRegistryConfig::default_timeout")]
    pub timeout_secs: u64,

    /// Cache expiration time
    #[serde(default = "RemoteRegistryConfig::default_cache_ttl")]
    pub cache_ttl_secs: u64,

    /// Whether to verify SSL certificates
    #[serde(default = "RemoteRegistryConfig::default_verify_ssl")]
    pub verify_ssl: bool,
}

impl RemoteRegistryConfig {
    fn default_timeout() -> u64 {
        30
    }

    fn default_cache_ttl() -> u64 {
        3600 // 1 hour
    }

    fn default_verify_ssl() -> bool {
        true
    }

    /// Create configuration with defaults
    pub fn new(url: String) -> Self {
        Self {
            url,
            token: None,
            timeout_secs: Self::default_timeout(),
            cache_ttl_secs: Self::default_cache_ttl(),
            verify_ssl: Self::default_verify_ssl(),
        }
    }

    /// Set authentication token
    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set cache TTL
    pub fn with_cache_ttl(mut self, secs: u64) -> Self {
        self.cache_ttl_secs = secs;
        self
    }
}

/// Remote registry client
pub struct RemoteRegistry {
    config: RemoteRegistryConfig,
    cache: HashMap<String, CachedEntry>,
}

/// Cached remote registry entry
#[derive(Clone, Debug)]
struct CachedEntry {
    /// Cached plugin data
    entry: PluginRegistryEntry,
    /// Cache timestamp
    cached_at: SystemTime,
}

impl CachedEntry {
    /// Check if cache entry has expired
    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed().unwrap_or(ttl) > ttl
    }
}

impl RemoteRegistry {
    /// Create new remote registry client
    pub fn new(config: RemoteRegistryConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Search remote registry for plugins
    pub fn search(&mut self, _query: &str, _limit: usize) -> Result<Vec<PluginRegistryEntry>> {
        // In a real implementation, this would make HTTP requests to the remote registry
        // For now, this is a placeholder that demonstrates the interface

        // In production, this would:
        // 1. Check cache first
        // 2. Make HTTP GET request to {registry_url}/api/search?q={query}&limit={limit}
        // 3. Parse JSON response
        // 4. Cache results with TTL
        // 5. Return results

        // This is designed to be compatible with the marketplace-registry API
        Ok(Vec::new())
    }

    /// Get plugin from remote registry
    pub fn get_plugin(&mut self, name: &str, version: Option<&str>) -> Result<PluginRegistryEntry> {
        // Check cache first
        let ttl = Duration::from_secs(self.config.cache_ttl_secs);
        let cache_key = if let Some(v) = version {
            format!("{}:{}", name, v)
        } else {
            name.to_string()
        };

        if let Some(cached) = self.cache.get(&cache_key) {
            if !cached.is_expired(ttl) {
                return Ok(cached.entry.clone());
            }
        }

        // In production, would make HTTP request here
        // GET {registry_url}/api/plugins/{name}/{version}
        // Parse response and cache it

        Err(anyhow::anyhow!(
            "Plugin {}:{} not found in remote registry",
            name,
            version.unwrap_or("latest")
        ))
    }

    /// Fetch latest version of a plugin
    pub fn get_latest(&mut self, name: &str) -> Result<PluginRegistryEntry> {
        self.get_plugin(name, None)
    }

    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Prune expired entries from cache
    pub fn prune_cache(&mut self) {
        let ttl = Duration::from_secs(self.config.cache_ttl_secs);
        self.cache.retain(|_, entry| !entry.is_expired(ttl));
    }
}

/// Combined local and remote registry client
pub struct HybridRegistry {
    local: LocalRegistry,
    remote: Option<RemoteRegistry>,
    /// Whether to check local registry first (true) or remote first (false)
    local_first: bool,
}

impl HybridRegistry {
    /// Create hybrid registry with only local
    pub fn local_only(local: LocalRegistry) -> Self {
        Self {
            local,
            remote: None,
            local_first: true,
        }
    }

    /// Create hybrid registry with both local and remote
    pub fn with_remote(local: LocalRegistry, remote: RemoteRegistry) -> Self {
        Self {
            local,
            remote: Some(remote),
            local_first: true,
        }
    }

    /// Set search priority (true: local first, false: remote first)
    pub fn set_local_first(mut self, local_first: bool) -> Self {
        self.local_first = local_first;
        self
    }

    /// Search for plugin in registry (local first by default)
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<PluginRegistryEntry>> {
        if self.local_first {
            // Search local first
            let local_results: Vec<_> = self.local.search(query).into_iter().take(limit).collect();

            if !local_results.is_empty() {
                return Ok(local_results);
            }

            // Fall back to remote if available
            if let Some(remote) = &mut self.remote {
                return remote.search(query, limit);
            }

            Ok(Vec::new())
        } else {
            // Search remote first
            if let Some(remote) = &mut self.remote {
                let remote_results = remote.search(query, limit)?;
                if !remote_results.is_empty() {
                    return Ok(remote_results);
                }
            }

            // Fall back to local
            let local_results: Vec<_> = self.local.search(query).into_iter().take(limit).collect();
            Ok(local_results)
        }
    }

    /// Get plugin from registry
    pub fn get(&mut self, name: &str, version: Option<&str>) -> Result<PluginRegistryEntry> {
        if self.local_first {
            // Try local first
            if let Some(v) = version {
                if let Some(entry) = self.local.find_by_version(name, v) {
                    return Ok(entry);
                }
            } else if let Some(entry) = self.local.find_by_name(name) {
                return Ok(entry);
            }

            // Try remote
            if let Some(remote) = &mut self.remote {
                return remote.get_plugin(name, version);
            }

            Err(anyhow::anyhow!("Plugin {} not found", name))
        } else {
            // Try remote first
            if let Some(remote) = &mut self.remote {
                if let Ok(entry) = remote.get_plugin(name, version) {
                    return Ok(entry);
                }
            }

            // Try local
            if let Some(v) = version {
                if let Some(entry) = self.local.find_by_version(name, v) {
                    return Ok(entry);
                }
            } else if let Some(entry) = self.local.find_by_name(name) {
                return Ok(entry);
            }

            Err(anyhow::anyhow!("Plugin {} not found", name))
        }
    }

    /// Register plugin in local registry
    pub fn register_local(&mut self, entry: PluginRegistryEntry) -> Result<()> {
        self.local.register(entry)
    }

    /// Get local registry reference
    pub fn local_registry(&self) -> &LocalRegistry {
        &self.local
    }

    /// Sync plugins from remote to local cache
    pub fn sync_from_remote(&mut self, plugins: Vec<&str>) -> Result<usize> {
        let mut synced = 0;

        if let Some(remote) = &mut self.remote {
            for plugin_name in plugins {
                if let Ok(entry) = remote.get_plugin(plugin_name, None) {
                    let _ = self.local.register(entry);
                    synced += 1;
                }
            }
        }

        Ok(synced)
    }

    /// Prune remote cache
    pub fn prune_remote_cache(&mut self) {
        if let Some(remote) = &mut self.remote {
            remote.prune_cache();
        }
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let remote_cache_size = self.remote.as_ref().map(|r| r.cache_size()).unwrap_or(0);

        CacheStats {
            local_plugins: self.local.count(),
            remote_cache_size,
            local_first: self.local_first,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub local_plugins: usize,
    pub remote_cache_size: usize,
    pub local_first: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_registry_config() {
        let config = RemoteRegistryConfig::new("https://registry.example.com".to_string());
        assert_eq!(config.url, "https://registry.example.com");
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.cache_ttl_secs, 3600);
        assert!(config.verify_ssl);
    }

    #[test]
    fn test_remote_registry_config_builder() {
        let config = RemoteRegistryConfig::new("https://api.example.com".to_string())
            .with_token("secret-token".to_string())
            .with_timeout(60)
            .with_cache_ttl(7200);

        assert_eq!(config.url, "https://api.example.com");
        assert_eq!(config.token, Some("secret-token".to_string()));
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.cache_ttl_secs, 7200);
    }

    #[test]
    fn test_hybrid_registry_local_only() {
        let local = LocalRegistry::new();
        let hybrid = HybridRegistry::local_only(local);

        assert!(hybrid.remote.is_none());
        assert!(hybrid.local_first);
    }

    #[test]
    fn test_hybrid_registry_with_remote() {
        let local = LocalRegistry::new();
        let config = RemoteRegistryConfig::new("https://example.com".to_string());
        let remote = RemoteRegistry::new(config);
        let hybrid = HybridRegistry::with_remote(local, remote);

        assert!(hybrid.remote.is_some());
        assert!(hybrid.local_first);
    }

    #[test]
    fn test_hybrid_registry_priority() {
        let local = LocalRegistry::new();
        let config = RemoteRegistryConfig::new("https://example.com".to_string());
        let remote = RemoteRegistry::new(config);

        let hybrid = HybridRegistry::with_remote(local, remote).set_local_first(false);
        assert!(!hybrid.local_first);
    }

    #[test]
    fn test_cached_entry_expiration() {
        let entry = PluginRegistryEntry {
            plugin_id: "test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };

        let cached = CachedEntry {
            entry,
            cached_at: SystemTime::now(),
        };

        // Should not be expired with generous TTL
        let generous_ttl = Duration::from_secs(3600);
        assert!(!cached.is_expired(generous_ttl));

        // Should be expired with very short TTL
        let short_ttl = Duration::from_secs(0);
        assert!(cached.is_expired(short_ttl));
    }

    #[test]
    fn test_hybrid_registry_search_local() -> Result<()> {
        let mut local = LocalRegistry::new();
        let entry = PluginRegistryEntry {
            plugin_id: "test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: Some("Test plugin".to_string()),
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };
        local.register(entry)?;

        let mut hybrid = HybridRegistry::local_only(local);
        let results = hybrid.search("test", 10)?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test");

        Ok(())
    }

    #[test]
    fn test_cache_stats() {
        let local = LocalRegistry::new();
        let config = RemoteRegistryConfig::new("https://example.com".to_string());
        let remote = RemoteRegistry::new(config);
        let hybrid = HybridRegistry::with_remote(local, remote);

        let stats = hybrid.cache_stats();
        assert_eq!(stats.local_plugins, 0);
        assert_eq!(stats.remote_cache_size, 0);
        assert!(stats.local_first);
    }
}
