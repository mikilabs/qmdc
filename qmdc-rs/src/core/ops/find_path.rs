//! Core `find_path` operation — find the shortest path between two nodes.
//!
//! BFS from `from_id` to `to_id`, recording the path. Returns an ordered chain
//! of nodes and edges, or an explicit `NoPath` error if no path exists.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use crate::core::error::{ErrorCode, ErrorEnvelope};
use crate::core::graph::{extract_edges, extract_nodes, GraphEdge, GraphNode, PathChain};
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

    // Build graph data
    let all_nodes = extract_nodes(index);
    let all_edges = extract_edges(index);

    let node_ids: HashSet<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();

    // Validate endpoints exist
    if !node_ids.contains(from_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", from_id),
        ));
    }
    if !node_ids.contains(to_id) {
        return Err(ErrorEnvelope::error(
            ErrorCode::NotFound,
            format!("no object with id '{}' found in workspace", to_id),
        ));
    }

    // Trivial case: same node
    if from_id == to_id {
        if let Some(node) = all_nodes.iter().find(|n| n.id == from_id) {
            let chain = PathChain {
                nodes: vec![node.clone()],
                edges: vec![],
            };
            return Ok(ErrorEnvelope::success(chain.to_value()));
        }
    }

    // BFS: find shortest path (treating edges as undirected for path-finding)
    let mut visited: HashSet<String> = HashSet::new();
    let mut predecessors: HashMap<String, (String, GraphEdge)> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    visited.insert(from_id.to_string());
    queue.push_back(from_id.to_string());

    let mut found = false;

    while let Some(current) = queue.pop_front() {
        if current == to_id {
            found = true;
            break;
        }

        // Find neighbors (both directions — path-finding is undirected)
        for edge in &all_edges {
            // Apply edge_type filter
            if let Some(et) = edge_type {
                if edge.edge_type != et {
                    continue;
                }
            }

            let neighbor = if edge.source == current {
                Some(&edge.target)
            } else if edge.target == current {
                Some(&edge.source)
            } else {
                None
            };

            if let Some(neighbor_id) = neighbor {
                if !visited.contains(neighbor_id.as_str()) {
                    visited.insert(neighbor_id.clone());
                    predecessors.insert(neighbor_id.clone(), (current.clone(), edge.clone()));
                    queue.push_back(neighbor_id.clone());
                }
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
    let mut path_node_ids: Vec<String> = Vec::new();
    let mut path_edges: Vec<GraphEdge> = Vec::new();
    let mut current = to_id.to_string();

    while let Some((prev, edge)) = predecessors.get(&current) {
        path_node_ids.push(current.clone());
        path_edges.push(edge.clone());
        current = prev.clone();
    }
    path_node_ids.push(from_id.to_string());

    // Reverse to get from→to order
    path_node_ids.reverse();
    path_edges.reverse();

    // Collect nodes in path order
    let path_nodes: Vec<GraphNode> = path_node_ids
        .iter()
        .filter_map(|id| all_nodes.iter().find(|n| n.id == *id).cloned())
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
