// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Plugin composition and meta-package support
///
/// This module provides support for composite plugins and plugin bundling:
/// - Composite plugin definitions
/// - Plugin bundling and aggregation
/// - Transitive dependency resolution
/// - Version conflict resolution
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A plugin component reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginComponent {
    pub name: String,
    pub version: String,
    pub required: bool,
    pub description: Option<String>,
}

impl PluginComponent {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            required: true,
            description: None,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }
}

/// Version conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    Newest,     // Use newest version
    Oldest,     // Use oldest version
    Exact,      // Require exact match (fail on conflict)
    Compatible, // Use compatible version range
}

impl ConflictResolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictResolution::Newest => "newest",
            ConflictResolution::Oldest => "oldest",
            ConflictResolution::Exact => "exact",
            ConflictResolution::Compatible => "compatible",
        }
    }
}

/// Version conflict information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConflict {
    pub plugin_name: String,
    pub versions: Vec<String>,
    pub requested_by: Vec<String>,
    pub resolved_version: String,
    pub resolution_strategy: ConflictResolution,
}

impl VersionConflict {
    pub fn new(
        plugin_name: &str,
        versions: Vec<String>,
        resolved_version: &str,
        strategy: ConflictResolution,
    ) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            versions,
            requested_by: Vec::new(),
            resolved_version: resolved_version.to_string(),
            resolution_strategy: strategy,
        }
    }
}

/// Composite plugin definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositePlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub components: Vec<PluginComponent>,
    pub transitive_dependencies: HashMap<String, String>,
    pub conflict_resolution: ConflictResolution,
}

impl CompositePlugin {
    pub fn new(name: &str, version: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            components: Vec::new(),
            transitive_dependencies: HashMap::new(),
            conflict_resolution: ConflictResolution::Newest,
        }
    }

    pub fn add_component(&mut self, component: PluginComponent) {
        self.components.push(component);
    }

    pub fn add_transitive_dependency(&mut self, name: &str, version: &str) {
        self.transitive_dependencies
            .insert(name.to_string(), version.to_string());
    }

    pub fn required_components(&self) -> Vec<&PluginComponent> {
        self.components.iter().filter(|c| c.required).collect()
    }

    pub fn optional_components(&self) -> Vec<&PluginComponent> {
        self.components.iter().filter(|c| !c.required).collect()
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn total_dependencies(&self) -> usize {
        self.component_count() + self.transitive_dependencies.len()
    }
}

/// Bundle specification for packaging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBundle {
    pub name: String,
    pub version: String,
    pub bundle_type: BundleType,
    pub plugins: Vec<String>,
    pub metadata: BundleMetadata,
}

/// Type of plugin bundle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleType {
    Standalone, // Single plugin
    Composite,  // Multiple plugins as one unit
    Collection, // Related plugins without strict deps
}

impl BundleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BundleType::Standalone => "standalone",
            BundleType::Composite => "composite",
            BundleType::Collection => "collection",
        }
    }
}

/// Bundle metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub author: String,
    pub license: String,
    pub created_at: String,
    pub compatible_versions: Vec<String>,
}

impl PluginBundle {
    pub fn new(name: &str, version: &str, bundle_type: BundleType) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            bundle_type,
            plugins: Vec::new(),
            metadata: BundleMetadata {
                author: "unknown".to_string(),
                license: "unknown".to_string(),
                created_at: chrono::Local::now()
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string(),
                compatible_versions: Vec::new(),
            },
        }
    }

    pub fn add_plugin(&mut self, name: String) {
        self.plugins.push(name);
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

/// Dependency resolution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyResolutionResult {
    pub requested: Vec<String>,
    pub resolved: HashMap<String, String>,
    pub conflicts: Vec<VersionConflict>,
    pub unresolvable: Vec<String>,
    pub success: bool,
}

impl DependencyResolutionResult {
    pub fn new() -> Self {
        Self {
            requested: Vec::new(),
            resolved: HashMap::new(),
            conflicts: Vec::new(),
            unresolvable: Vec::new(),
            success: true,
        }
    }

    pub fn add_conflict(&mut self, conflict: VersionConflict) {
        self.conflicts.push(conflict);
        self.success = false;
    }

    pub fn add_unresolvable(&mut self, name: String) {
        self.unresolvable.push(name);
        self.success = false;
    }

    pub fn resolved_count(&self) -> usize {
        self.resolved.len()
    }
}

impl Default for DependencyResolutionResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin composition manager
pub struct CompositionManager;

