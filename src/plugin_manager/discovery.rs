// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dynamic Plugin Discovery Module (CQ-003)
//!
//! Provides filesystem-based plugin discovery to replace hardcoded plugin lists.
//! Supports discovery from multiple directories with configurable filtering.
//!
//! # Features
//! - Discovers plugins by scanning directories for shared library files
//! - Filters plugins by naming patterns and ABI compatibility
//! - Supports exclusion patterns for test/utility plugins
//! - Integrates with AppConfig for configurable plugin directories
//!
//! # Usage
//! ```rust
//! use plugin_manager::discovery::{PluginDiscovery, DiscoveryConfig};
//!
//! let config = DiscoveryConfig::default();
//! let discovery = PluginDiscovery::new(config);
//! let plugins = discovery.discover_plugins()?;
//!
//! for plugin in &plugins {
//!     tracing::info!("Found: {} ({})", plugin.name, plugin.abi_version);
//! }
//! ```

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Information about a discovered plugin
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPlugin {
    /// Plugin name (without lib prefix and extension)
    pub name: String,
    /// Detected ABI version (v1 or v2)
    pub abi_version: String,
    /// Full path to the plugin library
    pub path: PathBuf,
    /// Plugin file size in bytes
    pub size: u64,
}

impl DiscoveredPlugin {
    /// Create a new discovered plugin entry
    pub fn new(name: String, abi_version: String, path: PathBuf, size: u64) -> Self {
        Self {
            name,
            abi_version,
            path,
            size,
        }
    }

    /// Check if this plugin matches a given name
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn matches_name(&self, pattern: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            self.name.starts_with(prefix)
        } else {
            self.name == pattern
        }
    }
}

impl std::fmt::Display for DiscoveredPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{} ({})",
            self.name,
            self.abi_version,
            self.path.display()
        )
    }
}

/// Configuration for plugin discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Directories to scan for plugins (in priority order)
    pub search_paths: Vec<PathBuf>,
    /// Patterns to exclude (supports * wildcard at end)
    pub exclude_patterns: Vec<String>,
    /// Patterns to include exclusively (if set, only matching plugins are included)
    pub include_patterns: Vec<String>,
    /// Whether to detect ABI version by probing the library
    pub probe_abi_version: bool,
    /// Whether to include debug builds (target/debug)
    pub include_debug_builds: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            search_paths: Self::default_search_paths(),
            exclude_patterns: vec![
                // Test and utility plugins (can be overridden)
                "test_*".to_string(),
                "simple_*".to_string(),
                "*_test".to_string(),
                "*_template".to_string(),
                // Proc macros and build-time dependencies (not plugins)
                "skylet_sdk_macros".to_string(),
            ],
            include_patterns: vec![],
            probe_abi_version: true,
            include_debug_builds: false,
        }
    }
}

impl DiscoveryConfig {
    /// Get default plugin search paths
    pub fn default_search_paths() -> Vec<PathBuf> {
        vec![
            PathBuf::from("./target/release"),
            PathBuf::from("./target/debug"),
            PathBuf::from("/usr/local/lib/skylet/plugins"),
            PathBuf::from("/usr/lib/skylet/plugins"),
        ]
    }

    /// Create config with custom search paths
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
            ..Default::default()
        }
    }

    /// Create config from a single plugin directory
    pub fn with_plugin_directory(dir: PathBuf) -> Self {
        Self {
            search_paths: vec![dir],
            ..Default::default()
        }
    }

    /// Add an exclusion pattern
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn exclude(mut self, pattern: &str) -> Self {
        self.exclude_patterns.push(pattern.to_string());
        self
    }

    /// Add an inclusion pattern
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn include(mut self, pattern: &str) -> Self {
        self.include_patterns.push(pattern.to_string());
        self
    }

    /// Enable debug builds in discovery
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn with_debug_builds(mut self, include: bool) -> Self {
        self.include_debug_builds = include;
        self
    }

    /// Check if a plugin name matches any exclusion pattern
    pub fn is_excluded(&self, name: &str) -> bool {
        for pattern in &self.exclude_patterns {
            if pattern.starts_with('*') && pattern.ends_with('*') {
                // *infix* pattern
                let infix = &pattern[1..pattern.len() - 1];
                if name.contains(infix) {
                    return true;
                }
            } else if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if name.starts_with(prefix) {
                    return true;
                }
            } else if pattern.starts_with('*') {
                let suffix = &pattern[1..];
                if name.ends_with(suffix) {
                    return true;
                }
            } else if name == pattern {
                return true;
            }
        }
        false
    }

    /// Check if a plugin name matches any inclusion pattern
    /// Returns true if no include patterns are set (include all)
    pub fn is_included(&self, name: &str) -> bool {
        if self.include_patterns.is_empty() {
            return true;
        }

        for pattern in &self.include_patterns {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if name.starts_with(prefix) {
                    return true;
                }
            } else if name == pattern {
                return true;
            }
        }
        false
    }
}

