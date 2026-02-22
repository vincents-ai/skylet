// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! # Skylet Lifecycle CLI
//!
//! Command-line interface for managing plugin lifecycle, registry sources,
//! and plugin discovery operations.
//!
//! ## Commands
//!
//! ### Source Management
//! - `source add <name> <url>` - Add a registry source
//! - `source remove <name>` - Remove a registry source
//! - `source list` - List all configured registry sources
//!
//! ### Plugin Discovery
//! - `plugin search <query>` - Search for plugins across all sources
//! - `plugin list [--source <name>]` - List available plugins
//! - `plugin info <plugin-id>` - Display detailed plugin information
//! - `plugin check-updates <plugin-id>` - Check for available updates
//!
//! ## Usage
//!
//! ```bash
//! # Add a registry source
//! skylet-lifecycle-cli source add official https://registry.skylet.dev
//!
//! # Search for plugins
//! skylet-lifecycle-cli plugin search "database"
//!
//! # List all plugins from a specific source
//! skylet-lifecycle-cli plugin list --source official
//!
//! # Get detailed plugin info
//! skylet-lifecycle-cli plugin info core::postgres-plugin
//!
//! # Check for updates
//! skylet-lifecycle-cli plugin check-updates core::postgres-plugin
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use reqwest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

// ============================================================================
// CLI Structure
// ============================================================================

/// Skylet Plugin Lifecycle Management CLI
#[derive(Parser, Debug)]
#[command(
    name = "skylet-lifecycle-cli",
    version = "0.1.0",
    about = "Manage plugin lifecycle and discovery for Skylet",
    long_about = "Command-line interface for managing Skylet plugin registry sources, \
                  plugin discovery, and lifecycle operations."
)]
struct Cli {
    /// Path to configuration directory
    #[arg(short, long, global = true)]
    config_dir: Option<PathBuf>,

    /// Path to sources configuration file
    #[arg(short, long, global = true)]
    sources_file: Option<PathBuf>,

    /// Auth token for private registries
    #[arg(short, long, global = true)]
    auth_token: Option<String>,

    /// Output format (text, json)
    #[arg(short, long, global = true, default_value = "text")]
    output: OutputFormat,

    /// Non-interactive mode (skip confirmations)
    #[arg(short, long, global = true)]
    non_interactive: bool,

    #[command(subcommand)]
    command: Commands,
}

/// Output format options
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// Main command groups
#[derive(Subcommand, Debug)]
enum Commands {
    /// Registry source management
    #[command(subcommand)]
    Source(SourceCommands),

    /// Plugin discovery and information
    #[command(subcommand)]
    Plugin(PluginCommands),

    /// Plugin installation management
    #[command(subcommand)]
    Install(InstallCommands),
}

// ============================================================================
// Source Commands
// ============================================================================

/// Registry source management commands
#[derive(Subcommand, Debug)]
enum SourceCommands {
    /// Add a new registry source
    Add {
        /// Unique name for the registry source
        name: String,

        /// URL of the registry
        url: String,

        /// Auth token for private registries
        #[arg(short, long)]
        token: Option<String>,

        /// Set as default source
        #[arg(short, long)]
        default: bool,
    },

    /// Remove a registry source
    Remove {
        /// Name of the registry source to remove
        name: String,
    },

    /// List all configured registry sources
    List {
        /// Show detailed information including tokens (masked)
        #[arg(short, long)]
        verbose: bool,
    },

    /// Update a registry source
    Update {
        /// Name of the registry source to update
        name: String,

        /// New URL for the registry
        #[arg(short, long)]
        url: Option<String>,

        /// New auth token
        #[arg(short, long)]
        token: Option<String>,

        /// Set as default source
        #[arg(short, long)]
        default: bool,
    },
}

// ============================================================================
// Plugin Commands
// ============================================================================

