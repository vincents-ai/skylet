// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ABI v2.0 compatibility layer
//!
//! This module provides compatibility between plugin-packager and the skylet-abi,
//! ensuring plugins packaged here are compatible with the plugin lifecycle
//! management system.

use serde::{Deserialize, Serialize};

/// ABI v2.0 specification levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ABIVersion {
    V1,
    V2,
    V3,
}

impl ABIVersion {
    /// Parse ABI version from string (e.g., "2.0")
    pub fn parse(version_str: &str) -> anyhow::Result<Self> {
        match version_str {
            "1.0" | "1" => Ok(ABIVersion::V1),
            "2.0" | "2" => Ok(ABIVersion::V2),
            "3.0" | "3" => Ok(ABIVersion::V3),
            _ => anyhow::bail!("Unknown ABI version: {}", version_str),
        }
    }

    /// Get as string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ABIVersion::V1 => "1.0",
            ABIVersion::V2 => "2.0",
            ABIVersion::V3 => "3.0",
        }
    }

    /// Check if this version is compatible with another
    pub fn is_compatible_with(&self, other: &ABIVersion) -> bool {
        // For now, only exact matches are compatible
        // This can be refined as we understand compatibility requirements
        self == other
    }
}

/// Plugin maturity level for plugin classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaturityLevel {
    Alpha,
    Beta,
    ReleaseCandidate,
    Stable,
    Deprecated,
}

impl MaturityLevel {
    /// Parse maturity level from string
    pub fn parse(level_str: &str) -> anyhow::Result<Self> {
        match level_str.to_lowercase().as_str() {
            "alpha" => Ok(MaturityLevel::Alpha),
            "beta" => Ok(MaturityLevel::Beta),
            "rc" | "releasecandidate" => Ok(MaturityLevel::ReleaseCandidate),
            "stable" | "production" => Ok(MaturityLevel::Stable),
            "deprecated" => Ok(MaturityLevel::Deprecated),
            _ => anyhow::bail!("Unknown maturity level: {}", level_str),
        }
    }

    /// Get as string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            MaturityLevel::Alpha => "alpha",
            MaturityLevel::Beta => "beta",
            MaturityLevel::ReleaseCandidate => "rc",
            MaturityLevel::Stable => "stable",
            MaturityLevel::Deprecated => "deprecated",
        }
    }
}

/// Plugin category for plugin classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginCategory {
    Utility,
    Database,
    Network,
    Storage,
    Security,
    Monitoring,
    Payment,
    Integration,
    Development,
    Other,
}

impl PluginCategory {
    /// Parse category from string
    pub fn parse(category_str: &str) -> anyhow::Result<Self> {
        match category_str.to_lowercase().as_str() {
            "utility" => Ok(PluginCategory::Utility),
            "database" => Ok(PluginCategory::Database),
            "network" => Ok(PluginCategory::Network),
            "storage" => Ok(PluginCategory::Storage),
            "security" => Ok(PluginCategory::Security),
            "monitoring" => Ok(PluginCategory::Monitoring),
            "payment" => Ok(PluginCategory::Payment),
            "integration" => Ok(PluginCategory::Integration),
            "development" => Ok(PluginCategory::Development),
            "other" => Ok(PluginCategory::Other),
            _ => anyhow::bail!("Unknown plugin category: {}", category_str),
        }
    }

    /// Get as string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginCategory::Utility => "utility",
            PluginCategory::Database => "database",
            PluginCategory::Network => "network",
            PluginCategory::Storage => "storage",
            PluginCategory::Security => "security",
            PluginCategory::Monitoring => "monitoring",
            PluginCategory::Payment => "payment",
            PluginCategory::Integration => "integration",
            PluginCategory::Development => "development",
            PluginCategory::Other => "other",
        }
    }
}

/// ABI v2.0 compatible plugin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABICompatibleInfo {
    /// Plugin name
    pub name: String,

    /// Semantic version (e.g., "1.2.3")
    pub version: String,

    /// Marketplace ABI version required
    pub abi_version: ABIVersion,

    /// Minimum Skylet version required
    pub skylet_version_min: Option<String>,

    /// Maximum Skylet version supported
    pub skylet_version_max: Option<String>,

    /// Plugin maturity level
    pub maturity_level: MaturityLevel,

    /// Plugin category
    pub category: PluginCategory,

    /// Author/organization
    pub author: Option<String>,

    /// License (SPDX identifier, e.g., "MIT", "Apache-2.0")
    pub license: Option<String>,

    /// Long description
    pub description: Option<String>,

    /// Plugin-provided capabilities (function exports)
    pub capabilities: Vec<CapabilityInfo>,

    /// Plugin dependencies
    pub dependencies: Vec<DependencyInfo>,

    /// Required resource specifications
    pub resources: ResourceRequirements,
}

