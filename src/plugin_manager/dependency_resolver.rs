// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
//! Plugin Dependency Resolution - CQ-004
//!
//! This module provides dependency ordering for plugin loading.
//! It extracts dependencies from plugin manifests and uses topological
//! sort to determine the correct loading order.
//!
//! ## Integration
//!
//! This module integrates with:
//! - `abi/src/dependencies/` - RFC-0005 Dependency Resolution
//! - `src/plugin_manager/manager.rs` - Plugin loading
//! - `src/main.rs` - Application startup

use anyhow::{anyhow, Result};
use skylet_abi::dependencies::{Dependency, DependencyGraph, PluginNode, Version, VersionReq};
use std::collections::HashMap;
use tracing::{debug, info};

/// Plugin manifest information extracted from plugin metadata
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Plugin name
    pub name: String,
    /// Plugin version (defaults to 1.0.0 if not specified)
    pub version: Version,
    /// List of dependencies as "name@version_req" strings
    pub dependencies: Vec<String>,
    /// ABI version (v1 or v2)
    pub abi_version: String,
}

/// Plugin dependency resolver that determines loading order
pub struct PluginDependencyResolver {
    /// Cache of discovered manifests
    manifests: HashMap<String, PluginManifest>,
}

impl PluginDependencyResolver {
    /// Create a new dependency resolver
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
        }
    }

    /// Register a plugin with its dependencies
    ///
    /// This is used for plugins where we can't easily extract the manifest
    /// (e.g., before the plugin is loaded).
    pub fn register_plugin(
        &mut self,
        name: &str,
        abi_version: &str,
        dependencies: Vec<String>,
        version: Option<&str>,
    ) {
        let version = version
            .and_then(|v| Version::parse(v).ok())
            .unwrap_or_else(|| Version::new(1, 0, 0));

        self.manifests.insert(
            name.to_string(),
            PluginManifest {
                name: name.to_string(),
                version,
                dependencies,
                abi_version: abi_version.to_string(),
            },
        );
    }

    /// Register a plugin from a manifest string
    ///
    /// Format: "plugin_name:abi_version:dep1,dep2,dep3"
    /// Example: "database:v2:config-manager,logging"
    pub fn register_from_string(&mut self, manifest_str: &str) -> Result<()> {
        let parts: Vec<&str> = manifest_str.split(':').collect();
        if parts.len() < 2 {
            return Err(anyhow!(
                "Invalid manifest format: expected 'name:abi_version[:deps]'"
            ));
        }

        let name = parts[0].to_string();
        let abi_version = parts[1].to_string();
        let dependencies = if parts.len() > 2 {
            parts[2].split(',').map(|s| s.trim().to_string()).collect()
        } else {
            Vec::new()
        };

        self.register_plugin(&name, &abi_version, dependencies, None);
        Ok(())
    }

    /// Resolve the loading order for all registered plugins
    ///
    /// Returns a list of (plugin_name, abi_version) tuples in the order
    /// they should be loaded (dependencies first).
    pub fn resolve_loading_order(&self) -> Result<Vec<(String, String)>> {
        let mut graph = DependencyGraph::new();

        // Add all plugins to the graph
        for manifest in self.manifests.values() {
            let node = self.build_plugin_node(manifest)?;
            graph.add_plugin(node);
        }

        // Check for cycles
        if graph.has_cycles() {
            let cycles = graph.detect_cycles();
            let cycle_strs: Vec<String> = cycles.iter().map(|c| c.join(" -> ")).collect();
            return Err(anyhow!(
                "Cyclic dependencies detected: {}",
                cycle_strs.join("; ")
            ));
        }

        // Get topological sort (dependencies first, then dependents)
        let order = graph
            .topological_sort()
            .map_err(|e| anyhow!("Failed to resolve dependencies: {}", e))?;

        // Convert to (name, abi_version) tuples
        let result: Vec<(String, String)> = order
            .into_iter()
            .filter_map(|name| {
                self.manifests.get(&name).map(|m| {
                    debug!("Resolved load order: {} ({})", m.name, m.abi_version);
                    (m.name.clone(), m.abi_version.clone())
                })
            })
            .collect();

        info!(
            "Resolved loading order for {} plugins (topological sort)",
            result.len()
        );

        Ok(result)
    }

    /// Build a PluginNode from a PluginManifest
    fn build_plugin_node(&self, manifest: &PluginManifest) -> Result<PluginNode> {
        let mut node = PluginNode::new(&manifest.name, manifest.version.clone());

        for dep_str in &manifest.dependencies {
            let dep = self.parse_dependency(dep_str)?;
            node = node.with_dependency(dep);
        }

        Ok(node)
    }

    /// Parse a dependency string into a Dependency struct
    ///
    /// Formats:
    /// - "plugin_name" -> any version
    /// - "plugin_name@1.0.0" -> exact version
    /// - "plugin_name@^1.0.0" -> caret constraint
    /// - "plugin_name@>=1.0.0,<2.0.0" -> range constraint
    fn parse_dependency(&self, dep_str: &str) -> Result<Dependency> {
        let parts: Vec<&str> = dep_str.splitn(2, '@').collect();
        let name = parts[0].trim();

        let version_req = if parts.len() > 1 {
            VersionReq::parse(parts[1].trim())
                .map_err(|e| anyhow!("Invalid version requirement '{}': {}", parts[1], e))?
        } else {
            VersionReq::any()
        };

        Ok(Dependency::new(name, version_req))
    }

    /// Get the number of registered plugins
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// Check if no plugins are registered
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    /// Get all registered plugins (unordered)
    pub fn plugins(&self) -> impl Iterator<Item = &PluginManifest> {
        self.manifests.values()
    }
}

