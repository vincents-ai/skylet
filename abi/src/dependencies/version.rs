// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Semantic Versioning for Plugin Dependencies
//!
//! This module provides comprehensive semantic versioning support including:
//! - Full semver 2.0 compliance with pre-release and build metadata
//! - Version parsing and comparison
//! - Compatibility checking
//!
//! RFC-0005: Plugin Dependency Resolution

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// A semantic version following semver 2.0 specification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Major version (breaking changes)
    pub major: u64,
    /// Minor version (new features, backward compatible)
    pub minor: u64,
    /// Patch version (bug fixes, backward compatible)
    pub patch: u64,
    /// Pre-release metadata (e.g., "alpha.1", "beta.2")
    pub pre: Option<Prerelease>,
    /// Build metadata (ignored in comparisons)
    pub build: Option<String>,
}

/// Pre-release version component
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Prerelease {
    /// Pre-release identifiers (e.g., ["alpha", "1"] for "alpha.1")
    pub identifiers: Vec<PrereleaseIdentifier>,
}

impl Prerelease {
    /// Create a new pre-release from identifiers
    pub fn new(identifiers: Vec<PrereleaseIdentifier>) -> Self {
        Self { identifiers }
    }

    /// Parse a pre-release string (e.g., "alpha.1")
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        if s.is_empty() {
            return Err(VersionError::InvalidPrerelease {
                input: s.to_string(),
                reason: "pre-release cannot be empty".to_string(),
            });
        }

        let identifiers = s
            .split('.')
            .map(PrereleaseIdentifier::parse)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { identifiers })
    }

    /// Check if this pre-release is empty
    pub fn is_empty(&self) -> bool {
        self.identifiers.is_empty()
    }
}

impl fmt::Display for Prerelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.identifiers.iter().map(|id| id.to_string()).collect();
        write!(f, "{}", parts.join("."))
    }
}

/// A pre-release identifier (either numeric or alphanumeric)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrereleaseIdentifier {
    /// Numeric identifier (e.g., "1", "42")
    Numeric(u64),
    /// Alphanumeric identifier (e.g., "alpha", "beta-2")
    Alpha(String),
}

impl PrereleaseIdentifier {
    /// Parse a pre-release identifier
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        if s.is_empty() {
            return Err(VersionError::InvalidPrerelease {
                input: s.to_string(),
                reason: "identifier cannot be empty".to_string(),
            });
        }

        // Check for leading zeros in numeric identifiers
        if s.chars().all(|c| c.is_ascii_digit()) {
            if s.len() > 1 && s.starts_with('0') {
                return Err(VersionError::InvalidPrerelease {
                    input: s.to_string(),
                    reason: "numeric identifier cannot have leading zeros".to_string(),
                });
            }
            s.parse::<u64>()
                .map(PrereleaseIdentifier::Numeric)
                .map_err(|_| VersionError::InvalidPrerelease {
                    input: s.to_string(),
                    reason: "numeric identifier overflow".to_string(),
                })
        } else {
            // Validate alphanumeric identifier
            if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(VersionError::InvalidPrerelease {
                    input: s.to_string(),
                    reason: "identifier must be alphanumeric or hyphen".to_string(),
                });
            }
            Ok(PrereleaseIdentifier::Alpha(s.to_string()))
        }
    }

    /// Check if this identifier is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(self, PrereleaseIdentifier::Numeric(_))
    }
}

impl fmt::Display for PrereleaseIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrereleaseIdentifier::Numeric(n) => write!(f, "{}", n),
            PrereleaseIdentifier::Alpha(s) => write!(f, "{}", s),
        }
    }
}

