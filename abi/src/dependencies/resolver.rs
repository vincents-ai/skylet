// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dependency Resolution and Conflict Detection
//!
//! This module provides:
//! - Version conflict detection
//! - Resolution strategies
//! - Dependency resolution algorithm
//! - Integration with the dependency graph
//!
//! RFC-0005: Plugin Dependency Resolution

use crate::dependencies::constraints::VersionReq;
use crate::dependencies::graph::{DependencyGraph, GraphError, PluginId};
use crate::dependencies::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Result of dependency resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    /// Resolved plugin versions
    pub plugins: HashMap<PluginId, ResolvedPlugin>,
    /// The dependency graph used
    pub graph: DependencyGraph,
    /// Any warnings generated during resolution
    pub warnings: Vec<ResolutionWarning>,
}

/// A resolved plugin with its selected version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPlugin {
    /// Plugin identifier
    pub id: PluginId,
    /// Selected version
    pub version: Version,
    /// Which plugins required this dependency (and their constraints)
    pub required_by: Vec<RequirementSource>,
}

/// Source of a requirement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementSource {
    /// Plugin that had the requirement
    pub plugin: PluginId,
    /// Version requirement
    pub version_req: VersionReq,
}

/// Warning generated during resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionWarning {
    /// Multiple versions available, selected one
    VersionSelected {
        plugin: PluginId,
        available: Vec<Version>,
        selected: Version,
    },
    /// Optional dependency not resolved
    OptionalNotResolved {
        plugin: PluginId,
        dependency: PluginId,
    },
    /// Pre-release version used
    PreReleaseUsed { plugin: PluginId, version: Version },
}

impl fmt::Display for ResolutionWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolutionWarning::VersionSelected {
                plugin,
                available,
                selected,
            } => {
                write!(
                    f,
                    "Selected version {} for {} from available: {:?}",
                    selected, plugin, available
                )
            }
            ResolutionWarning::OptionalNotResolved { plugin, dependency } => {
                write!(
                    f,
                    "Optional dependency {} of {} was not resolved",
                    dependency, plugin
                )
            }
            ResolutionWarning::PreReleaseUsed { plugin, version } => {
                write!(
                    f,
                    "Pre-release version {} used for plugin {}",
                    version, plugin
                )
            }
        }
    }
}

/// Conflict between version requirements
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionConflict {
    /// The plugin with conflicting requirements
    pub plugin: PluginId,
    /// The conflicting requirements
    pub requirements: Vec<ConflictRequirement>,
}

/// A requirement in a conflict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRequirement {
    /// Plugin that made the requirement
    pub required_by: PluginId,
    /// The version requirement
    pub version_req: VersionReq,
}

impl VersionConflict {
    /// Check if a version satisfies all requirements
    pub fn is_satisfied_by(&self, version: &Version) -> bool {
        self.requirements
            .iter()
            .all(|r| r.version_req.matches(version))
    }

    /// Find a version that satisfies all requirements (if any)
    pub fn find_satisfying_version(&self, candidates: &[Version]) -> Option<Version> {
        candidates.iter().find(|v| self.is_satisfied_by(v)).cloned()
    }
}

impl fmt::Display for VersionConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Version conflict for '{}': ", self.plugin)?;
        let reqs: Vec<String> = self
            .requirements
            .iter()
            .map(|r| format!("{} requires {}", r.required_by, r.version_req))
            .collect();
        write!(f, "{}", reqs.join(", "))
    }
}

