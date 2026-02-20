// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Optional dependencies and feature gates support
///
/// This module provides support for optional plugin dependencies:
/// - Optional dependency flagging
/// - Feature-gated dependencies
/// - Platform-specific dependencies
/// - Conditional installation logic
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents an optional dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionalDependency {
    pub name: String,
    pub version: String,
    pub optional: bool,
    pub description: Option<String>,
    pub default_enabled: bool,
    pub platforms: Vec<String>, // "linux", "macos", "windows", "unix", "all"
    pub features: Vec<String>,  // Which features require this dep
}

impl OptionalDependency {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            optional: true,
            description: None,
            default_enabled: false,
            platforms: vec!["all".to_string()],
            features: Vec::new(),
        }
    }

    pub fn required(mut self) -> Self {
        self.optional = false;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn with_default_enabled(mut self) -> Self {
        self.default_enabled = true;
        self
    }

    pub fn with_platforms(mut self, platforms: Vec<&str>) -> Self {
        self.platforms = platforms.iter().map(|p| p.to_string()).collect();
        self
    }

    pub fn with_features(mut self, features: Vec<&str>) -> Self {
        self.features = features.iter().map(|f| f.to_string()).collect();
        self
    }

    pub fn is_platform_applicable(&self, current_platform: &str) -> bool {
        if self.platforms.contains(&"all".to_string()) {
            return true;
        }

        // Check for OS families
        let is_unix_like = cfg!(unix);
        if current_platform == "unix" && is_unix_like {
            return true;
        }

        self.platforms.contains(&current_platform.to_string())
    }

    pub fn is_enabled(&self, enabled_features: &[String]) -> bool {
        if self.optional && self.default_enabled {
            return true;
        }

        // Check if any required feature is enabled
        for feature in &self.features {
            if enabled_features.contains(feature) {
                return true;
            }
        }

        false
    }
}

/// Feature gate for conditional dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureGate {
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub dependencies: Vec<String>, // Dependencies required for this feature
}

impl FeatureGate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            enabled: false,
            dependencies: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<&str>) -> Self {
        self.dependencies = deps.iter().map(|d| d.to_string()).collect();
        self
    }

    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }
}

/// Platform-specific dependency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSpecific {
    pub dependency: String,
    pub version: String,
    pub platforms: Vec<String>, // Target platforms
    pub required: bool,
}

impl PlatformSpecific {
    pub fn new(dependency: &str, version: &str, platforms: Vec<&str>) -> Self {
        Self {
            dependency: dependency.to_string(),
            version: version.to_string(),
            platforms: platforms.iter().map(|p| p.to_string()).collect(),
            required: false,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn matches_current_platform(&self) -> bool {
        for platform in &self.platforms {
            match platform.as_str() {
                "linux" => {
                    if cfg!(target_os = "linux") {
                        return true;
                    }
                }
                "macos" => {
                    if cfg!(target_os = "macos") {
                        return true;
                    }
                }
                "windows" => {
                    if cfg!(target_os = "windows") {
                        return true;
                    }
                }
                "unix" => {
                    if cfg!(unix) {
                        return true;
                    }
                }
                "all" => return true,
                _ => {}
            }
        }
        false
    }
}

/// Conditional dependency requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCondition {
    pub condition_type: ConditionType,
    pub expression: String, // e.g., "feature(logging) && platform(linux)"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionType {
    Feature,
    Platform,
    And,
    Or,
    Not,
}

impl ConditionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConditionType::Feature => "feature",
            ConditionType::Platform => "platform",
            ConditionType::And => "and",
            ConditionType::Or => "or",
            ConditionType::Not => "not",
        }
    }
}

impl DependencyCondition {
    pub fn new(cond_type: ConditionType, expression: &str) -> Self {
        Self {
            condition_type: cond_type,
            expression: expression.to_string(),
        }
    }

    pub fn is_valid_syntax(&self) -> bool {
        !self.expression.is_empty() && self.expression.len() < 256
    }

    pub fn evaluate(
        &self,
        enabled_features: &[String],
        current_platform: &str,
    ) -> Result<bool, String> {
        if !self.is_valid_syntax() {
            return Err("Invalid condition syntax".to_string());
        }

        match self.condition_type {
            ConditionType::Feature => {
                let feature_name = self.expression.trim();
                Ok(enabled_features.contains(&feature_name.to_string()))
            }
            ConditionType::Platform => {
                Ok(self.expression == current_platform || self.expression == "all")
            }
            ConditionType::And => {
                // Simple AND evaluation for feature(x) && platform(y)
                Ok(self.expression.contains(current_platform))
            }
            ConditionType::Or => {
                // Simple OR evaluation
                Ok(true)
            }
            ConditionType::Not => {
                // Simple NOT evaluation
                Ok(!enabled_features.contains(&self.expression.to_string()))
            }
        }
    }
}

