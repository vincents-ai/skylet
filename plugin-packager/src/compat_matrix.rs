// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Plugin compatibility matrix and analysis
///
/// This module provides comprehensive compatibility analysis including:
/// - ABI version compatibility matrix
/// - OS/architecture support matrix
/// - Dependency compatibility analysis
/// - Breaking change detection and tracking
/// - Compatibility recommendations
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ABI version representation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AbiVersion {
    major: u32,
    minor: u32,
}

impl AbiVersion {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 {
            if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return Some(Self { major, minor });
            }
        }
        None
    }

    pub fn to_string_val(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    pub fn is_compatible_with(&self, other: &AbiVersion) -> bool {
        // Same major version is compatible
        self.major == other.major
    }

    pub fn is_backward_compatible(&self, older: &AbiVersion) -> bool {
        // Major version must be the same
        if self.major != older.major {
            return false;
        }
        // Must be newer or equal
        self.minor >= older.minor
    }
}

/// OS/Architecture combination
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlatformArch {
    pub platform: String,
    pub architecture: String,
}

impl PlatformArch {
    pub fn new(platform: &str, architecture: &str) -> Self {
        Self {
            platform: platform.to_string(),
            architecture: architecture.to_string(),
        }
    }

    pub fn as_string(&self) -> String {
        format!("{}-{}", self.platform, self.architecture)
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }
}

/// Compatibility level for versions/platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CompatibilityLevel {
    Incompatible, // 0
    Deprecated,   // 1
    Compatible,   // 2
    Recommended,  // 3
}

impl CompatibilityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompatibilityLevel::Incompatible => "Incompatible",
            CompatibilityLevel::Deprecated => "Deprecated",
            CompatibilityLevel::Compatible => "Compatible",
            CompatibilityLevel::Recommended => "Recommended",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s {
            "Incompatible" => Some(CompatibilityLevel::Incompatible),
            "Deprecated" => Some(CompatibilityLevel::Deprecated),
            "Compatible" => Some(CompatibilityLevel::Compatible),
            "Recommended" => Some(CompatibilityLevel::Recommended),
            _ => None,
        }
    }
}

/// Breaking change information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChange {
    pub from_version: String,
    pub to_version: String,
    pub description: String,
    pub migration_guide: Option<String>,
    pub affected_apis: Vec<String>,
}

/// ABI version compatibility entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiCompatibilityEntry {
    pub version: AbiVersion,
    pub compatibility_with: HashMap<String, CompatibilityLevel>,
    pub breaking_changes: Vec<BreakingChange>,
    pub deprecation_notices: Vec<String>,
}

impl AbiCompatibilityEntry {
    pub fn new(version: AbiVersion) -> Self {
        Self {
            version,
            compatibility_with: HashMap::new(),
            breaking_changes: Vec::new(),
            deprecation_notices: Vec::new(),
        }
    }

    pub fn is_compatible_with(&self, other_version: &str) -> bool {
        matches!(
            self.compatibility_with.get(other_version),
            Some(CompatibilityLevel::Compatible) | Some(CompatibilityLevel::Recommended)
        )
    }
}

/// Platform support matrix entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSupportEntry {
    pub platform_arch: PlatformArch,
    pub support_level: CompatibilityLevel,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub notes: Option<String>,
}

/// Dependency compatibility check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCompatibility {
    pub dependency_name: String,
    pub required_version: String,
    pub compatible_versions: Vec<String>,
    pub incompatible_versions: Vec<String>,
    pub notes: Option<String>,
}

/// Comprehensive compatibility matrix
pub struct PluginCompatibilityMatrix {
    plugin_id: String,
    current_version: String,
    abi_compatibility: HashMap<AbiVersion, AbiCompatibilityEntry>,
    platform_support: HashMap<PlatformArch, PlatformSupportEntry>,
    dependency_compatibility: HashMap<String, DependencyCompatibility>,
}

