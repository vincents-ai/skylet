// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin metadata extraction utilities
//!
//! This module provides functions for extracting and working with plugin metadata,
//! including parsing manifests, extracting dependencies, and analyzing plugins.

use crate::abi_compat::{
    ABICompatibleInfo, ABIValidationResult, ABIValidator, ABIVersion,
    CapabilityInfo as ABICapability, DependencyInfo as ABIDependency, MarketplaceMetadata,
    MaturityLevel, MonetizationModel, PluginCategory, ResourceRequirements,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};

type Result<T> = anyhow::Result<T>;

/// Rich metadata extracted from a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier)
    pub name: String,

    /// Semantic version (e.g., "1.2.3")
    pub version: String,

    /// ABI version (e.g., "2.0")
    pub abi_version: String,

    /// Human-readable description
    pub description: Option<String>,

    /// Plugin author(s)
    pub authors: Option<Vec<String>>,

    /// License identifier (SPDX)
    pub license: Option<String>,

    /// Plugin keywords for searching
    pub keywords: Option<Vec<String>>,

    /// Categories/tags
    pub categories: Option<Vec<String>>,

    /// Repository URL
    pub repository: Option<String>,

    /// Homepage URL
    pub homepage: Option<String>,

    /// Documentation URL
    pub documentation: Option<String>,

    /// Plugin capabilities
    pub capabilities: Option<Vec<String>>,

    /// Plugin requirements
    pub requirements: Option<PluginRequirements>,

    /// Dependencies
    pub dependencies: Option<Vec<DependencyMetadata>>,
}

/// Plugin requirements metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequirements {
    /// Maximum concurrency for requests
    pub max_concurrency: Option<usize>,

    /// Minimum memory required (in MB)
    pub min_memory_mb: Option<usize>,

    /// Timeout for operations (in seconds)
    pub timeout_secs: Option<u64>,

    /// Whether plugin handles streaming
    pub supports_streaming: Option<bool>,
}

/// Dependency metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyMetadata {
    /// Dependency name
    pub name: String,

    /// Version requirement
    pub version: String,

    /// Whether this dependency is optional
    pub optional: Option<bool>,

    /// Features from dependency
    pub features: Option<Vec<String>>,
}

/// Plugin statistics
#[derive(Debug, Clone)]
pub struct PluginStats {
    /// Total size in bytes
    pub size_bytes: u64,

    /// Number of dependencies
    pub dependency_count: usize,

    /// Whether plugin has documentation
    pub has_documentation: bool,

    /// Whether plugin has changelog
    pub has_changelog: bool,

    /// Number of files in plugin
    pub file_count: usize,
}

impl PluginMetadata {
    /// Extract basic metadata from a manifest
    pub fn from_manifest(manifest_content: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct ManifestPackage {
            name: String,
            version: String,
            abi_version: String,
            description: Option<String>,
            authors: Option<Vec<String>>,
            author: Option<String>,
            license: Option<String>,
            keywords: Option<Vec<String>>,
            categories: Option<Vec<String>>,
            repository: Option<String>,
            homepage: Option<String>,
            documentation: Option<String>,
        }

        #[derive(Deserialize)]
        struct Capabilities {
            handles_requests: Option<bool>,
            provides_health_checks: Option<bool>,
            supports_streaming: Option<bool>,
            custom: Option<Vec<String>>,
        }

        #[derive(Deserialize)]
        struct Requirements {
            max_concurrency: Option<usize>,
            min_memory_mb: Option<usize>,
            timeout_secs: Option<u64>,
            supports_streaming: Option<bool>,
        }

        #[derive(Deserialize)]
        struct Dependency {
            name: String,
            version: String,
            optional: Option<bool>,
            features: Option<Vec<String>>,
        }

        #[derive(Deserialize)]
        struct FullManifest {
            #[serde(default)]
            package: Option<ManifestPackage>,
            #[serde(default)]
            capabilities: Option<Capabilities>,
            #[serde(default)]
            requirements: Option<Requirements>,
            #[serde(default)]
            dependencies: Option<Vec<Dependency>>,
        }

        let manifest: FullManifest =
            toml::from_str(manifest_content).context("parsing plugin manifest")?;

        let pkg = manifest
            .package
            .context("manifest must have [package] section")?;

        // Handle author(s)
        let authors = pkg.authors.or_else(|| pkg.author.map(|a| vec![a]));

        // Extract capabilities
        let capabilities = manifest.capabilities.and_then(|c| {
            let mut caps = Vec::new();
            if c.handles_requests.unwrap_or(false) {
                caps.push("handles_requests".to_string());
            }
            if c.provides_health_checks.unwrap_or(false) {
                caps.push("provides_health_checks".to_string());
            }
            if c.supports_streaming.unwrap_or(false) {
                caps.push("supports_streaming".to_string());
            }
            if let Some(custom) = c.custom {
                caps.extend(custom);
            }
            if !caps.is_empty() {
                Some(caps)
            } else {
                None
            }
        });

        // Extract requirements
        let requirements = manifest.requirements.map(|r| PluginRequirements {
            max_concurrency: r.max_concurrency,
            min_memory_mb: r.min_memory_mb,
            timeout_secs: r.timeout_secs,
            supports_streaming: r.supports_streaming,
        });

        // Extract dependencies
        let dependencies = manifest.dependencies.map(|deps| {
            deps.into_iter()
                .map(|d| DependencyMetadata {
                    name: d.name,
                    version: d.version,
                    optional: d.optional,
                    features: d.features,
                })
                .collect()
        });

        Ok(PluginMetadata {
            name: pkg.name,
            version: pkg.version,
            abi_version: pkg.abi_version,
            description: pkg.description,
            authors,
            license: pkg.license,
            keywords: pkg.keywords,
            categories: pkg.categories,
            repository: pkg.repository,
            homepage: pkg.homepage,
            documentation: pkg.documentation,
            capabilities,
            requirements,
            dependencies,
        })
    }

