//! D2 graph read-view types — read-only projections over the resolved index.
//!
//! Graph nodes are derived from objects: id, kind, namespace, label (= name field or id),
//! file, line. The graph tools take their edges from the index's resolved `edges` table
//! ([`resolved_edges`]): source and target are resolved `__id`s, edge_type is the field name
//! (or a text preamble key). [`extract_edges`] is the older text scan of `[[#target]]`
//! patterns, still used for the metamodel's edge-type inventory.

use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::envelope::BoundedEnvelope;
use crate::core::error::{ErrorCode, ErrorEnvelope};
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

// ---------------------------------------------------------------------------
// Adjacency view (QMD-86, GitHub #17)
// ---------------------------------------------------------------------------

/// Which edges count as adjacent to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Edges whose `source` is the node; the neighbour is the `target`.
    Outgoing,
    /// Edges whose `target` is the node; the neighbour is the `source`.
    Incoming,
    /// Either; an edge from the node to itself is reported once.
    Both,
}

/// Nodes and resolved edges of the index plus id-keyed lookups, built once per operation so
/// that node lookup and neighbour enumeration are O(1) per step instead of a scan of the
/// whole graph (QMD-86). Neighbours come out in the order of `edges`.
pub struct GraphView {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// id -> index into `nodes`; the first node wins when two objects share an id.
    node_by_id: HashMap<String, usize>,
    /// source id -> ascending indices into `edges`.
    outgoing: HashMap<String, Vec<usize>>,
    /// target id -> ascending indices into `edges`.
    incoming: HashMap<String, Vec<usize>>,
}

/// Edges as the index resolved them: the `edges` table `qmdc query` exposes, in insertion
/// order (QMD-87, GitHub #18). Each target is the resolved `__id`, through the same resolver
/// `workspace validate` agrees with, so `[[#b]]` for `part.b`, `[[#svc:gateway]]` and
/// `[[#lib::util]]` are followed, and an unresolvable or ambiguous reference is no edge.
pub fn resolved_edges(index: &ResolvedIndex) -> Result<Vec<GraphEdge>, Value> {
    let rows = index.db.resolved_edges().map_err(|e| {
        ErrorEnvelope::error(
            ErrorCode::InternalError,
            format!("reading resolved edges failed: {}", e),
        )
    })?;
    Ok(rows
        .into_iter()
        .map(|(source, source_field, edge_type, target)| GraphEdge {
            source,
            target,
            edge_type,
            source_field,
        })
        .collect())
}

impl GraphView {
    /// Extract nodes and resolved edges from the index and index them by id. O(V + E).
    ///
    /// Fails with an `internal-error` envelope only if the index's in-memory database
    /// cannot be read.
    pub fn build(index: &ResolvedIndex) -> Result<Self, Value> {
        let nodes = extract_nodes(index);
        let edges = resolved_edges(index)?;

        let mut node_by_id: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
        for (i, node) in nodes.iter().enumerate() {
            node_by_id.entry(node.id.clone()).or_insert(i);
        }

        let mut outgoing: HashMap<String, Vec<usize>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, edge) in edges.iter().enumerate() {
            outgoing.entry(edge.source.clone()).or_default().push(i);
            incoming.entry(edge.target.clone()).or_default().push(i);
        }

        Ok(GraphView {
            nodes,
            edges,
            node_by_id,
            outgoing,
            incoming,
        })
    }

    /// The node with this id, if any.
    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.node_by_id.get(id).map(|&i| &self.nodes[i])
    }

    /// True when an object with this id exists.
    pub fn contains(&self, id: &str) -> bool {
        self.node_by_id.contains_key(id)
    }

    /// Edges adjacent to `id` in `direction`, optionally only those of `edge_type`, each with
    /// the neighbour at its other end. Ordered as in `edges`.
    pub fn adjacent<'a>(
        &'a self,
        id: &str,
        direction: Direction,
        edge_type: Option<&str>,
    ) -> Vec<(&'a GraphEdge, &'a str)> {
        const NONE: &[usize] = &[];
        let out = match direction {
            Direction::Outgoing | Direction::Both => {
                self.outgoing.get(id).map_or(NONE, |v| v.as_slice())
            }
            Direction::Incoming => NONE,
        };
        let inc = match direction {
            Direction::Incoming | Direction::Both => {
                self.incoming.get(id).map_or(NONE, |v| v.as_slice())
            }
            Direction::Outgoing => NONE,
        };

        // Merge the two ascending index lists; an index in both is a self-loop, kept once
        // and reported from the outgoing side (neighbour = target), as a full scan did.
        let mut result = Vec::with_capacity(out.len() + inc.len());
        let (mut i, mut j) = (0, 0);
        while i < out.len() || j < inc.len() {
            let (idx, outgoing_side) = match (out.get(i), inc.get(j)) {
                (Some(&o), Some(&n)) if o == n => {
                    i += 1;
                    j += 1;
                    (o, true)
                }
                (Some(&o), Some(&n)) if o < n => {
                    i += 1;
                    (o, true)
                }
                (Some(&o), None) => {
                    i += 1;
                    (o, true)
                }
                (_, Some(&n)) => {
                    j += 1;
                    (n, false)
                }
                (None, None) => unreachable!(),
            };
            let edge = &self.edges[idx];
            if edge_type.is_some_and(|et| edge.edge_type != et) {
                continue;
            }
            let neighbour = if outgoing_side {
                edge.target.as_str()
            } else {
                edge.source.as_str()
            };
            result.push((edge, neighbour));
        }
        result
    }
}