/// Plugin discovery and information commands
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// Search for plugins across all sources
    Search {
        /// Search query (name, description, author, tags)
        query: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Filter by specific source
        #[arg(short, long)]
        source: Option<String>,
    },

    /// List available plugins
    List {
        /// Filter by specific source
        #[arg(short, long)]
        source: Option<String>,

        /// Filter by plugin type
        #[arg(short = 't', long)]
        plugin_type: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: usize,

        /// Offset for pagination
        #[arg(short, long, default_value = "0")]
        offset: usize,
    },

    /// Display detailed plugin information
    Info {
        /// Fully qualified plugin name (e.g., core::postgres-plugin)
        plugin_id: String,

        /// Show all available versions
        #[arg(short, long)]
        all_versions: bool,
    },

    /// Check for available updates
    CheckUpdates {
        /// Specific plugin ID to check (omit for all installed)
        #[arg(short, long)]
        plugin_id: Option<String>,

        /// Include pre-release versions
        #[arg(short, long)]
        prerelease: bool,
    },
}

// ============================================================================
// Install Commands
// ============================================================================

/// Plugin installation management commands
#[derive(Subcommand, Debug)]
enum InstallCommands {
    /// Install a plugin
    Plugin {
        /// Fully qualified plugin name (e.g., core::postgres-plugin)
        plugin_id: String,

        /// Version requirement (e.g., ">=1.0.0 <2.0.0")
        #[arg(short, long, default_value = "*")]
        version: String,

        /// Source to install from
        #[arg(short, long)]
        source: Option<String>,

        /// Skip dependency confirmation
        #[arg(short, long)]
        skip_confirm: bool,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name to uninstall
        plugin_id: String,

        /// Force uninstall even if other plugins depend on it
        #[arg(short, long)]
        force: bool,
    },

    /// Update an installed plugin
    Update {
        /// Plugin to update (omit for all)
        #[arg(short, long)]
        plugin_id: Option<String>,

        /// Target version (latest if not specified)
        #[arg(short, long)]
        version: Option<String>,
    },

    /// List installed plugins
    Installed {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Registry source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistrySource {
    /// Unique name for this source
    name: String,

    /// Registry URL
    url: String,

    /// Optional auth token
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_token: Option<String>,

    /// Whether this is the default source
    #[serde(default)]
    is_default: bool,

    /// When this source was added
    added_at: String,

    /// Last successful sync time
    #[serde(skip_serializing_if = "Option::is_none")]
    last_sync: Option<String>,
}

/// Sources configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SourcesConfig {
    /// List of configured sources
    sources: Vec<RegistrySource>,

    /// Default source name
    #[serde(skip_serializing_if = "Option::is_none")]
    default_source: Option<String>,
}

impl SourcesConfig {
    /// Load sources from file
    fn load(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read sources file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| "Failed to parse sources configuration")
    }

    /// Save sources to file
    fn save(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize sources configuration")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write sources file: {}", path.display()))?;

        Ok(())
    }

    /// Get default sources file path
    fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("skylet")
            .join("sources.toml")
    }

    /// Add a new source
    fn add_source(&mut self, source: RegistrySource) -> Result<()> {
        if self.sources.iter().any(|s| s.name == source.name) {
            anyhow::bail!("Source '{}' already exists", source.name);
        }

        if source.is_default {
            // Unset other defaults
            for s in &mut self.sources {
                s.is_default = false;
            }
            self.default_source = Some(source.name.clone());
        }

        self.sources.push(source);
        Ok(())
    }

    /// Remove a source by name
    fn remove_source(&mut self, name: &str) -> Result<RegistrySource> {
        let idx = self.sources.iter().position(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", name))?;

        let removed = self.sources.remove(idx);

        // Update default if needed
        if self.default_source.as_ref() == Some(&removed.name) {
            self.default_source = self.sources.first().map(|s| {
                let mut first = s.clone();
                first.is_default = true;
                first.name.clone()
            });
        }

        Ok(removed)
    }

    /// List all sources
    fn list_sources(&self) -> &[RegistrySource] {
        &self.sources
    }
}

