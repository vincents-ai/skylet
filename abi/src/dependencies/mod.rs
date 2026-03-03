// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Dependency Resolution System
//!
//! This module implements RFC-0005: Plugin Dependency Resolution
//!
//! ## Features
//!
//! - **Semantic Versioning**: Full semver 2.0 support with pre-release and build metadata
//! - **Version Constraints**: Cargo-style constraint parsing (^, ~, *, >, <, =, >=, <=)
//! - **Dependency Graph**: Graph construction with cycle detection
//! - **Topological Sort**: Correct plugin activation/deactivation order
//! - **Conflict Detection**: Identify and report version conflicts
//! - **Resolution**: Automatic version selection with multiple strategies
//!
//! ## Quick Start
//!
//! ```ignore
//! use skylet_abi::dependencies::*;
//!
//! // Parse a version
//! let version = Version::parse("1.2.3-alpha.1+build.123").unwrap();
//!
//! // Parse a version constraint
//! let req = VersionReq::parse("^1.2.0").unwrap();
//! assert!(req.matches(&Version::new(1, 5, 0)));
//!
//! // Build a dependency graph
//! let mut graph = DependencyGraph::new();
//! graph.add_plugin(PluginNode::new("plugin-a", Version::new(1, 0, 0)));
//! graph.add_plugin(
//!     PluginNode::new("plugin-b", Version::new(1, 0, 0))
//!         .with_dependency(Dependency::new("plugin-a", VersionReq::parse("^1.0.0").unwrap()))
//! );
//!
//! // Get activation order
//! let order = graph.activation_order().unwrap();
//!
//! // Resolve dependencies
//! let resolver = DependencyResolver::new()
//!     .add_plugin_versions("plugin-a", vec![Version::new(1, 0, 0), Version::new(1, 1, 0)]);
//! let resolution = resolver.resolve(&graph).unwrap();
//! ```
//!
//! ## Version Constraints
//!
//! | Constraint | Description | Example |
//! |------------|-------------|---------|
//! | `^1.2.3` | Caret - compatible version (default) | `^1.2.3` matches `1.5.0` but not `2.0.0` |
//! | `~1.2.3` | Tilde - approximately equivalent | `~1.2.3` matches `1.2.9` but not `1.3.0` |
//! | `=1.2.3` | Exact match | `=1.2.3` matches only `1.2.3` |
//! | `>=1.2.3` | Greater than or equal | `>=1.2.3` matches `1.2.3`, `1.3.0`, `2.0.0` |
//! | `>1.2.3` | Greater than | `>1.2.3` matches `1.2.4` but not `1.2.3` |
//! | `<=1.2.3` | Less than or equal | `<=1.2.3` matches `1.2.3`, `1.2.0`, `1.0.0` |
//! | `<1.2.3` | Less than | `<1.2.3` matches `1.2.2` but not `1.2.3` |
//! | `*` | Any version | Matches any version |
//!
//! ## Dependency Graph Operations
//!
//! - **Cycle Detection**: Identify circular dependencies
//! - **Topological Sort**: Order plugins by dependencies
//! - **Transitive Dependencies**: Find all dependencies of a plugin
//! - **Transitive Dependents**: Find all plugins that depend on a plugin
//!
//! ## Resolution Strategies
//!
//! - **Newest** (default): Select the highest compatible version
//! - **Oldest**: Select the lowest compatible version
//! - **CloseToTarget**: Select version closest to a target (future)

pub mod constraints;
pub mod graph;
pub mod resolver;
pub mod version;