impl CompositionManager {
    /// Resolve transitive dependencies for a composite plugin
    pub fn resolve_transitive_dependencies(
        composite: &CompositePlugin,
        dependency_graph: &HashMap<String, Vec<String>>,
    ) -> DependencyResolutionResult {
        let mut result = DependencyResolutionResult::new();
        let mut visited = HashSet::new();
        let mut to_process = vec![composite.name.clone()];

        while let Some(current) = to_process.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Get components that match current plugin
            let matching_components: Vec<_> = composite
                .components
                .iter()
                .filter(|c| c.name == current)
                .collect();

            for component in matching_components {
                result
                    .resolved
                    .insert(component.name.clone(), component.version.clone());
                result.requested.push(component.name.clone());

                // Add to processing queue
                if let Some(deps) = dependency_graph.get(&component.name) {
                    for dep in deps {
                        if !visited.contains(dep) {
                            to_process.push(dep.clone());
                        }
                    }
                }
            }
        }

        result
    }

    /// Detect version conflicts in composite plugin
    pub fn detect_version_conflicts(composite: &CompositePlugin) -> Vec<VersionConflict> {
        let mut version_map: HashMap<String, Vec<String>> = HashMap::new();

        // Collect all versions for each plugin
        for component in &composite.components {
            version_map
                .entry(component.name.clone())
                .or_default()
                .push(component.version.clone());
        }

        let mut conflicts = Vec::new();

        // Detect conflicts
        for (plugin_name, versions) in version_map {
            if versions.len() > 1 {
                let resolved = versions[0].clone();
                conflicts.push(VersionConflict::new(
                    &plugin_name,
                    versions,
                    &resolved,
                    composite.conflict_resolution,
                ));
            }
        }

        conflicts
    }

    /// Validate composite plugin integrity
    pub fn validate_composite(composite: &CompositePlugin) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check minimum components
        if composite.components.is_empty() {
            result.add_error("Composite plugin has no components");
        }

        // Check for conflicts
        let conflicts = Self::detect_version_conflicts(composite);
        if !conflicts.is_empty() {
            result.add_warning(&format!("Found {} version conflicts", conflicts.len()));
        }

        // Check required components
        let required_count = composite.required_components().len();
        if required_count == 0 {
            result.add_warning("No required components in composite plugin");
        }

        result
    }

    /// Merge multiple composite plugins
    pub fn merge_composites(plugins: &[&CompositePlugin]) -> Result<CompositePlugin, String> {
        if plugins.is_empty() {
            return Err("No plugins to merge".to_string());
        }

        let merged_name = format!("merged-{}", chrono::Utc::now().timestamp_millis());
        let mut merged = CompositePlugin::new(&merged_name, "1.0.0", "Merged composite plugin");

        for plugin in plugins {
            for component in &plugin.components {
                merged.add_component(component.clone());
            }

            for (name, version) in &plugin.transitive_dependencies {
                merged.add_transitive_dependency(name.as_str(), version.as_str());
            }
        }

        Ok(merged)
    }

    /// Extract components from a composite plugin
    pub fn extract_components(composite: &CompositePlugin) -> Vec<PluginComponent> {
        composite.components.clone()
    }

    /// Calculate composite plugin size
    pub fn calculate_size(composite: &CompositePlugin) -> CompositeSize {
        CompositeSize {
            components: composite.components.len(),
            required: composite.required_components().len(),
            optional: composite.optional_components().len(),
            transitive_deps: composite.transitive_dependencies.len(),
            total: composite.total_dependencies(),
        }
    }
}

