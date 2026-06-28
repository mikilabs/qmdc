//! Core `describe_metamodel` operation — aggregate kind/edge statistics.
//!
//! Returns kinds present, per-kind object counts, fields-per-kind, edge-type density;
//! optionally scoped by namespace.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::core::error::ErrorEnvelope;
use crate::core::fields::QmdcObject;
use crate::core::graph::{extract_edges, EdgeTypeSummary, KindSummary, MetamodelInventory};
use crate::core::resolved_index::ResolvedIndex;

/// Describe the metamodel of the workspace — kinds, fields, edge densities.
///
/// # Arguments
/// * `index` — the resolved workspace index.
/// * `namespace` — optional: scope to objects in this namespace only.
///
/// # Returns
/// - `Ok(Value)` — success envelope with metamodel inventory.
pub fn describe_metamodel(index: &ResolvedIndex, namespace: Option<&str>) -> Result<Value, Value> {
    // Aggregate objects by kind
    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut kind_fields: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut total_objects: usize = 0;

    for obj in index.objects() {
        // Apply namespace filter
        if let Some(ns) = namespace {
            let obj_ns = obj.namespace();
            if obj_ns != ns {
                continue;
            }
        }

        let kind = obj
            .get("__kind")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        total_objects += 1;
        *kind_counts.entry(kind.clone()).or_insert(0) += 1;

        // Collect non-__ field names
        if let Some(map) = obj.as_object() {
            let fields = kind_fields.entry(kind).or_default();
            for field_name in map.keys() {
                if !field_name.starts_with("__") {
                    fields.insert(field_name.clone());
                }
            }
        }
    }

    // Build kind summaries (sorted by kind name)
    let kinds: Vec<KindSummary> = kind_counts
        .iter()
        .map(|(kind, &count)| {
            let mut fields: Vec<String> = kind_fields
                .get(kind)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            fields.sort();
            KindSummary {
                kind: kind.clone(),
                count,
                fields,
            }
        })
        .collect();

    // Count edges by type, respecting namespace filter
    let all_edges = extract_edges(index);
    let mut edge_type_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_edges: usize = 0;

    // Build a set of valid object IDs in scope (for namespace filtering of edges)
    let in_scope_ids: BTreeSet<String> = index
        .objects()
        .iter()
        .filter(|obj| {
            if let Some(ns) = namespace {
                let obj_ns = obj.namespace();
                obj_ns == ns
            } else {
                true
            }
        })
        .filter_map(|obj| {
            obj.get("__id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    for edge in &all_edges {
        // Only count edges where source is in scope
        if in_scope_ids.contains(&edge.source) {
            total_edges += 1;
            *edge_type_counts.entry(edge.edge_type.clone()).or_insert(0) += 1;
        }
    }

    // Build edge type summaries (sorted by edge_type)
    let edge_types: Vec<EdgeTypeSummary> = edge_type_counts
        .iter()
        .map(|(et, &count)| EdgeTypeSummary {
            edge_type: et.clone(),
            count,
        })
        .collect();

    let inventory = MetamodelInventory {
        kinds,
        edge_types,
        total_objects,
        total_edges,
    };

    Ok(ErrorEnvelope::success(inventory.to_value()))
}