// Re-export main types for convenience
pub use constraints::{Comparator, ConstraintError, VersionConstraint, VersionReq};
pub use graph::{Dependency, DependencyEdge, DependencyGraph, GraphError, PluginId, PluginNode};
pub use resolver::{
    ConflictRequirement, DependencyResolver, RequirementSource, Resolution, ResolutionError,
    ResolutionStrategy, ResolutionWarning, ResolvedPlugin, VersionConflict,
};
pub use version::{Prerelease, PrereleaseIdentifier, Version, VersionError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_simple() {
        // Create a simple dependency graph
        let mut graph = DependencyGraph::new();

        // Add plugins
        graph.add_plugin(PluginNode::new("core", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("database", Version::new(1, 0, 0)).with_dependency(Dependency::new(
                "core",
                VersionReq::parse("^1.0.0").unwrap(),
            )),
        );
        graph.add_plugin(
            PluginNode::new("api", Version::new(1, 0, 0)).with_dependencies(vec![
                Dependency::new("core", VersionReq::parse("^1.0.0").unwrap()),
                Dependency::new("database", VersionReq::parse("^1.0.0").unwrap()),
            ]),
        );

        // Verify no cycles
        assert!(!graph.has_cycles());

        // Get activation order (dependencies activated first, then dependents)
        let activation = graph.activation_order().unwrap();
        assert_eq!(activation[0], "core");
        assert_eq!(activation[1], "database");
        assert_eq!(activation[2], "api");

        // Get deactivation order (dependents deactivated first, then dependencies)
        let deactivation = graph.deactivation_order().unwrap();
        assert_eq!(deactivation[0], "api");
        assert_eq!(deactivation[1], "database");
        assert_eq!(deactivation[2], "core");
    }

    #[test]
    fn test_integration_resolution() {
        let mut graph = DependencyGraph::new();

        graph.add_plugin(PluginNode::new("lib-a", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("app", Version::new(1, 0, 0)).with_dependency(Dependency::new(
                "lib-a",
                VersionReq::parse(">=1.0.0, <2.0.0").unwrap(),
            )),
        );

        let resolver = DependencyResolver::new().add_plugin_versions(
            "lib-a",
            vec![
                Version::new(1, 0, 0),
                Version::new(1, 2, 0),
                Version::new(1, 5, 0),
                Version::new(2, 0, 0),
            ],
        );

        let resolution = resolver.resolve(&graph).unwrap();

        // Should select highest version in range
        let lib_a = resolution.plugins.get("lib-a").unwrap();
        assert_eq!(lib_a.version, Version::new(1, 5, 0));
    }

    #[test]
    fn test_integration_conflict() {
        let mut graph = DependencyGraph::new();

        graph.add_plugin(PluginNode::new("shared", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("plugin-a", Version::new(1, 0, 0)).with_dependency(Dependency::new(
                "shared",
                VersionReq::parse("^1.0.0").unwrap(),
            )),
        );
        graph.add_plugin(
            PluginNode::new("plugin-b", Version::new(1, 0, 0)).with_dependency(Dependency::new(
                "shared",
                VersionReq::parse("^2.0.0").unwrap(),
            )),
        );

        let resolver =
            DependencyResolver::new().add_plugin_versions("shared", vec![Version::new(1, 5, 0)]);

        // Should detect conflict
        let conflicts = resolver.detect_conflicts(&graph);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].plugin, "shared");
    }

    #[test]
    fn test_integration_prerelease() {
        let mut graph = DependencyGraph::new();
        graph.add_plugin(PluginNode::new(
            "beta-plugin",
            Version::parse("2.0.0-beta.1").unwrap(),
        ));

        // Without pre-release enabled
        let resolver = DependencyResolver::new()
            .include_prerelease(false)
            .add_plugin_versions(
                "beta-plugin",
                vec![
                    Version::new(1, 0, 0),
                    Version::parse("2.0.0-beta.1").unwrap(),
                ],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("beta-plugin").unwrap().version,
            Version::new(1, 0, 0)
        );

        // With pre-release enabled
        let resolver = DependencyResolver::new()
            .include_prerelease(true)
            .add_plugin_versions(
                "beta-plugin",
                vec![
                    Version::new(1, 0, 0),
                    Version::parse("2.0.0-beta.1").unwrap(),
                ],
            );

        let resolution = resolver.resolve(&graph).unwrap();
        assert_eq!(
            resolution.plugins.get("beta-plugin").unwrap().version,
            Version::parse("2.0.0-beta.1").unwrap()
        );
    }

    #[test]
    fn test_integration_transitive() {
        let mut graph = DependencyGraph::new();

        // A -> B -> C -> D
        graph.add_plugin(PluginNode::new("d", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("c", Version::new(1, 0, 0))
                .with_dependency(Dependency::new("d", VersionReq::any())),
        );
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0))
                .with_dependency(Dependency::new("c", VersionReq::any())),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0))
                .with_dependency(Dependency::new("b", VersionReq::any())),
        );

        // Transitive dependencies of A should include B, C, D
        let trans_deps = graph.transitive_dependencies("a");
        assert!(trans_deps.contains("b"));
        assert!(trans_deps.contains("c"));
        assert!(trans_deps.contains("d"));
        assert_eq!(trans_deps.len(), 3);

        // Transitive dependents of D should include A, B, C
        let trans_dependents = graph.transitive_dependents("d");
        assert!(trans_dependents.contains("a"));
        assert!(trans_dependents.contains("b"));
        assert!(trans_dependents.contains("c"));
        assert_eq!(trans_dependents.len(), 3);
    }
}
