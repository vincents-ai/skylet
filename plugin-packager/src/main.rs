// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use clap::{Parser, Subcommand};
use plugin_packager::{LocalRegistry, PluginRegistryEntry};
use std::fs;
use std::path::PathBuf;
use tracing;

#[derive(Parser)]
#[command(name = "skylet-plugin")]
#[command(version = "0.1.0")]
#[command(about = "Skylet Plugin Management Tool - Package, verify, and manage plugins", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Packaging commands
    #[command(subcommand)]
    Package(PackageCommands),

    /// Plugin management commands
    #[command(subcommand)]
    Plugin(PluginCommands),
}

#[derive(Subcommand)]
enum PackageCommands {
    /// Pack a plugin directory into a .tar.gz artifact
    Pack {
        /// Source plugin directory
        src: PathBuf,
        /// Output artifact path
        out: PathBuf,
    },
    /// Pack a plugin directory with RFC-0003 compliant target-specific naming
    PackTarget {
        /// Source plugin directory
        src: PathBuf,
        /// Output directory for artifact
        #[arg(short, long)]
        output_dir: PathBuf,
        /// Target triple (e.g., x86_64-unknown-linux-gnu)
        #[arg(short, long)]
        target: String,
    },
    /// Verify an artifact and its checksum
    Verify {
        /// Artifact file path
        artifact: PathBuf,
        /// Optional checksum file path
        checksum: Option<PathBuf>,
    },
    /// Publish an artifact to a registry (RFC-0003)
    Publish {
        /// Path to the artifact file (.tar.gz)
        artifact: PathBuf,
        /// Registry URL (e.g., https://marketplace.skylet.dev)
        #[arg(short, long)]
        registry: String,
        /// Authentication token
        #[arg(short = 't', long)]
        token: String,
        /// Skip verification before publishing
        #[arg(long)]
        skip_verify: bool,
    },
    /// Validate an artifact without publishing
    ValidateArtifact {
        /// Path to the artifact file
        artifact: PathBuf,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// Create a new plugin from template
    New {
        /// Plugin name (e.g., my-api-plugin)
        name: String,
        /// Template to use: api-integration, database, communication, basic
        #[arg(short, long, default_value = "basic")]
        template: String,
        /// Output directory (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Plugin description
        #[arg(short, long)]
        description: Option<String>,
        /// Plugin author
        #[arg(short, long)]
        author: Option<String>,
        /// Initialize git repository
        #[arg(long)]
        git: bool,
    },

    /// List available plugins
    List {
        /// Filter by plugin type (optional)
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Show plugin information
    Info {
        /// Plugin name or path
        plugin: PathBuf,
    },

    /// Install a plugin from archive
    Install {
        /// Path to plugin archive (.tar.gz)
        archive: PathBuf,
        /// Installation directory (default: ~/.skylet/plugins)
        #[arg(short, long)]
        dest: Option<PathBuf>,
    },

    /// Remove an installed plugin
    Remove {
        /// Plugin name
        name: String,
        /// Plugin directory (default: ~/.skylet/plugins)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },

    /// Search for plugins
    Search {
        /// Search query
        query: String,
    },

    /// Validate a plugin manifest
    Validate {
        /// Path to plugin.toml
        manifest: PathBuf,
    },

    /// Upgrade an installed plugin
    Upgrade {
        /// Plugin name to upgrade
        name: String,
        /// Path to new plugin archive (.tar.gz)
        archive: PathBuf,
        /// Skip backup before upgrade
        #[arg(long)]
        no_backup: bool,
        /// Allow breaking changes (major version bump)
        #[arg(long)]
        allow_breaking: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Package(pcmd) => handle_package_commands(pcmd),
        Commands::Plugin(pcmd) => handle_plugin_commands(pcmd),
    }
}

fn handle_package_commands(cmd: PackageCommands) -> anyhow::Result<()> {
    match cmd {
        PackageCommands::Pack { src, out } => {
            let checksum = plugin_packager::pack_dir(&src, &out)
                .with_context(|| "failed to pack directory")?;
            tracing::info!("Wrote artifact: {}", out.display());
            tracing::info!("Wrote checksum: {}", checksum.display());
            Ok(())
        }
        PackageCommands::PackTarget {
            src,
            output_dir,
            target,
        } => {
            let checksum = plugin_packager::pack_dir_with_target(&src, &output_dir, &target)
                .with_context(|| "failed to pack directory with target")?;
            // The artifact name is constructed as: <name>-v<version>-<target>.tar.gz
            // Extract just the artifact name from the checksum path
            let artifact_name = checksum
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.trim_end_matches(".sha256"))
                .unwrap_or("artifact.tar.gz");
            let artifact_path = output_dir.join(artifact_name);
            tracing::info!("Wrote artifact: {}", artifact_path.display());
            tracing::info!("Wrote checksum: {}", checksum.display());
            Ok(())
        }
        PackageCommands::Verify { artifact, checksum } => {
            plugin_packager::verify_artifact(&artifact, checksum.as_deref())
                .with_context(|| "verification failed")?;
            tracing::info!("OK: artifact verified");
            Ok(())
        }
        PackageCommands::Publish {
            artifact,
            registry,
            token,
            skip_verify,
        } => handle_publish(&artifact, &registry, &token, skip_verify),
        PackageCommands::ValidateArtifact { artifact } => handle_validate_artifact(&artifact),
    }
}

fn handle_plugin_commands(cmd: PluginCommands) -> anyhow::Result<()> {
    match cmd {
        PluginCommands::New {
            name,
            template,
            output,
            description,
            author,
            git,
        } => new_plugin(&name, &template, output, description, author, git),
        PluginCommands::List { filter } => list_plugins(filter),
        PluginCommands::Info { plugin } => show_plugin_info(&plugin),
        PluginCommands::Install { archive, dest } => install_plugin(&archive, dest),
        PluginCommands::Remove { name, dir } => remove_plugin(&name, dir),
        PluginCommands::Search { query } => search_plugins(&query),
        PluginCommands::Validate { manifest } => validate_manifest(&manifest),
        PluginCommands::Upgrade {
            name,
            archive,
            no_backup,
            allow_breaking,
        } => upgrade_plugin(&name, &archive, no_backup, allow_breaking),
    }
}

/// Create a new plugin from template
fn new_plugin(
    name: &str,
    template: &str,
    output: Option<PathBuf>,
    description: Option<String>,
    author: Option<String>,
    git: bool,
) -> anyhow::Result<()> {
    use std::process::Command;

    // Validate plugin name (must be valid Rust crate name)
    if !is_valid_crate_name(name) {
        anyhow::bail!(
            "Invalid plugin name '{}'. Must be lowercase, alphanumeric with hyphens, and start with a letter.",
            name
        );
    }

    // Determine output directory
    let output_dir = output.unwrap_or_else(|| PathBuf::from(".")).join(name);

    if output_dir.exists() {
        anyhow::bail!(
            "Directory already exists: {}. Use a different name or remove the existing directory.",
            output_dir.display()
        );
    }

    tracing::info!(
        "Creating new plugin '{}' from '{}' template...",
        name,
        template
    );

    // Create directory structure
    fs::create_dir_all(output_dir.join("src"))?;

    // Get template content
    let (cargo_toml, lib_rs, plugin_toml) =
        get_template_files(name, template, &description, &author)?;

    // Write files
    fs::write(output_dir.join("Cargo.toml"), cargo_toml)?;
    fs::write(output_dir.join("src/lib.rs"), lib_rs)?;
    fs::write(output_dir.join("plugin.toml"), plugin_toml)?;
    fs::write(
        output_dir.join("README.md"),
        get_readme_template(name, template, &description),
    )?;
    fs::write(output_dir.join(".gitignore"), get_gitignore_template())?;

    tracing::info!("  Created {}/Cargo.toml", name);
    tracing::info!("  Created {}/src/lib.rs", name);
    tracing::info!("  Created {}/plugin.toml", name);
    tracing::info!("  Created {}/README.md", name);
    tracing::info!("  Created {}/.gitignore", name);

    // Initialize git if requested
    if git {
        tracing::info!("Initializing git repository...");
        let status = Command::new("git")
            .args(["init"])
            .current_dir(&output_dir)
            .status();

        if let Ok(s) = status {
            if s.success() {
                tracing::info!("  Initialized git repository");
            }
        }
    }

    tracing::info!("");
    tracing::info!("✓ Plugin '{}' created successfully!", name);
    tracing::info!("");
    tracing::info!("Next steps:");
    tracing::info!("  cd {}", name);
    tracing::info!("  cargo build --release");
    tracing::info!("  skylet-plugin package pack . {}.tar.gz", name);
    tracing::info!("");
    tracing::info!("For development:");
    tracing::info!("  cargo test");
    tracing::info!("  cargo clippy");

    Ok(())
}

/// Validate crate name follows Rust conventions
fn is_valid_crate_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_lowercase() {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Get template files for a plugin
fn get_template_files(
    name: &str,
    template: &str,
    description: &Option<String>,
    author: &Option<String>,
) -> anyhow::Result<(String, String, String)> {
    let desc = description
        .clone()
        .unwrap_or_else(|| format!("A {} plugin for Skylet", template));
    let auth = author
        .clone()
        .unwrap_or_else(|| "Plugin Author".to_string());
    let snake_name = name.replace('-', "_");

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{desc}"
authors = ["{auth}"]
license = "MIT"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = {{ version = "0.2" }}
skylet-plugin-common = {{ version = "0.5" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
anyhow = "1.0"
{extra_deps}

[dev-dependencies]
"#,
        name = name,
        desc = desc,
        auth = auth,
        extra_deps = get_extra_deps(template),
    );

    let lib_rs = get_lib_rs_template(name, &snake_name, template, &desc);
    let plugin_toml = get_plugin_toml_template(name, &desc, &auth, template);

    Ok((cargo_toml, lib_rs, plugin_toml))
}

/// Get extra dependencies based on template type
fn get_extra_deps(template: &str) -> String {
    match template {
        "api-integration" => r#"ureq = "2.9"
tokio = { version = "1.0", features = ["rt", "macros"] }"#
            .to_string(),
        "database" => r#"sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }
tokio = { version = "1.0", features = ["rt", "macros"] }"#
            .to_string(),
        "communication" => {
            r#"tokio = { version = "1.0", features = ["rt", "macros", "sync"] }"#.to_string()
        }
        _ => String::new(),
    }
}

/// Get lib.rs template using V2 ABI
fn get_lib_rs_template(name: &str, _snake_name: &str, template: &str, description: &str) -> String {
    let plugin_type = match template {
        "api-integration" => "Integration",
        "database" => "Database",
        "communication" => "Communication",
        _ => "Extension",
    };

    format!(
        r#"//! {description}
//!
//! This plugin was generated with `skylet-plugin plugin new`.

use skylet_plugin_common::{{
    skylet_plugin_v2, CapabilityBuilder, ServiceInfoBuilder, TagsBuilder,
    static_cstr, cstr_ptr,
}};
use skylet_abi::v2::*;
use serde_json::json;
use std::ffi::{{CStr, CString}};
use std::os::raw::c_char;

// Plugin static strings
static_cstr!(PLUGIN_NAME, "{name}");
static_cstr!(PLUGIN_VERSION, "0.1.0");
static_cstr!(PLUGIN_DESCRIPTION, "{description}");
static_cstr!(PLUGIN_AUTHOR, "Plugin Author");

// Define capabilities
static CAPABILITIES: &[&CStr] = &[
    // Add your plugin capabilities here
    // Example: c"read_config", c"write_data"
];

// Define tags
static TAGS: &[&CStr] = &[
    // Add your plugin tags here
    // Example: c"{template}", c"skylet"
];

/// Generate the V2 ABI entry points
skylet_plugin_v2!(
    name: PLUGIN_NAME,
    version: PLUGIN_VERSION,
    description: PLUGIN_DESCRIPTION,
    author: PLUGIN_AUTHOR,
    plugin_type: PluginType::{plugin_type},
    capabilities: CAPABILITIES,
    tags: TAGS,
    init: plugin_init_impl,
    shutdown: plugin_shutdown_impl,
    execute: plugin_execute_impl,
);

/// Plugin initialization
fn plugin_init_impl(_ctx: *const PluginContextV2) -> PluginResultV2 {{
    // Initialize your plugin state here
    // Example: set up connections, load config, etc.
    PluginResultV2::Success
}}

/// Plugin shutdown
fn plugin_shutdown_impl(_ctx: *const PluginContextV2) -> PluginResultV2 {{
    // Clean up plugin resources here
    // Example: close connections, flush buffers, etc.
    PluginResultV2::Success
}}

/// Plugin execute - main entry point for plugin actions
fn plugin_execute_impl(
    _ctx: *const PluginContextV2,
    action: *const c_char,
    args: *const c_char,
) -> *mut c_char {{
    // Parse action and arguments
    let action_str = if action.is_null() {{
        ""
    }} else {{
        unsafe {{ CStr::from_ptr(action) }}.to_str().unwrap_or("")
    }};

    let args_str = if args.is_null() {{
        "{{}}"
    }} else {{
        unsafe {{ CStr::from_ptr(args) }}.to_str().unwrap_or("{{}}")
    }};

    // Parse JSON arguments
    let args_json: serde_json::Value = serde_json::from_str(args_str).unwrap_or(json!({{}}));

    // Handle different actions
    let result = match action_str {{
        "ping" => handle_ping(&args_json),
        "info" => handle_info(),
        _ => Err(format!("Unknown action: {{}}", action_str)),
    }};

    // Convert result to JSON string
    let response = match result {{
        Ok(value) => json!({{ "success": true, "data": value }}),
        Err(error) => json!({{ "success": false, "error": error }}),
    }};

    // Return as C string (caller must free)
    match CString::new(response.to_string()) {{
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }}
}}

/// Handle ping action
fn handle_ping(args: &serde_json::Value) -> Result<serde_json::Value, String> {{
    let message = args.get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("pong");
    
    Ok(json!({{
        "response": message,
        "plugin": "{name}",
        "version": "0.1.0"
    }}))
}}

/// Handle info action
fn handle_info() -> Result<serde_json::Value, String> {{
    Ok(json!({{
        "name": "{name}",
        "version": "0.1.0",
        "description": "{description}",
        "abi_version": "2.0.0"
    }}))
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_handle_ping() {{
        let args = json!({{"message": "hello"}});
        let result = handle_ping(&args).unwrap();
        assert_eq!(result["response"], "hello");
        assert_eq!(result["plugin"], "{name}");
    }}

    #[test]
    fn test_handle_info() {{
        let result = handle_info().unwrap();
        assert_eq!(result["name"], "{name}");
        assert_eq!(result["abi_version"], "2.0.0");
    }}
}}
"#,
        name = name,
        description = description,
        plugin_type = plugin_type,
        template = template,
    )
}

/// Get plugin.toml manifest template
fn get_plugin_toml_template(name: &str, description: &str, author: &str, template: &str) -> String {
    let category = match template {
        "api-integration" => "integration",
        "database" => "database",
        "communication" => "communication",
        _ => "extension",
    };

    format!(
        r#"# Skylet Plugin Manifest (RFC-0003 compliant)
# See: https://github.com/vincents-ai/skylet/blob/main/docs/PLUGIN_CONTRACT.md

[package]
name = "{name}"
version = "0.1.0"
abi_version = "2.0.0"
description = "{description}"
authors = ["{author}"]
license = "MIT"
repository = ""
homepage = ""

[plugin]
type = "{category}"
entry_point = "lib{snake_name}"
min_skylet_version = "0.2.0"

# Capabilities this plugin provides
capabilities = []

# Permissions this plugin requires
[permissions]
network = false
filesystem = false
secrets = false

# Plugin-specific configuration schema
[config]
# Define your config schema here
# Example:
# api_url = {{ type = "string", required = true }}
# timeout_seconds = {{ type = "integer", default = 30 }}

# Build configuration
[build]
# Target triples to build for
targets = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
]
"#,
        name = name,
        description = description,
        author = author,
        category = category,
        snake_name = name.replace('-', "_"),
    )
}

/// Get README template
fn get_readme_template(name: &str, template: &str, description: &Option<String>) -> String {
    let desc = description
        .clone()
        .unwrap_or_else(|| format!("A {} plugin for Skylet", template));

    format!(
        r#"# {name}

{desc}

## Overview

This plugin was generated using `skylet-plugin plugin new` with the `{template}` template.

## Building

```bash
cargo build --release
```

The plugin binary will be at `target/release/lib{snake_name}.so` (Linux) or `.dylib` (macOS).

## Packaging

```bash
skylet-plugin package pack . {name}.tar.gz
```

## Testing

```bash
cargo test
```

## Configuration

Configure the plugin in your Skylet config:

```toml
[[plugins]]
name = "{name}"
path = "/path/to/lib{snake_name}.so"
enabled = true

[plugins.config]
# Add your configuration here
```

## Actions

This plugin supports the following actions:

- `ping` - Health check, returns plugin info
- `info` - Returns plugin metadata

## License

MIT
"#,
        name = name,
        desc = desc,
        template = template,
        snake_name = name.replace('-', "_"),
    )
}

/// Get .gitignore template
fn get_gitignore_template() -> String {
    r#"/target
Cargo.lock
*.so
*.dylib
*.dll
*.tar.gz
*.sha256
.env
.env.local
"#
    .to_string()
}

fn list_plugins(_filter: Option<String>) -> anyhow::Result<()> {
    tracing::info!("Available plugins:\n");

    // Check default installation directory
    let plugin_dir = get_plugin_dir()?;

    if !plugin_dir.exists() {
        tracing::info!("No plugins installed yet. Install plugins with: skylet-plugin plugin install <archive>");
        return Ok(());
    }

    let entries = fs::read_dir(&plugin_dir)?;
    let mut plugins = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("plugin.toml");
            if manifest_path.exists() {
                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Ok(name) = extract_name_from_manifest(&content) {
                        if let Ok(version) = extract_version_from_manifest(&content) {
                            plugins.push((name, version));
                        }
                    }
                }
            }
        }
    }

