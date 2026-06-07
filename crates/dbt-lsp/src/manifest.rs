use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct DbtNode {
    pub unique_id: String,
    pub name: String,
    pub original_file_path: String,
    pub resource_type: String,
    #[serde(default)]
    pub source_name: Option<String>,
    pub depends_on: DependsOn,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DependsOn {
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default)]
    nodes: HashMap<String, DbtNode>,
    #[serde(default)]
    sources: HashMap<String, DbtNode>,
}

/// All nodes (models + sources) keyed by unique_id.
#[derive(Debug, Default)]
pub struct ManifestGraph {
    pub nodes: HashMap<String, DbtNode>,
}

impl ManifestGraph {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawManifest = serde_json::from_str(json)?;
        let mut nodes = raw.nodes;
        nodes.extend(raw.sources);
        Ok(ManifestGraph { nodes })
    }

    /// Find a model node by its short name (e.g. "orders").
    pub fn find_model(&self, name: &str) -> Option<&DbtNode> {
        self.nodes
            .values()
            .find(|n| n.resource_type == "model" && n.name == name)
    }

    /// Find a source node by source_name and table name.
    pub fn find_source(&self, source_name: &str, table_name: &str) -> Option<&DbtNode> {
        self.nodes.values().find(|n| {
            n.resource_type == "source"
                && n.name == table_name
                && n.source_name.as_deref() == Some(source_name)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        r#"{
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
        }"#
    }

    #[test]
    fn test_parse_nodes_and_sources() {
        let graph = ManifestGraph::from_json(fixture()).unwrap();
        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn test_find_model() {
        let graph = ManifestGraph::from_json(fixture()).unwrap();
        let node = graph.find_model("orders").unwrap();
        assert_eq!(node.original_file_path, "models/orders.sql");
        assert_eq!(node.depends_on.nodes, vec!["model.proj.stg_orders"]);
    }

    #[test]
    fn test_find_source() {
        let graph = ManifestGraph::from_json(fixture()).unwrap();
        let node = graph.find_source("jaffle_shop", "raw_orders").unwrap();
        assert_eq!(node.name, "raw_orders");
    }

    #[test]
    fn test_find_model_missing_returns_none() {
        let graph = ManifestGraph::from_json(fixture()).unwrap();
        assert!(graph.find_model("nonexistent").is_none());
    }
}
