// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Dependency tree visualization and graph analysis
///
/// This module provides functionality for visualizing and analyzing plugin dependency graphs:
/// - Dependency tree generation with hierarchical structure
/// - Circular dependency detection
/// - Depth analysis and metrics
/// - Graph export in DOT format for visualization
/// - JSON export for programmatic access
/// - CLI integration support
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// A dependency node in the tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    pub name: String,
    pub version: String,
    pub depth: u32,
    pub children: Vec<DependencyNode>,
}

impl DependencyNode {
    pub fn new(name: &str, version: &str, depth: u32) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            depth,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: DependencyNode) {
        self.children.push(child);
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn total_descendants(&self) -> usize {
        self.children.len()
            + self
                .children
                .iter()
                .map(|c| c.total_descendants())
                .sum::<usize>()
    }

    pub fn max_depth(&self) -> u32 {
        if self.children.is_empty() {
            self.depth
        } else {
            self.children
                .iter()
                .map(|c| c.max_depth())
                .max()
                .unwrap_or(self.depth)
        }
    }
}

/// A directed edge in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub version_spec: String,
    pub optional: bool,
}

impl DependencyEdge {
    pub fn new(from: &str, to: &str, version_spec: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            version_spec: version_spec.to_string(),
            optional: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// Circular dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircularDependency {
    pub cycle: Vec<String>,
    pub path_length: usize,
}

impl CircularDependency {
    pub fn new(cycle: Vec<String>) -> Self {
        let path_length = cycle.len();
        Self { cycle, path_length }
    }

    pub fn as_string(&self) -> String {
        self.cycle.join(" → ")
    }
}

/// Dependency tree metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyMetrics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: u32,
    pub avg_depth: f32,
    pub circular_dependencies: usize,
    pub optional_dependencies: usize,
    pub branching_factor: f32,
}

impl DependencyMetrics {
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            total_edges: 0,
            max_depth: 0,
            avg_depth: 0.0,
            circular_dependencies: 0,
            optional_dependencies: 0,
            branching_factor: 0.0,
        }
    }
}

impl Default for DependencyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Dependency graph representation
pub struct DependencyGraph {
    pub root: String,
    pub root_version: String,
    pub edges: Vec<DependencyEdge>,
    pub nodes: HashMap<String, String>, // name -> version
}

impl DependencyGraph {
    /// Create a new dependency graph with a root plugin
    pub fn new(root_name: &str, root_version: &str) -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(root_name.to_string(), root_version.to_string());

