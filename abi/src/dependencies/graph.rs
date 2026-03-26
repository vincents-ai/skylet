// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dependency Graph Builder and Topological Sort
//!
//! This module provides:
//! - Dependency graph representation
//! - Graph construction from plugin manifests
//! - Topological sorting for correct activation order
//! - Cycle detection
//!
//! RFC-0005: Plugin Dependency Resolution

use crate::dependencies::constraints::VersionReq;
use crate::dependencies::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Unique identifier for a plugin in the dependency graph
pub type PluginId = String;

/// A dependency requirement from one plugin to another
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// The name of the required plugin
    pub name: PluginId,
    /// Version requirements
    pub version_req: VersionReq,
    /// Whether this is an optional dependency
    pub optional: bool,
    /// Features enabled for this dependency
    pub features: Vec<String>,
}

impl Dependency {
    /// Create a new required dependency
    pub fn new(name: impl Into<String>, version_req: VersionReq) -> Self {
        Self {
            name: name.into(),
            version_req,
            optional: false,
            features: Vec::new(),
        }
    }

    /// Create an optional dependency
    pub fn optional(name: impl Into<String>, version_req: VersionReq) -> Self {
        Self {
            name: name.into(),
            version_req,
            optional: true,
            features: Vec::new(),
        }
    }

    /// Add features to this dependency
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.version_req)?;
        if self.optional {
            write!(f, " (optional)")?;
        }
        Ok(())
    }
}

/// A node in the dependency graph representing a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginNode {
    /// Plugin identifier
    pub id: PluginId,
    /// Plugin version
    pub version: Version,
    /// Direct dependencies
    pub dependencies: Vec<Dependency>,
    /// Features provided by this plugin
    pub features: HashSet<String>,
    /// Whether this plugin is optional
    pub optional: bool,
}

impl PluginNode {
    /// Create a new plugin node
    pub fn new(id: impl Into<String>, version: Version) -> Self {
        Self {
            id: id.into(),
            version,
            dependencies: Vec::new(),
            features: HashSet::new(),
            optional: false,
        }
    }

    /// Add a dependency to this plugin
    pub fn with_dependency(mut self, dep: Dependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Add multiple dependencies
    pub fn with_dependencies(mut self, deps: Vec<Dependency>) -> Self {
        self.dependencies.extend(deps);
        self
    }

    /// Add features to this plugin
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features.into_iter().collect();
        self
    }
}

/// A directed edge in the dependency graph
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// Source plugin (the one that has the dependency)
    pub from: PluginId,
    /// Target plugin (the dependency)
    pub to: PluginId,
    /// The dependency specification
    pub dependency: Dependency,
}