/// Errors that can occur during resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    /// Cycle in dependencies
    CyclicDependency { cycles: Vec<String> },
    /// Missing dependency
    MissingDependency {
        plugin: PluginId,
        dependency: PluginId,
    },
    /// Unresolvable version conflict
    UnresolvableConflict { conflict: VersionConflict },
    /// No version satisfies requirements
    NoSatisfyingVersion {
        plugin: PluginId,
        requirements: Vec<ConflictRequirement>,
        available: Vec<Version>,
    },
    /// Plugin not found
    PluginNotFound { plugin: PluginId },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolutionError::CyclicDependency { cycles } => {
                write!(f, "Cyclic dependency: {}", cycles.join(" -> "))
            }
            ResolutionError::MissingDependency { plugin, dependency } => {
                write!(
                    f,
                    "Plugin '{}' requires missing dependency '{}'",
                    plugin, dependency
                )
            }
            ResolutionError::UnresolvableConflict { conflict } => {
                write!(f, "Unresolvable conflict: {}", conflict)
            }
            ResolutionError::NoSatisfyingVersion {
                plugin,
                requirements,
                available,
            } => {
                let reqs: Vec<String> = requirements
                    .iter()
                    .map(|r| r.version_req.to_string())
                    .collect();
                write!(
                    f,
                    "No version of '{}' satisfies {} (available: {:?})",
                    plugin,
                    reqs.join(", "),
                    available
                )
            }
            ResolutionError::PluginNotFound { plugin } => {
                write!(f, "Plugin '{}' not found", plugin)
            }
        }
    }
}

impl std::error::Error for ResolutionError {}

/// The dependency resolver
#[derive(Debug, Clone)]
pub struct DependencyResolver {
    /// Available plugins and their versions
    available: HashMap<PluginId, Vec<Version>>,
    /// Resolution strategy
    strategy: ResolutionStrategy,
    /// Whether to include pre-release versions
    include_prerelease: bool,
}

/// Strategy for version selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolutionStrategy {
    /// Select the highest compatible version (default)
    #[default]
    Newest,
    /// Select the lowest compatible version
    Oldest,
    /// Select the version closest to a target
    CloseToTarget,
}

impl DependencyResolver {
    /// Create a new resolver
    pub fn new() -> Self {
        Self {
            available: HashMap::new(),
            strategy: ResolutionStrategy::Newest,
            include_prerelease: false,
        }
    }

    /// Add available versions for a plugin
    pub fn add_plugin_versions(
        mut self,
        plugin: impl Into<String>,
        versions: Vec<Version>,
    ) -> Self {
        self.available.insert(plugin.into(), versions);
        self
    }

