use crate::manifest::{DbtNode, ManifestGraph};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct LineageNode {
    pub unique_id: String,
    pub name: String,
    pub resource_type: String,
    pub source_name: Option<String>,
    pub file_path: String,
    pub depth: usize,
    pub children: Vec<LineageNode>,
}

impl LineageNode {
    fn from_dbt(node: &DbtNode, depth: usize) -> Self {
        LineageNode {
            unique_id: node.unique_id.clone(),
            name: node.name.clone(),
            resource_type: node.resource_type.clone(),
            source_name: node.source_name.clone(),
            file_path: node.original_file_path.clone(),
            depth,
            children: vec![],
        }
    }
}

/// Build upstream tree (ancestors the node depends on).
pub fn upstream(
    graph: &ManifestGraph,
    root_id: &str,
    max_depth: usize,
) -> Vec<LineageNode> {
    build_tree(graph, root_id, max_depth, true)
}

/// Build downstream tree (descendants that depend on the node).
pub fn downstream(
    graph: &ManifestGraph,
    root_id: &str,
    max_depth: usize,
) -> Vec<LineageNode> {
    build_tree(graph, root_id, max_depth, false)
}

fn build_tree(
    graph: &ManifestGraph,
    root_id: &str,
    max_depth: usize,
    is_upstream: bool,
) -> Vec<LineageNode> {
    /// Returns children of `node_id` at the given `depth`.
    /// `visited` tracks nodes already included to prevent cycles.
    fn recurse(
        graph: &ManifestGraph,
        node_id: &str,
        depth: usize,
        max_depth: usize,
        is_upstream: bool,
        visited: &mut HashSet<String>,
    ) -> Vec<LineageNode> {
        if depth > max_depth {
            return vec![];
        }

        let neighbor_ids: Vec<String> = if is_upstream {
            graph
                .nodes
                .get(node_id)
                .map(|n| n.depends_on.nodes.clone())
                .unwrap_or_default()
        } else {
            graph
                .nodes
                .values()
                .filter(|n| n.depends_on.nodes.iter().any(|dep| dep == node_id))
                .map(|n| n.unique_id.clone())
                .collect()
        };

        let unvisited: Vec<String> = neighbor_ids
            .into_iter()
            .filter(|id| !visited.contains(id))
            .collect();

        let mut result = Vec::new();
        for id in &unvisited {
            if let Some(dbt_node) = graph.nodes.get(id) {
                let mut lineage_node = LineageNode::from_dbt(dbt_node, depth);
                visited.insert(id.clone());
                lineage_node.children =
                    recurse(graph, id, depth + 1, max_depth, is_upstream, visited);
                result.push(lineage_node);
            }
        }
        result
    }

    let mut visited = HashSet::new();
    visited.insert(root_id.to_string());
    recurse(graph, root_id, 1, max_depth, is_upstream, &mut visited)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ManifestGraph;

    fn fixture_graph() -> ManifestGraph {
        let json = r#"{
            "nodes": {
                "model.proj.orders": {
                    "unique_id": "model.proj.orders",
                    "name": "orders",
                    "original_file_path": "models/orders.sql",
                    "resource_type": "model",
                    "depends_on": { "nodes": ["model.proj.stg_orders"] }
                },
                "model.proj.stg_orders": {
                    "unique_id": "model.proj.stg_orders",
                    "name": "stg_orders",
                    "original_file_path": "models/stg_orders.sql",
                    "resource_type": "model",
                    "depends_on": { "nodes": ["source.proj.jaffle_shop.raw_orders"] }
                },
                "model.proj.fct_revenue": {
                    "unique_id": "model.proj.fct_revenue",
                    "name": "fct_revenue",
                    "original_file_path": "models/fct_revenue.sql",
                    "resource_type": "model",
                    "depends_on": { "nodes": ["model.proj.orders"] }
                }
            },
            "sources": {
                "source.proj.jaffle_shop.raw_orders": {
                    "unique_id": "source.proj.jaffle_shop.raw_orders",
                    "name": "raw_orders",
                    "source_name": "jaffle_shop",
                    "original_file_path": "models/sources.yml",
                    "resource_type": "source",
                    "depends_on": { "nodes": [] }
                }
            }
        }"#;
        ManifestGraph::from_json(json).unwrap()
    }

    #[test]
    fn test_upstream_two_levels() {
        let graph = fixture_graph();
        let result = upstream(&graph, "model.proj.orders", 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "stg_orders");
        assert_eq!(result[0].children.len(), 1);
        assert_eq!(result[0].children[0].name, "raw_orders");
    }

    #[test]
    fn test_downstream_one_level() {
        let graph = fixture_graph();
        let result = downstream(&graph, "model.proj.orders", 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "fct_revenue");
    }

    #[test]
    fn test_max_depth_respected() {
        let graph = fixture_graph();
        // max_depth=1: orders → stg_orders (depth 1), but raw_orders (depth 2) cut off
        let result = upstream(&graph, "model.proj.orders", 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].children.len(), 0); // depth 2 cut off
    }

    #[test]
    fn test_no_upstream_for_source() {
        let graph = fixture_graph();
        let result = upstream(&graph, "source.proj.jaffle_shop.raw_orders", 5);
        assert_eq!(result.len(), 0);
    }
}
