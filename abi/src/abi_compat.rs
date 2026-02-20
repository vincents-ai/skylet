// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! ABI Version Compatibility Checking
//!
//! This module provides comprehensive ABI version validation and compatibility checking,
//! including version parsing, compatibility matrices, and breaking change detection.
//!
//! RFC-0004 Phase 2: Dynamic Plugin Loading - Week 3

use std::fmt;

/// Semantic version representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticVersion {
    /// Major version (breaking changes)
    pub major: u32,

    /// Minor version (new features, backward compatible)
    pub minor: u32,

    /// Patch version (bug fixes, backward compatible)
    pub patch: u32,
}

impl SemanticVersion {
    /// Create a new semantic version
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a semantic version from a string (e.g., "1.2.3")
    pub fn parse(version_str: &str) -> Result<Self, AbiVersionError> {
        let parts: Vec<&str> = version_str.split('.').collect();

        if parts.len() != 3 {
            return Err(AbiVersionError::InvalidFormat {
                version: version_str.to_string(),
                expected: "major.minor.patch".to_string(),
            });
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| AbiVersionError::InvalidFormat {
                version: version_str.to_string(),
                expected: "major.minor.patch".to_string(),
            })?;

        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| AbiVersionError::InvalidFormat {
                version: version_str.to_string(),
                expected: "major.minor.patch".to_string(),
            })?;

        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| AbiVersionError::InvalidFormat {
                version: version_str.to_string(),
                expected: "major.minor.patch".to_string(),
            })?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    /// Check if this version is compatible with a required version
    /// Compatible if: same major, this minor >= required minor, patch can be anything
    pub fn is_compatible_with(&self, required: Self) -> bool {
        if self.major != required.major {
            return false;
        }
        if self.minor < required.minor {
            return false;
        }
        true
    }

    /// Get the version as a string
    pub fn to_string_version(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Check if this is a major version change compared to another
    pub fn is_major_change(&self, other: Self) -> bool {
        self.major != other.major
    }

    /// Check if this is a minor version change compared to another
    pub fn is_minor_change(&self, other: Self) -> bool {
        self.major == other.major && self.minor != other.minor
    }

    /// Check if this is a patch version change compared to another
    pub fn is_patch_change(&self, other: Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch != other.patch
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// ABI compatibility constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityConstraint {
    /// Exact version match required
    Exact,

    /// Same major version, minor and patch can differ
    SameMajor,

    /// Same major and minor, patch can differ
    SameMinor,

    /// Any version acceptable
    Any,
}

impl CompatibilityConstraint {
    /// Check if two versions satisfy this constraint
    pub fn is_satisfied(&self, required: SemanticVersion, provided: SemanticVersion) -> bool {
        match self {
            CompatibilityConstraint::Exact => required == provided,
            CompatibilityConstraint::SameMajor => required.major == provided.major,
            CompatibilityConstraint::SameMinor => {
                required.major == provided.major && required.minor == provided.minor
            }
            CompatibilityConstraint::Any => true,
        }
    }
}

impl fmt::Display for CompatibilityConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompatibilityConstraint::Exact => write!(f, "exact version"),
            CompatibilityConstraint::SameMajor => write!(f, "same major version"),
            CompatibilityConstraint::SameMinor => write!(f, "same major.minor version"),
            CompatibilityConstraint::Any => write!(f, "any version"),
        }
    }
}

/// ABI compatibility information for a plugin
#[derive(Debug, Clone)]
pub struct AbiCompatibility {
    /// The plugin's ABI version
    pub plugin_version: SemanticVersion,

    /// Minimum required loader/host version
    pub min_loader_version: SemanticVersion,

    /// Maximum compatible loader/host version (None = no upper limit)
    pub max_loader_version: Option<SemanticVersion>,

    /// Compatibility constraint with the loader
    pub constraint: CompatibilityConstraint,

    /// Describes any known breaking changes
    pub breaking_changes: Vec<String>,

    /// Describes any deprecated features
    pub deprecated_features: Vec<String>,
}

impl AbiCompatibility {
    /// Create a new ABI compatibility structure
    pub fn new(plugin_version: SemanticVersion, min_loader_version: SemanticVersion) -> Self {
        Self {
            plugin_version,
            min_loader_version,
            max_loader_version: None,
            constraint: CompatibilityConstraint::SameMajor,
            breaking_changes: Vec::new(),
            deprecated_features: Vec::new(),
        }
    }

