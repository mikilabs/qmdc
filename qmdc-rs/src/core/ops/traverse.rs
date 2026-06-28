//! Core `traverse` operation — BFS graph traversal from a start node.
//!
//! Collects nodes and edges within direction/depth/edge-type bounds,
//! returning a subgraph wrapped in a BoundedEnvelope (NFR-4).

use std::collections::{HashSet, VecDeque};

use serde_json::Value;

use crate::core::envelope::DEFAULT_LIMIT;
use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::graph::{extract_edges, extract_nodes, GraphEdge, GraphNode, Subgraph};
use crate::core::resolved_index::ResolvedIndex;

/// Maximum traversal depth allowed.
const MAX_DEPTH: usize = 50;

/// Traverse the graph starting from `start_id`, collecting connected nodes/edges
/// within the specified direction, depth, and optional edge-type filter.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `start_id` — the object ID to start from. Leading `#` stripped.
/// * `edge_type` — optional: only follow edges of this type (field name).
/// * `direction` — one of `"outgoing"`, `"incoming"`, `"both"`.
/// * `depth` — maximum traversal depth (1..=MAX_DEPTH).
///
/// # Returns
/// - `Ok(Value)` — success envelope with subgraph nodes (bounded) + edges.
/// - `Err(Value)` — `invalid-argument` for bad direction/depth; `not-found` for missing start.
pub fn traverse(
    index: &ResolvedIndex,
    start_id: &str,
    edge_type: Option<&str>,
    direction: &str,
    depth: usize,
) -> Result<Value, Value> {
    let start_id = normalize_id(start_id);

    // Validate direction
    if !matches!(direction, "outgoing" | "incoming" | "both") {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            format!(
                "direction must be 'outgoing', 'incoming', or 'both'; got '{}'",
                direction
            ),
        ));
    }

    // Validate depth
    if depth == 0 || depth > MAX_DEPTH {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            format!("depth must be between 1 and {}; got {}", MAX_DEPTH, depth),
        ));
    }

    // Validate start_id
    if start_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "start_id must not be empty",
        ));
    }

    // Build graph data
    let all_nodes = extract_nodes(index);
    let all_edges = extract_edges(index);

    // Check start node exists
    let node_ids: HashSet<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();
    if !node_ids.contains(start_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", start_id),
        ));
    }

    // BFS
    let mut visited: HashSet<String> = HashSet::new();
    let mut result_nodes: Vec<GraphNode> = Vec::new();
    let mut result_edges: Vec<GraphEdge> = Vec::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    visited.insert(start_id.to_string());
    queue.push_back((start_id.to_string(), 0));

    while let Some((current_id, current_depth)) = queue.pop_front() {
        // Add node to results
        if let Some(node) = all_nodes.iter().find(|n| n.id == current_id) {
            result_nodes.push(node.clone());
        }

        // Stop expanding at max depth
        if current_depth >= depth {
            continue;
        }

        // Find adjacent edges based on direction
        let adjacent = find_adjacent_edges(&all_edges, &current_id, direction, edge_type);

        for (edge, neighbor_id) in adjacent {
            result_edges.push(edge.clone());

            if !visited.contains(&neighbor_id) {
                visited.insert(neighbor_id.clone());
                queue.push_back((neighbor_id, current_depth + 1));
            }
        }
    }

    let subgraph = Subgraph {
        nodes: result_nodes,
        edges: result_edges,
    };

    let payload = subgraph.to_bounded_value(DEFAULT_LIMIT);
    Ok(ErrorEnvelope::success(payload))
}

/// Find edges adjacent to `node_id` given direction and optional edge_type filter.
/// Returns tuples of (edge, neighbor_id).
fn find_adjacent_edges<'a>(
    all_edges: &'a [GraphEdge],
    node_id: &str,
    direction: &str,
    edge_type_filter: Option<&str>,
) -> Vec<(&'a GraphEdge, String)> {
    let mut results = Vec::new();

    for edge in all_edges {
        // Apply edge_type filter
        if let Some(et) = edge_type_filter {
            if edge.edge_type != et {
                continue;
            }
        }

        match direction {
            "outgoing" => {
                if edge.source == node_id {
                    results.push((edge, edge.target.clone()));
                }
            }
            "incoming" => {
                if edge.target == node_id {
                    results.push((edge, edge.source.clone()));
                }
            }
            "both" => {
                if edge.source == node_id {
                    results.push((edge, edge.target.clone()));
                } else if edge.target == node_id {
                    results.push((edge, edge.source.clone()));
                }
            }
            _ => {}
        }
    }

    results
}

/// Strip leading `#` from an id string, trim whitespace.
fn normalize_id(id: &str) -> &str {
    let trimmed = id.trim();
    trimmed.strip_prefix('#').unwrap_or(trimmed)
}