impl PartialOrd for PrereleaseIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrereleaseIdentifier {
    fn cmp(&self, other: &Self) -> Ordering {
        // Numeric identifiers always have lower precedence than alphanumeric
        match (self, other) {
            (PrereleaseIdentifier::Numeric(a), PrereleaseIdentifier::Numeric(b)) => a.cmp(b),
            (PrereleaseIdentifier::Numeric(_), PrereleaseIdentifier::Alpha(_)) => Ordering::Less,
            (PrereleaseIdentifier::Alpha(_), PrereleaseIdentifier::Numeric(_)) => Ordering::Greater,
            (PrereleaseIdentifier::Alpha(a), PrereleaseIdentifier::Alpha(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Prerelease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Prerelease {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare identifier by identifier
        for (a, b) in self.identifiers.iter().zip(other.identifiers.iter()) {
            match a.cmp(b) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }
        // Longer pre-release has higher precedence
        self.identifiers.len().cmp(&other.identifiers.len())
    }
}

impl Version {
    /// Create a new version
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: None,
        }
    }

    /// Create a version with pre-release metadata
    pub fn with_pre(mut self, pre: Prerelease) -> Self {
        self.pre = Some(pre);
        self
    }

    /// Create a version with build metadata
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.build = Some(build.into());
        self
    }

    /// Parse a version string (e.g., "1.2.3", "1.2.3-alpha.1+build.123")
    pub fn parse(version_str: &str) -> Result<Self, VersionError> {
        let version_str = version_str.trim();

        if version_str.is_empty() {
            return Err(VersionError::InvalidFormat {
                version: version_str.to_string(),
                reason: "version string cannot be empty".to_string(),
            });
        }

        // Split off build metadata first (+)
        let (version_part, build) = match version_str.split_once('+') {
            Some((v, b)) => {
                if b.is_empty() {
                    return Err(VersionError::InvalidBuildMetadata {
                        input: version_str.to_string(),
                        reason: "build metadata cannot be empty after '+'".to_string(),
                    });
                }
                (v, Some(b.to_string()))
            }
            None => (version_str, None),
        };

        // Split off pre-release (-)
        let (core_version, pre) = match version_part.split_once('-') {
            Some((v, p)) => {
                if p.is_empty() {
                    return Err(VersionError::InvalidPrerelease {
                        input: version_str.to_string(),
                        reason: "pre-release cannot be empty after '-'".to_string(),
                    });
                }
                (v, Some(Prerelease::parse(p)?))
            }
            None => (version_part, None),
        };

        // Parse core version (major.minor.patch)
        let parts: Vec<&str> = core_version.split('.').collect();
        if parts.len() != 3 {
            return Err(VersionError::InvalidFormat {
                version: version_str.to_string(),
                reason: format!("expected major.minor.patch, got {} parts", parts.len()),
            });
        }

        let major = parts[0]
            .parse::<u64>()
            .map_err(|_| VersionError::InvalidFormat {
                version: version_str.to_string(),
                reason: format!("invalid major version: {}", parts[0]),
            })?;

        let minor = parts[1]
            .parse::<u64>()
            .map_err(|_| VersionError::InvalidFormat {
                version: version_str.to_string(),
                reason: format!("invalid minor version: {}", parts[1]),
            })?;

        let patch = parts[2]
            .parse::<u64>()
            .map_err(|_| VersionError::InvalidFormat {
                version: version_str.to_string(),
                reason: format!("invalid patch version: {}", parts[2]),
            })?;

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    /// Check if this is a pre-release version
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }

    /// Check if this version is stable (no pre-release)
    pub fn is_stable(&self) -> bool {
        self.pre.is_none()
    }

    /// Get the base version without pre-release or build metadata
    pub fn base_version(&self) -> Self {
        Self::new(self.major, self.minor, self.patch)
    }

    /// Bump the major version
    pub fn bump_major(&self) -> Self {
        Self::new(self.major + 1, 0, 0)
    }

    /// Bump the minor version
    pub fn bump_minor(&self) -> Self {
        Self::new(self.major, self.minor + 1, 0)
    }

    /// Bump the patch version
    pub fn bump_patch(&self) -> Self {
        Self::new(self.major, self.minor, self.patch + 1)
    }

    /// Check compatibility with another version (same major version)
    pub fn is_compatible_with(&self, other: &Version) -> bool {
        self.major == other.major && self.major > 0
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major.minor.patch
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Pre-release versions have lower precedence
        match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater, // stable > pre-release
            (Some(_), None) => Ordering::Less,    // pre-release < stable
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.pre {
            write!(f, "-{}", pre)?;
        }
        if let Some(ref build) = self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new(0, 1, 0)
    }
}

/// Errors that can occur when parsing or manipulating versions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// Invalid version format
    InvalidFormat { version: String, reason: String },
    /// Invalid pre-release format
    InvalidPrerelease { input: String, reason: String },
    /// Invalid build metadata
    InvalidBuildMetadata { input: String, reason: String },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::InvalidFormat { version, reason } => {
                write!(f, "Invalid version format '{}': {}", version, reason)
            }
            VersionError::InvalidPrerelease { input, reason } => {
                write!(f, "Invalid pre-release '{}': {}", input, reason)
            }
            VersionError::InvalidBuildMetadata { input, reason } => {
                write!(f, "Invalid build metadata '{}': {}", input, reason)
            }
        }
    }
}