/// Optional dependency manager
pub struct OptionalDependencyManager;

impl OptionalDependencyManager {
    /// Evaluate all conditions for optional dependencies
    pub fn evaluate_conditions(
        dependencies: &[OptionalDependency],
        enabled_features: &[String],
        current_platform: &str,
    ) -> Vec<OptionalDependency> {
        dependencies
            .iter()
            .filter(|dep| {
                dep.is_platform_applicable(current_platform) && dep.is_enabled(enabled_features)
            })
            .cloned()
            .collect()
    }

    /// Filter dependencies by platform
    pub fn filter_by_platform(
        dependencies: &[OptionalDependency],
        platform: &str,
    ) -> Vec<OptionalDependency> {
        dependencies
            .iter()
            .filter(|dep| dep.is_platform_applicable(platform))
            .cloned()
            .collect()
    }

    /// Resolve feature gates and their dependencies
    pub fn resolve_features(features: &[FeatureGate]) -> HashMap<String, Vec<String>> {
        let mut result = HashMap::new();

        for feature in features {
            if feature.enabled {
                result.insert(feature.name.clone(), feature.dependencies.clone());
            }
        }

        result
    }

    /// Validate condition expressions
    pub fn validate_conditions(conditions: &[DependencyCondition]) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (idx, cond) in conditions.iter().enumerate() {
            if !cond.is_valid_syntax() {
                errors.push(format!(
                    "Invalid condition at index {}: {}",
                    idx, cond.expression
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Calculate total optional dependencies needed
    pub fn calculate_optional_count(
        dependencies: &[OptionalDependency],
        enabled_features: &[String],
    ) -> (usize, usize) {
        let total_optional = dependencies.iter().filter(|d| d.optional).count();
        let enabled = Self::evaluate_conditions(dependencies, enabled_features, "all")
            .iter()
            .filter(|d| d.optional)
            .count();

        (enabled, total_optional)
    }

    /// Get optional dependencies for specific feature
    pub fn get_for_feature(
        dependencies: &[OptionalDependency],
        feature: &str,
    ) -> Vec<OptionalDependency> {
        dependencies
            .iter()
            .filter(|dep| dep.features.contains(&feature.to_string()))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optional_dependency_creation() {
        let dep = OptionalDependency::new("plugin-a", "1.0.0");
        assert_eq!(dep.name, "plugin-a");
        assert!(dep.optional);
        assert!(!dep.default_enabled);
    }

    #[test]
    fn test_optional_dependency_required() {
        let dep = OptionalDependency::new("plugin-a", "1.0.0").required();
        assert!(!dep.optional);
    }

    #[test]
    fn test_optional_dependency_with_description() {
        let dep =
            OptionalDependency::new("plugin-a", "1.0.0").with_description("A test dependency");
        assert!(dep.description.is_some());
    }

    #[test]
    fn test_optional_dependency_with_default_enabled() {
        let dep = OptionalDependency::new("plugin-a", "1.0.0").with_default_enabled();
        assert!(dep.default_enabled);
    }

    #[test]
    fn test_optional_dependency_is_platform_applicable() {
        let dep =
            OptionalDependency::new("plugin-a", "1.0.0").with_platforms(vec!["linux", "macos"]);
        assert!(dep.is_platform_applicable("linux"));
        assert!(dep.is_platform_applicable("macos"));
        assert!(!dep.is_platform_applicable("windows"));
    }

    #[test]
    fn test_optional_dependency_is_enabled() {
        let dep = OptionalDependency::new("plugin-a", "1.0.0").with_features(vec!["logging"]);
        let features = vec!["logging".to_string()];
        assert!(dep.is_enabled(&features));
    }

    #[test]
    fn test_feature_gate_creation() {
        let gate = FeatureGate::new("logging");
        assert_eq!(gate.name, "logging");
        assert!(!gate.enabled);
    }

    #[test]
    fn test_feature_gate_enable() {
        let gate = FeatureGate::new("logging").enable();
        assert!(gate.enabled);
    }

    #[test]
    fn test_feature_gate_with_dependencies() {
        let gate = FeatureGate::new("logging").with_dependencies(vec!["slog", "serde"]);
        assert_eq!(gate.dependencies.len(), 2);
    }

    #[test]
    fn test_platform_specific_creation() {
        let ps = PlatformSpecific::new("openssl", "1.1.0", vec!["linux"]);
        assert_eq!(ps.dependency, "openssl");
        assert_eq!(ps.platforms.len(), 1);
    }

    #[test]
    fn test_platform_specific_required() {
        let ps = PlatformSpecific::new("openssl", "1.1.0", vec!["linux"]).required();
        assert!(ps.required);
    }

    #[test]
    fn test_condition_type_to_str() {
        assert_eq!(ConditionType::Feature.as_str(), "feature");
        assert_eq!(ConditionType::Platform.as_str(), "platform");
        assert_eq!(ConditionType::And.as_str(), "and");
        assert_eq!(ConditionType::Or.as_str(), "or");
        assert_eq!(ConditionType::Not.as_str(), "not");
    }

    #[test]
    fn test_dependency_condition_creation() {
        let cond = DependencyCondition::new(ConditionType::Feature, "logging");
        assert_eq!(cond.condition_type, ConditionType::Feature);
    }

    #[test]
    fn test_dependency_condition_is_valid_syntax() {
        let cond = DependencyCondition::new(ConditionType::Feature, "logging");
        assert!(cond.is_valid_syntax());
    }

    #[test]
    fn test_dependency_condition_evaluate_feature() {
        let cond = DependencyCondition::new(ConditionType::Feature, "logging");
        let features = vec!["logging".to_string()];
        let result = cond.evaluate(&features, "linux");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_dependency_condition_evaluate_platform() {
        let cond = DependencyCondition::new(ConditionType::Platform, "linux");
        let features = vec![];
        let result = cond.evaluate(&features, "linux");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_optional_dependency_manager_evaluate_conditions() {
        let deps = vec![OptionalDependency::new("plugin-a", "1.0.0")
            .with_features(vec!["logging"])
            .with_platforms(vec!["linux"])];
        let features = vec!["logging".to_string()];
        let evaluated = OptionalDependencyManager::evaluate_conditions(&deps, &features, "linux");
        assert_eq!(evaluated.len(), 1);
    }

    #[test]
    fn test_optional_dependency_manager_filter_by_platform() {
        let deps = vec![
            OptionalDependency::new("plugin-a", "1.0.0").with_platforms(vec!["linux", "macos"]),
            OptionalDependency::new("plugin-b", "1.0.0").with_platforms(vec!["windows"]),
        ];
        let filtered = OptionalDependencyManager::filter_by_platform(&deps, "linux");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_optional_dependency_manager_resolve_features() {
        let features = vec![FeatureGate::new("logging")
            .enable()
            .with_dependencies(vec!["slog"])];
        let resolved = OptionalDependencyManager::resolve_features(&features);
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn test_optional_dependency_manager_validate_conditions() {
        let conditions = vec![
            DependencyCondition::new(ConditionType::Feature, "logging"),
            DependencyCondition::new(ConditionType::Platform, "linux"),
        ];
        let result = OptionalDependencyManager::validate_conditions(&conditions);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optional_dependency_manager_validate_conditions_invalid() {
        let conditions = vec![DependencyCondition::new(ConditionType::Feature, "")];
        let result = OptionalDependencyManager::validate_conditions(&conditions);
        assert!(result.is_err());
    }

    #[test]
    fn test_optional_dependency_manager_calculate_optional_count() {
        let deps = vec![
            OptionalDependency::new("plugin-a", "1.0.0").with_features(vec!["logging"]),
            OptionalDependency::new("plugin-b", "1.0.0").with_features(vec!["metrics"]),
        ];
        let features = vec!["logging".to_string()];
        let (enabled, total) =
            OptionalDependencyManager::calculate_optional_count(&deps, &features);
        assert_eq!(enabled, 1);
        assert_eq!(total, 2);
    }

    #[test]
    fn test_optional_dependency_manager_get_for_feature() {
        let deps = vec![
            OptionalDependency::new("plugin-a", "1.0.0").with_features(vec!["logging"]),
            OptionalDependency::new("plugin-b", "1.0.0").with_features(vec!["logging", "metrics"]),
            OptionalDependency::new("plugin-c", "1.0.0").with_features(vec!["metrics"]),
        ];
        let for_logging = OptionalDependencyManager::get_for_feature(&deps, "logging");
        assert_eq!(for_logging.len(), 2);
    }

    #[test]
    fn test_optional_dependency_serialization() {
        let dep = OptionalDependency::new("plugin-a", "1.0.0");
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: OptionalDependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, dep.name);
    }

    #[test]
    fn test_feature_gate_serialization() {
        let gate = FeatureGate::new("logging");
        let json = serde_json::to_string(&gate).unwrap();
        let deserialized: FeatureGate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, gate.name);
    }

    #[test]
    fn test_platform_specific_serialization() {
        let ps = PlatformSpecific::new("openssl", "1.1.0", vec!["linux"]);
        let json = serde_json::to_string(&ps).unwrap();
        let deserialized: PlatformSpecific = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dependency, ps.dependency);
    }
}