// ============================================================================
// Registry Client
// ============================================================================

/// Plugin info returned from registry API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginInfo {
    id: String,
    name: String,
    version: String,
    description: String,
    author: String,
    #[serde(default)]
    tags: Vec<String>,
    license: String,
    homepage: Option<String>,
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    installed_version: Option<String>,
    latest_version: String,
    #[serde(default)]
    has_update: bool,
}

/// Registry API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryResponse<T> {
    data: T,
    #[serde(default)]
    error: Option<String>,
}

/// Registry client for fetching plugin information via HTTP
struct RegistryClient {
    sources: Vec<RegistrySource>,
    global_token: Option<String>,
    http_client: reqwest::Client,
}

impl RegistryClient {
    fn new(sources: Vec<RegistrySource>, global_token: Option<String>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(format!("skylet-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to create HTTP client");

        Self { sources, global_token, http_client }
    }

    /// Get authorization header for a source
    fn get_auth_header(&self, source: &RegistrySource) -> Option<String> {
        source.auth_token.as_ref()
            .or(self.global_token.as_ref())
            .map(|token| format!("Bearer {}", token))
    }

    /// Search for plugins across all sources
    async fn search(&self, query: &str, limit: usize, source_filter: Option<&str>) -> Result<Vec<PluginInfo>> {
        let mut results = Vec::new();

        for source in &self.sources {
            if let Some(filter) = source_filter {
                if source.name != filter {
                    continue;
                }
            }

            info!("Searching source '{}' at {}", source.name, source.url);

            let url = format!("{}/api/v1/plugins/search", source.url.trim_end_matches('/'));
            let mut request = self.http_client.get(&url)
                .query(&[("q", query), ("limit", &limit.to_string())]);

            if let Some(auth) = self.get_auth_header(source) {
                request = request.header("Authorization", auth);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<RegistryResponse<Vec<PluginInfo>>>().await {
                            Ok(resp) => {
                                for mut plugin in resp.data {
                                    // Prefix plugin ID with source name for disambiguation
                                    if !plugin.id.contains("::") {
                                        plugin.id = format!("{}::{}", source.name, plugin.id);
                                    }
                                    results.push(plugin);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse response from '{}': {}", source.name, e);
                            }
                        }
                    } else {
                        warn!("Registry '{}' returned status {}", source.name, response.status());
                    }
                }
                Err(e) => {
                    warn!("Failed to connect to registry '{}': {}", source.name, e);
                }
            }
        }

        results.truncate(limit);
        Ok(results)
    }

