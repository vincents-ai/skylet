// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Plugin artifact publishing to registry
///
/// This module provides functionality for publishing plugin artifacts to
/// the Skylet marketplace or private registries. It handles:
/// - Artifact validation before publishing
/// - Checksum computation and verification
/// - Upload with authentication
/// - Publishing metadata extraction from artifacts
///
/// RFC-0003: Plugin Package and Artifact Specification
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::abi_compat::{
    ABICompatibleInfo, ABIVersion, MarketplaceMetadata, MaturityLevel, PluginCategory,
    ResourceRequirements,
};
use crate::marketplace::{MarketplaceClient, PublishRequest, PublishSignature};
use crate::platform::ArtifactMetadata;

/// Publishing client for plugin artifacts
#[derive(Clone)]
pub struct ArtifactPublisher {
    client: MarketplaceClient,
}

/// Configuration for artifact publishing
#[derive(Debug, Clone)]
pub struct PublishConfig {
    /// Registry base URL (e.g., "https://marketplace.skylet.dev")
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
    /// URL to view plugin in marketplace (if available)
    pub marketplace_url: Option<String>,
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
        let client = MarketplaceClient::with_auth(config.registry_url, config.auth_token);
        Self { client }
    }

    /// Create a publisher with an existing client
    pub fn with_client(client: MarketplaceClient) -> Self {
        Self { client }
    }

    /// Publish an artifact to the registry
    ///
    /// This function:
    /// 1. Validates the artifact (unless skip_verify is true)
    /// 2. Extracts metadata from the artifact
    /// 3. Computes the checksum
    /// 4. Uploads to the registry with authentication
    ///
    /// # Arguments
    /// * `artifact_path` - Path to the .tar.gz artifact
    /// * `config` - Publishing configuration
    ///
    /// # Returns
    /// * `ArtifactPublishResult` on success
    ///
    /// # Example
    /// ```no_run
    /// use plugin_packager::publish::{ArtifactPublisher, PublishConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let config = PublishConfig {
    ///         registry_url: "https://marketplace.skylet.dev".to_string(),
    ///         auth_token: "my-token".to_string(),
    ///         skip_verify: false,
    ///         as_draft: false,
    ///         sign: false,
    ///         key_id: None,
    ///     };
    ///     
    ///     let publisher = ArtifactPublisher::new(config);
    ///     let result = publisher.publish(
    ///         std::path::Path::new("./my-plugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz"),
    ///         None,
    ///     ).await?;
    ///     
    ///     tracing::info!("Published: {} v{}", result.plugin_id, result.version);
    ///     Ok(())
    /// }
    /// ```
    pub async fn publish(
        &self,
        artifact_path: &Path,
        signatures: Option<Vec<PublishSignature>>,
    ) -> Result<ArtifactPublishResult> {
        // Step 1: Validate artifact exists
        if !artifact_path.exists() {
            return Err(anyhow!("Artifact not found: {}", artifact_path.display()));
        }

        // Step 2: Parse artifact metadata from filename
        let filename = artifact_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid artifact filename")?;

        let artifact_metadata = ArtifactMetadata::parse(filename)
            .with_context(|| format!("Failed to parse artifact name: {}", filename))?;

        // Step 3: Verify artifact (RFC-0003 compliance)
        crate::verify_artifact(artifact_path, None)
            .with_context(|| "Artifact verification failed")?;

        // Step 4: Compute checksum
        let checksum = compute_artifact_checksum(artifact_path)?;

        // Step 5: Extract plugin metadata from artifact
        let plugin_metadata = extract_plugin_metadata(artifact_path)?;

        // Step 6: Build ABI-compatible info
        let abi_version = ABIVersion::parse(&plugin_metadata.abi_version).unwrap_or(ABIVersion::V1);

        let maturity_level = plugin_metadata
            .maturity
            .as_deref()
            .and_then(|m| MaturityLevel::parse(m).ok())
            .unwrap_or(MaturityLevel::Alpha);

        let category = plugin_metadata
            .plugin_type
            .as_deref()
            .and_then(|c| PluginCategory::parse(c).ok())
            .unwrap_or(PluginCategory::Other);

        let abi_info = ABICompatibleInfo {
            name: artifact_metadata.name.clone(),
            version: artifact_metadata.version.clone(),
            abi_version,
            skylet_version_min: None,
            skylet_version_max: None,
            maturity_level,
            category,
            author: plugin_metadata.author,
            license: plugin_metadata.license,
            description: plugin_metadata.description,
            capabilities: vec![],
            dependencies: vec![],
            resources: ResourceRequirements::default(),
            marketplace: MarketplaceMetadata::default(),
        };

        // Step 7: Build publish request
        let package_url = format!("file://{}", artifact_path.canonicalize()?.display());

        let request = PublishRequest {
            metadata: abi_info,
            package_url,
            checksum: checksum.clone(),
            signatures,
        };

        // Step 8: Publish to marketplace
        let response = self.client.publish(request).await?;

        // Step 9: Build result
        Ok(ArtifactPublishResult {
            plugin_id: response.id,
            version: response.version,
            download_url: format!(
                "{}/artifacts/{}-v{}-{}.tar.gz",
                self.client.base_url(),
                artifact_metadata.name,
                artifact_metadata.version,
                artifact_metadata.target_triple
            ),
            checksum,
            status: format!("{:?}", response.status),
            message: response.message,
            marketplace_url: response.verification_url,
        })
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
        self.client.has_auth()
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
            registry_url: "https://marketplace.example.com".to_string(),
            auth_token: "secret-token".to_string(),
            skip_verify: false,
            as_draft: false,
            sign: false,
            key_id: None,
        };

        assert_eq!(config.registry_url, "https://marketplace.example.com");
        assert!(!config.skip_verify);
    }

    #[test]
    fn test_artifact_publisher_creation() {
        let config = PublishConfig {
            registry_url: "https://marketplace.example.com".to_string(),
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
