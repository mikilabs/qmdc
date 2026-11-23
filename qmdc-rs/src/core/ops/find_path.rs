//! Core `find_path` operation — find the shortest path between two nodes.
//!
//! BFS from `from_id` to `to_id`, recording the path. Returns an ordered chain
//! of nodes and edges, or an explicit `NoPath` error if no path exists.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::graph::{Direction, GraphEdge, GraphNode, GraphView, PathChain};
use crate::core::resolved_index::ResolvedIndex;

/// Find the shortest path from `from_id` to `to_id` in the graph.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `from_id` — the source object ID. Leading `#` stripped.
/// * `to_id` — the target object ID. Leading `#` stripped.
/// * `edge_type` — optional: only traverse edges of this type (field name).
///
/// # Returns
/// - `Ok(Value)` — success envelope with the ordered path chain.
/// - `Err(Value)` — `not-found` for missing endpoints; `no-path` if unreachable.
pub fn find_path(
    index: &ResolvedIndex,
    from_id: &str,
    to_id: &str,
    edge_type: Option<&str>,
) -> Result<Value, Value> {
    let from_id = normalize_id(from_id);
    let to_id = normalize_id(to_id);

    // Validate inputs
    if from_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "from_id must not be empty",
        ));
    }
    if to_id.is_empty() {
        return Err(ErrorEnvelope::error(
            ErrorCode::InvalidArgument,
            "to_id must not be empty",
        ));
    }

    // Build graph data once: node lookup and neighbour enumeration are O(1) per step
    // (QMD-86, GitHub #17).
    let graph = GraphView::build(index);

    // Validate endpoints exist
    if !graph.contains(from_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", from_id),
        ));
    }
    if !graph.contains(to_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", to_id),
        ));
    }

    // Trivial case: same node
    if from_id == to_id {
        if let Some(node) = graph.node(from_id) {
            let chain = PathChain {
                nodes: vec![node.clone()],
                edges: vec![],
            };
            return Ok(ErrorEnvelope::success(chain.to_value()));
        }
    }

    // BFS: find shortest path (treating edges as undirected for path-finding)
    let mut visited: HashSet<&str> = HashSet::new();
    let mut predecessors: HashMap<&str, (&str, &GraphEdge)> = HashMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    visited.insert(from_id);
    queue.push_back(from_id);

    let mut found = false;

    while let Some(current) = queue.pop_front() {
        if current == to_id {
            found = true;
            break;
        }

        // Neighbors in both directions — path-finding is undirected
        for (edge, neighbor_id) in graph.adjacent(current, Direction::Both, edge_type) {
            if visited.insert(neighbor_id) {
                predecessors.insert(neighbor_id, (current, edge));
                queue.push_back(neighbor_id);
            }
        }
    }

    if !found {
        return Err(ErrorEnvelope::error(
            ErrorCode::NoPath,
            format!("no path exists between '{}' and '{}'", from_id, to_id),
        ));
    }

    // Reconstruct path from predecessors
    let mut path_node_ids: Vec<&str> = Vec::new();
    let mut path_edges: Vec<GraphEdge> = Vec::new();
    let mut current = to_id;

    while let Some(&(prev, edge)) = predecessors.get(current) {
        path_node_ids.push(current);
        path_edges.push(edge.clone());
        current = prev;
    }
    path_node_ids.push(from_id);

    // Reverse to get from→to order
    path_node_ids.reverse();
    path_edges.reverse();

    // Collect nodes in path order
    let path_nodes: Vec<GraphNode> = path_node_ids
        .iter()
        .filter_map(|id| graph.node(id).cloned())
        .collect();

    let chain = PathChain {
        nodes: path_nodes,
        edges: path_edges,
    };

    Ok(ErrorEnvelope::success(chain.to_value()))
}

/// Strip leading `#` from an id string, trim whitespace.
fn normalize_id(id: &str) -> &str {
    let trimmed = id.trim();
    trimmed.strip_prefix('#').unwrap_or(trimmed)
}
