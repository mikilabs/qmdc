//! Value parsing: YAML/JSON field values

use serde_json::{json, Value};

/// Parse field value to appropriate JSON type
pub fn parse_field_value(s: &str) -> (Value, &'static str) {
    let trimmed = s.trim();

    if trimmed.is_empty() {
        return (Value::Null, "null");
    }

    if trimmed == "true" {
        return (json!(true), "boolean");
    }
    if trimmed == "false" {
        return (json!(false), "boolean");
    }

    if trimmed == "null" || trimmed == "~" {
        return (Value::Null, "null");
    }

    // Check for multiple references: [[#a]], [[#b]], [[#c]]
    // This is NOT a YAML array (which would be [[[#a]], [[#b]]])
    // Pattern: starts with [[, contains ]], [[ but NOT starts with [[[
    if trimmed.starts_with("[[") && !trimmed.starts_with("[[[") && trimmed.contains("]], [[") {
        let items: Vec<Value> = trimmed
            .split("]], [[")
            .enumerate()
            .map(|(i, s)| {
                let s = s.trim();
                // First item needs ]] added, last needs [[ added, middle needs both
                let item = if i == 0 {
                    format!("{}]]", s)
                } else if !s.ends_with("]]") {
                    format!("[[{}]]", s)
                } else {
                    format!("[[{}", s)
                };
                json!(item)
            })
            .collect();
        return (json!(items), "ref_array");
    }

    // Check for YAML array [a, b, c] but NOT single references [[#...]]
    // Array syntax: [item1, item2, ...] where first char after [ is not [
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        // A value starting with `[[` (but not `[[[`) is a single reference, e.g.
        // `[[#id]]` or `[[#id]][]` (ref with a trailing array marker). It is kept
        // as a string, NOT parsed as a YAML array. This matches Python/TS:
        // `is_single_ref = starts_with("[[") && !starts_with("[[[")`.
        // Arrays of refs use `[[[#a]], [[#b]]]` (triple `[`), which is not a single ref.
        let is_single_ref = trimmed.starts_with("[[") && !trimmed.starts_with("[[[");
        if is_single_ref {
            return (json!(trimmed), "string");
        }
        // It's an array
        let inner = &trimmed[1..trimmed.len() - 1];
        let items: Vec<Value> = parse_yaml_array_items(inner);
        return (json!(items), "array");
    }

    // Try parse as integer
    if let Ok(n) = trimmed.parse::<i64>() {
        return (json!(n), "number");
    }

    // Try parse as float
    if let Ok(f) = trimmed.parse::<f64>() {
        if f.is_finite() {
            return (json!(f), "number");
        }
    }

    // Handle quoted strings - strip quotes
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        let unquoted = &trimmed[1..trimmed.len() - 1];
        return (json!(unquoted), "string");
    }

    (json!(trimmed), "string")
}

/// Parse YAML array items like [a, b, c] or ["hello world", 42, true]
/// Also handles [[#ref1]], [[#ref2]] arrays
pub fn parse_yaml_array_items(inner: &str) -> Vec<Value> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut bracket_depth = 0;

    for c in inner.chars() {
        if !in_quotes && (c == '"' || c == '\'') {
            in_quotes = true;
            quote_char = c;
            current.push(c);
        } else if in_quotes && c == quote_char {
            in_quotes = false;
            current.push(c);
        } else if !in_quotes && c == '[' {
            bracket_depth += 1;
            current.push(c);
        } else if !in_quotes && c == ']' {
            bracket_depth -= 1;
            current.push(c);
        } else if !in_quotes && c == ',' && bracket_depth == 0 {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                items.push(parse_array_item(trimmed));
            }
            current.clear();
        } else {
            current.push(c);
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        items.push(parse_array_item(trimmed));
    }

    items
}

/// Parse a single array item, preserving [[#ref]] as strings
pub fn parse_array_item(s: &str) -> Value {
    let trimmed = s.trim();

    // Check for reference [[#...]]
    if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
        return json!(trimmed);
    }

    // Otherwise use normal parsing
    parse_field_value(trimmed).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_primitives() {
        assert_eq!(parse_field_value("true").0, json!(true));
        assert_eq!(parse_field_value("false").0, json!(false));
        assert_eq!(parse_field_value("null").0, Value::Null);
        assert_eq!(parse_field_value("~").0, Value::Null);
        assert_eq!(parse_field_value("42").0, json!(42));
        assert_eq!(parse_field_value("3.15").0, json!(3.15));
        assert_eq!(parse_field_value("hello").0, json!("hello"));
    }

    #[test]
    fn test_parse_quoted_strings() {
        assert_eq!(parse_field_value("\"hello world\"").0, json!("hello world"));
        assert_eq!(
            parse_field_value("'single quotes'").0,
            json!("single quotes")
        );
    }

    #[test]
    fn test_parse_arrays() {
        assert_eq!(parse_field_value("[a, b, c]").0, json!(["a", "b", "c"]));
        assert_eq!(parse_field_value("[1, 2, 3]").0, json!([1, 2, 3]));
        assert_eq!(parse_field_value("[true, false]").0, json!([true, false]));
    }

    #[test]
    fn test_parse_reference_array() {
        // Single reference is a string
        assert_eq!(parse_field_value("[[#task1]]").0, json!("[[#task1]]"));

        // Multiple references are an array
        let result = parse_field_value("[[#a]], [[#b]]").0;
        assert!(result.is_array());
    }
}