/// Composite plugin size metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeSize {
    pub components: usize,
    pub required: usize,
    pub optional: usize,
    pub transitive_deps: usize,
    pub total: usize,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: &str) {
        self.errors.push(error.to_string());
        self.valid = false;
    }

    pub fn add_warning(&mut self, warning: &str) {
        self.warnings.push(warning.to_string());
    }

    pub fn is_valid(&self) -> bool {
        self.valid && self.errors.is_empty()
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_component_creation() {
        let component = PluginComponent::new("plugin-a", "1.0.0");
        assert_eq!(component.name, "plugin-a");
        assert_eq!(component.version, "1.0.0");
        assert!(component.required);
    }

    #[test]
    fn test_plugin_component_optional() {
        let component = PluginComponent::new("plugin-a", "1.0.0").optional();
        assert!(!component.required);
    }

    #[test]
    fn test_plugin_component_with_description() {
        let component = PluginComponent::new("plugin-a", "1.0.0").with_description("A test plugin");
        assert!(component.description.is_some());
    }

    #[test]
    fn test_conflict_resolution_to_str() {
        assert_eq!(ConflictResolution::Newest.as_str(), "newest");
        assert_eq!(ConflictResolution::Oldest.as_str(), "oldest");
        assert_eq!(ConflictResolution::Exact.as_str(), "exact");
    }

    #[test]
    fn test_version_conflict_creation() {
        let conflict = VersionConflict::new(
            "plugin-a",
            vec!["1.0.0".to_string(), "2.0.0".to_string()],
            "2.0.0",
            ConflictResolution::Newest,
        );
        assert_eq!(conflict.plugin_name, "plugin-a");
        assert_eq!(conflict.versions.len(), 2);
    }

    #[test]
    fn test_composite_plugin_creation() {
        let composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        assert_eq!(composite.name, "composite");
        assert_eq!(composite.component_count(), 0);
    }

    #[test]
    fn test_composite_plugin_add_component() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        let component = PluginComponent::new("plugin-a", "1.0.0");
        composite.add_component(component);
        assert_eq!(composite.component_count(), 1);
    }

    #[test]
    fn test_composite_plugin_required_components() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_component(PluginComponent::new("plugin-a", "1.0.0"));
        composite.add_component(PluginComponent::new("plugin-b", "1.0.0").optional());
        let required = composite.required_components();
        assert_eq!(required.len(), 1);
    }

    #[test]
    fn test_composite_plugin_optional_components() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_component(PluginComponent::new("plugin-a", "1.0.0"));
        composite.add_component(PluginComponent::new("plugin-b", "1.0.0").optional());
        let optional = composite.optional_components();
        assert_eq!(optional.len(), 1);
    }

    #[test]
    fn test_composite_plugin_add_transitive_dependency() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_transitive_dependency("dep-a", "1.0.0");
        assert_eq!(composite.transitive_dependencies.len(), 1);
    }

    #[test]
    fn test_bundle_type_to_str() {
        assert_eq!(BundleType::Standalone.as_str(), "standalone");
        assert_eq!(BundleType::Composite.as_str(), "composite");
        assert_eq!(BundleType::Collection.as_str(), "collection");
    }

    #[test]
    fn test_plugin_bundle_creation() {
        let bundle = PluginBundle::new("bundle", "1.0.0", BundleType::Composite);
        assert_eq!(bundle.name, "bundle");
        assert_eq!(bundle.plugin_count(), 0);
    }

    #[test]
    fn test_plugin_bundle_add_plugin() {
        let mut bundle = PluginBundle::new("bundle", "1.0.0", BundleType::Composite);
        bundle.add_plugin("plugin-a".to_string());
        assert_eq!(bundle.plugin_count(), 1);
    }

    #[test]
    fn test_dependency_resolution_result_creation() {
        let result = DependencyResolutionResult::new();
        assert!(result.success);
        assert_eq!(result.resolved_count(), 0);
    }

    #[test]
    fn test_dependency_resolution_result_add_conflict() {
        let mut result = DependencyResolutionResult::new();
        let conflict = VersionConflict::new(
            "plugin-a",
            vec!["1.0.0".to_string()],
            "1.0.0",
            ConflictResolution::Newest,
        );
        result.add_conflict(conflict);
        assert!(!result.success);
    }

    #[test]
    fn test_dependency_resolution_result_add_unresolvable() {
        let mut result = DependencyResolutionResult::new();
        result.add_unresolvable("plugin-x".to_string());
        assert!(!result.success);
    }

    #[test]
    fn test_composition_manager_detect_conflicts() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_component(PluginComponent::new("plugin-a", "1.0.0"));
        composite.add_component(PluginComponent::new("plugin-a", "2.0.0"));
        let conflicts = CompositionManager::detect_version_conflicts(&composite);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_composition_manager_validate_composite() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_component(PluginComponent::new("plugin-a", "1.0.0"));
        let result = CompositionManager::validate_composite(&composite);
        assert!(result.is_valid());
    }

    #[test]
    fn test_composition_manager_validate_empty_composite() {
        let composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        let result = CompositionManager::validate_composite(&composite);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_composition_manager_calculate_size() {
        let mut composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        composite.add_component(PluginComponent::new("plugin-a", "1.0.0"));
        composite.add_component(PluginComponent::new("plugin-b", "1.0.0").optional());
        composite.add_transitive_dependency("dep-a", "1.0.0");
        let size = CompositionManager::calculate_size(&composite);
        assert_eq!(size.components, 2);
        assert_eq!(size.required, 1);
        assert_eq!(size.optional, 1);
    }

    #[test]
    fn test_composite_plugin_serialization() {
        let composite = CompositePlugin::new("composite", "1.0.0", "A composite plugin");
        let json = serde_json::to_string(&composite).unwrap();
        let deserialized: CompositePlugin = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, composite.name);
    }

    #[test]
    fn test_plugin_bundle_serialization() {
        let bundle = PluginBundle::new("bundle", "1.0.0", BundleType::Composite);
        let json = serde_json::to_string(&bundle).unwrap();
        let deserialized: PluginBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, bundle.name);
    }

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult::new();
        assert!(result.valid);
    }

    #[test]
    fn test_validation_result_add_error() {
        let mut result = ValidationResult::new();
        result.add_error("Test error");
        assert!(!result.valid);
    }

    #[test]
    fn test_validation_result_add_warning() {
        let mut result = ValidationResult::new();
        result.add_warning("Test warning");
        assert!(result.valid);
    }
}