impl std::error::Error for VersionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_new() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_none());
        assert!(v.build.is_none());
    }

    #[test]
    fn test_version_parse_simple() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_with_prerelease() {
        let v = Version::parse("1.2.3-alpha.1").unwrap();
        assert_eq!(v.major, 1);
        assert!(v.pre.is_some());
        let pre = v.pre.unwrap();
        assert_eq!(pre.identifiers.len(), 2);
    }

    #[test]
    fn test_version_parse_with_build() {
        let v = Version::parse("1.2.3+build.123").unwrap();
        assert_eq!(v.build, Some("build.123".to_string()));
    }

    #[test]
    fn test_version_parse_full() {
        let v = Version::parse("1.2.3-beta.2+exp.sha.5114f85").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_some());
        assert!(v.build.is_some());
    }

    #[test]
    fn test_version_parse_invalid() {
        assert!(Version::parse("").is_err());
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("a.b.c").is_err());
        assert!(Version::parse("1.2.3-").is_err());
        assert!(Version::parse("1.2.3+").is_err());
    }

    #[test]
    fn test_version_ordering() {
        let v1_0_0 = Version::new(1, 0, 0);
        let v1_0_1 = Version::new(1, 0, 1);
        let v1_1_0 = Version::new(1, 1, 0);
        let v2_0_0 = Version::new(2, 0, 0);

        assert!(v1_0_0 < v1_0_1);
        assert!(v1_0_1 < v1_1_0);
        assert!(v1_1_0 < v2_0_0);
    }

    #[test]
    fn test_version_prerelease_ordering() {
        let v_stable = Version::new(1, 0, 0);
        let v_alpha = Version::parse("1.0.0-alpha").unwrap();
        let v_beta = Version::parse("1.0.0-beta").unwrap();

        assert!(v_alpha < v_beta);
        assert!(v_beta < v_stable);
    }

    #[test]
    fn test_version_display() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let v_pre = Version::parse("1.2.3-alpha.1+build").unwrap();
        assert_eq!(v_pre.to_string(), "1.2.3-alpha.1+build");
    }

    #[test]
    fn test_version_bump() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.bump_major(), Version::new(2, 0, 0));
        assert_eq!(v.bump_minor(), Version::new(1, 3, 0));
        assert_eq!(v.bump_patch(), Version::new(1, 2, 4));
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0_0 = Version::new(1, 0, 0);
        let v1_9_0 = Version::new(1, 9, 0);
        let v2_0_0 = Version::new(2, 0, 0);
        let v0_1_0 = Version::new(0, 1, 0);
        let v0_2_0 = Version::new(0, 2, 0);

        assert!(v1_0_0.is_compatible_with(&v1_9_0));
        assert!(!v1_0_0.is_compatible_with(&v2_0_0));
        // 0.x versions are not considered compatible with each other
        assert!(!v0_1_0.is_compatible_with(&v0_2_0));
    }

    #[test]
    fn test_prerelease_identifier_ordering() {
        let alpha = PrereleaseIdentifier::Alpha("alpha".to_string());
        let beta = PrereleaseIdentifier::Alpha("beta".to_string());
        let num1 = PrereleaseIdentifier::Numeric(1);
        let num2 = PrereleaseIdentifier::Numeric(2);

        assert!(num1 < num2);
        assert!(num2 < alpha);
        assert!(alpha < beta);
    }

    #[test]
    fn test_version_is_stable() {
        let stable = Version::new(1, 0, 0);
        let prerelease = Version::parse("1.0.0-alpha").unwrap();

        assert!(stable.is_stable());
        assert!(!prerelease.is_stable());
        assert!(prerelease.is_prerelease());
    }
}