    /// Get total dependency count (including optional)
    pub fn dependency_count(&self) -> usize {
        self.dependencies.as_ref().map(|d| d.len()).unwrap_or(0)
    }

    /// Get required dependency count (excluding optional)
    pub fn required_dependency_count(&self) -> usize {
        self.dependencies
            .as_ref()
            .map(|d| {
                d.iter()
                    .filter(|dep| !dep.optional.unwrap_or(false))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Get optional dependency count
    pub fn optional_dependency_count(&self) -> usize {
        self.dependencies
            .as_ref()
            .map(|d| d.iter().filter(|dep| dep.optional.unwrap_or(false)).count())
            .unwrap_or(0)
    }

    /// Check if plugin has all required metadata
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty()
            && !self.version.trim().is_empty()
            && !self.abi_version.trim().is_empty()
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "{} v{} (ABI: {})\n  {}\n  Dependencies: {} (required: {}, optional: {})",
            self.name,
            self.version,
            self.abi_version,
            self.description
                .as_ref()
                .unwrap_or(&"No description".to_string()),
            self.dependency_count(),
            self.required_dependency_count(),
            self.optional_dependency_count()
        )
    }

    /// Convert to ABI v2.0 compatible info for marketplace integration
    pub fn to_abi_compatible(&self) -> Result<ABICompatibleInfo> {
        // Parse ABI version
        let abi_version = ABIVersion::parse(&self.abi_version)?;

        // Convert capabilities
        let capabilities = self
            .capabilities
            .as_ref()
            .map(|caps| {
                caps.iter()
                    .map(|cap| ABICapability {
                        name: cap.clone(),
                        description: None,
                        required_permission: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert dependencies
        let dependencies = self
            .dependencies
            .as_ref()
            .map(|deps| {
                deps.iter()
                    .map(|dep| ABIDependency {
                        name: dep.name.clone(),
                        version_range: dep.version.clone(),
                        required: !dep.optional.unwrap_or(false),
                        service_type: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Extract resource requirements from PluginRequirements
        let resources = if let Some(reqs) = &self.requirements {
            ResourceRequirements {
                min_cpu_cores: 1,
                max_cpu_cores: reqs.max_concurrency.unwrap_or(4) as u32,
                min_memory_mb: reqs.min_memory_mb.unwrap_or(256) as u32,
                max_memory_mb: (reqs.min_memory_mb.unwrap_or(256) * 2) as u32,
                min_disk_mb: 100,
                max_disk_mb: 1024,
                requires_gpu: false,
            }
        } else {
            ResourceRequirements::default()
        };

        Ok(ABICompatibleInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            abi_version,
            skylet_version_min: None,
            skylet_version_max: None,
            maturity_level: MaturityLevel::Alpha, // Default to alpha until specified
            category: PluginCategory::Utility,    // Default category
            author: self.authors.as_ref().and_then(|a| a.first().cloned()),
            license: self.license.clone(),
            description: self.description.clone(),
            capabilities,
            dependencies,
            resources,
            marketplace: MarketplaceMetadata {
                repository: self.repository.clone(),
                documentation: self.documentation.clone(),
                support_url: None,
                monetization: MonetizationModel::Free,
                price_cents: None,
                platforms: vec![],
                custom: Default::default(),
            },
        })
    }

    /// Validate ABI v2.0 compatibility
    pub fn validate_abi_compatibility(&self) -> Result<ABIValidationResult> {
        let abi_info = self.to_abi_compatible()?;
        Ok(ABIValidator::validate(&abi_info))
    }

    /// Check if plugin meets minimum ABI v2.0 requirements
    pub fn is_abi_v2_compatible(&self) -> Result<bool> {
        let abi_version = ABIVersion::parse(&self.abi_version)?;
        Ok(matches!(abi_version, ABIVersion::V2 | ABIVersion::V3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_basic() -> Result<()> {
        let manifest = r#"
[package]
name = "test-plugin"
version = "1.0.0"
abi_version = "2.0"
description = "A test plugin"
license = "MIT"
"#;

        let metadata = PluginMetadata::from_manifest(manifest)?;
        assert_eq!(metadata.name, "test-plugin");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.abi_version, "2.0");
        assert_eq!(metadata.description, Some("A test plugin".to_string()));

        Ok(())
    }

    #[test]
    fn test_extract_metadata_with_dependencies() -> Result<()> {
        let manifest = r#"
[package]
name = "consumer-plugin"
version = "1.0.0"
abi_version = "2.0"

[[dependencies]]
name = "base-plugin"
version = "^1.0.0"
optional = false

[[dependencies]]
name = "optional-plugin"
version = ">=1.0.0"
optional = true
"#;

        let metadata = PluginMetadata::from_manifest(manifest)?;
        assert_eq!(metadata.dependency_count(), 2);
        assert_eq!(metadata.required_dependency_count(), 1);
        assert_eq!(metadata.optional_dependency_count(), 1);

        Ok(())
    }

    #[test]
    fn test_extract_metadata_with_capabilities() -> Result<()> {
        let manifest = r#"
[package]
name = "advanced-plugin"
version = "1.0.0"
abi_version = "2.0"

[capabilities]
handles_requests = true
provides_health_checks = true
custom = ["custom_feature"]

[requirements]
max_concurrency = 10
min_memory_mb = 256
timeout_secs = 30
"#;

        let metadata = PluginMetadata::from_manifest(manifest)?;
        assert!(metadata.capabilities.is_some());
        assert_eq!(metadata.capabilities.unwrap().len(), 3);
        assert!(metadata.requirements.is_some());
        let reqs = metadata.requirements.unwrap();
        assert_eq!(reqs.max_concurrency, Some(10));
        assert_eq!(reqs.min_memory_mb, Some(256));

        Ok(())
    }

    #[test]
    fn test_metadata_summary() -> Result<()> {
        let manifest = r#"
[package]
name = "summary-test"
version = "2.0.0"
abi_version = "2.0"
description = "A summary test plugin"
"#;

        let metadata = PluginMetadata::from_manifest(manifest)?;
        let summary = metadata.summary();
        assert!(summary.contains("summary-test"));
        assert!(summary.contains("v2.0.0"));
        assert!(summary.contains("A summary test plugin"));

        Ok(())
    }

    #[test]
    fn test_metadata_validation() -> Result<()> {
        let manifest = r#"
[package]
name = "valid-plugin"
version = "1.0.0"
abi_version = "2.0"
"#;

        let metadata = PluginMetadata::from_manifest(manifest)?;
        assert!(metadata.is_valid());

        // Test invalid metadata
        let invalid = PluginMetadata {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            authors: None,
            license: None,
            keywords: None,
            categories: None,
            repository: None,
            homepage: None,
            documentation: None,
            capabilities: None,
            requirements: None,
            dependencies: None,
        };
        assert!(!invalid.is_valid());

        Ok(())
    }
}