    if plugins.is_empty() {
        tracing::info!("No plugins found in {}", plugin_dir.display());
    } else {
        tracing::info!("  {:30} Version", "Name");
        tracing::info!("  {}", "-".repeat(45));
        for (name, version) in plugins {
            tracing::info!("  {:30} {}", name, version);
        }
    }

    Ok(())
}

fn show_plugin_info(plugin: &PathBuf) -> anyhow::Result<()> {
    let manifest_path = if plugin.is_dir() {
        plugin.join("plugin.toml")
    } else {
        plugin.clone()
    };

    if !manifest_path.exists() {
        anyhow::bail!("Plugin manifest not found: {}", manifest_path.display());
    }

    let content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read manifest: {}", manifest_path.display()))?;

    tracing::info!("Plugin Information:");
    tracing::info!("{}", content);

    Ok(())
}

fn install_plugin(archive: &PathBuf, dest: Option<PathBuf>) -> anyhow::Result<()> {
    // Verify archive first
    plugin_packager::verify_artifact(archive, None)
        .with_context(|| "archive verification failed")?;

    let dest_dir = if let Some(d) = dest {
        d
    } else {
        get_plugin_dir()?
    };
    fs::create_dir_all(&dest_dir).with_context(|| {
        format!(
            "failed to create destination directory: {}",
            dest_dir.display()
        )
    })?;

    // Extract archive
    let tar_file = std::fs::File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(tar_file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(&dest_dir)
        .with_context(|| "failed to extract archive")?;

    // Try to register in local registry
    // Get the extracted plugin directory (should be the only dir in dest_dir)
    if let Ok(entries) = fs::read_dir(&dest_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    if let Ok(content) = fs::read_to_string(&manifest_path) {
                        if let (Ok(name), Ok(version), Ok(abi)) = (
                            extract_name_from_manifest(&content),
                            extract_version_from_manifest(&content),
                            extract_abi_from_manifest(&content),
                        ) {
                            // Create registry entry and register
                            let entry = PluginRegistryEntry {
                                plugin_id: name.clone(),
                                name,
                                version,
                                abi_version: abi,
                                description: None,
                                author: None,
                                license: None,
                                keywords: None,
                                dependencies: None,
                            };

                            // Load existing registry or create new one
                            let registry_file = get_registry_file()?;
                            let mut registry = if registry_file.exists() {
                                plugin_packager::RegistryPersistence::load(&registry_file)
                                    .unwrap_or_else(|_| LocalRegistry::new())
                            } else {
                                LocalRegistry::new()
                            };

                            // Register the plugin
                            registry.register(entry)?;

                            // Save registry
                            let _ = plugin_packager::RegistryPersistence::save(
                                &registry,
                                &registry_file,
                            );
                            tracing::info!("  → Registered in local registry");
                        }
                    }
                }
            }
        }
    }

    tracing::info!("Plugin installed successfully to: {}", dest_dir.display());
    Ok(())
}