/// Plugin discovery engine
pub struct PluginDiscovery {
    config: DiscoveryConfig,
}

impl PluginDiscovery {
    /// Create a new plugin discovery engine with the given configuration
    pub fn new(config: DiscoveryConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DiscoveryConfig::default())
    }

    /// Create with a single plugin directory
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn with_directory(dir: PathBuf) -> Self {
        Self::new(DiscoveryConfig::with_plugin_directory(dir))
    }

    /// Get the configuration
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// Discover all plugins in configured search paths
    ///
    /// Returns a list of discovered plugins, sorted by name for deterministic loading order.
    pub fn discover_plugins(&self) -> Result<Vec<DiscoveredPlugin>> {
        let mut plugins: HashMap<String, DiscoveredPlugin> = HashMap::new();

        for search_path in &self.config.search_paths {
            if !search_path.exists() {
                debug!("Search path does not exist: {}", search_path.display());
                continue;
            }

            // Skip debug builds if configured
            if !self.config.include_debug_builds
                && search_path.ends_with("debug")
                && self
                    .config
                    .search_paths
                    .iter()
                    .any(|p| p.ends_with("release"))
            {
                debug!("Skipping debug directory (include_debug_builds=false)");
                continue;
            }

            let found = self.scan_directory(search_path)?;
            for plugin in found {
                // Only add if not already found (first path wins)
                if !plugins.contains_key(&plugin.name) {
                    debug!("Discovered plugin: {}", plugin);
                    plugins.insert(plugin.name.clone(), plugin);
                }
            }
        }

        // Filter by inclusion/exclusion patterns
        let filtered: Vec<DiscoveredPlugin> = plugins
            .into_values()
            .filter(|p| {
                if self.config.is_excluded(&p.name) {
                    debug!("Excluding plugin: {}", p.name);
                    false
                } else if !self.config.is_included(&p.name) {
                    debug!("Not in inclusion list: {}", p.name);
                    false
                } else {
                    true
                }
            })
            .collect();

        // Sort by name for deterministic order
        let mut sorted = filtered;
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        info!(
            "Discovered {} plugins from {} search paths",
            sorted.len(),
            self.config.search_paths.len()
        );

        Ok(sorted)
    }

    /// Discover plugins and return as (name, abi_version) tuples for loading
    pub fn discover_for_loading(&self) -> Result<Vec<(String, String)>> {
        let plugins = self.discover_plugins()?;
        Ok(plugins
            .into_iter()
            .map(|p| (p.name, p.abi_version))
            .collect())
    }

    /// Scan a single directory for plugins
    fn scan_directory(&self, dir: &Path) -> Result<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        let entries = std::fs::read_dir(dir)
            .map_err(|e| anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| anyhow!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            // Must be a file (not a directory or symlink to directory)
            if !path.is_file() {
                continue;
            }

            // Check if it's a shared library
            if !self.is_plugin_library(&path) {
                continue;
            }

            // Extract plugin name
            let name = match self.extract_plugin_name(&path) {
                Some(n) => n,
                None => {
                    debug!("Skipping file with invalid name: {}", path.display());
                    continue;
                }
            };

            // Get file metadata
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to read metadata for {}: {}", path.display(), e);
                    continue;
                }
            };

            // Detect ABI version
            let abi_version = if self.config.probe_abi_version {
                self.detect_abi_version(&path).unwrap_or_else(|| {
                    // Fallback: infer from naming convention
                    if name.contains("_v2") || name.ends_with("_plugin") {
                        "v2".to_string()
                    } else {
                        "v1".to_string()
                    }
                })
            } else {
                // Default to v2 for new plugins
                "v2".to_string()
            };

            plugins.push(DiscoveredPlugin::new(
                name,
                abi_version,
                path,
                metadata.len(),
            ));
        }

        Ok(plugins)
    }

    /// Check if a path is a valid plugin library
    fn is_plugin_library(&self, path: &Path) -> bool {
        // Check extension
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        #[cfg(target_os = "macos")]
        let valid_extensions = ["dylib"];
        #[cfg(target_os = "windows")]
        let valid_extensions = ["dll"];
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let valid_extensions = ["so"];

        if !valid_extensions.contains(&extension) {
            return false;
        }

        // Must have lib prefix
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        filename.starts_with("lib")
    }

    /// Extract plugin name from library path
    fn extract_plugin_name(&self, path: &Path) -> Option<String> {
        let filename = path.file_name()?.to_str()?;
        let extension = path.extension()?.to_str()?;

        // Remove lib prefix and extension
        if filename.starts_with("lib") {
            let name = filename
                .strip_prefix("lib")?
                .strip_suffix(&format!(".{}", extension))?;
            Some(name.to_string())
        } else {
            None
        }
    }

    /// Detect ABI version by probing the library for symbols
    ///
    /// Returns Some("v2") if plugin_init_v2 is found, Some("v1") if only plugin_init is found
    fn detect_abi_version(&self, path: &Path) -> Option<String> {
        // Safety: We're only probing for symbols, not executing code
        unsafe {
            let library = libloading::Library::new(path).ok()?;

            // Check for v2 ABI first
            let has_v2: bool = library
                .get::<libloading::Symbol<()>>(b"plugin_init_v2\0")
                .is_ok();

            if has_v2 {
                return Some("v2".to_string());
            }

            // Check for v1 ABI
            let has_v1: bool = library
                .get::<libloading::Symbol<()>>(b"plugin_init\0")
                .is_ok();

            if has_v1 {
                return Some("v1".to_string());
            }

            // No known ABI symbols found
            None
        }
    }

    /// Get list of all discovered plugin names
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn plugin_names(&self) -> Result<Vec<String>> {
        let plugins = self.discover_plugins()?;
        Ok(plugins.into_iter().map(|p| p.name).collect())
    }

    /// Check if a specific plugin exists
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn plugin_exists(&self, name: &str) -> bool {
        for search_path in &self.config.search_paths {
            if !search_path.exists() {
                continue;
            }

            #[cfg(target_os = "macos")]
            let extensions = ["dylib"];
            #[cfg(target_os = "windows")]
            let extensions = ["dll"];
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let extensions = ["so"];

            for ext in &extensions {
                let plugin_path = search_path.join(format!("lib{}.{}", name, ext));
                if plugin_path.exists() {
                    return true;
                }
            }
        }
        false
    }

    /// Get the path to a specific plugin
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn find_plugin(&self, name: &str) -> Option<PathBuf> {
        for search_path in &self.config.search_paths {
            if !search_path.exists() {
                continue;
            }

            #[cfg(target_os = "macos")]
            let extensions = ["dylib"];
            #[cfg(target_os = "windows")]
            let extensions = ["dll"];
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let extensions = ["so"];

            for ext in &extensions {
                let plugin_path = search_path.join(format!("lib{}.{}", name, ext));
                if plugin_path.exists() {
                    return Some(plugin_path);
                }
            }
        }
        None
    }
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_plugin_new() {
        let plugin = DiscoveredPlugin::new(
            "test_plugin".to_string(),
            "v2".to_string(),
            PathBuf::from("/path/to/libtest_plugin.so"),
            1024,
        );

        assert_eq!(plugin.name, "test_plugin");
        assert_eq!(plugin.abi_version, "v2");
        assert_eq!(plugin.size, 1024);
    }

    #[test]
    fn test_discovered_plugin_matches_name() {
        let plugin = DiscoveredPlugin::new(
            "github_api_plugin".to_string(),
            "v2".to_string(),
            PathBuf::from("/path/to/libgithub_api_plugin.so"),
            1024,
        );

        assert!(plugin.matches_name("github_api_plugin"));
        assert!(plugin.matches_name("github_*"));
        assert!(!plugin.matches_name("npm_*"));
    }

    #[test]
    fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();

        assert!(!config.search_paths.is_empty());
        assert!(config.exclude_patterns.contains(&"test_*".to_string()));
        assert!(config.probe_abi_version);
        assert!(!config.include_debug_builds);
    }

    #[test]
    fn test_discovery_config_exclusion() {
        let config = DiscoveryConfig::default();

        assert!(config.is_excluded("test_plugin"));
        assert!(config.is_excluded("simple_v2_plugin"));
        assert!(config.is_excluded("git_operations_template"));
        assert!(!config.is_excluded("github_api_plugin"));
        assert!(!config.is_excluded("workflow_orchestrator"));
    }

    #[test]
    fn test_discovery_config_inclusion() {
        // Default: include all
        let config = DiscoveryConfig::default();
        assert!(config.is_included("any_plugin"));

        // With include patterns
        let config = DiscoveryConfig::default()
            .include("github_*")
            .include("npm_*");

        assert!(config.is_included("github_api_plugin"));
        assert!(config.is_included("npm_registry_plugin"));
        assert!(!config.is_included("telegram_bot_adapter"));
    }

    #[test]
    fn test_discovery_config_custom_exclude() {
        let config = DiscoveryConfig::default()
            .exclude("internal_*")
            .exclude("deprecated_*");

        assert!(config.is_excluded("internal_utils"));
        assert!(config.is_excluded("deprecated_plugin"));
        assert!(!config.is_excluded("github_api_plugin"));
    }

    #[test]
    fn test_extract_plugin_name() {
        let discovery = PluginDiscovery::with_defaults();

        assert_eq!(
            discovery.extract_plugin_name(Path::new("/path/to/libgithub_api_plugin.so")),
            Some("github_api_plugin".to_string())
        );
        assert_eq!(
            discovery.extract_plugin_name(Path::new("/path/to/libtest.dylib")),
            Some("test".to_string())
        );
        assert_eq!(
            discovery.extract_plugin_name(Path::new("/path/to/invalid.dll")),
            None
        );
    }

    #[test]
    fn test_is_plugin_library() {
        let discovery = PluginDiscovery::with_defaults();

        // Valid: platform-specific extension
        #[cfg(target_os = "macos")]
        assert!(discovery.is_plugin_library(Path::new("/path/to/libplugin.dylib")));
        #[cfg(target_os = "windows")]
        assert!(discovery.is_plugin_library(Path::new("/path/to/libplugin.dll")));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(discovery.is_plugin_library(Path::new("/path/to/libplugin.so")));

        // Invalid
        assert!(!discovery.is_plugin_library(Path::new("/path/to/plugin.so"))); // no lib prefix
        assert!(!discovery.is_plugin_library(Path::new("/path/to/libplugin.txt"))); // wrong extension
        assert!(!discovery.is_plugin_library(Path::new("/path/to/"))); // directory
    }

    #[test]
    fn test_discovery_config_with_paths() {
        let paths = vec![
            PathBuf::from("/custom/path1"),
            PathBuf::from("/custom/path2"),
        ];
        let config = DiscoveryConfig::with_paths(paths.clone());

        assert_eq!(config.search_paths, paths);
        assert!(config.exclude_patterns.contains(&"test_*".to_string()));
    }

    #[test]
    fn test_discovery_config_with_plugin_directory() {
        let config = DiscoveryConfig::with_plugin_directory(PathBuf::from("/plugins"));

        assert_eq!(config.search_paths.len(), 1);
        assert_eq!(config.search_paths[0], PathBuf::from("/plugins"));
    }

    #[test]
    fn test_discovery_config_with_debug_builds() {
        let config = DiscoveryConfig::default().with_debug_builds(true);
        assert!(config.include_debug_builds);

        let config = DiscoveryConfig::default().with_debug_builds(false);
        assert!(!config.include_debug_builds);
    }

    #[test]
    fn test_discovered_plugin_display() {
        let plugin = DiscoveredPlugin::new(
            "test_plugin".to_string(),
            "v2".to_string(),
            PathBuf::from("/path/to/libtest_plugin.so"),
            1024,
        );

        let display = format!("{}", plugin);
        assert!(display.contains("test_plugin"));
        assert!(display.contains("v2"));
        assert!(display.contains("/path/to/libtest_plugin.so"));
    }

    #[test]
    fn test_discovery_filtering_combination() {
        // Test that exclude patterns are applied before include patterns
        let config = DiscoveryConfig::default()
            .include("test_*") // Try to include test plugins
            .exclude("test_*"); // But exclude them too

        // Exclude wins (plugin is excluded)
        assert!(config.is_excluded("test_plugin"));
    }
}