/// The dependency graph
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyGraph {
    /// All plugins in the graph
    nodes: HashMap<PluginId, PluginNode>,
    /// Edges representing dependencies
    edges: Vec<DependencyEdge>,
    /// Reverse index: for each plugin, which plugins depend on it
    dependents: HashMap<PluginId, HashSet<PluginId>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plugin to the graph
    pub fn add_plugin(&mut self, node: PluginNode) {
        let id = node.id.clone();
        for dep in &node.dependencies {
            let edge = DependencyEdge {
                from: id.clone(),
                to: dep.name.clone(),
                dependency: dep.clone(),
            };
            self.edges.push(edge);

            // Update reverse index
            self.dependents
                .entry(dep.name.clone())
                .or_default()
                .insert(id.clone());
        }
        self.nodes.insert(id, node);
    }

    /// Get a plugin by ID
    pub fn get_plugin(&self, id: &str) -> Option<&PluginNode> {
        self.nodes.get(id)
    }

    /// Get all plugins
    pub fn plugins(&self) -> impl Iterator<Item = &PluginNode> {
        self.nodes.values()
    }

    /// Get direct dependencies of a plugin
    pub fn dependencies_of(&self, id: &str) -> Vec<&Dependency> {
        self.nodes
            .get(id)
            .map(|n| n.dependencies.iter().collect())
            .unwrap_or_default()
    }

    /// Get plugins that depend on the given plugin
    pub fn dependents_of(&self, id: &str) -> Option<&HashSet<PluginId>> {
        self.dependents.get(id)
    }

    /// Get all edges in the graph
    pub fn edges(&self) -> &[DependencyEdge] {
        &self.edges
    }

    /// Check if the graph contains a plugin
    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Get the number of plugins in the graph
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Detect cycles in the dependency graph
    /// Returns a list of cycles found (each cycle is a list of plugin IDs)
    pub fn detect_cycles(&self) -> Vec<Vec<PluginId>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node_id in self.nodes.keys() {
            if !visited.contains(node_id) {
                self.detect_cycles_dfs(
                    node_id,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn detect_cycles_dfs(
        &self,
        node_id: &str,
        visited: &mut HashSet<PluginId>,
        rec_stack: &mut HashSet<PluginId>,
        path: &mut Vec<PluginId>,
        cycles: &mut Vec<Vec<PluginId>>,
    ) {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());
        path.push(node_id.to_string());

        if let Some(node) = self.nodes.get(node_id) {
            for dep in &node.dependencies {
                if !visited.contains(&dep.name) {
                    self.detect_cycles_dfs(&dep.name, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(&dep.name) {
                    // Found a cycle - extract it
                    if let Some(start_idx) = path.iter().position(|p| p == &dep.name) {
                        let cycle: Vec<PluginId> = path[start_idx..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(node_id);
    }

    /// Check if the graph has any cycles
    pub fn has_cycles(&self) -> bool {
        !self.detect_cycles().is_empty()
    }

    /// Perform topological sort
    /// Returns plugins in dependency order (dependencies before dependents)
    /// Returns an error if the graph has cycles
    pub fn topological_sort(&self) -> Result<Vec<PluginId>, GraphError> {
        let cycles = self.detect_cycles();
        if !cycles.is_empty() {
            return Err(GraphError::CyclicDependency {
                cycles: cycles.iter().map(|c| c.join(" -> ")).collect(),
            });
        }

        // Kahn's algorithm
        // Edges go from dependent -> dependency (from = dependent, to = dependency).
        // We want dependencies first, so count in-degree based on the "from" side
        // (i.e., how many dependencies a node has). Nodes with no dependencies
        // (root dependencies) start with in-degree 0 and are processed first.
        let mut in_degree: HashMap<PluginId, usize> =
            self.nodes.keys().map(|id| (id.clone(), 0)).collect();

        // Count incoming edges: each edge means "from depends on to",
        // so increment in-degree for the dependent (from) side
        for edge in &self.edges {
            *in_degree.entry(edge.from.clone()).or_insert(0) += 1;
        }

        // Queue of nodes with no dependencies (in-degree 0 = root dependencies)
        let mut queue: VecDeque<PluginId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.clone());

            // For each node that depends on the current node, reduce its in-degree
            if let Some(dependent_ids) = self.dependents.get(&node_id) {
                for dep_id in dependent_ids {
                    if let Some(deg) = in_degree.get_mut(dep_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep_id.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles (shouldn't happen since we checked above)
        if result.len() != self.nodes.len() {
            return Err(GraphError::CyclicDependency {
                cycles: vec!["Unknown cycle detected".to_string()],
            });
        }

        Ok(result)
    }

    /// Get the activation order (topological sort — dependencies before dependents)
    /// This gives the order to activate plugins (dependencies activated first)
    pub fn activation_order(&self) -> Result<Vec<PluginId>, GraphError> {
        self.topological_sort()
    }

    /// Get the deactivation order (reverse topological sort — dependents before dependencies)
    /// This gives the order to deactivate plugins (dependents deactivated first)
    pub fn deactivation_order(&self) -> Result<Vec<PluginId>, GraphError> {
        let mut order = self.topological_sort()?;
        order.reverse();
        Ok(order)
    }

    /// Find all transitive dependencies of a plugin
    pub fn transitive_dependencies(&self, id: &str) -> HashSet<PluginId> {
        let mut deps = HashSet::new();
        let mut to_visit = VecDeque::new();

        if let Some(node) = self.nodes.get(id) {
            for dep in &node.dependencies {
                to_visit.push_back(dep.name.clone());
            }
        }

        while let Some(dep_id) = to_visit.pop_front() {
            if deps.insert(dep_id.clone()) {
                if let Some(node) = self.nodes.get(&dep_id) {
                    for dep in &node.dependencies {
                        if !deps.contains(&dep.name) {
                            to_visit.push_back(dep.name.clone());
                        }
                    }
                }
            }
        }

        deps
    }

    /// Find all transitive dependents of a plugin
    pub fn transitive_dependents(&self, id: &str) -> HashSet<PluginId> {
        let mut dependents = HashSet::new();
        let mut to_visit = VecDeque::new();

        if let Some(direct_dependents) = self.dependents.get(id) {
            for dep_id in direct_dependents {
                to_visit.push_back(dep_id.clone());
            }
        }

        while let Some(dep_id) = to_visit.pop_front() {
            if dependents.insert(dep_id.clone()) {
                if let Some(indirect) = self.dependents.get(&dep_id) {
                    for indirect_id in indirect {
                        if !dependents.contains(indirect_id) {
                            to_visit.push_back(indirect_id.clone());
                        }
                    }
                }
            }
        }

        dependents
    }
}

/// Errors related to dependency graph operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Cycle detected in dependencies
    CyclicDependency { cycles: Vec<String> },
    /// Missing dependency
    MissingDependency { plugin: String, dependency: String },
    /// Version conflict
    VersionConflict {
        plugin: String,
        dependency: String,
        required: String,
        available: String,
    },
    /// Plugin not found
    PluginNotFound { plugin: String },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::CyclicDependency { cycles } => {
                write!(f, "Cyclic dependencies detected: {}", cycles.join("; "))
            }
            GraphError::MissingDependency { plugin, dependency } => {
                write!(
                    f,
                    "Plugin '{}' requires missing dependency '{}'",
                    plugin, dependency
                )
            }
            GraphError::VersionConflict {
                plugin,
                dependency,
                required,
                available,
            } => {
                write!(
                    f,
                    "Plugin '{}' requires {} {}, but {} is available",
                    plugin, dependency, required, available
                )
            }
            GraphError::PluginNotFound { plugin } => {
                write!(f, "Plugin '{}' not found", plugin)
            }
        }
    }
}

impl std::error::Error for GraphError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependencies::constraints::VersionReq;

    fn make_dep(name: &str, version: &str) -> Dependency {
        Dependency::new(name, VersionReq::parse(version).unwrap())
    }

    #[test]
    fn test_plugin_node_new() {
        let node = PluginNode::new("test-plugin", Version::new(1, 0, 0));
        assert_eq!(node.id, "test-plugin");
        assert_eq!(node.version, Version::new(1, 0, 0));
        assert!(node.dependencies.is_empty());
    }

    #[test]
    fn test_plugin_node_with_dependency() {
        let node = PluginNode::new("main", Version::new(1, 0, 0))
            .with_dependency(make_dep("dep1", "^1.0.0"));

        assert_eq!(node.dependencies.len(), 1);
        assert_eq!(node.dependencies[0].name, "dep1");
    }

    #[test]
    fn test_dependency_graph_add_plugin() {
        let mut graph = DependencyGraph::new();

        let node = PluginNode::new("plugin-a", Version::new(1, 0, 0));
        graph.add_plugin(node);

        assert!(graph.contains("plugin-a"));
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn test_dependency_graph_dependencies() {
        let mut graph = DependencyGraph::new();

        let dep_b = PluginNode::new("plugin-b", Version::new(1, 0, 0));
        graph.add_plugin(dep_b);

        let main = PluginNode::new("plugin-a", Version::new(1, 0, 0))
            .with_dependency(make_dep("plugin-b", "^1.0.0"));
        graph.add_plugin(main);

        let deps = graph.dependencies_of("plugin-a");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "plugin-b");
    }

    #[test]
    fn test_dependency_graph_dependents() {
        let mut graph = DependencyGraph::new();

        let dep = PluginNode::new("plugin-b", Version::new(1, 0, 0));
        graph.add_plugin(dep);

        let main = PluginNode::new("plugin-a", Version::new(1, 0, 0))
            .with_dependency(make_dep("plugin-b", "^1.0.0"));
        graph.add_plugin(main);

        let dependents = graph.dependents_of("plugin-b").unwrap();
        assert!(dependents.contains("plugin-a"));
    }

    #[test]
    fn test_topological_sort_simple() {
        let mut graph = DependencyGraph::new();

        // C depends on B, B depends on A
        // Order should be: A, B, C
        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("a", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("c", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let order = graph.topological_sort().unwrap();
        assert_eq!(order.len(), 3);

        // A should come before B, B should come before C
        let a_idx = order.iter().position(|p| p == "a").unwrap();
        let b_idx = order.iter().position(|p| p == "b").unwrap();
        let c_idx = order.iter().position(|p| p == "c").unwrap();

        assert!(a_idx < b_idx);
        assert!(b_idx < c_idx);
    }

    #[test]
    fn test_topological_sort_diamond() {
        // Diamond dependency: D -> B, D -> C, B -> A, C -> A
        // Order should have A first, then B and C in any order, then D
        let mut graph = DependencyGraph::new();

        graph.add_plugin(PluginNode::new("a", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("a", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("c", Version::new(1, 0, 0)).with_dependency(make_dep("a", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("d", Version::new(1, 0, 0))
                .with_dependencies(vec![make_dep("b", "^1.0.0"), make_dep("c", "^1.0.0")]),
        );

        let order = graph.topological_sort().unwrap();
        assert_eq!(order.len(), 4);

        let a_idx = order.iter().position(|p| p == "a").unwrap();
        let b_idx = order.iter().position(|p| p == "b").unwrap();
        let c_idx = order.iter().position(|p| p == "c").unwrap();
        let d_idx = order.iter().position(|p| p == "d").unwrap();

        assert!(a_idx < b_idx);
        assert!(a_idx < c_idx);
        assert!(b_idx < d_idx);
        assert!(c_idx < d_idx);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();

        // A -> B -> C -> A (cycle)
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("a", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("c", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        assert!(graph.has_cycles());
        assert!(graph.topological_sort().is_err());
    }

    #[test]
    fn test_transitive_dependencies() {
        let mut graph = DependencyGraph::new();

        // A -> B -> C
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let transitive = graph.transitive_dependencies("a");
        assert!(transitive.contains("b"));
        assert!(transitive.contains("c"));
        assert_eq!(transitive.len(), 2);
    }

    #[test]
    fn test_transitive_dependents() {
        let mut graph = DependencyGraph::new();

        // A -> B -> C
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let dependents = graph.transitive_dependents("c");
        assert!(dependents.contains("a"));
        assert!(dependents.contains("b"));
        assert_eq!(dependents.len(), 2);
    }

    #[test]
    fn test_activation_order() {
        let mut graph = DependencyGraph::new();

        // Dependencies: A -> B -> C
        // Activation should be: C, B, A (dependencies activated first)
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let order = graph.activation_order().unwrap();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_deactivation_order() {
        let mut graph = DependencyGraph::new();

        // Dependencies: A -> B -> C
        // Deactivation should be: A, B, C (dependents deactivated first)
        graph.add_plugin(PluginNode::new("c", Version::new(1, 0, 0)));
        graph.add_plugin(
            PluginNode::new("b", Version::new(1, 0, 0)).with_dependency(make_dep("c", "^1.0.0")),
        );
        graph.add_plugin(
            PluginNode::new("a", Version::new(1, 0, 0)).with_dependency(make_dep("b", "^1.0.0")),
        );

        let order = graph.deactivation_order().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}