/// Plugin capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    /// Capability name (e.g., "http_request", "file_read")
    pub name: String,

    /// Capability description
    pub description: Option<String>,

    /// Required permission level (e.g., "user", "system", "root")
    pub required_permission: Option<String>,
}

/// Plugin dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Dependency name
    pub name: String,

    /// Semantic version range (e.g., ">=1.0.0,<2.0.0")
    pub version_range: String,

    /// Whether this dependency is required
    pub required: bool,

    /// Service type/category for the dependency
    pub service_type: Option<String>,
}

/// Resource requirements specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Minimum CPU cores
    pub min_cpu_cores: u32,

    /// Maximum CPU cores
    pub max_cpu_cores: u32,

    /// Minimum memory in MB
    pub min_memory_mb: u32,

    /// Maximum memory in MB
    pub max_memory_mb: u32,

    /// Minimum disk space in MB
    pub min_disk_mb: u32,

    /// Maximum disk space in MB
    pub max_disk_mb: u32,

    /// Whether GPU is required
    pub requires_gpu: bool,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            min_cpu_cores: 1,
            max_cpu_cores: 4,
            min_memory_mb: 256,
            max_memory_mb: 1024,
            min_disk_mb: 100,
            max_disk_mb: 1024,
            requires_gpu: false,
        }
    }
}

/// ABI v2.0 validation result
#[derive(Debug, Clone)]
pub struct ABIValidationResult {
    /// Whether the plugin is ABI v2.0 compatible
    pub is_compatible: bool,

    /// Validation warnings
    pub warnings: Vec<String>,

    /// Validation errors
    pub errors: Vec<String>,

    /// Recommended fixes
    pub recommendations: Vec<String>,
}