fn remove_plugin(name: &str, dir: Option<PathBuf>) -> anyhow::Result<()> {
    let plugin_dir = if let Some(d) = dir {
        d
    } else {
        get_plugin_dir()?
    };
    let plugin_path = plugin_dir.join(name);

    if !plugin_path.exists() {
        anyhow::bail!("Plugin not found: {}", plugin_path.display());
    }

    fs::remove_dir_all(&plugin_path)
        .with_context(|| format!("failed to remove plugin: {}", plugin_path.display()))?;

    tracing::info!("Plugin '{}' removed successfully", name);
    Ok(())
}

fn search_plugins(query: &str) -> anyhow::Result<()> {
    tracing::info!("Searching for plugins matching: '{}'\n", query);

    // Try to load registry first
    let registry_file = get_registry_file()?;
    if registry_file.exists() {
        match plugin_packager::RegistryPersistence::load(&registry_file) {
            Ok(registry) => {
                let results = registry.search(query);
                if !results.is_empty() {
                    tracing::info!("Found in registry:\n");
                    for entry in &results {
                        tracing::info!("  {} (v{})", entry.name, entry.version);
                        if let Some(desc) = &entry.description {
                            tracing::info!("    {}", desc);
                        }
                    }
                }
            }
            Err(_) => {
                // Registry file corrupted or empty, fall back to filesystem
            }
        }
    }

    // Also search in filesystem
    let plugin_dir = get_plugin_dir()?;
    if !plugin_dir.exists() {
        tracing::info!("No plugins installed yet.");
        return Ok(());
    }

    let entries = fs::read_dir(&plugin_dir)?;
    let mut found = 0;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(dir_name) = path.file_name() {
                if let Some(name) = dir_name.to_str() {
                    if name.to_lowercase().contains(&query.to_lowercase()) {
                        let manifest_path = path.join("plugin.toml");
                        if let Ok(content) = fs::read_to_string(&manifest_path) {
                            if let (Ok(name), Ok(version)) = (
                                extract_name_from_manifest(&content),
                                extract_version_from_manifest(&content),
                            ) {
                                tracing::info!("  {} (v{})", name, version);
                                found += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if found == 0 {
        tracing::info!("No plugins found matching '{}'", query);
    } else {
        tracing::info!("\nFound {} plugin(s)", found);
    }

    Ok(())
}

fn validate_manifest(manifest: &PathBuf) -> anyhow::Result<()> {
    let content = fs::read_to_string(manifest)
        .with_context(|| format!("failed to read manifest: {}", manifest.display()))?;

    // Try to parse as TOML
    let _parsed: toml::Value =
        toml::from_str(&content).with_context(|| "failed to parse manifest as TOML")?;

    // Check for required fields
    if !content.contains("name") {
        anyhow::bail!("Missing required field: 'name'");
    }
    if !content.contains("version") {
        anyhow::bail!("Missing required field: 'version'");
    }
    if !content.contains("abi_version") {
        anyhow::bail!("Missing required field: 'abi_version'");
    }

    tracing::info!("✓ Manifest is valid");
    tracing::info!(
        "  Name:        {}",
        extract_name_from_manifest(&content).unwrap_or_default()
    );
    tracing::info!(
        "  Version:     {}",
        extract_version_from_manifest(&content).unwrap_or_default()
    );
    tracing::info!(
        "  ABI Version: {}",
        extract_abi_from_manifest(&content).unwrap_or_default()
    );

    Ok(())
}

fn extract_manifest_from_archive(archive: &PathBuf) -> anyhow::Result<String> {
    use std::io::Read;

    let tar_file = std::fs::File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(tar_file);
    let mut tar_archive = tar::Archive::new(decoder);

    for entry in tar_archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.file_name().and_then(|n| n.to_str()) == Some("plugin.toml") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            return Ok(content);
        }
    }

    anyhow::bail!("plugin.toml not found in archive")
}

fn upgrade_plugin(
    name: &str,
    archive: &PathBuf,
    no_backup: bool,
    allow_breaking: bool,
) -> anyhow::Result<()> {
    use plugin_packager::BackupManager;

    // Get plugin directory
    let plugin_dir = get_plugin_dir()?;
    let plugin_path = plugin_dir.join(name);

    // Check plugin exists
    if !plugin_path.exists() {
        anyhow::bail!("Plugin '{}' not found", name);
    }

    // Get current version from manifest
    let current_manifest = plugin_path.join("plugin.toml");
    if !current_manifest.exists() {
        anyhow::bail!("Plugin manifest not found: {}", current_manifest.display());
    }

    let current_version = extract_version_from_manifest(&fs::read_to_string(&current_manifest)?)?;

    // Verify the new archive first
    plugin_packager::verify_artifact(archive, None)
        .with_context(|| "archive verification failed")?;

    // Get new version from archive
    let new_version = extract_version_from_manifest(
        &extract_manifest_from_archive(archive)
            .with_context(|| "failed to read manifest from archive")?,
    )?;

    // Check if upgrade is available
    let upgrade_info = plugin_packager::UpgradeInfo::new(
        name.to_string(),
        current_version.clone(),
        new_version.clone(),
    )?;

    if !plugin_packager::upgrade::UpgradeInfo::is_available(&current_version, &new_version)? {
        tracing::info!(
            "Plugin '{}' is already at version {} or newer",
            name,
            current_version
        );
        return Ok(());
    }

    // Check for breaking changes
    if upgrade_info.is_breaking && !allow_breaking {
        tracing::info!(
            "Breaking change detected (v{} -> v{})",
            current_version,
            new_version
        );
        tracing::info!("Use --allow-breaking to proceed with breaking changes");
        return Ok(());
    }

    // Backup if enabled
    let backup_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?
        .join(".skylet/backups");
    let mut backup_manager = BackupManager::new(backup_dir)?;

    let backup_path = if !no_backup {
        tracing::info!(
            "Creating backup of plugin '{}' v{}...",
            name,
            current_version
        );
        backup_manager.backup_plugin(name, &current_version, &plugin_path)?
    } else {
        tracing::info!("Skipping backup (use --no-backup flag)");
        PathBuf::new()
    };

    // Remove old version
    tracing::info!("Removing old plugin version...");
    fs::remove_dir_all(&plugin_path)?;

    // Extract new version
    tracing::info!("Installing new plugin version...");
    let tar_file = std::fs::File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(tar_file);
    let mut tar_archive = tar::Archive::new(decoder);
    tar_archive
        .unpack(&plugin_dir)
        .with_context(|| "failed to extract archive")?;

    // Update registry
    let registry_file = get_registry_file()?;
    let mut registry = if registry_file.exists() {
        plugin_packager::RegistryPersistence::load(&registry_file)
            .unwrap_or_else(|_| LocalRegistry::new())
    } else {
        LocalRegistry::new()
    };

    // Remove old entry and add new one
    let abi_version = extract_abi_from_manifest(
        &extract_manifest_from_archive(archive)
            .with_context(|| "failed to read manifest from archive")?,
    )?;
    let entry = PluginRegistryEntry {
        plugin_id: name.to_string(),
        name: name.to_string(),
        version: new_version.clone(),
        abi_version,
        description: None,
        author: None,
        license: None,
        keywords: None,
        dependencies: None,
    };

    registry.register(entry)?;
    let _ = plugin_packager::RegistryPersistence::save(&registry, &registry_file);

    tracing::info!(
        "✓ Plugin upgraded successfully: {} (v{} -> v{})",
        name,
        current_version,
        new_version
    );

    if !no_backup {
        tracing::info!("  Backup available at: {}", backup_path.display());
    }

    Ok(())
}

fn get_plugin_dir() -> anyhow::Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    Ok(home.join(".skylet/plugins"))
}

fn get_registry_file() -> anyhow::Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    Ok(home.join(".skylet/registry.json"))
}

fn extract_name_from_manifest(content: &str) -> anyhow::Result<String> {
    let value: toml::Value = toml::from_str(content)?;

    // Try [package] section first (v2 format)
    if let Some(package) = value.get("package") {
        if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
            return Ok(name.to_string());
        }
    }

    // Fall back to top-level name (v1 format)
    if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
        return Ok(name.to_string());
    }

    Err(anyhow::anyhow!("name not found in manifest"))
}