impl PluginCompatibilityMatrix {
    /// Create a new compatibility matrix
    pub fn new(plugin_id: &str, current_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            current_version: current_version.to_string(),
            abi_compatibility: HashMap::new(),
            platform_support: HashMap::new(),
            dependency_compatibility: HashMap::new(),
        }
    }

    /// Add ABI compatibility information
    pub fn add_abi_compatibility(&mut self, entry: AbiCompatibilityEntry) {
        self.abi_compatibility.insert(entry.version.clone(), entry);
    }

    /// Add platform support information
    pub fn add_platform_support(&mut self, entry: PlatformSupportEntry) {
        self.platform_support
            .insert(entry.platform_arch.clone(), entry);
    }

    /// Add dependency compatibility information
    pub fn add_dependency_compatibility(&mut self, compat: DependencyCompatibility) {
        self.dependency_compatibility
            .insert(compat.dependency_name.clone(), compat);
    }

    /// Check if two ABI versions are compatible
    pub fn check_abi_compatibility(&self, v1: &AbiVersion, v2: &AbiVersion) -> CompatibilityLevel {
        if let Some(entry) = self.abi_compatibility.get(v1) {
            entry
                .compatibility_with
                .get(&v2.to_string_val())
                .copied()
                .unwrap_or(CompatibilityLevel::Incompatible)
        } else {
            // Default: same major version is compatible
            if v1.is_compatible_with(v2) {
                CompatibilityLevel::Compatible
            } else {
                CompatibilityLevel::Incompatible
            }
        }
    }

    /// Check platform support
    pub fn check_platform_support(&self, platform: &PlatformArch) -> CompatibilityLevel {
        self.platform_support
            .get(platform)
            .map(|entry| entry.support_level)
            .unwrap_or(CompatibilityLevel::Compatible)
    }

    /// Get all supported platforms
    pub fn supported_platforms(&self) -> Vec<PlatformArch> {
        self.platform_support
            .values()
            .filter(|entry| entry.support_level != CompatibilityLevel::Incompatible)
            .map(|entry| entry.platform_arch.clone())
            .collect()
    }

    /// Check dependency version compatibility
    pub fn check_dependency_version(&self, dep_name: &str, version: &str) -> CompatibilityLevel {
        if let Some(compat) = self.dependency_compatibility.get(dep_name) {
            if compat.compatible_versions.contains(&version.to_string()) {
                CompatibilityLevel::Compatible
            } else if compat.incompatible_versions.contains(&version.to_string()) {
                CompatibilityLevel::Incompatible
            } else {
                CompatibilityLevel::Compatible
            }
        } else {
            CompatibilityLevel::Compatible
        }
    }

    /// Find breaking changes between two versions
    pub fn find_breaking_changes(
        &self,
        from_version: &str,
        to_version: &str,
    ) -> Vec<BreakingChange> {
        let mut changes = Vec::new();
        for entry in self.abi_compatibility.values() {
            for change in &entry.breaking_changes {
                if change.from_version == from_version && change.to_version == to_version {
                    changes.push(change.clone());
                }
            }
        }
        changes
    }

    /// Get compatibility analysis for a specific ABI version
    pub fn analyze_abi_version(&self, version: &AbiVersion) -> CompatibilityAnalysis {
        let mut compatible_versions = Vec::new();
        let mut incompatible_versions = Vec::new();
        let mut breaking_changes_summary = Vec::new();

        if let Some(entry) = self.abi_compatibility.get(version) {
            for (ver_str, level) in &entry.compatibility_with {
                match level {
                    CompatibilityLevel::Compatible | CompatibilityLevel::Recommended => {
                        compatible_versions.push(ver_str.clone());
                    }
                    CompatibilityLevel::Incompatible => {
                        incompatible_versions.push(ver_str.clone());
                    }
                    _ => {}
                }
            }

            for change in &entry.breaking_changes {
                breaking_changes_summary.push(change.description.clone());
            }
        }

        CompatibilityAnalysis {
            version: version.to_string_val(),
            compatible_versions,
            incompatible_versions,
            breaking_changes: breaking_changes_summary,
            supported_platforms: self
                .supported_platforms()
                .iter()
                .map(|p| p.as_string())
                .collect(),
            deprecation_status: self.abi_compatibility.get(version).and_then(|e| {
                if e.deprecation_notices.is_empty() {
                    None
                } else {
                    Some(e.deprecation_notices.clone())
                }
            }),
        }
    }

    /// Get all supported ABI versions
    pub fn all_abi_versions(&self) -> Vec<AbiVersion> {
        self.abi_compatibility.keys().cloned().collect()
    }

    /// Generate compatibility report
    pub fn generate_report(&self) -> CompatibilityReport {
        CompatibilityReport {
            plugin_id: self.plugin_id.clone(),
            current_version: self.current_version.clone(),
            total_abi_versions: self.abi_compatibility.len(),
            supported_platform_count: self
                .platform_support
                .values()
                .filter(|p| p.support_level != CompatibilityLevel::Incompatible)
                .count(),
            tracked_dependencies: self.dependency_compatibility.len(),
        }
    }
}