    /// Set the resolution strategy
    pub fn with_strategy(mut self, strategy: ResolutionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Include pre-release versions
    pub fn include_prerelease(mut self, include: bool) -> Self {
        self.include_prerelease = include;
        self
    }

    /// Resolve dependencies for a set of plugins
    pub fn resolve(&self, graph: &DependencyGraph) -> Result<Resolution, ResolutionError> {
        // Check for cycles first
        let cycles = graph.detect_cycles();
        if !cycles.is_empty() {
            return Err(ResolutionError::CyclicDependency {
                cycles: cycles.iter().map(|c| c.join(" -> ")).collect(),
            });
        }

        let mut resolution = Resolution {
            plugins: HashMap::new(),
            graph: graph.clone(),
            warnings: Vec::new(),
        };

        // Get topological order for resolution
        let order = graph.topological_sort().map_err(|e| match e {
            GraphError::CyclicDependency { cycles } => ResolutionError::CyclicDependency { cycles },
            _ => ResolutionError::PluginNotFound {
                plugin: "unknown".to_string(),
            },
        })?;

        // Collect all version requirements for each plugin
        let requirements = self.collect_requirements(graph);

        // Resolve each plugin in topological order
        for plugin_id in &order {
            self.resolve_plugin(plugin_id, &requirements, &mut resolution)?;
        }

        Ok(resolution)
    }

    fn collect_requirements(
        &self,
        graph: &DependencyGraph,
    ) -> HashMap<PluginId, Vec<ConflictRequirement>> {
        let mut requirements: HashMap<PluginId, Vec<ConflictRequirement>> = HashMap::new();

        for edge in graph.edges() {
            requirements
                .entry(edge.to.clone())
                .or_default()
                .push(ConflictRequirement {
                    required_by: edge.from.clone(),
                    version_req: edge.dependency.version_req.clone(),
                });
        }

        requirements
    }

    fn resolve_plugin(
        &self,
        plugin_id: &str,
        requirements: &HashMap<PluginId, Vec<ConflictRequirement>>,
        resolution: &mut Resolution,
    ) -> Result<(), ResolutionError> {
        // Get plugin node
        let node = resolution
            .graph
            .get_plugin(plugin_id)
            .ok_or_else(|| ResolutionError::PluginNotFound {
                plugin: plugin_id.to_string(),
            })?
            .clone();

        // Get available versions
        let available = self.available.get(plugin_id).cloned().unwrap_or_else(|| {
            // If no versions registered, use the plugin's own version
            vec![node.version.clone()]
        });

        // Filter by pre-release
        let candidates: Vec<Version> = if self.include_prerelease {
            available
        } else {
            available.into_iter().filter(|v| v.is_stable()).collect()
        };

        // Get requirements for this plugin
        let plugin_reqs = requirements.get(plugin_id).cloned().unwrap_or_default();

        // Find a version that satisfies all requirements
        let selected = self.select_version(&candidates, &plugin_reqs)?;

        // Record the resolution
        let required_by: Vec<RequirementSource> = plugin_reqs
            .iter()
            .map(|r| RequirementSource {
                plugin: r.required_by.clone(),
                version_req: r.version_req.clone(),
            })
            .collect();

        // Add warning for pre-release usage
        if selected.is_prerelease() {
            resolution.warnings.push(ResolutionWarning::PreReleaseUsed {
                plugin: plugin_id.to_string(),
                version: selected.clone(),
            });
        }

        resolution.plugins.insert(
            plugin_id.to_string(),
            ResolvedPlugin {
                id: plugin_id.to_string(),
                version: selected,
                required_by,
            },
        );

        Ok(())
    }

    fn select_version(
        &self,
        candidates: &[Version],
        requirements: &[ConflictRequirement],
    ) -> Result<Version, ResolutionError> {
        if candidates.is_empty() {
            return Err(ResolutionError::NoSatisfyingVersion {
                plugin: "unknown".to_string(),
                requirements: requirements.to_vec(),
                available: vec![],
            });
        }

        // Filter candidates by all requirements
        let satisfying: Vec<&Version> = candidates
            .iter()
            .filter(|v| requirements.iter().all(|r| r.version_req.matches(v)))
            .collect();

        if satisfying.is_empty() {
            return Err(ResolutionError::NoSatisfyingVersion {
                plugin: "unknown".to_string(),
                requirements: requirements.to_vec(),
                available: candidates.to_vec(),
            });
        }

        // Select based on strategy
        let selected = match self.strategy {
            ResolutionStrategy::Newest => satisfying.iter().max().unwrap(),
            ResolutionStrategy::Oldest => satisfying.iter().min().unwrap(),
            ResolutionStrategy::CloseToTarget => satisfying.iter().max().unwrap(), // Default to newest
        };

        Ok((*selected).clone())
    }

    /// Detect conflicts in the dependency graph
    pub fn detect_conflicts(&self, graph: &DependencyGraph) -> Vec<VersionConflict> {
        let mut conflicts = Vec::new();
        let requirements = self.collect_requirements(graph);

        for (plugin, reqs) in requirements {
            // Get available versions
            let available = self.available.get(&plugin).cloned().unwrap_or_default();

            // Check if any version satisfies all requirements
            let has_satisfying = available
                .iter()
                .any(|v| reqs.iter().all(|r| r.version_req.matches(v)));

            if !has_satisfying && !reqs.is_empty() {
                conflicts.push(VersionConflict {
                    plugin,
                    requirements: reqs,
                });
            }
        }

        conflicts
    }

    /// Check if all dependencies can be satisfied
    pub fn can_resolve(&self, graph: &DependencyGraph) -> bool {
        // Check for cycles
        if graph.has_cycles() {
            return false;
        }

        // Check for conflicts
        self.detect_conflicts(graph).is_empty()
    }
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependencies::constraints::VersionReq;
    use crate::dependencies::graph::Dependency;
    use crate::PluginNode;

    fn make_dep(name: &str, version: &str) -> Dependency {
        Dependency::new(name, VersionReq::parse(version).unwrap())
    }

    #[test]
    fn test_resolver_simple() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));

        let resolver = DependencyResolver::new();
        let resolution = resolver.resolve(&graph).unwrap();

        assert!(resolution.plugins.contains_key("a"));
        assert_eq!(
            resolution.plugins.get("a").unwrap().version,
            Version::new(1, 0, 0)
        );
    }

    #[test]
    fn test_resolver_with_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("b", Version::new(1, 5, 0)));
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let resolver = DependencyResolver::new().add_plugin_versions(
            "b",
            vec![
                Version::new(1, 0, 0),
                Version::new(1, 5, 0),
                Version::new(2, 0, 0),
            ],
        );

        let resolution = resolver.resolve(&graph).unwrap();

        // Should select 1.5.0 (highest in ^1.x)
        let b_version = &resolution.plugins.get("b").unwrap().version;
        assert_eq!(*b_version, Version::new(1, 5, 0));
    }

    #[test]
    fn test_resolver_conflict_detection() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^2.0.0")),
        );

        let resolver = DependencyResolver::new()
            .add_plugin_versions("c", vec![Version::new(1, 0, 0), Version::new(1, 5, 0)]);

        let conflicts = resolver.detect_conflicts(&graph);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].plugin, "c");
    }

    #[test]
    fn test_resolver_no_satisfying_version() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("b", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^2.0.0")),
        );

        let resolver =
            DependencyResolver::new().add_plugin_versions("b", vec![Version::new(1, 0, 0)]);

        let result = resolver.resolve(&graph);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolver_strategy_newest() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));

        let resolver = DependencyResolver::new()
            .with_strategy(ResolutionStrategy::Newest)
            .add_plugin_versions(
                "a",
                vec![
                    Version::new(1, 0, 0),
                    Version::new(1, 5, 0),
                    Version::new(1, 2, 0),
                ],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("a").unwrap().version,
            Version::new(1, 5, 0)
        );
    }

    #[test]
    fn test_resolver_strategy_oldest() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));

        let resolver = DependencyResolver::new()
            .with_strategy(ResolutionStrategy::Oldest)
            .add_plugin_versions(
                "a",
                vec![
                    Version::new(1, 0, 0),
                    Version::new(1, 5, 0),
                    Version::new(1, 2, 0),
                ],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("a").unwrap().version,
            Version::new(1, 0, 0)
        );
    }

    #[test]
    fn test_resolver_prerelease_excluded() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));

        let resolver = DependencyResolver::new()
            .include_prerelease(false)
            .add_plugin_versions(
                "a",
                vec![Version::new(1, 0, 0), Version::parse("2.0.0-beta").unwrap()],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("a").unwrap().version,
            Version::new(1, 0, 0)
        );
    }

    #[test]
    fn test_resolver_prerelease_included() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));

        let resolver = DependencyResolver::new()
            .include_prerelease(true)
            .add_plugin_versions(
                "a",
                vec![Version::new(1, 0, 0), Version::parse("2.0.0-beta").unwrap()],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("a").unwrap().version,
            Version::parse("2.0.0-beta").unwrap()
        );

        // Should have pre-release warning
        assert!(resolution
            .warnings
            .iter()
            .any(|w| matches!(w, ResolutionWarning::PreReleaseUsed { .. })));
    }

    #[test]
    fn test_resolution_order() {
        // Test that resolution follows topological order
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let resolver = DependencyResolver::new();
        let resolution = resolver.resolve(&graph).unwrap();

        assert_eq!(resolution.plugins.len(), 3);
        assert!(resolution.plugins.contains_key("a"));
        assert!(resolution.plugins.contains_key("b"));
        assert!(resolution.plugins.contains_key("c"));
    }

    #[test]
    fn test_version_conflict_display() {
        let conflict = VersionConflict {
            plugin: "test-plugin".to_string(),
            requirements: vec![
                ConflictRequirement {
                    required_by: "plugin-a".to_string(),
                    version_req: VersionReq::parse("^1.0.0").unwrap(),
                },
                ConflictRequirement {
                    required_by: "plugin-b".to_string(),
                    version_req: VersionReq::parse("^2.0.0").unwrap(),
                },
            ],
        };

        let display = conflict.to_string();
        assert!(display.contains("test-plugin"));
        assert!(display.contains("plugin-a"));
        assert!(display.contains("plugin-b"));
    }

    #[test]
    fn test_can_resolve() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new("b", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let resolver =
            DependencyResolver::new().add_plugin_versions("b", vec![Version::new(1, 0, 0)]);

        assert!(resolver.can_resolve(&graph));
    }

    #[test]
    fn test_cannot_resolve_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("a", "^1.0.0")),
        );

        let resolver = DependencyResolver::new();
        assert!(!resolver.can_resolve(&graph));
    }
}
