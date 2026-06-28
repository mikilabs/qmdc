//! Output formatting: OutputFormat enum and JSON building

use indexmap::IndexMap;
use serde_json::{json, Value};

/// Output format for parsed QMDC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Minimal output: only __id (if explicit), __kind (if not __Object), and data fields
    Minimal,
    /// Standard output: all metadata except __line, __references, __positions
    #[default]
    Standard,
    /// Full output: all metadata including __line, __references, __positions
    Full,
}

/// Build JSON object from internal map representation
pub fn build_from_map(obj_map: &IndexMap<String, Value>, format: OutputFormat) -> Value {
    match format {
        OutputFormat::Minimal => {
            let mut result = IndexMap::new();

            // In minimal: include __id if explicit, __kind if not __Object
            let has_explicit_id = !obj_map
                .get("__has_explicit_id")
                .and_then(|v| v.as_bool())
                .map(|b| !b) // __has_explicit_id: false means NOT explicit
                .unwrap_or(false); // if field missing, assume explicit

            if has_explicit_id {
                if let Some(v) = obj_map.get("__id") {
                    result.insert("__id".to_string(), v.clone());
                }
            }

            if let Some(v) = obj_map.get("__local_id") {
                result.insert("__local_id".to_string(), v.clone());
            }

            // Include __kind if it's not __Object and not system type
            if let Some(kind) = obj_map.get("__kind").and_then(|v| v.as_str()) {
                if kind != "__Object" && !kind.starts_with("__") {
                    result.insert("__kind".to_string(), json!(kind));
                }
            }

            // Data fields
            for (k, v) in obj_map {
                if !k.starts_with("__") {
                    result.insert(k.clone(), v.clone());
                }
            }

            // Include __labels if present (needed for rebuild)
            if let Some(v) = obj_map.get("__labels") {
                result.insert("__labels".to_string(), v.clone());
            }

            json!(result)
        }
        OutputFormat::Standard => {
            let mut result = IndexMap::new();

            // Order per expected: __id, __local_id, __label, __kind, __container, __parent, __parent_field, __comments, data, __types, __syntax, __level, __has_explicit_id
            if let Some(v) = obj_map.get("__id") {
                result.insert("__id".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__local_id") {
                result.insert("__local_id".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__label") {
                result.insert("__label".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__kind") {
                result.insert("__kind".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__container") {
                result.insert("__container".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__parent") {
                result.insert("__parent".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__parent_field") {
                result.insert("__parent_field".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__comments") {
                result.insert("__comments".to_string(), v.clone());
            }

            // Data fields
            for (k, v) in obj_map {
                if !k.starts_with("__") {
                    result.insert(k.clone(), v.clone());
                }
            }

            if let Some(v) = obj_map.get("__types") {
                result.insert("__types".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__syntax") {
                result.insert("__syntax".to_string(), v.clone());
            }

            // __level before __labels (only for non-array-element objects)
            let is_array_element = obj_map
                .get("__is_array_element")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_array_element {
                if let Some(v) = obj_map.get("__level") {
                    result.insert("__level".to_string(), v.clone());
                }
            }

            if let Some(v) = obj_map.get("__has_explicit_id") {
                result.insert("__has_explicit_id".to_string(), v.clone());
            }

            if let Some(v) = obj_map.get("__labels") {
                result.insert("__labels".to_string(), v.clone());
            }

            json!(result)
        }
        OutputFormat::Full => {
            let mut result = IndexMap::new();

            if let Some(v) = obj_map.get("__id") {
                result.insert("__id".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__local_id") {
                result.insert("__local_id".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__label") {
                result.insert("__label".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__kind") {
                result.insert("__kind".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__container") {
                result.insert("__container".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__parent") {
                result.insert("__parent".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__parent_field") {
                result.insert("__parent_field".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__comments") {
                result.insert("__comments".to_string(), v.clone());
            }

            for (k, v) in obj_map {
                if !k.starts_with("__") {
                    result.insert(k.clone(), v.clone());
                }
            }

            if let Some(v) = obj_map.get("__types") {
                result.insert("__types".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__syntax") {
                result.insert("__syntax".to_string(), v.clone());
            }

            // __level (only for non-array-element objects)
            let is_array_element = obj_map
                .get("__is_array_element")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !is_array_element {
                if let Some(v) = obj_map.get("__level") {
                    result.insert("__level".to_string(), v.clone());
                }
            }

            if let Some(v) = obj_map.get("__line") {
                result.insert("__line".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__has_explicit_id") {
                result.insert("__has_explicit_id".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__references") {
                result.insert("__references".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__positions") {
                result.insert("__positions".to_string(), v.clone());
            }
            if let Some(v) = obj_map.get("__labels") {
                result.insert("__labels".to_string(), v.clone());
            }

            json!(result)
        }
    }
}