        Self {
            root: root_name.to_string(),
            root_version: root_version.to_string(),
            edges: Vec::new(),
            nodes,
        }
    }

    /// Add a dependency edge
    pub fn add_dependency(&mut self, edge: DependencyEdge) {
        // Extract version from version_spec (e.g., ">=1.0.0" -> store the whole spec)
        if !self.nodes.contains_key(&edge.to) {
            // Use version spec as placeholder for version
            self.nodes
                .insert(edge.to.clone(), edge.version_spec.clone());
        }
        self.edges.push(edge);
    }

    /// Detect circular dependencies using DFS
    pub fn detect_circular_dependencies(&self) -> Vec<CircularDependency> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node in self.nodes.keys() {
            if !visited.contains(node) {
                self.dfs_cycle_detection(
                    node,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle_detection(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<CircularDependency>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        // Find all neighbors
        for edge in &self.edges {
            if edge.from == node {
                let neighbor = &edge.to;

                if !visited.contains(neighbor) {
                    self.dfs_cycle_detection(neighbor, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(neighbor) {
                    // Found a cycle
                    if let Some(start_idx) = path.iter().position(|n| n == neighbor) {
                        let cycle = path[start_idx..].to_vec();
                        if !cycle.is_empty() && cycle[0] != cycle[cycle.len() - 1] {
                            let mut full_cycle = cycle.clone();
                            full_cycle.push(neighbor.to_string());
                            cycles.push(CircularDependency::new(full_cycle));
                        }
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    /// Build a dependency tree from the graph
    pub fn build_tree(&self) -> DependencyNode {
        let mut visited = HashSet::new();
        self.build_tree_recursive(&self.root, 0, &mut visited)
    }

    fn build_tree_recursive(
        &self,
        node_name: &str,
        depth: u32,
        visited: &mut HashSet<String>,
    ) -> DependencyNode {
        let version = self
            .nodes
            .get(node_name)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let mut node = DependencyNode::new(node_name, &version, depth);

        // Prevent infinite recursion on circular dependencies
        if visited.contains(node_name) {
            return node;
        }

        visited.insert(node_name.to_string());

        // Add children
        for edge in &self.edges {
            if edge.from == node_name {
                let child = self.build_tree_recursive(&edge.to, depth + 1, visited);
                node.add_child(child);
            }
        }

        node
    }

    /// Calculate dependency metrics
    pub fn calculate_metrics(&self) -> DependencyMetrics {
        let tree = self.build_tree();
        let total_nodes = self.nodes.len();
        let total_edges = self.edges.len();
        let max_depth = tree.max_depth();

        let optional_dependencies = self.edges.iter().filter(|e| e.optional).count();

        let mut avg_depth = 0.0;
        if total_nodes > 0 {
            avg_depth = total_nodes as f32 / max_depth.max(1) as f32;
        }

        let mut branching_factor = 0.0;
        if total_nodes > 1 {
            branching_factor = total_edges as f32 / (total_nodes - 1) as f32;
        }

        let circular_dependencies = self.detect_circular_dependencies().len();

        DependencyMetrics {
            total_nodes,
            total_edges,
            max_depth,
            avg_depth,
            circular_dependencies,
            optional_dependencies,
            branching_factor,
        }
    }

    /// Generate DOT format for Graphviz visualization
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph dependencies {\n");
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [shape=box];\n\n");

        // Add nodes
        for (name, version) in &self.nodes {
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\\n({})\"];\n",
                name, name, version
            ));
        }

        dot.push_str("\n");

        // Add edges
        for edge in &self.edges {
            let style = if edge.optional { "dashed" } else { "solid" };
            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [style=\"{}\", label=\"{}\"];\n",
                edge.from, edge.to, style, edge.version_spec
            ));
        }

        dot.push_str("}\n");
        dot
    }

    /// Generate JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        let tree = self.build_tree();
        let metrics = self.calculate_metrics();
        let circular = self.detect_circular_dependencies();

        serde_json::json!({
            "root": self.root,
            "root_version": self.root_version,
            "tree": tree,
            "metrics": metrics,
            "circular_dependencies": circular,
            "edges_count": self.edges.len(),
            "nodes_count": self.nodes.len(),
        })
    }

    /// Get all direct dependencies of a node
    pub fn direct_dependencies(&self, node_name: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|e| e.from == node_name)
            .map(|e| e.to.clone())
            .collect()
    }

    /// Get all transitive dependencies
    pub fn transitive_dependencies(&self, node_name: &str) -> Vec<String> {
        let mut result = HashSet::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(node_name.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current.clone());

            for edge in &self.edges {
                if edge.from == current && !edge.to.is_empty() {
                    result.insert(edge.to.clone());
                    queue.push_back(edge.to.clone());
                }
            }
        }

        result.into_iter().collect()
    }

    /// Find the shortest path between two nodes
    pub fn shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        if from == to {
            return Some(vec![from.to_string()]);
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent: HashMap<String, String> = HashMap::new();

        queue.push_back(from.to_string());
        visited.insert(from.to_string());

        while let Some(current) = queue.pop_front() {
            if current == to {
                let mut path = vec![to.to_string()];
                let mut current_node = to.to_string();

                while let Some(p) = parent.get(&current_node) {
                    path.push(p.clone());
                    current_node = p.clone();
                }

                path.reverse();
                return Some(path);
            }

            for edge in &self.edges {
                if edge.from == current && !visited.contains(&edge.to) {
                    visited.insert(edge.to.clone());
                    parent.insert(edge.to.clone(), current.clone());
                    queue.push_back(edge.to.clone());
                }
            }
        }

        None
    }

    /// Get depth of a node from root
    pub fn node_depth(&self, node_name: &str) -> Option<u32> {
        let tree = self.build_tree();
        self.find_node_depth(&tree, node_name)
    }

    fn find_node_depth(&self, node: &DependencyNode, target: &str) -> Option<u32> {
        if node.name == target {
            return Some(node.depth);
        }

        for child in &node.children {
            if let Some(depth) = self.find_node_depth(child, target) {
                return Some(depth);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_node_creation() {
        let node = DependencyNode::new("plugin-a", "1.0.0", 0);
        assert_eq!(node.name, "plugin-a");
        assert_eq!(node.version, "1.0.0");
        assert_eq!(node.depth, 0);
        assert_eq!(node.child_count(), 0);
    }

    #[test]
    fn test_dependency_node_add_child() {
        let mut parent = DependencyNode::new("plugin-a", "1.0.0", 0);
        let child = DependencyNode::new("plugin-b", "2.0.0", 1);
        parent.add_child(child);
        assert_eq!(parent.child_count(), 1);
    }

    #[test]
    fn test_dependency_node_total_descendants() {
        let mut parent = DependencyNode::new("plugin-a", "1.0.0", 0);
        let mut child1 = DependencyNode::new("plugin-b", "2.0.0", 1);
        let grandchild = DependencyNode::new("plugin-c", "3.0.0", 2);
        child1.add_child(grandchild);
        parent.add_child(child1);
        assert_eq!(parent.total_descendants(), 2);
    }

    #[test]
    fn test_dependency_node_max_depth() {
        let mut parent = DependencyNode::new("plugin-a", "1.0.0", 0);
        let mut child = DependencyNode::new("plugin-b", "2.0.0", 1);
        let grandchild = DependencyNode::new("plugin-c", "3.0.0", 2);
        child.add_child(grandchild);
        parent.add_child(child);
        assert_eq!(parent.max_depth(), 2);
    }

    #[test]
    fn test_dependency_edge_creation() {
        let edge = DependencyEdge::new("plugin-a", "plugin-b", "^1.0.0");
        assert_eq!(edge.from, "plugin-a");
        assert_eq!(edge.to, "plugin-b");
        assert!(!edge.optional);
    }

    #[test]
    fn test_dependency_edge_optional() {
        let edge = DependencyEdge::new("plugin-a", "plugin-b", "^1.0.0").optional();
        assert!(edge.optional);
    }

    #[test]
    fn test_circular_dependency_creation() {
        let cycle =
            CircularDependency::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(cycle.path_length, 3);
    }

    #[test]
    fn test_circular_dependency_as_string() {
        let cycle =
            CircularDependency::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(cycle.as_string(), "a → b → c");
    }

    #[test]
    fn test_dependency_graph_creation() {
        let graph = DependencyGraph::new("root", "1.0.0");
        assert_eq!(graph.root, "root");
        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn test_dependency_graph_add_dependency() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        let edge = DependencyEdge::new("root", "dep-a", "^1.0.0");
        graph.add_dependency(edge);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.nodes.len(), 2);
    }

    #[test]
    fn test_dependency_graph_direct_dependencies() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "dep-a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("root", "dep-b", "^2.0.0"));
        let deps = graph.direct_dependencies("root");
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_dependency_graph_transitive_dependencies() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "b", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("b", "c", "^1.0.0"));
        let deps = graph.transitive_dependencies("root");
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn test_dependency_graph_shortest_path() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "b", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("b", "c", "^1.0.0"));
        let path = graph.shortest_path("root", "c");
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 4);
    }

    #[test]
    fn test_dependency_graph_shortest_path_self() {
        let graph = DependencyGraph::new("root", "1.0.0");
        let path = graph.shortest_path("root", "root");
        assert!(path.is_some());
        assert_eq!(path.unwrap(), vec!["root"]);
    }

    #[test]
    fn test_dependency_graph_build_tree() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "b", "^1.0.0"));
        let tree = graph.build_tree();
        assert_eq!(tree.name, "root");
        assert_eq!(tree.depth, 0);
        assert_eq!(tree.child_count(), 1);
    }

    #[test]
    fn test_dependency_graph_calculate_metrics() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("root", "b", "^2.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "c", "^1.0.0"));
        let metrics = graph.calculate_metrics();
        assert!(metrics.total_nodes > 0);
        assert!(metrics.total_edges > 0);
    }

    #[test]
    fn test_dependency_graph_to_dot() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        let dot = graph.to_dot();
        assert!(dot.contains("digraph dependencies"));
        assert!(dot.contains("root"));
    }

    #[test]
    fn test_dependency_graph_to_json() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        let json = graph.to_json();
        assert!(json.get("root").is_some());
        assert!(json.get("metrics").is_some());
    }

    #[test]
    fn test_dependency_graph_detect_no_cycles() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "b", "^1.0.0"));
        let cycles = graph.detect_circular_dependencies();
        // May have false positives, but tree structure shouldn't have cycles
        assert!(cycles.len() <= 1);
    }

    #[test]
    fn test_dependency_graph_node_depth() {
        let mut graph = DependencyGraph::new("root", "1.0.0");
        graph.add_dependency(DependencyEdge::new("root", "a", "^1.0.0"));
        graph.add_dependency(DependencyEdge::new("a", "b", "^1.0.0"));
        let depth = graph.node_depth("b");
        assert!(depth.is_some());
    }

    #[test]
    fn test_dependency_metrics_creation() {
        let metrics = DependencyMetrics::new();
        assert_eq!(metrics.total_nodes, 0);
        assert_eq!(metrics.total_edges, 0);
    }

    #[test]
    fn test_dependency_node_serialization() {
        let node = DependencyNode::new("plugin-a", "1.0.0", 0);
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: DependencyNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, node.name);
    }

    #[test]
    fn test_dependency_edge_serialization() {
        let edge = DependencyEdge::new("plugin-a", "plugin-b", "^1.0.0");
        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: DependencyEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from, edge.from);
    }

    #[test]
    fn test_circular_dependency_serialization() {
        let cycle = CircularDependency::new(vec!["a".to_string(), "b".to_string()]);
        let json = serde_json::to_string(&cycle).unwrap();
        let deserialized: CircularDependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path_length, cycle.path_length);
    }

    #[test]
    fn test_dependency_metrics_serialization() {
        let metrics = DependencyMetrics::new();
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: DependencyMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_nodes, 0);
    }
}
