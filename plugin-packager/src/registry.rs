// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Registry integration for plugin-packager
//!
//! This module provides integration with the marketplace registry system,
//! allowing plugins to be registered, discovered, and managed through the registry.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Plugin metadata for registry registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistryEntry {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub abi_version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub dependencies: Option<Vec<PluginDependency>>,
}

/// Plugin dependency for registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub name: String,
    pub version_requirement: String,
}

/// In-memory plugin registry for local installations
pub struct LocalRegistry {
    plugins: HashMap<String, PluginRegistryEntry>,
}

impl LocalRegistry {
    /// Create a new local registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin in the local registry
    pub fn register(&mut self, entry: PluginRegistryEntry) -> Result<()> {
        let key = format!("{}:{}", entry.name, entry.version);
        self.plugins.insert(key, entry);
        Ok(())
    }

    /// Find a plugin by name
    pub fn find_by_name(&self, name: &str) -> Option<PluginRegistryEntry> {
        self.plugins.values().find(|p| p.name == name).cloned()
    }

    /// Find a plugin by name and version
    pub fn find_by_version(&self, name: &str, version: &str) -> Option<PluginRegistryEntry> {
        let key = format!("{}:{}", name, version);
        self.plugins.get(&key).cloned()
    }

    /// List all registered plugins
    pub fn list_all(&self) -> Vec<PluginRegistryEntry> {
        self.plugins.values().cloned().collect()
    }