    /// Set the maximum compatible loader version
    pub fn with_max_version(mut self, max_version: SemanticVersion) -> Self {
        self.max_loader_version = Some(max_version);
        self
    }

    /// Set the compatibility constraint
    pub fn with_constraint(mut self, constraint: CompatibilityConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// Add a breaking change description
    pub fn add_breaking_change(mut self, description: impl Into<String>) -> Self {
        self.breaking_changes.push(description.into());
        self
    }

    /// Add a deprecated feature
    pub fn add_deprecated_feature(mut self, feature: impl Into<String>) -> Self {
        self.deprecated_features.push(feature.into());
        self
    }

    /// Check if compatible with a loader version
    pub fn is_compatible_with(
        &self,
        loader_version: SemanticVersion,
    ) -> Result<(), AbiVersionError> {
        // Check minimum version
        if loader_version < self.min_loader_version {
            return Err(AbiVersionError::VersionTooOld {
                plugin_version: self.plugin_version,
                required_min: self.min_loader_version,
                provided: loader_version,
            });
        }

        // Check maximum version if specified
        if let Some(max_version) = self.max_loader_version {
            if loader_version > max_version {
                return Err(AbiVersionError::VersionTooNew {
                    plugin_version: self.plugin_version,
                    required_max: max_version,
                    provided: loader_version,
                });
            }
        }

        // Check constraint
        if !self
            .constraint
            .is_satisfied(self.plugin_version, loader_version)
        {
            return Err(AbiVersionError::ConstraintViolation {
                plugin_version: self.plugin_version,
                loader_version,
                constraint: self.constraint,
            });
        }

        Ok(())
    }
}

impl fmt::Display for AbiCompatibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ABI {} (requires loader >= {})",
            self.plugin_version, self.min_loader_version
        )?;
        if let Some(max) = self.max_loader_version {
            write!(f, " and < {}", max)?;
        }
        Ok(())
    }
}

/// Predefined ABI versions
pub mod versions {
    use super::SemanticVersion;

    /// ABI v2.0 - Current stable
    pub const ABI_V2_0: SemanticVersion = SemanticVersion::new(2, 0, 0);

    /// ABI v2.1 - With streaming support
    pub const ABI_V2_1: SemanticVersion = SemanticVersion::new(2, 1, 0);

    /// ABI v2.2 - With hot reload
    pub const ABI_V2_2: SemanticVersion = SemanticVersion::new(2, 2, 0);

    /// ABI v3.0 - Next major version (not yet released)
    pub const ABI_V3_0: SemanticVersion = SemanticVersion::new(3, 0, 0);
}

/// Errors that can occur during ABI version checking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiVersionError {
    /// Version string format is invalid
    InvalidFormat { version: String, expected: String },

    /// Plugin requires a newer loader version
    VersionTooOld {
        plugin_version: SemanticVersion,
        required_min: SemanticVersion,
        provided: SemanticVersion,
    },

    /// Plugin requires an older loader version
    VersionTooNew {
        plugin_version: SemanticVersion,
        required_max: SemanticVersion,
        provided: SemanticVersion,
    },

    /// Compatibility constraint not satisfied
    ConstraintViolation {
        plugin_version: SemanticVersion,
        loader_version: SemanticVersion,
        constraint: CompatibilityConstraint,
    },

    /// Major version mismatch
    MajorVersionMismatch {
        plugin_version: SemanticVersion,
        loader_version: SemanticVersion,
    },
}

impl fmt::Display for AbiVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbiVersionError::InvalidFormat { version, expected } => {
                write!(
                    f,
                    "Invalid version format '{}': expected {}",
                    version, expected
                )
            }
            AbiVersionError::VersionTooOld {
                plugin_version,
                required_min,
                provided,
            } => {
                write!(
                    f,
                    "Plugin ABI {} requires loader >= {}, but {} provided",
                    plugin_version, required_min, provided
                )
            }
            AbiVersionError::VersionTooNew {
                plugin_version,
                required_max,
                provided,
            } => {
                write!(
                    f,
                    "Plugin ABI {} requires loader < {}, but {} provided",
                    plugin_version, required_max, provided
                )
            }
            AbiVersionError::ConstraintViolation {
                plugin_version,
                loader_version,
                constraint,
            } => {
                write!(
                    f,
                    "Plugin ABI {} requires {}, but loader {} provided",
                    plugin_version, constraint, loader_version
                )
            }
            AbiVersionError::MajorVersionMismatch {
                plugin_version,
                loader_version,
            } => {
                write!(
                    f,
                    "Major version mismatch: plugin {}, loader {}",
                    plugin_version, loader_version
                )
            }
        }
    }
}

