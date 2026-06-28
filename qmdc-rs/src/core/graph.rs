//! D2 graph read-view types — read-only projections over the resolved index.
//!
//! Graph nodes are derived from objects: id, kind, namespace, label (= name field or id),
//! file, line. Graph edges are derived by scanning all objects' non-`__` fields for
//! `[[#target]]` patterns — each match creates an edge: source=object.__id, target=matched_id,
//! edge_type=field_name.

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

use crate::core::envelope::BoundedEnvelope;
use crate::core::fields::QmdcObject;
use crate::core::resolved_index::ResolvedIndex;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A node in the graph — read-only projection of an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub namespace: String,
    pub label: String,
    pub file: String,
    pub line: i64,
}

impl GraphNode {
    /// Serialize to a JSON value.
    pub fn to_value(&self) -> Value {
        json!({
            "id": self.id,
            "kind": self.kind,
            "namespace": self.namespace,
            "label": self.label,
            "file": self.file,
            "line": self.line,
        })
    }
}

/// An edge in the graph — derived from a reference in a field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub source_field: String,
}

impl GraphEdge {
    /// Serialize to a JSON value.
    pub fn to_value(&self) -> Value {
        json!({
            "source": self.source,
            "target": self.target,
            "edge_type": self.edge_type,
            "source_field": self.source_field,
        })
    }
}

/// A subgraph — a subset of nodes and edges with bounded envelope metadata.
#[derive(Debug, Clone)]
pub struct Subgraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl Subgraph {
    /// Serialize to a bounded envelope JSON value.
    ///
    /// Both `nodes` (the envelope `items`) and `edges` are bounded (NFR-4): neither is
    /// silently dropped. Edge truncation is reported via `edges_truncated`/`edges_remaining`.
    pub fn to_bounded_value(&self, limit: usize) -> Value {
        let node_values: Vec<Value> = self.nodes.iter().map(|n| n.to_value()).collect();
        let edge_values: Vec<Value> = self.edges.iter().map(|e| e.to_value()).collect();
        let envelope = BoundedEnvelope::from_items(node_values, limit, 0);
        let mut val = envelope.to_value();
        let (edges, edges_truncated, edges_remaining) =
            crate::core::envelope::bound_list(edge_values, limit);
        val["edges"] = json!(edges);
        val["edges_truncated"] = json!(edges_truncated);
        if edges_truncated {
            val["edges_remaining"] = json!(edges_remaining);
        }
        val
    }
}

/// An ordered path chain — the result of `find_path`.
#[derive(Debug, Clone)]
pub struct PathChain {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl PathChain {
    /// Serialize to a JSON value.
    pub fn to_value(&self) -> Value {
        json!({
            "nodes": self.nodes.iter().map(|n| n.to_value()).collect::<Vec<_>>(),
            "edges": self.edges.iter().map(|e| e.to_value()).collect::<Vec<_>>(),
            "length": self.edges.len(),
        })
    }
}

/// Metamodel inventory — aggregate kind/edge statistics.
#[derive(Debug, Clone)]
pub struct MetamodelInventory {
    pub kinds: Vec<KindSummary>,
    pub edge_types: Vec<EdgeTypeSummary>,
    pub total_objects: usize,
    pub total_edges: usize,
}

/// Per-kind summary in the metamodel inventory.
#[derive(Debug, Clone)]
pub struct KindSummary {
    pub kind: String,
    pub count: usize,
    pub fields: Vec<String>,
}

/// Per-edge-type summary in the metamodel inventory.
#[derive(Debug, Clone)]
pub struct EdgeTypeSummary {
    pub edge_type: String,
    pub count: usize,
}

impl MetamodelInventory {
    /// Serialize to a JSON value.
    pub fn to_value(&self) -> Value {
        json!({
            "kinds": self.kinds.iter().map(|k| json!({
                "kind": k.kind,
                "count": k.count,
                "fields": k.fields,
            })).collect::<Vec<_>>(),
            "edge_types": self.edge_types.iter().map(|e| json!({
                "edge_type": e.edge_type,
                "count": e.count,
            })).collect::<Vec<_>>(),
            "total_objects": self.total_objects,
            "total_edges": self.total_edges,
        })
    }
}

// ---------------------------------------------------------------------------
// Edge extraction
// ---------------------------------------------------------------------------

/// Regex for `[[#target_id]]` patterns in field values.
fn ref_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[#([^\]]+)\]\]").unwrap())
}

/// Extract all edges from the resolved index by scanning object fields.
pub fn extract_edges(index: &ResolvedIndex) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let re = ref_pattern();

    for obj in index.objects() {
        let source_id = match obj.get("__id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };

        if let Some(map) = obj.as_object() {
            for (field_name, field_value) in map {
                if field_name.starts_with("__") {
                    continue;
                }
                extract_edges_from_value(source_id, field_name, field_value, re, &mut edges);
            }
        }
    }

    edges
}

/// Recursively extract edges from a field value (handles strings and arrays).
fn extract_edges_from_value(
    source_id: &str,
    field_name: &str,
    value: &Value,
    re: &Regex,
    edges: &mut Vec<GraphEdge>,
) {
    match value {
        Value::String(s) => {
            for cap in re.captures_iter(s) {
                if let Some(target) = cap.get(1) {
                    edges.push(GraphEdge {
                        source: source_id.to_string(),
                        target: target.as_str().to_string(),
                        edge_type: field_name.to_string(),
                        source_field: field_name.to_string(),
                    });
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_edges_from_value(source_id, field_name, item, re, edges);
            }
        }
        _ => {}
    }
}

/// Build a GraphNode from a parsed object JSON value.
pub fn node_from_object(obj: &Value) -> Option<GraphNode> {
    let id = obj.get("__id").and_then(|v| v.as_str())?;
    let kind = obj.kind();
    let namespace = obj.namespace();
    let file = obj.file();
    let line = obj.line();

    // label = name field, or id as fallback
    let label = obj.get("name").and_then(|v| v.as_str()).unwrap_or(id);

    Some(GraphNode {
        id: id.to_string(),
        kind: kind.to_string(),
        namespace: namespace.to_string(),
        label: label.to_string(),
        file: file.to_string(),
        line,
    })
}

/// Build all GraphNodes from the index.
pub fn extract_nodes(index: &ResolvedIndex) -> Vec<GraphNode> {
    index
        .objects()
        .iter()
        .filter_map(node_from_object)
        .collect()
}