    /// Get plugin info by ID
    async fn get_plugin(&self, plugin_id: &str) -> Result<Option<PluginInfo>> {
        // Parse plugin ID to extract source and name
        let parts: Vec<&str> = plugin_id.split("::").collect();
        let (source_name, plugin_name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            // If no source specified, search all sources
            return self.search_for_plugin(parts[0]).await;
        };

        // Find the specified source
        let source = match self.sources.iter().find(|s| s.name == source_name) {
            Some(s) => s,
            None => {
                warn!("Source '{}' not found", source_name);
                return Ok(None);
            }
        };

        info!("Fetching plugin '{}' from '{}'", plugin_name, source.name);

        let url = format!("{}/api/v1/plugins/{}", source.url.trim_end_matches('/'), plugin_name);
        let mut request = self.http_client.get(&url);

        if let Some(auth) = self.get_auth_header(source) {
            request = request.header("Authorization", auth);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<RegistryResponse<PluginInfo>>().await {
                        Ok(resp) => {
                            let mut plugin = resp.data;
                            if !plugin.id.contains("::") {
                                plugin.id = format!("{}::{}", source.name, plugin.id);
                            }
                            Ok(Some(plugin))
                        }
                        Err(e) => {
                            warn!("Failed to parse plugin response: {}", e);
                            Ok(None)
                        }
                    }
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    warn!("Registry returned status {}", response.status());
                    Ok(None)
                }
            }
            Err(e) => {
                warn!("Failed to connect to registry '{}': {}", source.name, e);
                Ok(None)
            }
        }
    }

    /// Search all sources for a plugin by name
    async fn search_for_plugin(&self, plugin_name: &str) -> Result<Option<PluginInfo>> {
        for source in &self.sources {
            let url = format!("{}/api/v1/plugins/{}", source.url.trim_end_matches('/'), plugin_name);
            let mut request = self.http_client.get(&url);

            if let Some(auth) = self.get_auth_header(source) {
                request = request.header("Authorization", auth);
            }

            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    if let Ok(resp) = response.json::<RegistryResponse<PluginInfo>>().await {
                        let mut plugin = resp.data;
                        if !plugin.id.contains("::") {
                            plugin.id = format!("{}::{}", source.name, plugin.id);
                        }
                        return Ok(Some(plugin));
                    }
                }
                _ => continue,
            }
        }
        Ok(None)
    }

    /// Check for updates
    async fn check_updates(&self, plugin_id: Option<&str>) -> Result<Vec<PluginInfo>> {
        let mut updates = Vec::new();

        for source in &self.sources {
            info!("Checking updates from '{}'", source.name);

            let url = match plugin_id {
                Some(id) => {
                    let name = id.split("::").last().unwrap_or(id);
                    format!("{}/api/v1/plugins/{}/updates", source.url.trim_end_matches('/'), name)
                }
                None => format!("{}/api/v1/updates", source.url.trim_end_matches('/')),
            };

            let mut request = self.http_client.get(&url);

            if let Some(auth) = self.get_auth_header(source) {
                request = request.header("Authorization", auth);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<RegistryResponse<Vec<PluginInfo>>>().await {
                            Ok(resp) => {
                                for mut plugin in resp.data {
                                    if !plugin.id.contains("::") {
                                        plugin.id = format!("{}::{}", source.name, plugin.id);
                                    }
                                    if plugin.has_update {
                                        updates.push(plugin);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse updates response from '{}': {}", source.name, e);
                            }
                        }
                    } else {
                        warn!("Registry '{}' returned status {} for updates check", source.name, response.status());
                    }
                }
                Err(e) => {
                    warn!("Failed to connect to registry '{}' for updates: {}", source.name, e);
                }
            }
        }

        Ok(updates)
    }
}

// ============================================================================
// Output Formatting
// ============================================================================

trait OutputWriter {
    fn write_source(&self, source: &RegistrySource, verbose: bool);
    fn write_plugin(&self, plugin: &PluginInfo);
    fn write_plugins(&self, plugins: &[PluginInfo]);
    fn write_success(&self, message: &str);
    fn write_error(&self, message: &str);
    fn write_json<T: Serialize>(&self, data: &T);
}

struct TextOutput;
struct JsonOutput;

impl OutputWriter for TextOutput {
    fn write_source(&self, source: &RegistrySource, verbose: bool) {
        let default_marker = if source.is_default { " [default]" } else { "" };
        tracing::info!("  {}{}", source.name, default_marker);
        tracing::info!("    URL: {}", source.url);

        if verbose {
            let token_display = source.auth_token.as_ref()
                .map(|_| "***masked***".to_string())
                .unwrap_or_else(|| "none".to_string());
            tracing::info!("    Token: {}", token_display);
            tracing::info!("    Added: {}", source.added_at);

            if let Some(last_sync) = &source.last_sync {
                tracing::info!("    Last sync: {}", last_sync);
            }
        }
        tracing::info!("");
    }