impl std::error::Error for AbiVersionError {}

#[cfg(test)]
mod tests {
    use super::*;

    // SemanticVersion tests
    #[test]
    fn test_semantic_version_new() {
        let ver = SemanticVersion::new(1, 2, 3);
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 3);
    }

    #[test]
    fn test_semantic_version_parse() {
        let ver = SemanticVersion::parse("1.2.3").unwrap();
        assert_eq!(ver.major, 1);
        assert_eq!(ver.minor, 2);
        assert_eq!(ver.patch, 3);
    }

    #[test]
    fn test_semantic_version_parse_invalid() {
        assert!(SemanticVersion::parse("1.2").is_err());
        assert!(SemanticVersion::parse("1.2.3.4").is_err());
        assert!(SemanticVersion::parse("a.b.c").is_err());
    }

    #[test]
    fn test_semantic_version_is_compatible_with() {
        let v1_2_0 = SemanticVersion::new(1, 2, 0);
        let v1_2_3 = SemanticVersion::new(1, 2, 3);
        let v1_3_0 = SemanticVersion::new(1, 3, 0);
        let v2_0_0 = SemanticVersion::new(2, 0, 0);

        assert!(v1_2_3.is_compatible_with(v1_2_0));
        assert!(v1_3_0.is_compatible_with(v1_2_0));
        assert!(!v2_0_0.is_compatible_with(v1_2_0));
    }

    #[test]
    fn test_semantic_version_to_string() {
        let ver = SemanticVersion::new(1, 2, 3);
        assert_eq!(ver.to_string_version(), "1.2.3");
    }

    #[test]
    fn test_semantic_version_is_major_change() {
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v2_0_0 = SemanticVersion::new(2, 0, 0);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);

        assert!(v2_0_0.is_major_change(v1_0_0));
        assert!(!v1_1_0.is_major_change(v1_0_0));
    }

    #[test]
    fn test_semantic_version_is_minor_change() {
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);
        let v2_0_0 = SemanticVersion::new(2, 0, 0);

        assert!(v1_1_0.is_minor_change(v1_0_0));
        assert!(!v2_0_0.is_minor_change(v1_0_0));
    }

    #[test]
    fn test_semantic_version_is_patch_change() {
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_0_1 = SemanticVersion::new(1, 0, 1);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);

        assert!(v1_0_1.is_patch_change(v1_0_0));
        assert!(!v1_1_0.is_patch_change(v1_0_0));
    }

    #[test]
    fn test_semantic_version_ordering() {
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_0_1 = SemanticVersion::new(1, 0, 1);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);
        let v2_0_0 = SemanticVersion::new(2, 0, 0);

        assert!(v1_0_0 < v1_0_1);
        assert!(v1_0_1 < v1_1_0);
        assert!(v1_1_0 < v2_0_0);
    }

    #[test]
    fn test_semantic_version_display() {
        let ver = SemanticVersion::new(1, 2, 3);
        assert_eq!(format!("{}", ver), "1.2.3");
    }

    // CompatibilityConstraint tests
    #[test]
    fn test_compatibility_constraint_exact() {
        let constraint = CompatibilityConstraint::Exact;
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_0_1 = SemanticVersion::new(1, 0, 1);

        assert!(constraint.is_satisfied(v1_0_0, v1_0_0));
        assert!(!constraint.is_satisfied(v1_0_0, v1_0_1));
    }

    #[test]
    fn test_compatibility_constraint_same_major() {
        let constraint = CompatibilityConstraint::SameMajor;
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);
        let v2_0_0 = SemanticVersion::new(2, 0, 0);

        assert!(constraint.is_satisfied(v1_0_0, v1_1_0));
        assert!(!constraint.is_satisfied(v1_0_0, v2_0_0));
    }

    #[test]
    fn test_compatibility_constraint_same_minor() {
        let constraint = CompatibilityConstraint::SameMinor;
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v1_0_1 = SemanticVersion::new(1, 0, 1);
        let v1_1_0 = SemanticVersion::new(1, 1, 0);

        assert!(constraint.is_satisfied(v1_0_0, v1_0_1));
        assert!(!constraint.is_satisfied(v1_0_0, v1_1_0));
    }

    #[test]
    fn test_compatibility_constraint_any() {
        let constraint = CompatibilityConstraint::Any;
        let v1_0_0 = SemanticVersion::new(1, 0, 0);
        let v9_9_9 = SemanticVersion::new(9, 9, 9);

        assert!(constraint.is_satisfied(v1_0_0, v9_9_9));
    }

    #[test]
    fn test_compatibility_constraint_display() {
        assert_eq!(
            format!("{}", CompatibilityConstraint::Exact),
            "exact version"
        );
        assert_eq!(
            format!("{}", CompatibilityConstraint::SameMajor),
            "same major version"
        );
    }

    // AbiCompatibility tests
    #[test]
    fn test_abi_compatibility_new() {
        let min = SemanticVersion::new(2, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min);

        assert_eq!(abi.plugin_version, plugin);
        assert_eq!(abi.min_loader_version, min);
        assert!(abi.max_loader_version.is_none());
    }

    #[test]
    fn test_abi_compatibility_with_max_version() {
        let min = SemanticVersion::new(2, 0, 0);
        let max = SemanticVersion::new(3, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min).with_max_version(max);

        assert_eq!(abi.max_loader_version, Some(max));
    }

    #[test]
    fn test_abi_compatibility_is_compatible_with_ok() {
        let min = SemanticVersion::new(2, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let loader = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min);

        assert!(abi.is_compatible_with(loader).is_ok());
    }

    #[test]
    fn test_abi_compatibility_is_compatible_with_too_old() {
        let min = SemanticVersion::new(2, 1, 0);
        let plugin = SemanticVersion::new(2, 2, 0);
        let loader = SemanticVersion::new(2, 0, 0);
        let abi = AbiCompatibility::new(plugin, min);

        assert!(abi.is_compatible_with(loader).is_err());
    }

    #[test]
    fn test_abi_compatibility_is_compatible_with_too_new() {
        let min = SemanticVersion::new(2, 0, 0);
        let max = SemanticVersion::new(2, 1, 0);
        let plugin = SemanticVersion::new(2, 0, 5);
        let loader = SemanticVersion::new(3, 0, 0);
        let abi = AbiCompatibility::new(plugin, min).with_max_version(max);

        assert!(abi.is_compatible_with(loader).is_err());
    }

    #[test]
    fn test_abi_compatibility_breaking_changes() {
        let min = SemanticVersion::new(2, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min)
            .add_breaking_change("Changed return type of plugin_init")
            .add_breaking_change("Removed plugin_cleanup");

        assert_eq!(abi.breaking_changes.len(), 2);
    }

    #[test]
    fn test_abi_compatibility_deprecated_features() {
        let min = SemanticVersion::new(2, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min)
            .add_deprecated_feature("plugin_legacy_init")
            .add_deprecated_feature("plugin_legacy_shutdown");

        assert_eq!(abi.deprecated_features.len(), 2);
    }

    #[test]
    fn test_abi_compatibility_display() {
        let min = SemanticVersion::new(2, 0, 0);
        let plugin = SemanticVersion::new(2, 1, 0);
        let abi = AbiCompatibility::new(plugin, min);
        let msg = format!("{}", abi);

        assert!(msg.contains("2.1.0"));
        assert!(msg.contains("2.0.0"));
    }

    // AbiVersionError tests
    #[test]
    fn test_abi_version_error_display() {
        let err = AbiVersionError::InvalidFormat {
            version: "1.2".to_string(),
            expected: "major.minor.patch".to_string(),
        };
        assert!(err.to_string().contains("1.2"));
    }

    #[test]
    fn test_abi_version_error_is_error_trait() {
        use std::error::Error;
        let err: Box<dyn Error> = Box::new(AbiVersionError::InvalidFormat {
            version: "bad".to_string(),
            expected: "1.2.3".to_string(),
        });
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_predefined_versions() {
        assert_eq!(versions::ABI_V2_0.major, 2);
        assert_eq!(versions::ABI_V2_1.minor, 1);
        assert_eq!(versions::ABI_V2_2.patch, 0);
    }
}