fn extract_version_from_manifest(content: &str) -> anyhow::Result<String> {
    let value: toml::Value = toml::from_str(content)?;

    // Try [package] section first (v2 format)
    if let Some(package) = value.get("package") {
        if let Some(version) = package.get("version").and_then(|v| v.as_str()) {
            return Ok(version.to_string());
        }
    }

    // Fall back to top-level version (v1 format)
    if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
        return Ok(version.to_string());
    }

    Err(anyhow::anyhow!("version not found in manifest"))
}

fn extract_abi_from_manifest(content: &str) -> anyhow::Result<String> {
    let value: toml::Value = toml::from_str(content)?;

    // Try [package] section first (v2 format)
    if let Some(package) = value.get("package") {
        if let Some(abi) = package.get("abi_version").and_then(|v| v.as_str()) {
            return Ok(abi.to_string());
        }
    }

    // Fall back to top-level abi_version (v1 format)
    if let Some(abi) = value.get("abi_version").and_then(|v| v.as_str()) {
        return Ok(abi.to_string());
    }

    Err(anyhow::anyhow!("abi_version not found in manifest"))
}

/// Handle artifact publishing to registry (RFC-0003)
fn handle_publish(
    artifact_path: &PathBuf,
    registry_url: &str,
    token: &str,
    skip_verify: bool,
) -> anyhow::Result<()> {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    tracing::info!("Publishing artifact: {}", artifact_path.display());
    tracing::info!("Registry: {}", registry_url);

    // Validate artifact exists
    if !artifact_path.exists() {
        anyhow::bail!("Artifact not found: {}", artifact_path.display());
    }

    // Create publisher config
    let config = PublishConfig {
        registry_url: registry_url.to_string(),
        auth_token: token.to_string(),
        skip_verify,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);

    // Validate artifact first (unless skipped)
    if !skip_verify {
        tracing::info!("Validating artifact...");
        let local_artifact = publisher
            .validate(artifact_path)
            .with_context(|| "Artifact validation failed")?;

        tracing::info!("  Plugin:    {}", local_artifact.metadata.name);
        tracing::info!("  Version:   {}", local_artifact.metadata.version);
        tracing::info!("  Target:    {}", local_artifact.metadata.target_triple);
        tracing::info!("  Checksum:  {}", &local_artifact.checksum[..16]);
    }

    // Note: Actual publishing requires async runtime
    // For now, we'll show a message and the command that would be run
    tracing::info!("Ready to publish. Run with tokio runtime for actual upload:");
    tracing::info!("  Artifact: {}", artifact_path.display());
    tracing::info!("  Registry: {}", registry_url);
    tracing::info!("Note: Async publishing is available programmatically via:");
    tracing::info!("  plugin_packager::publish::ArtifactPublisher::publish()");

    Ok(())
}

/// Handle artifact validation without publishing (RFC-0003)
fn handle_validate_artifact(artifact_path: &PathBuf) -> anyhow::Result<()> {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    tracing::info!("Validating artifact: {}", artifact_path.display());

    // Create a dummy publisher just for validation
    let config = PublishConfig {
        registry_url: "http://localhost".to_string(),
        auth_token: "".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);

    match publisher.validate(artifact_path) {
        Ok(local_artifact) => {
            tracing::info!("✓ Artifact is valid (RFC-0003 compliant)");
            tracing::info!("Metadata:");
            tracing::info!("  Plugin:     {}", local_artifact.metadata.name);
            tracing::info!("  Version:    {}", local_artifact.metadata.version);
            tracing::info!("  Target:     {}", local_artifact.metadata.target_triple);
            tracing::info!("  Platform:   {:?}", local_artifact.metadata.platform);
            tracing::info!("  Checksum:   {}", local_artifact.checksum);
            tracing::info!("Artifact path: {}", local_artifact.path.display());
            Ok(())
        }
        Err(e) => {
            tracing::info!("✗ Artifact validation failed:");
            tracing::info!("  {}", e);
            Err(e)
        }
    }
}