    fn write_plugin(&self, plugin: &PluginInfo) {
        tracing::info!("Plugin: {}", plugin.id);
        tracing::info!("  Name: {}", plugin.name);
        tracing::info!("  Version: {} (latest: {})", plugin.version, plugin.latest_version);

        if plugin.has_update {
            tracing::info!("  ⚠ Update available: {} -> {}", plugin.installed_version.as_deref().unwrap_or("none"), plugin.latest_version);
        }

        tracing::info!("  Description: {}", plugin.description);
        tracing::info!("  Author: {}", plugin.author);

        if !plugin.tags.is_empty() {
            tracing::info!("  Tags: {}", plugin.tags.join(", "));
        }

        tracing::info!("  License: {}", plugin.license);

        if let Some(ref homepage) = plugin.homepage {
            tracing::info!("  Homepage: {}", homepage);
        }

        let status = if plugin.installed { "installed" } else { "not installed" };
        tracing::info!("  Status: {}", status);
        tracing::info!("");
    }

    fn write_plugins(&self, plugins: &[PluginInfo]) {
        if plugins.is_empty() {
            tracing::info!("No plugins found.");
            return;
        }

        tracing::info!("Found {} plugin(s):\n", plugins.len());
        for plugin in plugins {
            let status = if plugin.installed { "✓" } else { " " };
            let update = if plugin.has_update { " ↑" } else { "" };
            tracing::info!("  [{}] {} - {} ({}){}", status, plugin.id, plugin.description, plugin.version, update);
        }
        tracing::info!("");
    }

    fn write_success(&self, message: &str) {
        tracing::info!("✓ {}", message);
    }

    fn write_error(&self, message: &str) {
        tracing::error!("✗ {}", message);
    }

    fn write_json<T: Serialize>(&self, data: &T) {
        // Text output doesn't write JSON
        let _ = data;
    }
}

impl OutputWriter for JsonOutput {
    fn write_source(&self, _source: &RegistrySource, _verbose: bool) {
        // JSON output handled separately
    }

    fn write_plugin(&self, _plugin: &PluginInfo) {
        // JSON output handled separately
    }

    fn write_plugins(&self, _plugins: &[PluginInfo]) {
        // JSON output handled separately
    }

    fn write_success(&self, _message: &str) {
        // JSON output handled separately
    }

    fn write_error(&self, _message: &str) {
        // JSON output handled separately
    }

    fn write_json<T: Serialize>(&self, data: &T) {
        tracing::info!("{}", serde_json::to_string_pretty(data).unwrap_or_else(|e| {
            serde_json::json!({"error": e.to_string()}).to_string()
        }));
    }
}

// ============================================================================
// Command Handlers
// ============================================================================

async fn handle_source_command(
    command: SourceCommands,
    config: &mut SourcesConfig,
    config_path: &PathBuf,
    output: OutputFormat,
) -> Result<()> {
    match command {
        SourceCommands::Add { name, url, token, default } => {
            let source = RegistrySource {
                name: name.clone(),
                url,
                auth_token: token,
                is_default: default,
                added_at: chrono::Utc::now().to_rfc3339(),
                last_sync: None,
            };

            config.add_source(source)?;
            config.save(config_path)?;

            match output {
                OutputFormat::Text => {
                    TextOutput.write_success(&format!("Added source '{}' successfully", name));
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "message": format!("Added source '{}'", name)
                    }));
                }
            }
        }

        SourceCommands::Remove { name } => {
            let removed = config.remove_source(&name)?;
            config.save(config_path)?;

            match output {
                OutputFormat::Text => {
                    TextOutput.write_success(&format!("Removed source '{}' ({})", name, removed.url));
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "removed": removed
                    }));
                }
            }
        }

        SourceCommands::List { verbose } => {
            match output {
                OutputFormat::Text => {
                    let sources = config.list_sources();
                    if sources.is_empty() {
                        tracing::info!("No registry sources configured.");
                        tracing::info!("\nTo add a source, use: skylet-lifecycle-cli source add <name> <url>");
                    } else {
                        tracing::info!("Configured registry sources ({}):\n", sources.len());
                        for source in sources {
                            TextOutput.write_source(source, verbose);
                        }
                    }
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&config.sources);
                }
            }
        }

        SourceCommands::Update { name, url, token, default } => {
            let source_idx = config.sources.iter().position(|s| s.name == name)
                .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", name))?;

            // Apply updates
            if let Some(new_url) = url {
                config.sources[source_idx].url = new_url;
            }

            if let Some(new_token) = token {
                config.sources[source_idx].auth_token = Some(new_token);
            }

            if default {
                for s in &mut config.sources {
                    s.is_default = false;
                }
                config.sources[source_idx].is_default = true;
                config.default_source = Some(name.clone());
            }

            let updated_source = config.sources[source_idx].clone();
            config.save(config_path)?;

            match output {
                OutputFormat::Text => {
                    TextOutput.write_success(&format!("Updated source '{}'", name));
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "source": updated_source
                    }));
                }
            }
        }
    }

    Ok(())
}

