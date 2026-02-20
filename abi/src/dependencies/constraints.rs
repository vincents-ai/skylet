// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Version Constraint Parsing and Matching
//!
//! This module provides Cargo-style version constraint support including:
//! - Caret (^) constraints - compatible with version
//! - Tilde (~) constraints - approximately equivalent
//! - Exact (=) constraints - exact version match
//! - Comparison constraints (<, >, <=, >=)
//! - Wildcard (*) constraints - any version
//! - Range constraints (e.g., ">= 1.0.0, < 2.0.0")
//!
//! RFC-0005: Plugin Dependency Resolution

use crate::dependencies::version::{Version, VersionError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A version constraint that can be matched against versions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionConstraint {
    /// The constraint type
    pub comparator: Comparator,
    /// The version to compare against
    pub version: Version,
}

/// Comparison operators for version constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Comparator {
    /// Exact match (= 1.2.3)
    Exact,
    /// Greater than (> 1.2.3)
    Greater,
    /// Greater than or equal (>= 1.2.3)
    GreaterEq,
    /// Less than (< 1.2.3)
    Less,
    /// Less than or equal (<= 1.2.3)
    LessEq,
    /// Caret - compatible with version (^1.2.3 means >=1.2.3, <2.0.0)
    Caret,
    /// Tilde - approximately equivalent (~1.2.3 means >=1.2.3, <1.3.0)
    Tilde,
    /// Wildcard - any version (* or 1.* or 1.2.*)
    Wildcard,
}

impl VersionConstraint {
    /// Create a new version constraint
    pub fn new(comparator: Comparator, version: Version) -> Self {
        Self {
            comparator,
            version,
        }
    }

    /// Parse a constraint string (e.g., "^1.2.3", ">= 1.0.0, < 2.0.0")
    pub fn parse(input: &str) -> Result<Self, ConstraintError> {
        let input = input.trim();

        if input.is_empty() {
            return Err(ConstraintError::EmptyInput);
        }

        // Handle wildcard
        if input == "*" {
            return Ok(Self {
                comparator: Comparator::Wildcard,
                version: Version::new(0, 0, 0),
            });
        }

        // Extract comparator and version
        let (comparator, version_str) = Self::parse_comparator(input)?;
        let version = Version::parse(version_str).map_err(|e| ConstraintError::InvalidVersion {
            input: input.to_string(),
            source: e,
        })?;

        // Handle wildcard versions (1.*, 1.2.*)
        if version_str.ends_with(".*") {
            return Ok(Self {
                comparator: Comparator::Wildcard,
                version,
            });
        }

        Ok(Self {
            comparator,
            version,
        })
    }