impl Default for PluginDependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to resolve plugin loading order from a list
///
/// Takes a list of (name, abi_version) tuples and returns them in
/// dependency-resolved order.
///
/// # Example
///
/// ```ignore
/// let plugins = vec![
///     ("database", "v2"),
///     ("config-manager", "v1"),
///     ("api-server", "v2"),  // depends on database
/// ];
///
/// let ordered = resolve_plugin_order(plugins, |name| {
///     // Return dependencies for each plugin
///     match name {
///         "api-server" => Some(vec!["database".to_string()]),
///         _ => None,
///     }
/// });
/// ```
pub fn resolve_plugin_order<F>(
    plugins: Vec<(String, String)>,
    get_dependencies: F,
) -> Result<Vec<(String, String)>>
where
    F: Fn(&str) -> Option<Vec<String>>,
{
    let mut resolver = PluginDependencyResolver::new();

    for (name, abi_version) in &plugins {
        let deps = get_dependencies(name).unwrap_or_default();
        resolver.register_plugin(name, abi_version, deps, None);
    }

    resolver.resolve_loading_order()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_dependency_order() {
        let mut resolver = PluginDependencyResolver::new();

        // database depends on config-manager
        resolver.register_plugin("config-manager", "v1", vec![], None);
        resolver.register_plugin("database", "v2", vec!["config-manager".to_string()], None);
        resolver.register_plugin("api-server", "v2", vec!["database".to_string()], None);

        let order = resolver.resolve_loading_order().unwrap();

        // config-manager should come before database
        // database should come before api-server
        let config_idx = order
            .iter()
            .position(|(n, _)| n == "config-manager")
            .unwrap();
        let db_idx = order.iter().position(|(n, _)| n == "database").unwrap();
        let api_idx = order.iter().position(|(n, _)| n == "api-server").unwrap();

        assert!(
            config_idx < db_idx,
            "config-manager should load before database"
        );
        assert!(db_idx < api_idx, "database should load before api-server");
    }

    #[test]
    fn test_independent_plugins() {
        let mut resolver = PluginDependencyResolver::new();

        // Independent plugins can load in any order
        resolver.register_plugin("plugin-a", "v1", vec![], None);
        resolver.register_plugin("plugin-b", "v2", vec![], None);
        resolver.register_plugin("plugin-c", "v2", vec![], None);

        let order = resolver.resolve_loading_order().unwrap();

        assert_eq!(order.len(), 3);
        let names: Vec<&str> = order.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"plugin-a"));
        assert!(names.contains(&"plugin-b"));
        assert!(names.contains(&"plugin-c"));
    }

    #[test]
    fn test_cycle_detection() {
        let mut resolver = PluginDependencyResolver::new();

        // Create a cycle: A -> B -> C -> A
        resolver.register_plugin("plugin-a", "v1", vec!["plugin-b".to_string()], None);
        resolver.register_plugin("plugin-b", "v1", vec!["plugin-c".to_string()], None);
        resolver.register_plugin("plugin-c", "v1", vec!["plugin-a".to_string()], None);

        let result = resolver.resolve_loading_order();
        assert!(result.is_err(), "Should detect cycle");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Cyclic"),
            "Error should mention cyclic dependencies"
        );
    }

    #[test]
    fn test_version_constraints() {
        let mut resolver = PluginDependencyResolver::new();

        resolver.register_plugin("core", "v1", vec![], Some("1.0.0"));
        resolver.register_plugin(
            "plugin-a",
            "v2",
            vec!["core@^1.0.0".to_string()],
            Some("2.0.0"),
        );

        let order = resolver.resolve_loading_order().unwrap();
        assert_eq!(order.len(), 2);

        // core should come before plugin-a
        let core_idx = order.iter().position(|(n, _)| n == "core").unwrap();
        let a_idx = order.iter().position(|(n, _)| n == "plugin-a").unwrap();
        assert!(core_idx < a_idx);
    }

    #[test]
    fn test_diamond_dependency() {
        let mut resolver = PluginDependencyResolver::new();

        // Diamond: app depends on A and B, both depend on core
        resolver.register_plugin("core", "v1", vec![], None);
        resolver.register_plugin("plugin-a", "v1", vec!["core".to_string()], None);
        resolver.register_plugin("plugin-b", "v1", vec!["core".to_string()], None);
        resolver.register_plugin(
            "app",
            "v2",
            vec!["plugin-a".to_string(), "plugin-b".to_string()],
            None,
        );

        let order = resolver.resolve_loading_order().unwrap();

        // core should come before A and B
        // A and B should come before app
        let core_idx = order.iter().position(|(n, _)| n == "core").unwrap();
        let a_idx = order.iter().position(|(n, _)| n == "plugin-a").unwrap();
        let b_idx = order.iter().position(|(n, _)| n == "plugin-b").unwrap();
        let app_idx = order.iter().position(|(n, _)| n == "app").unwrap();

        assert!(core_idx < a_idx);
        assert!(core_idx < b_idx);
        assert!(a_idx < app_idx);
        assert!(b_idx < app_idx);
    }

    #[test]
    fn test_resolve_plugin_order_function() {
        let plugins = vec![
            ("database".to_string(), "v2".to_string()),
            ("config".to_string(), "v1".to_string()),
        ];

        let deps_fn = |name: &str| match name {
            "database" => Some(vec!["config".to_string()]),
            _ => None,
        };

        let order = resolve_plugin_order(plugins, deps_fn).unwrap();

        assert_eq!(order.len(), 2);
        assert_eq!(order[0].0, "config"); // config should load first
        assert_eq!(order[1].0, "database");
    }

    #[test]
    fn test_parse_dependency() {
        let resolver = PluginDependencyResolver::new();

        // Simple name
        let dep = resolver.parse_dependency("plugin-name").unwrap();
        assert_eq!(dep.name, "plugin-name");

        // With version constraint
        let dep = resolver.parse_dependency("plugin-name@^1.0.0").unwrap();
        assert_eq!(dep.name, "plugin-name");
        assert!(dep.version_req.matches(&Version::new(1, 5, 0)));
        assert!(!dep.version_req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_register_from_string() {
        let mut resolver = PluginDependencyResolver::new();

        resolver
            .register_from_string("database:v2:config-manager,logging")
            .unwrap();

        let manifest = resolver.manifests.get("database").unwrap();
        assert_eq!(manifest.name, "database");
        assert_eq!(manifest.abi_version, "v2");
        assert_eq!(manifest.dependencies.len(), 2);
        assert!(manifest
            .dependencies
            .contains(&"config-manager".to_string()));
        assert!(manifest.dependencies.contains(&"logging".to_string()));
    }
}