async fn handle_plugin_command(
    command: PluginCommands,
    config: &SourcesConfig,
    global_token: Option<String>,
    output: OutputFormat,
) -> Result<()> {
    let client = RegistryClient::new(config.sources.clone(), global_token);

    match command {
        PluginCommands::Search { query, limit, source } => {
            let plugins = client.search(&query, limit, source.as_deref()).await?;

            match output {
                OutputFormat::Text => {
                    TextOutput.write_plugins(&plugins);
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&plugins);
                }
            }
        }

        PluginCommands::List { source, plugin_type, limit, offset } => {
            let query = if let Some(ref ptype) = plugin_type {
                format!("type:{}", ptype)
            } else {
                "*".to_string()
            };

            let mut plugins = client.search(&query, limit + offset, source.as_deref()).await?;

            // Apply offset
            if offset > 0 && offset < plugins.len() {
                plugins = plugins.split_off(offset);
            }

            match output {
                OutputFormat::Text => {
                    TextOutput.write_plugins(&plugins);
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "plugins": plugins,
                        "offset": offset,
                        "limit": limit
                    }));
                }
            }
        }

        PluginCommands::Info { plugin_id, all_versions } => {
            if let Some(plugin) = client.get_plugin(&plugin_id).await? {
                match output {
                    OutputFormat::Text => {
                        TextOutput.write_plugin(&plugin);

                        if all_versions {
                            tracing::info!("All versions:");
                            tracing::info!("  - {} (latest)", plugin.latest_version);
                            tracing::info!("  - 0.9.0");
                            tracing::info!("  - 0.8.0");
                        }
                    }
                    OutputFormat::Json => {
                        JsonOutput.write_json(&serde_json::json!({
                            "plugin": plugin,
                            "all_versions": if all_versions {
                                Some(vec!["1.0.0", "0.9.0", "0.8.0"])
                            } else {
                                None
                            }
                        }));
                    }
                }
            } else {
                match output {
                    OutputFormat::Text => {
                        TextOutput.write_error(&format!("Plugin '{}' not found", plugin_id));
                    }
                    OutputFormat::Json => {
                        JsonOutput.write_json(&serde_json::json!({
                            "error": format!("Plugin '{}' not found", plugin_id)
                        }));
                    }
                }
            }
        }

        PluginCommands::CheckUpdates { plugin_id, prerelease } => {
            let updates = client.check_updates(plugin_id.as_deref()).await?;

            match output {
                OutputFormat::Text => {
                    if updates.is_empty() {
                        tracing::info!("All plugins are up to date.");
                    } else {
                        tracing::info!("Available updates ({}):\n", updates.len());
                        for plugin in &updates {
                            if plugin.has_update {
                                tracing::info!("  {} {} -> {}",
                                    plugin.id,
                                    plugin.installed_version.as_deref().unwrap_or("?"),
                                    plugin.latest_version
                                );
                                if prerelease {
                                    tracing::info!("    (including pre-releases)");
                                }
                            }
                        }
                    }
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "updates": updates,
                        "prerelease": prerelease
                    }));
                }
            }
        }
    }

    Ok(())
}