impl ABIValidationResult {
    /// Create new validation result
    pub fn new(is_compatible: bool) -> Self {
        Self {
            is_compatible,
            warnings: Vec::new(),
            errors: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    /// Add warning
    pub fn add_warning(&mut self, message: String) {
        self.warnings.push(message);
    }

    /// Add error
    pub fn add_error(&mut self, message: String) {
        self.errors.push(message);
        self.is_compatible = false;
    }

    /// Add recommendation
    pub fn add_recommendation(&mut self, message: String) {
        self.recommendations.push(message);
    }

    /// Check if validation passed
    pub fn passed(&self) -> bool {
        self.is_compatible && self.errors.is_empty()
    }
}

/// ABI compatibility validator
pub struct ABIValidator;

impl ABIValidator {
    /// Validate ABI v2.0 compatibility
    pub fn validate(info: &ABICompatibleInfo) -> ABIValidationResult {
        let mut result = ABIValidationResult::new(true);

        // Validate ABI version
        if info.abi_version == ABIVersion::V1 {
            result.add_warning(
                "ABI v1.0 is deprecated. Consider upgrading to v2.0 for new features.".to_string(),
            );
        } else if info.abi_version == ABIVersion::V3 {
            result.add_warning(
                "ABI v3.0 is experimental. Some compatibility issues may occur.".to_string(),
            );
        }

        // Validate maturity level
        if info.maturity_level == MaturityLevel::Alpha {
            result.add_warning("Plugin is marked as alpha. It may not be stable.".to_string());
        } else if info.maturity_level == MaturityLevel::Deprecated {
            result.add_error("Plugin is marked as deprecated.".to_string());
        }

        // Validate capabilities
        if info.capabilities.is_empty() {
            result.add_recommendation(
                "Plugin declares no capabilities. Add at least one capability for discoverability."
                    .to_string(),
            );
        }

        // Validate resource requirements
        if info.resources.min_cpu_cores > info.resources.max_cpu_cores {
            result
                .add_error("Resource requirement error: min_cpu_cores > max_cpu_cores".to_string());
        }
        if info.resources.min_memory_mb > info.resources.max_memory_mb {
            result
                .add_error("Resource requirement error: min_memory_mb > max_memory_mb".to_string());
        }
        if info.resources.min_disk_mb > info.resources.max_disk_mb {
            result.add_error("Resource requirement error: min_disk_mb > max_disk_mb".to_string());
        }

        // Validate dependencies
        for dep in &info.dependencies {
            if dep.name.is_empty() {
                result.add_error("Dependency with empty name found.".to_string());
            }
            if dep.version_range.is_empty() {
                result.add_error(format!(
                    "Dependency '{}' has empty version range.",
                    dep.name
                ));
            }
        }

        // Validate license
        if info.license.is_none() {
            result.add_recommendation(
                "No license specified. Add a license for marketplace publishing.".to_string(),
            );
        }

        // Validate author
        if info.author.is_none() {
            result.add_warning("No author specified.".to_string());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version_parse() {
        assert_eq!(ABIVersion::parse("1.0").unwrap(), ABIVersion::V1);
        assert_eq!(ABIVersion::parse("2.0").unwrap(), ABIVersion::V2);
        assert_eq!(ABIVersion::parse("3.0").unwrap(), ABIVersion::V3);
        assert_eq!(ABIVersion::parse("2").unwrap(), ABIVersion::V2);
    }

    #[test]
    fn test_abi_version_as_str() {
        assert_eq!(ABIVersion::V1.as_str(), "1.0");
        assert_eq!(ABIVersion::V2.as_str(), "2.0");
        assert_eq!(ABIVersion::V3.as_str(), "3.0");
    }

    #[test]
    fn test_abi_version_compatibility() {
        assert!(ABIVersion::V2.is_compatible_with(&ABIVersion::V2));
        assert!(!ABIVersion::V2.is_compatible_with(&ABIVersion::V1));
    }

    #[test]
    fn test_maturity_level_parse() {
        assert_eq!(MaturityLevel::parse("alpha").unwrap(), MaturityLevel::Alpha);
        assert_eq!(
            MaturityLevel::parse("stable").unwrap(),
            MaturityLevel::Stable
        );
        assert_eq!(
            MaturityLevel::parse("production").unwrap(),
            MaturityLevel::Stable
        );
    }

    #[test]
    fn test_plugin_category_parse() {
        assert_eq!(
            PluginCategory::parse("database").unwrap(),
            PluginCategory::Database
        );
        assert_eq!(
            PluginCategory::parse("security").unwrap(),
            PluginCategory::Security
        );
    }

    #[test]
    fn test_resource_requirements_default() {
        let res = ResourceRequirements::default();
        assert_eq!(res.min_cpu_cores, 1);
        assert_eq!(res.min_memory_mb, 256);
    }

    #[test]
    fn test_abi_validation_basic() {
        let info = ABICompatibleInfo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            abi_version: ABIVersion::V2,
            skylet_version_min: None,
            skylet_version_max: None,
            maturity_level: MaturityLevel::Stable,
            category: PluginCategory::Utility,
            author: Some("Test Author".to_string()),
            license: Some("MIT".to_string()),
            description: Some("Test plugin".to_string()),
            capabilities: vec![],
            dependencies: vec![],
            resources: ResourceRequirements::default(),
        };

        let result = ABIValidator::validate(&info);
        assert!(result.is_compatible);
        // Note: result.passed() checks for no errors AND compatibility
        // This result has warnings/recommendations but no errors
        assert!(
            !result.errors.is_empty()
                || !result.warnings.is_empty()
                || !result.recommendations.is_empty()
        );
    }

    #[test]
    fn test_abi_validation_errors() {
        let mut resources = ResourceRequirements::default();
        resources.min_cpu_cores = 10;
        resources.max_cpu_cores = 2; // Invalid: min > max

        let info = ABICompatibleInfo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            abi_version: ABIVersion::V2,
            skylet_version_min: None,
            skylet_version_max: None,
            maturity_level: MaturityLevel::Stable,
            category: PluginCategory::Utility,
            author: None,
            license: None,
            description: None,
            capabilities: vec![],
            dependencies: vec![],
            resources,
        };

        let result = ABIValidator::validate(&info);
        assert!(!result.is_compatible);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_abi_validation_deprecated() {
        let info = ABICompatibleInfo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            abi_version: ABIVersion::V2,
            skylet_version_min: None,
            skylet_version_max: None,
            maturity_level: MaturityLevel::Deprecated,
            category: PluginCategory::Utility,
            author: None,
            license: None,
            description: None,
            capabilities: vec![],
            dependencies: vec![],
            resources: ResourceRequirements::default(),
        };

        let result = ABIValidator::validate(&info);
        assert!(!result.is_compatible);
        assert!(result.errors.iter().any(|e| e.contains("deprecated")));
    }
}