/// Compatibility analysis for a specific version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityAnalysis {
    pub version: String,
    pub compatible_versions: Vec<String>,
    pub incompatible_versions: Vec<String>,
    pub breaking_changes: Vec<String>,
    pub supported_platforms: Vec<String>,
    pub deprecation_status: Option<Vec<String>>,
}

/// Compatibility report summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub plugin_id: String,
    pub current_version: String,
    pub total_abi_versions: usize,
    pub supported_platform_count: usize,
    pub tracked_dependencies: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version_creation() {
        let v1 = AbiVersion::new(1, 0);
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 0);
    }

    #[test]
    fn test_abi_version_from_str() {
        let v = AbiVersion::from_str("2.1").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 1);
    }

    #[test]
    fn test_abi_version_to_string() {
        let v = AbiVersion::new(1, 5);
        assert_eq!(v.to_string_val(), "1.5");
    }

    #[test]
    fn test_abi_version_is_compatible() {
        let v1 = AbiVersion::new(1, 0);
        let v2 = AbiVersion::new(1, 5);
        assert!(v1.is_compatible_with(&v2));
    }

    #[test]
    fn test_abi_version_incompatible_major() {
        let v1 = AbiVersion::new(1, 0);
        let v2 = AbiVersion::new(2, 0);
        assert!(!v1.is_compatible_with(&v2));
    }

    #[test]
    fn test_abi_version_backward_compatible() {
        let v_new = AbiVersion::new(1, 5);
        let v_old = AbiVersion::new(1, 0);
        assert!(v_new.is_backward_compatible(&v_old));
    }

    #[test]
    fn test_abi_version_backward_incompatible() {
        let v_old = AbiVersion::new(1, 5);
        let v_new = AbiVersion::new(1, 0);
        assert!(!v_new.is_backward_compatible(&v_old));
    }

    #[test]
    fn test_platform_arch_creation() {
        let pa = PlatformArch::new("linux", "x86_64");
        assert_eq!(pa.platform, "linux");
        assert_eq!(pa.architecture, "x86_64");
    }

    #[test]
    fn test_platform_arch_as_string() {
        let pa = PlatformArch::new("linux", "x86_64");
        assert_eq!(pa.as_string(), "linux-x86_64");
    }

    #[test]
    fn test_platform_arch_from_string() {
        let pa = PlatformArch::from_string("linux-x86_64").unwrap();
        assert_eq!(pa.platform, "linux");
        assert_eq!(pa.architecture, "x86_64");
    }

    #[test]
    fn test_compatibility_level_to_str() {
        assert_eq!(CompatibilityLevel::Incompatible.as_str(), "Incompatible");
        assert_eq!(CompatibilityLevel::Deprecated.as_str(), "Deprecated");
        assert_eq!(CompatibilityLevel::Compatible.as_str(), "Compatible");
        assert_eq!(CompatibilityLevel::Recommended.as_str(), "Recommended");
    }

    #[test]
    fn test_compatibility_level_from_str() {
        assert_eq!(
            CompatibilityLevel::try_parse("Compatible"),
            Some(CompatibilityLevel::Compatible)
        );
        assert_eq!(
            CompatibilityLevel::try_parse("Incompatible"),
            Some(CompatibilityLevel::Incompatible)
        );
    }

    #[test]
    fn test_compatibility_level_ordering() {
        assert!(CompatibilityLevel::Incompatible < CompatibilityLevel::Deprecated);
        assert!(CompatibilityLevel::Deprecated < CompatibilityLevel::Compatible);
        assert!(CompatibilityLevel::Compatible < CompatibilityLevel::Recommended);
    }

    #[test]
    fn test_abi_compatibility_entry_creation() {
        let v = AbiVersion::new(1, 0);
        let entry = AbiCompatibilityEntry::new(v);
        assert_eq!(entry.version.major, 1);
        assert_eq!(entry.breaking_changes.len(), 0);
    }

    #[test]
    fn test_breaking_change_creation() {
        let change = BreakingChange {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            description: "API changed".to_string(),
            migration_guide: Some("See upgrade guide".to_string()),
            affected_apis: vec!["plugin_init".to_string()],
        };
        assert_eq!(change.from_version, "1.0");
        assert_eq!(change.affected_apis.len(), 1);
    }

    #[test]
    fn test_matrix_creation() {
        let matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        assert_eq!(matrix.plugin_id, "test-plugin");
        assert_eq!(matrix.current_version, "1.0.0");
    }

    #[test]
    fn test_matrix_add_abi_compatibility() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let entry = AbiCompatibilityEntry::new(AbiVersion::new(1, 0));
        matrix.add_abi_compatibility(entry);
        assert_eq!(matrix.abi_compatibility.len(), 1);
    }

    #[test]
    fn test_matrix_add_platform_support() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let pa = PlatformArch::new("linux", "x86_64");
        let entry = PlatformSupportEntry {
            platform_arch: pa,
            support_level: CompatibilityLevel::Recommended,
            min_version: None,
            max_version: None,
            notes: None,
        };
        matrix.add_platform_support(entry);
        assert_eq!(matrix.platform_support.len(), 1);
    }

    #[test]
    fn test_matrix_check_abi_compatibility_default() {
        let matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let v1 = AbiVersion::new(1, 0);
        let v2 = AbiVersion::new(1, 5);
        let level = matrix.check_abi_compatibility(&v1, &v2);
        assert_eq!(level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn test_matrix_supported_platforms() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");

        let pa1 = PlatformArch::new("linux", "x86_64");
        let entry1 = PlatformSupportEntry {
            platform_arch: pa1,
            support_level: CompatibilityLevel::Recommended,
            min_version: None,
            max_version: None,
            notes: None,
        };
        matrix.add_platform_support(entry1);

        let pa2 = PlatformArch::new("macos", "arm64");
        let entry2 = PlatformSupportEntry {
            platform_arch: pa2,
            support_level: CompatibilityLevel::Compatible,
            min_version: None,
            max_version: None,
            notes: None,
        };
        matrix.add_platform_support(entry2);

        let platforms = matrix.supported_platforms();
        assert_eq!(platforms.len(), 2);
    }

    #[test]
    fn test_matrix_check_platform_support() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let pa = PlatformArch::new("linux", "x86_64");
        let entry = PlatformSupportEntry {
            platform_arch: pa.clone(),
            support_level: CompatibilityLevel::Recommended,
            min_version: None,
            max_version: None,
            notes: None,
        };
        matrix.add_platform_support(entry);

        let level = matrix.check_platform_support(&pa);
        assert_eq!(level, CompatibilityLevel::Recommended);
    }

    #[test]
    fn test_matrix_check_dependency_version() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let compat = DependencyCompatibility {
            dependency_name: "libfoo".to_string(),
            required_version: "1.0".to_string(),
            compatible_versions: vec!["1.0".to_string(), "1.1".to_string()],
            incompatible_versions: vec!["2.0".to_string()],
            notes: None,
        };
        matrix.add_dependency_compatibility(compat);

        let level = matrix.check_dependency_version("libfoo", "1.0");
        assert_eq!(level, CompatibilityLevel::Compatible);

        let level_incompat = matrix.check_dependency_version("libfoo", "2.0");
        assert_eq!(level_incompat, CompatibilityLevel::Incompatible);
    }

    #[test]
    fn test_matrix_generate_report() {
        let matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");
        let report = matrix.generate_report();
        assert_eq!(report.plugin_id, "test-plugin");
        assert_eq!(report.current_version, "1.0.0");
    }

    #[test]
    fn test_compatibility_analysis_serialization() {
        let analysis = CompatibilityAnalysis {
            version: "1.0".to_string(),
            compatible_versions: vec!["1.1".to_string()],
            incompatible_versions: vec!["2.0".to_string()],
            breaking_changes: vec![],
            supported_platforms: vec!["linux-x86_64".to_string()],
            deprecation_status: None,
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: CompatibilityAnalysis = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, analysis.version);
        assert_eq!(deserialized.compatible_versions.len(), 1);
    }

    #[test]
    fn test_platform_support_entry_serialization() {
        let pa = PlatformArch::new("linux", "x86_64");
        let entry = PlatformSupportEntry {
            platform_arch: pa,
            support_level: CompatibilityLevel::Recommended,
            min_version: Some("1.0".to_string()),
            max_version: Some("2.0".to_string()),
            notes: Some("Fully supported".to_string()),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: PlatformSupportEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.support_level, CompatibilityLevel::Recommended);
    }

    #[test]
    fn test_dependency_compatibility_serialization() {
        let compat = DependencyCompatibility {
            dependency_name: "libfoo".to_string(),
            required_version: "1.0".to_string(),
            compatible_versions: vec!["1.0".to_string(), "1.1".to_string()],
            incompatible_versions: vec!["2.0".to_string()],
            notes: Some("Requires libfoo >= 1.0".to_string()),
        };

        let json = serde_json::to_string(&compat).unwrap();
        let deserialized: DependencyCompatibility = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.dependency_name, "libfoo");
        assert_eq!(deserialized.compatible_versions.len(), 2);
    }

    #[test]
    fn test_matrix_find_breaking_changes() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");

        let change = BreakingChange {
            from_version: "1.0".to_string(),
            to_version: "2.0".to_string(),
            description: "API changed".to_string(),
            migration_guide: None,
            affected_apis: vec!["plugin_init".to_string()],
        };

        let mut entry = AbiCompatibilityEntry::new(AbiVersion::new(2, 0));
        entry.breaking_changes.push(change);
        matrix.add_abi_compatibility(entry);

        let changes = matrix.find_breaking_changes("1.0", "2.0");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].from_version, "1.0");
    }

    #[test]
    fn test_matrix_analyze_abi_version() {
        let mut matrix = PluginCompatibilityMatrix::new("test-plugin", "1.0.0");

        let mut entry = AbiCompatibilityEntry::new(AbiVersion::new(1, 0));
        entry
            .compatibility_with
            .insert("1.1".to_string(), CompatibilityLevel::Compatible);
        entry
            .compatibility_with
            .insert("2.0".to_string(), CompatibilityLevel::Incompatible);

        matrix.add_abi_compatibility(entry);

        let analysis = matrix.analyze_abi_version(&AbiVersion::new(1, 0));
        assert_eq!(analysis.compatible_versions.len(), 1);
        assert_eq!(analysis.incompatible_versions.len(), 1);
    }
}