    /// Search for plugins by keyword
    pub fn search(&self, query: &str) -> Vec<PluginRegistryEntry> {
        let q = query.to_lowercase();
        self.plugins
            .values()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || p.keywords
                        .as_ref()
                        .map(|k| k.iter().any(|kw| kw.to_lowercase().contains(&q)))
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Get the latest version of a plugin
    pub fn get_latest(&self, name: &str) -> Option<PluginRegistryEntry> {
        self.plugins
            .values()
            .filter(|p| p.name == name)
            .max_by(|a, b| {
                // Simple version comparison (highest version first)
                parse_version(&a.version).cmp(&parse_version(&b.version))
            })
            .cloned()
    }

    /// Remove a plugin from the registry
    pub fn remove(&mut self, name: &str, version: &str) -> Result<()> {
        let key = format!("{}:{}", name, version);
        self.plugins.remove(&key);
        Ok(())
    }

    /// Check if a plugin is registered
    pub fn exists(&self, name: &str, version: &str) -> bool {
        let key = format!("{}:{}", name, version);
        self.plugins.contains_key(&key)
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for LocalRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a version string into a sortable tuple
/// Simple semantic version parsing for comparison
fn parse_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts
        .get(0)
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    let minor = parts
        .get(1)
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

/// Represents a version requirement (e.g., ">=1.0.0", "1.2.x", "2.0.0")
#[derive(Debug, Clone)]
pub struct VersionRequirement {
    pub requirement: String,
}

impl VersionRequirement {
    /// Create a new version requirement
    pub fn new(requirement: String) -> Self {
        Self { requirement }
    }

    /// Check if a version matches this requirement
    pub fn matches(&self, version: &str) -> bool {
        let req = self.requirement.trim();

        // Check for wildcard first (before exact match check)
        if req.ends_with('x') || req.ends_with('X') {
            // 1.2.x wildcard match
            let req_parts: Vec<&str> = req.split('.').collect();
            let actual_parts: Vec<&str> = version.split('.').collect();

            // Check all parts up to the wildcard
            for (i, req_part) in req_parts.iter().enumerate() {
                if *req_part == "x" || *req_part == "X" {
                    // We've matched all non-wildcard parts
                    return true;
                }
                if i >= actual_parts.len() {
                    return false;
                }
                if actual_parts[i] != *req_part {
                    return false;
                }
            }
            return true;
        }

        // Exact version match (no operators)
        if !req.starts_with(&['>', '<', '=', '!', '~', '^'][..]) {
            return version == req;
        }

        let (op, ver) = if let Some(stripped) = req.strip_prefix(">=") {
            (">=", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix("<=") {
            ("<=", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix("!=") {
            ("!=", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix('>') {
            (">", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix('<') {
            ("<", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix('~') {
            // ~1.2.3 := >=1.2.3, <1.3.0
            ("~", stripped.trim())
        } else if let Some(stripped) = req.strip_prefix('^') {
            // ^1.2.3 := >=1.2.3, <2.0.0 (caret allows minor/patch changes)
            ("^", stripped.trim())
        } else {
            ("=", req)
        };

        let req_ver = parse_version(ver);
        let act_ver = parse_version(version);

        match op {
            "=" => req_ver == act_ver,
            ">" => act_ver > req_ver,
            ">=" => act_ver >= req_ver,
            "<" => act_ver < req_ver,
            "<=" => act_ver <= req_ver,
            "!=" => req_ver != act_ver,
            "~" => {
                // ~1.2.3 := >=1.2.3, <1.3.0
                let (rmaj, rmin, rpatch) = req_ver;
                let (amaj, amin, apatch) = act_ver;
                amaj == rmaj && amin == rmin && apatch >= rpatch
            }
            "^" => {
                // ^1.2.3 := >=1.2.3, <2.0.0
                let (rmaj, _, _) = req_ver;
                let (amaj, _, _) = act_ver;
                amaj == rmaj && act_ver >= req_ver
            }
            _ => false,
        }
    }
}

/// Dependency resolver for plugin installation
pub struct DependencyResolver {
    registry: LocalRegistry,
}

/// Result of dependency resolution
#[derive(Debug, Clone)]
pub struct DependencyResolution {
    /// Ordered list of plugins to install (from leaf to root)
    pub install_order: Vec<String>,
    /// Map of plugin name to specific version to install
    pub version_map: HashMap<String, String>,
    /// Any unmet dependencies
    pub unmet_dependencies: Vec<UnmetDependency>,
}

/// Represents an unmet dependency
#[derive(Debug, Clone)]
pub struct UnmetDependency {
    pub plugin_name: String,
    pub required_by: String,
    pub version_requirement: String,
}

impl DependencyResolver {
    /// Create a new dependency resolver
    pub fn new(registry: LocalRegistry) -> Self {
        Self { registry }
    }

    /// Resolve dependencies for a plugin
    pub fn resolve(&self, plugin_name: &str, plugin_version: &str) -> Result<DependencyResolution> {
        let mut install_order = Vec::new();
        let mut version_map = HashMap::new();
        let mut unmet = Vec::new();
        let mut visited = std::collections::HashSet::new();

        self.resolve_recursive(
            plugin_name,
            plugin_version,
            &mut install_order,
            &mut version_map,
            &mut unmet,
            &mut visited,
        );

        Ok(DependencyResolution {
            install_order,
            version_map,
            unmet_dependencies: unmet,
        })
    }

    fn resolve_recursive(
        &self,
        plugin_name: &str,
        plugin_version: &str,
        install_order: &mut Vec<String>,
        version_map: &mut HashMap<String, String>,
        unmet: &mut Vec<UnmetDependency>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        let key = format!("{}:{}", plugin_name, plugin_version);
        if visited.contains(&key) {
            return; // Already processed this plugin@version
        }
        visited.insert(key);

        // Find the plugin
        if let Some(entry) = self.registry.find_by_version(plugin_name, plugin_version) {
            // Process dependencies first (depth-first)
            if let Some(deps) = &entry.dependencies {
                for dep in deps {
                    // Find matching version
                    let req = VersionRequirement::new(dep.version_requirement.clone());
                    if let Some(matching) = self.find_matching_version(&dep.name, &req) {
                        // Recursively resolve
                        self.resolve_recursive(
                            &dep.name,
                            &matching,
                            install_order,
                            version_map,
                            unmet,
                            visited,
                        );
                    } else {
                        // Dependency not found in registry
                        unmet.push(UnmetDependency {
                            plugin_name: dep.name.clone(),
                            required_by: plugin_name.to_string(),
                            version_requirement: dep.version_requirement.clone(),
                        });
                    }
                }
            }

            // Add this plugin to install order
            if !install_order.contains(&plugin_name.to_string()) {
                install_order.push(plugin_name.to_string());
                version_map.insert(plugin_name.to_string(), plugin_version.to_string());
            }
        }
    }

    fn find_matching_version(
        &self,
        plugin_name: &str,
        requirement: &VersionRequirement,
    ) -> Option<String> {
        let all = self.registry.list_all();
        let matching: Vec<_> = all
            .iter()
            .filter(|p| p.name == plugin_name && requirement.matches(&p.version))
            .collect();

        // Return the latest matching version
        matching
            .iter()
            .max_by(|a, b| parse_version(&a.version).cmp(&parse_version(&b.version)))
            .map(|p| p.version.clone())
    }
}

/// Registry persistence (for future: save/load to disk)
pub struct RegistryPersistence;

impl RegistryPersistence {
    /// Save registry to file
    pub fn save(registry: &LocalRegistry, path: &Path) -> Result<()> {
        let entries = registry.list_all();
        let json = serde_json::to_string_pretty(&entries).context("serializing registry")?;
        std::fs::write(path, json).context("writing registry file")?;
        Ok(())
    }

    /// Load registry from file
    pub fn load(path: &Path) -> Result<LocalRegistry> {
        let content = std::fs::read_to_string(path).context("reading registry file")?;
        let entries: Vec<PluginRegistryEntry> =
            serde_json::from_str(&content).context("deserializing registry")?;

        let mut registry = LocalRegistry::new();
        for entry in entries {
            registry.register(entry)?;
        }
        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register() {
        let mut registry = LocalRegistry::new();
        let entry = PluginRegistryEntry {
            plugin_id: "test-plugin".to_string(),
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };

        registry.register(entry).unwrap();
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_registry_find() {
        let mut registry = LocalRegistry::new();
        let entry = PluginRegistryEntry {
            plugin_id: "my-plugin".to_string(),
            name: "my-plugin".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: Some("My plugin".to_string()),
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };

        registry.register(entry).unwrap();

        let found = registry.find_by_name("my-plugin");
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, "1.0.0");
    }

    #[test]
    fn test_registry_search() {
        let mut registry = LocalRegistry::new();

        for i in 0..3 {
            let entry = PluginRegistryEntry {
                plugin_id: format!("plugin-{}", i),
                name: format!("plugin-{}", i),
                version: "1.0.0".to_string(),
                abi_version: "2.0".to_string(),
                description: Some(format!("Test plugin number {}", i)),
                author: None,
                license: None,
                keywords: Some(vec!["test".to_string()]),
                dependencies: None,
            };
            registry.register(entry).unwrap();
        }

        let results = registry.search("test");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_registry_latest_version() {
        let mut registry = LocalRegistry::new();

        for version in &["0.1.0", "1.0.0", "2.0.0"] {
            let entry = PluginRegistryEntry {
                plugin_id: "versioned-plugin".to_string(),
                name: "versioned-plugin".to_string(),
                version: version.to_string(),
                abi_version: "2.0".to_string(),
                description: None,
                author: None,
                license: None,
                keywords: None,
                dependencies: None,
            };
            registry.register(entry).unwrap();
        }

        let latest = registry.get_latest("versioned-plugin");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, "2.0.0");
    }

    // Version requirement tests
    #[test]
    fn test_version_requirement_exact() {
        let req = VersionRequirement::new("1.0.0".to_string());
        assert!(req.matches("1.0.0"));
        assert!(!req.matches("1.0.1"));
        assert!(!req.matches("1.1.0"));
    }

    #[test]
    fn test_version_requirement_greater_than() {
        let req = VersionRequirement::new(">1.0.0".to_string());
        assert!(!req.matches("1.0.0"));
        assert!(req.matches("1.0.1"));
        assert!(req.matches("1.1.0"));
        assert!(req.matches("2.0.0"));
    }

    #[test]
    fn test_version_requirement_greater_than_or_equal() {
        let req = VersionRequirement::new(">=1.0.0".to_string());
        assert!(req.matches("1.0.0"));
        assert!(req.matches("1.0.1"));
        assert!(req.matches("2.0.0"));
        assert!(!req.matches("0.9.9"));
    }

    #[test]
    fn test_version_requirement_less_than() {
        let req = VersionRequirement::new("<2.0.0".to_string());
        assert!(req.matches("1.9.9"));
        assert!(req.matches("1.0.0"));
        assert!(!req.matches("2.0.0"));
        assert!(!req.matches("3.0.0"));
    }

    #[test]
    fn test_version_requirement_less_than_or_equal() {
        let req = VersionRequirement::new("<=2.0.0".to_string());
        assert!(req.matches("1.0.0"));
        assert!(req.matches("2.0.0"));
        assert!(!req.matches("2.0.1"));
        assert!(!req.matches("3.0.0"));
    }

    #[test]
    fn test_version_requirement_not_equal() {
        let req = VersionRequirement::new("!=1.0.0".to_string());
        assert!(!req.matches("1.0.0"));
        assert!(req.matches("1.0.1"));
        assert!(req.matches("2.0.0"));
    }

    #[test]
    fn test_version_requirement_tilde() {
        // ~1.2.3 := >=1.2.3, <1.3.0
        let req = VersionRequirement::new("~1.2.3".to_string());
        assert!(!req.matches("1.2.2"));
        assert!(req.matches("1.2.3"));
        assert!(req.matches("1.2.10"));
        assert!(!req.matches("1.3.0"));
    }

    #[test]
    fn test_version_requirement_caret() {
        // ^1.2.3 := >=1.2.3, <2.0.0
        let req = VersionRequirement::new("^1.2.3".to_string());
        assert!(!req.matches("1.2.2"));
        assert!(req.matches("1.2.3"));
        assert!(req.matches("1.9.0"));
        assert!(req.matches("1.100.100"));
        assert!(!req.matches("2.0.0"));
    }

    #[test]
    fn test_version_requirement_wildcard() {
        let req = VersionRequirement::new("1.2.x".to_string());
        assert!(req.matches("1.2.0"));
        assert!(req.matches("1.2.1"));
        assert!(req.matches("1.2.100"));
        assert!(!req.matches("1.3.0"));
        assert!(!req.matches("2.2.0"));
    }

    // Dependency resolver tests
    #[test]
    fn test_dependency_resolver_no_dependencies() {
        let mut registry = LocalRegistry::new();
        let entry = PluginRegistryEntry {
            plugin_id: "plugin-a".to_string(),
            name: "plugin-a".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };
        registry.register(entry).unwrap();

        let resolver = DependencyResolver::new(registry);
        let result = resolver.resolve("plugin-a", "1.0.0").unwrap();

        assert_eq!(result.install_order, vec!["plugin-a"]);
        assert_eq!(
            result.version_map.get("plugin-a"),
            Some(&"1.0.0".to_string())
        );
        assert_eq!(result.unmet_dependencies.len(), 0);
    }

    #[test]
    fn test_dependency_resolver_with_single_dependency() {
        let mut registry = LocalRegistry::new();

        // Register plugin-b (dependency)
        let entry_b = PluginRegistryEntry {
            plugin_id: "plugin-b".to_string(),
            name: "plugin-b".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };
        registry.register(entry_b).unwrap();

        // Register plugin-a (depends on plugin-b)
        let entry_a = PluginRegistryEntry {
            plugin_id: "plugin-a".to_string(),
            name: "plugin-a".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: Some(vec![PluginDependency {
                name: "plugin-b".to_string(),
                version_requirement: "1.0.0".to_string(),
            }]),
        };
        registry.register(entry_a).unwrap();

        let resolver = DependencyResolver::new(registry);
        let result = resolver.resolve("plugin-a", "1.0.0").unwrap();

        // plugin-b should be installed first
        assert_eq!(result.install_order, vec!["plugin-b", "plugin-a"]);
        assert_eq!(result.unmet_dependencies.len(), 0);
    }

    #[test]
    fn test_dependency_resolver_with_missing_dependency() {
        let mut registry = LocalRegistry::new();

        // Register plugin-a without plugin-b in registry
        let entry_a = PluginRegistryEntry {
            plugin_id: "plugin-a".to_string(),
            name: "plugin-a".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: Some(vec![PluginDependency {
                name: "plugin-b".to_string(),
                version_requirement: ">=1.0.0".to_string(),
            }]),
        };
        registry.register(entry_a).unwrap();

        let resolver = DependencyResolver::new(registry);
        let result = resolver.resolve("plugin-a", "1.0.0").unwrap();

        // Should report unmet dependency
        assert_eq!(result.unmet_dependencies.len(), 1);
        assert_eq!(result.unmet_dependencies[0].plugin_name, "plugin-b");
        assert_eq!(result.unmet_dependencies[0].required_by, "plugin-a");
    }

    #[test]
    fn test_dependency_resolver_version_matching() {
        let mut registry = LocalRegistry::new();

        // Register multiple versions of plugin-b
        for version in &["0.9.0", "1.0.0", "1.5.0", "2.0.0"] {
            let entry = PluginRegistryEntry {
                plugin_id: "plugin-b".to_string(),
                name: "plugin-b".to_string(),
                version: version.to_string(),
                abi_version: "2.0".to_string(),
                description: None,
                author: None,
                license: None,
                keywords: None,
                dependencies: None,
            };
            registry.register(entry).unwrap();
        }

        // Register plugin-a depending on plugin-b >=1.0.0, <2.0.0
        let entry_a = PluginRegistryEntry {
            plugin_id: "plugin-a".to_string(),
            name: "plugin-a".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: Some(vec![PluginDependency {
                name: "plugin-b".to_string(),
                version_requirement: "^1.0.0".to_string(), // >=1.0.0, <2.0.0
            }]),
        };
        registry.register(entry_a).unwrap();

        let resolver = DependencyResolver::new(registry);
        let result = resolver.resolve("plugin-a", "1.0.0").unwrap();

        // Should select latest matching version (1.5.0)
        assert_eq!(
            result.version_map.get("plugin-b"),
            Some(&"1.5.0".to_string())
        );
        assert_eq!(result.unmet_dependencies.len(), 0);
    }
}