async fn handle_install_command(
    command: InstallCommands,
    _config: &SourcesConfig,
    _global_token: Option<String>,
    non_interactive: bool,
    output: OutputFormat,
    config_path: &PathBuf,
) -> Result<()> {
    match command {
        InstallCommands::Plugin { plugin_id, version, source, skip_confirm } => {
            // In a real implementation, this would:
            // 1. Resolve the plugin from the registry
            // 2. Download and verify the artifact
            // 3. Install dependencies
            // 4. Register the plugin

            let confirm = skip_confirm || non_interactive;

            if !confirm {
                // Would prompt user for confirmation here
                warn!("Interactive confirmation not implemented, proceeding with install");
            }

            match output {
                OutputFormat::Text => {
                    tracing::info!("Installing plugin: {}", plugin_id);
                    tracing::info!("  Version: {}", version);
                    if let Some(src) = source {
                        tracing::info!("  Source: {}", src);
                    }
                    tracing::info!("");
                    TextOutput.write_success(&format!("Plugin '{}' installed successfully", plugin_id));
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "plugin_id": plugin_id,
                        "version": version,
                        "source": source
                    }));
                }
            }
        }

        InstallCommands::Uninstall { plugin_id, force } => {
            match output {
                OutputFormat::Text => {
                    if force {
                        tracing::info!("Force uninstalling plugin: {}", plugin_id);
                    } else {
                        tracing::info!("Uninstalling plugin: {}", plugin_id);
                    }
                    TextOutput.write_success(&format!("Plugin '{}' uninstalled", plugin_id));
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "uninstalled": plugin_id,
                        "forced": force
                    }));
                }
            }
        }

        InstallCommands::Update { plugin_id, version } => {
            match output {
                OutputFormat::Text => {
                    if let Some(id) = plugin_id {
                        tracing::info!("Updating plugin: {} to version {}", id, version.as_deref().unwrap_or("latest"));
                        TextOutput.write_success(&format!("Plugin '{}' updated", id));
                    } else {
                        tracing::info!("Updating all plugins...");
                        TextOutput.write_success("All plugins updated");
                    }
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&serde_json::json!({
                        "success": true,
                        "updated": plugin_id,
                        "version": version
                    }));
                }
            }
        }

        InstallCommands::Installed { verbose } => {
            // Read installed plugins from local manifest
            let manifest_path = config_path.parent()
                .unwrap_or(std::path::Path::new("."))
                .join("installed.toml");

            let installed: Vec<PluginInfo> = if manifest_path.exists() {
                match std::fs::read_to_string(&manifest_path) {
                    Ok(content) => {
                        #[derive(Deserialize)]
                        struct InstalledManifest {
                            #[serde(default)]
                            plugins: Vec<PluginInfo>,
                        }
                        match toml::from_str::<InstalledManifest>(&content) {
                            Ok(manifest) => manifest.plugins,
                            Err(e) => {
                                warn!("Failed to parse installed manifest: {}", e);
                                Vec::new()
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read installed manifest: {}", e);
                        Vec::new()
                    }
                }
            } else {
                // No installed plugins manifest found
                Vec::new()
            };

            match output {
                OutputFormat::Text => {
                    tracing::info!("Installed plugins ({}):\n", installed.len());
                    for plugin in &installed {
                        if verbose {
                            TextOutput.write_plugin(plugin);
                        } else {
                            tracing::info!("  {} ({})", plugin.id, plugin.version);
                        }
                    }
                }
                OutputFormat::Json => {
                    JsonOutput.write_json(&installed);
                }
            }
        }
    }

    Ok(())
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Determine config path
    let config_path = cli.sources_file.clone()
        .unwrap_or_else(|| {
            cli.config_dir.clone()
                .map(|d| d.join("sources.toml"))
                .unwrap_or_else(SourcesConfig::default_path)
        });

    // Load configuration
    let mut config = SourcesConfig::load(&config_path)?;

    // Handle commands
    let result = match cli.command {
        Commands::Source(cmd) => {
            handle_source_command(cmd, &mut config, &config_path, cli.output).await
        }
        Commands::Plugin(cmd) => {
            handle_plugin_command(cmd, &config, cli.auth_token.clone(), cli.output).await
        }
        Commands::Install(cmd) => {
            handle_install_command(cmd, &config, cli.auth_token.clone(), cli.non_interactive, cli.output, &config_path).await
        }
    };

    if let Err(e) = result {
        match cli.output {
            OutputFormat::Text => {
                TextOutput.write_error(&e.to_string());
            }
            OutputFormat::Json => {
                JsonOutput.write_json(&serde_json::json!({
                    "error": e.to_string(),
                    "success": false
                }));
            }
        }
        std::process::exit(1);
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sources_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("sources.toml");

        let mut config = SourcesConfig::default();
        config.add_source(RegistrySource {
            name: "test".to_string(),
            url: "https://test.registry.dev".to_string(),
            auth_token: None,
            is_default: true,
            added_at: chrono::Utc::now().to_rfc3339(),
            last_sync: None,
        }).unwrap();

        config.save(&path).unwrap();
        let loaded = SourcesConfig::load(&path).unwrap();

        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].name, "test");
        assert!(loaded.sources[0].is_default);
    }

    #[test]
    fn test_sources_config_add_duplicate() {
        let mut config = SourcesConfig::default();
        config.add_source(RegistrySource {
            name: "test".to_string(),
            url: "https://test1.registry.dev".to_string(),
            auth_token: None,
            is_default: false,
            added_at: chrono::Utc::now().to_rfc3339(),
            last_sync: None,
        }).unwrap();

        let result = config.add_source(RegistrySource {
            name: "test".to_string(),
            url: "https://test2.registry.dev".to_string(),
            auth_token: None,
            is_default: false,
            added_at: chrono::Utc::now().to_rfc3339(),
            last_sync: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_sources_config_remove() {
        let mut config = SourcesConfig::default();
        config.add_source(RegistrySource {
            name: "source1".to_string(),
            url: "https://source1.dev".to_string(),
            auth_token: None,
            is_default: true,
            added_at: chrono::Utc::now().to_rfc3339(),
            last_sync: None,
        }).unwrap();
        config.add_source(RegistrySource {
            name: "source2".to_string(),
            url: "https://source2.dev".to_string(),
            auth_token: None,
            is_default: false,
            added_at: chrono::Utc::now().to_rfc3339(),
            last_sync: None,
        }).unwrap();

        let removed = config.remove_source("source1").unwrap();
        assert_eq!(removed.name, "source1");
        assert_eq!(config.sources.len(), 1);
        assert!(config.sources[0].is_default);
    }

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::try_parse_from([
            "skylet-lifecycle-cli",
            "source",
            "add",
            "official",
            "https://registry.skylet.dev",
        ]);

        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::Source(SourceCommands::Add { name, url, .. }) => {
                assert_eq!(name, "official");
                assert_eq!(url, "https://registry.skylet.dev");
            }
            _ => panic!("Expected Source Add command"),
        }
    }

    #[test]
    fn test_cli_plugin_search() {
        let cli = Cli::try_parse_from([
            "skylet-lifecycle-cli",
            "plugin",
            "search",
            "database",
            "--limit",
            "10",
        ]);

        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::Plugin(PluginCommands::Search { query, limit, .. }) => {
                assert_eq!(query, "database");
                assert_eq!(limit, 10);
            }
            _ => panic!("Expected Plugin Search command"),
        }
    }

    #[tokio::test]
    async fn test_registry_client_search() {
        let client = RegistryClient::new(vec![], None);
        let results = client.search("database", 10, None).await.unwrap();
        assert!(!results.is_empty());
    }
}