    fn parse_comparator(input: &str) -> Result<(Comparator, &str), ConstraintError> {
        let input = input.trim_start();

        // Check for two-character operators first
        if let Some(rest) = input.strip_prefix(">=") {
            return Ok((Comparator::GreaterEq, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix("<=") {
            return Ok((Comparator::LessEq, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix("^") {
            return Ok((Comparator::Caret, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix("~") {
            return Ok((Comparator::Tilde, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix("=") {
            return Ok((Comparator::Exact, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix(">") {
            return Ok((Comparator::Greater, rest.trim()));
        }
        if let Some(rest) = input.strip_prefix("<") {
            return Ok((Comparator::Less, rest.trim()));
        }

        // No explicit comparator means caret (Cargo-style)
        Ok((Comparator::Caret, input))
    }

    /// Check if a version satisfies this constraint
    pub fn matches(&self, version: &Version) -> bool {
        match self.comparator {
            Comparator::Exact => version == &self.version,
            Comparator::Greater => version > &self.version,
            Comparator::GreaterEq => version >= &self.version,
            Comparator::Less => version < &self.version,
            Comparator::LessEq => version <= &self.version,
            Comparator::Caret => self.matches_caret(version),
            Comparator::Tilde => self.matches_tilde(version),
            Comparator::Wildcard => self.matches_wildcard(version),
        }
    }

    fn matches_caret(&self, version: &Version) -> bool {
        // ^1.2.3 := >=1.2.3, <2.0.0
        // ^0.2.3 := >=0.2.3, <0.3.0
        // ^0.0.3 := >=0.0.3, <0.0.4
        // ^0.0 := >=0.0.0, <0.1.0
        // ^0 := >=0.0.0, <1.0.0

        if version < &self.version {
            return false;
        }

        if self.version.major > 0 {
            // ^1.2.3 -> < 2.0.0
            version.major == self.version.major
        } else if self.version.minor > 0 {
            // ^0.2.3 -> < 0.3.0
            version.major == 0 && version.minor == self.version.minor
        } else {
            // ^0.0.3 -> < 0.0.4
            version.major == 0 && version.minor == 0 && version.patch == self.version.patch
        }
    }

    fn matches_tilde(&self, version: &Version) -> bool {
        // ~1.2.3 := >=1.2.3, <1.3.0
        // ~1.2 := >=1.2.0, <1.3.0
        // ~1 := >=1.0.0, <2.0.0

        if version < &self.version {
            return false;
        }

        version.major == self.version.major && version.minor == self.version.minor
    }

    fn matches_wildcard(&self, version: &Version) -> bool {
        // * matches anything
        // 1.* matches any 1.x.x
        // 1.2.* matches any 1.2.x

        if self.version.major == 0 && self.version.minor == 0 && self.version.patch == 0 {
            // Pure wildcard (*)
            return true;
        }

        if self.version.minor == 0 && self.version.patch == 0 {
            // Major wildcard (1.*)
            return version.major == self.version.major;
        }

        if self.version.patch == 0 {
            // Major.minor wildcard (1.2.*)
            return version.major == self.version.major && version.minor == self.version.minor;
        }

        // Shouldn't reach here for proper wildcards
        version >= &self.version
    }
}

impl fmt::Display for VersionConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self.comparator {
            Comparator::Exact => "=",
            Comparator::Greater => ">",
            Comparator::GreaterEq => ">=",
            Comparator::Less => "<",
            Comparator::LessEq => "<=",
            Comparator::Caret => "^",
            Comparator::Tilde => "~",
            Comparator::Wildcard => "*",
        };
        write!(f, "{}{}", op, self.version)
    }
}

/// A version requirement with multiple constraints (e.g., ">= 1.0.0, < 2.0.0")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionReq {
    /// The constraints that must all be satisfied
    pub constraints: Vec<VersionConstraint>,
}

impl VersionReq {
    /// Create an empty requirement (matches any version)
    pub fn any() -> Self {
        Self {
            constraints: vec![],
        }
    }

    /// Create a requirement from a single constraint
    pub fn single(constraint: VersionConstraint) -> Self {
        Self {
            constraints: vec![constraint],
        }
    }

    /// Parse a requirement string
    /// Supports: "^1.2.3", ">= 1.0.0, < 2.0.0", "~1.2", "1.2.3", "*"
    pub fn parse(input: &str) -> Result<Self, ConstraintError> {
        let input = input.trim();

        if input.is_empty() || input == "*" {
            return Ok(Self::any());
        }

        let mut constraints = Vec::new();

        // Split on comma for range constraints
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            constraints.push(VersionConstraint::parse(part)?);
        }

        if constraints.is_empty() {
            return Ok(Self::any());
        }

        Ok(Self { constraints })
    }

    /// Check if a version satisfies all constraints
    pub fn matches(&self, version: &Version) -> bool {
        if self.constraints.is_empty() {
            return true; // No constraints = any version
        }
        self.constraints.iter().all(|c| c.matches(version))
    }

    /// Check if this is an "any" requirement
    pub fn is_any(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Add a constraint to this requirement
    pub fn with_constraint(mut self, constraint: VersionConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.constraints.is_empty() {
            write!(f, "*")
        } else {
            let parts: Vec<String> = self.constraints.iter().map(|c| c.to_string()).collect();
            write!(f, "{}", parts.join(", "))
        }
    }
}

impl Default for VersionReq {
    fn default() -> Self {
        Self::any()
    }
}

/// Errors that can occur when parsing constraints
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintError {
    /// Empty constraint input
    EmptyInput,
    /// Invalid constraint format
    InvalidFormat { input: String, reason: String },
    /// Invalid version in constraint
    InvalidVersion { input: String, source: VersionError },
    /// Conflicting constraints
    ConflictingConstraints { constraints: String },
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintError::EmptyInput => write!(f, "Empty constraint input"),
            ConstraintError::InvalidFormat { input, reason } => {
                write!(f, "Invalid constraint format '{}': {}", input, reason)
            }
            ConstraintError::InvalidVersion { input, source } => {
                write!(f, "Invalid version in constraint '{}': {}", input, source)
            }
            ConstraintError::ConflictingConstraints { constraints } => {
                write!(f, "Conflicting constraints: {}", constraints)
            }
        }
    }
}

impl std::error::Error for ConstraintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConstraintError::InvalidVersion { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_parse_caret() {
        let c = VersionConstraint::parse("^1.2.3").unwrap();
        assert_eq!(c.comparator, Comparator::Caret);
        assert_eq!(c.version, Version::new(1, 2, 3));
    }

    #[test]
    fn test_constraint_parse_tilde() {
        let c = VersionConstraint::parse("~1.2.3").unwrap();
        assert_eq!(c.comparator, Comparator::Tilde);
        assert_eq!(c.version, Version::new(1, 2, 3));
    }

    #[test]
    fn test_constraint_parse_exact() {
        let c = VersionConstraint::parse("=1.2.3").unwrap();
        assert_eq!(c.comparator, Comparator::Exact);
    }

    #[test]
    fn test_constraint_parse_comparison() {
        let c1 = VersionConstraint::parse(">=1.0.0").unwrap();
        assert_eq!(c1.comparator, Comparator::GreaterEq);

        let c2 = VersionConstraint::parse(">1.0.0").unwrap();
        assert_eq!(c2.comparator, Comparator::Greater);

        let c3 = VersionConstraint::parse("<=2.0.0").unwrap();
        assert_eq!(c3.comparator, Comparator::LessEq);

        let c4 = VersionConstraint::parse("<2.0.0").unwrap();
        assert_eq!(c4.comparator, Comparator::Less);
    }

    #[test]
    fn test_constraint_parse_implicit_caret() {
        let c = VersionConstraint::parse("1.2.3").unwrap();
        assert_eq!(c.comparator, Comparator::Caret);
    }

    #[test]
    fn test_constraint_parse_wildcard() {
        let c = VersionConstraint::parse("*").unwrap();
        assert_eq!(c.comparator, Comparator::Wildcard);
    }

    #[test]
    fn test_constraint_matches_exact() {
        let c = VersionConstraint::parse("=1.2.3").unwrap();
        assert!(c.matches(&Version::new(1, 2, 3)));
        assert!(!c.matches(&Version::new(1, 2, 4)));
    }

    #[test]
    fn test_constraint_matches_greater() {
        let c = VersionConstraint::parse(">1.2.3").unwrap();
        assert!(!c.matches(&Version::new(1, 2, 3)));
        assert!(c.matches(&Version::new(1, 2, 4)));
        assert!(c.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn test_constraint_matches_caret() {
        let c = VersionConstraint::parse("^1.2.3").unwrap();

        // Matches
        assert!(c.matches(&Version::new(1, 2, 3)));
        assert!(c.matches(&Version::new(1, 2, 4)));
        assert!(c.matches(&Version::new(1, 9, 9)));

        // Doesn't match
        assert!(!c.matches(&Version::new(1, 2, 2)));
        assert!(!c.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_constraint_matches_caret_zero() {
        // ^0.2.3 means >=0.2.3, <0.3.0
        let c = VersionConstraint::parse("^0.2.3").unwrap();

        assert!(c.matches(&Version::new(0, 2, 3)));
        assert!(c.matches(&Version::new(0, 2, 9)));
        assert!(!c.matches(&Version::new(0, 3, 0)));
        assert!(!c.matches(&Version::new(1, 0, 0)));
    }

    #[test]
    fn test_constraint_matches_tilde() {
        let c = VersionConstraint::parse("~1.2.3").unwrap();

        // Matches (same major.minor)
        assert!(c.matches(&Version::new(1, 2, 3)));
        assert!(c.matches(&Version::new(1, 2, 9)));

        // Doesn't match
        assert!(!c.matches(&Version::new(1, 2, 2)));
        assert!(!c.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn test_constraint_matches_wildcard() {
        let c = VersionConstraint::parse("*").unwrap();
        assert!(c.matches(&Version::new(0, 0, 1)));
        assert!(c.matches(&Version::new(1, 0, 0)));
        assert!(c.matches(&Version::new(99, 99, 99)));
    }

    #[test]
    fn test_version_req_single() {
        let req = VersionReq::parse("^1.2.3").unwrap();

        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_version_req_range() {
        let req = VersionReq::parse(">= 1.0.0, < 2.0.0").unwrap();

        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(1, 9, 9)));
        assert!(!req.matches(&Version::new(0, 9, 9)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_version_req_any() {
        let req = VersionReq::parse("*").unwrap();

        assert!(req.matches(&Version::new(0, 0, 1)));
        assert!(req.matches(&Version::new(99, 99, 99)));
    }

    #[test]
    fn test_version_req_display() {
        let req = VersionReq::parse(">= 1.0.0, < 2.0.0").unwrap();
        assert_eq!(req.to_string(), ">=1.0.0, <2.0.0");
    }

    #[test]
    fn test_constraint_display() {
        let c = VersionConstraint::parse("^1.2.3").unwrap();
        assert_eq!(c.to_string(), "^1.2.3");

        let c2 = VersionConstraint::parse(">=1.0.0").unwrap();
        assert_eq!(c2.to_string(), ">=1.0.0");
    }
}
