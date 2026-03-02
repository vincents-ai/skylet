// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Plugin artifact publishing to registry
///
/// This module provides functionality for publishing plugin artifacts to
/// registries. It handles:
/// - Artifact validation before publishing
/// - Checksum computation and verification
/// - Publishing metadata extraction from artifacts
///
/// RFC-0003: Plugin Package and Artifact Specification
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::platform::ArtifactMetadata;

/// Publishing client for plugin artifacts
#[derive(Clone)]
pub struct ArtifactPublisher {
    config: PublishConfig,
}

/// Configuration for artifact publishing
#[derive(Debug, Clone)]
pub struct PublishConfig {
    /// Registry base URL (e.g., "https://registry.example.com")
    pub registry_url: String,
    /// Authentication token (required for publishing)
    pub auth_token: String,
    /// Whether to skip verification before publishing
    pub skip_verify: bool,
    /// Whether to publish as draft (not publicly visible)
    pub as_draft: bool,
    /// Whether to sign the artifact before publishing
    pub sign: bool,
    /// Signing key ID (if signing)
    pub key_id: Option<String>,
}

/// Result of a publish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPublishResult {
    /// Plugin ID in the registry
    pub plugin_id: String,
    /// Published version
    pub version: String,
    /// Artifact download URL
    pub download_url: String,
    /// Artifact checksum (SHA256)
    pub checksum: String,
    /// Publish status
    pub status: String,
    /// Human-readable message
    pub message: String,
    /// URL to view plugin in registry (if available)
    pub registry_url: Option<String>,
}

/// Local artifact information for publishing
#[derive(Debug, Clone)]
pub struct LocalArtifact {
    /// Path to the artifact file
    pub path: std::path::PathBuf,
    /// Artifact metadata parsed from filename
    pub metadata: ArtifactMetadata,
    /// Computed SHA256 checksum
    pub checksum: String,
}

impl ArtifactPublisher {
    /// Create a new artifact publisher
    pub fn new(config: PublishConfig) -> Self {
        Self { config }
    }

    /// Validate an artifact without publishing
    ///
    /// Returns metadata about the artifact if valid
    pub fn validate(&self, artifact_path: &Path) -> Result<LocalArtifact> {
        if !artifact_path.exists() {
            return Err(anyhow!("Artifact not found: {}", artifact_path.display()));
        }

        let filename = artifact_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid artifact filename")?;

        let metadata = ArtifactMetadata::parse(filename)?;

        crate::verify_artifact(artifact_path, None)?;

        let checksum = compute_artifact_checksum(artifact_path)?;

        Ok(LocalArtifact {
            path: artifact_path.to_path_buf(),
            metadata,
            checksum,
        })
    }

    /// Check if the publisher has valid authentication
    pub fn is_authenticated(&self) -> bool {
        !self.config.auth_token.is_empty()
    }

    /// Get the configured registry URL
    pub fn registry_url(&self) -> &str {
        &self.config.registry_url
    }
}

/// Compute SHA256 checksum of an artifact
fn compute_artifact_checksum(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Extract plugin metadata from an artifact
#[allow(dead_code)]
fn extract_plugin_metadata(artifact_path: &Path) -> Result<PluginMetadataExtract> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use std::io::Read;

    let file = File::open(artifact_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    // Find and extract plugin.toml
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.file_name().and_then(|n| n.to_str()) == Some("plugin.toml") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;

            return parse_plugin_toml(&content);
        }
    }

    Err(anyhow!("plugin.toml not found in artifact"))
}

/// Plugin metadata extracted from artifact
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct PluginMetadataExtract {
    abi_version: String,
    description: Option<String>,
    author: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    homepage: Option<String>,
    documentation: Option<String>,
    plugin_type: Option<String>,
    maturity: Option<String>,
    provides_services: Option<Vec<String>>,
    requires_services: Option<Vec<String>>,
}

/// Parse plugin.toml content
#[allow(dead_code)]
fn parse_plugin_toml(content: &str) -> Result<PluginMetadataExtract> {
    let value: toml::Value = toml::from_str(content)?;

    let get_str = |key: &str| -> Option<String> {
        value
            .get("package")
            .and_then(|p| p.get(key))
            .or_else(|| value.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let get_vec = |key: &str| -> Option<Vec<String>> {
        value
            .get("package")
            .and_then(|p| p.get(key))
            .or_else(|| value.get(key))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
    };

    Ok(PluginMetadataExtract {
        abi_version: get_str("abi_version").unwrap_or_else(|| "1".to_string()),
        description: get_str("description"),
        author: get_str("author"),
        license: get_str("license"),
        repository: get_str("repository"),
        keywords: get_vec("keywords"),
        categories: get_vec("categories"),
        homepage: get_str("homepage"),
        documentation: get_str("documentation"),
        plugin_type: get_str("plugin_type"),
        maturity: get_str("maturity"),
        provides_services: get_vec("provides_services"),
        requires_services: get_vec("requires_services"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_config() {
        let config = PublishConfig {
            registry_url: "https://registry.example.com".to_string(),
            auth_token: "secret-token".to_string(),
            skip_verify: false,
            as_draft: false,
            sign: false,
            key_id: None,
        };

        assert_eq!(config.registry_url, "https://registry.example.com");
        assert!(!config.skip_verify);
    }

    #[test]
    fn test_artifact_publisher_creation() {
        let config = PublishConfig {
            registry_url: "https://registry.example.com".to_string(),
            auth_token: "secret-token".to_string(),
            skip_verify: false,
            as_draft: false,
            sign: false,
            key_id: None,
        };

        let publisher = ArtifactPublisher::new(config);
        assert!(publisher.is_authenticated());
    }

    #[test]
    fn test_parse_plugin_toml() {
        let content = r#"
[package]
name = "test-plugin"
version = "1.0.0"
abi_version = "2"
description = "A test plugin"
author = "Test Author"
license = "MIT"
keywords = ["test", "plugin"]
"#;

        let metadata = parse_plugin_toml(content).unwrap();
        assert_eq!(metadata.abi_version, "2");
        assert_eq!(metadata.description, Some("A test plugin".to_string()));
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(metadata.license, Some("MIT".to_string()));
        assert_eq!(
            metadata.keywords,
            Some(vec!["test".to_string(), "plugin".to_string()])
        );
    }

    #[test]
    fn test_parse_plugin_toml_flat() {
        let content = r#"
name = "flat-plugin"
version = "0.1.0"
abi_version = "1"
description = "Flat format plugin"
"#;

        let metadata = parse_plugin_toml(content).unwrap();
        assert_eq!(metadata.abi_version, "1");
        assert_eq!(metadata.description, Some("Flat format plugin".to_string()));
    }
}
