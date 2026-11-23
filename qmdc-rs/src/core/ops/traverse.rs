//! Core `traverse` operation — BFS graph traversal from a start node.
//!
//! Collects nodes and edges within direction/depth/edge-type bounds,
//! returning a subgraph wrapped in a BoundedEnvelope (NFR-4).

use std::collections::{HashSet, VecDeque};

use serde_json::Value;

use crate::core::envelope::DEFAULT_LIMIT;
use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::graph::{Direction, GraphEdge, GraphNode, GraphView, Subgraph};
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

    // Build graph data once: node lookup and neighbour enumeration are O(1) per step
    // (QMD-86, GitHub #17).
    let graph = GraphView::build(index);
    let direction = match direction {
        "outgoing" => Direction::Outgoing,
        "incoming" => Direction::Incoming,
        _ => Direction::Both,
    };

    // Check start node exists
    if !graph.contains(start_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", start_id),
        ));
    }

    // BFS
    let mut visited: HashSet<&str> = HashSet::new();
    let mut result_nodes: Vec<GraphNode> = Vec::new();
    let mut result_edges: Vec<GraphEdge> = Vec::new();
    let mut queue: VecDeque<(&str, usize)> = VecDeque::new();

    visited.insert(start_id);
    queue.push_back((start_id, 0));

    while let Some((current_id, current_depth)) = queue.pop_front() {
        // Add node to results (a dangling reference target has no node)
        if let Some(node) = graph.node(current_id) {
            result_nodes.push(node.clone());
        }

        // Stop expanding at max depth
        if current_depth >= depth {
            continue;
        }

        for (edge, neighbor_id) in graph.adjacent(current_id, direction, edge_type) {
            result_edges.push(edge.clone());

            if visited.insert(neighbor_id) {
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

/// Strip leading `#` from an id string, trim whitespace.
fn normalize_id(id: &str) -> &str {
    let trimmed = id.trim();
    trimmed.strip_prefix('#').unwrap_or(trimmed)
}
