//! Value parsing: YAML/JSON field values

use serde_json::{json, Value};

/// The largest integer magnitude QMD.md reads as a number: 2^53 - 1, the point up to which every
/// integer survives a round trip through an IEEE-754 double. An interoperable JSON reader promises
/// no more than that, and TypeScript cannot exceed it at all, so a longer literal stays a string.
const MAX_EXACT_INTEGER: u64 = 9_007_199_254_740_991;

/// The smallest non-zero decimal magnitude every host writes WITHOUT an exponent. QMD.md's numeric
/// grammar has no exponent form, so a smaller value could not be written back as a number at
/// all -- and the three hosts disagree on where they switch and how they pad the exponent.
const MIN_PLAIN_DECIMAL: f64 = 1e-4;

/// True when a value LOOKS like a number but QMD.md cannot carry it, so the caller reports
/// `unsupported_number_format` instead of letting it become a string in silence.
///
/// Two groups: a shape the format does not define (`1e5`, `.5`, `5.`, `+1`, `1_000`, `0x1f`), and a
/// shape it does define carrying a magnitude outside the range it can represent and write back.
/// Ordinary strings must never match -- `2026-09-24`, `12:30:00`, `1.0.2`, `1 000` are values, not
/// failed numbers. Mirrors `is_unsupported_number` in the Python and TypeScript parsers.
pub fn is_unsupported_number(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    if is_integer_or_decimal(trimmed) {
        // A supported shape, so only the magnitude bounds can reject it. Asking
        // `parse_supported_number` keeps those bounds in ONE place.
        return parse_supported_number(trimmed).is_none();
    }
    let body = trimmed
        .strip_prefix('-')
        .or_else(|| trimmed.strip_prefix('+'))
        .unwrap_or(trimmed);
    if body.is_empty() {
        return false;
    }
    // Digit separators: 1_000, 1_0.5
    if body.contains('_')
        && body.starts_with(|c: char| c.is_ascii_digit())
        && body
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'_' || b == b'.')
        && body.matches('.').count() <= 1
    {
        return true;
    }
    // Another base: 0x1f, 0o17, 0b1010
    if let Some(rest) = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .or_else(|| body.strip_prefix("0o"))
        .or_else(|| body.strip_prefix("0O"))
        .or_else(|| body.strip_prefix("0b"))
        .or_else(|| body.strip_prefix("0B"))
    {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit());
    }
    // A bare leading dot (.5) or trailing dot (5.), and exponent forms (1e5, 1.5e-3, 2E3).
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(idx) => (&body[..idx], Some(&body[idx + 1..])),
        None => (body, None),
    };
    if !is_decimal_shape(mantissa) {
        return false;
    }
    match exponent {
        None => {
            // No exponent, so the only unsupported shapes left are the dot ones and a unary plus.
            mantissa.starts_with('.') || mantissa.ends_with('.') || trimmed.starts_with('+')
        }
        Some(exp) => {
            let digits = exp.strip_prefix(['+', '-']).unwrap_or(exp);
            !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
        }
    }
}

/// `123`, `1.5`, `.5` or `5.` -- at least one digit, at most one dot, digits elsewhere.
fn is_decimal_shape(s: &str) -> bool {
    if s.is_empty() || s.matches('.').count() > 1 {
        return false;
    }
    s.bytes().any(|b| b.is_ascii_digit()) && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Parse a value as a number, or `None` when QMD.md cannot carry it. The single place the numeric
/// bounds live, so `is_unsupported_number` cannot drift from them.
///
/// The grammar is checked with hand-written byte inspection rather than the regex Python and
/// TypeScript use. Not because a regex would be recompiled per call — the crate's `OnceLock<Regex>`
/// idiom (see `utils.rs`, `graph.rs`, `workspace.rs`) would compile it once — but because even a
/// cached regex MATCH costs more than scanning a handful of ASCII bytes, and this runs once per
/// field of every object in the workspace. `is_integer_or_decimal` is the equivalent of
/// `^-?\d+(\.\d+)?$`.
fn parse_supported_number(trimmed: &str) -> Option<Value> {
    if !is_integer_or_decimal(trimmed) {
        return None;
    }
    if trimmed.contains('.') {
        // The upper bound applies to a decimal too, and it earns its place twice over: beyond it the
        // three JSON writers disagree on the SPELLING of the same double (Rust and Python reach for
        // exponent form where JavaScript prints all the digits), so stopping here removes that whole
        // class rather than chasing it. The lower bound is the same argument from the other end.
        let f = trimmed.parse::<f64>().ok()?;
        if !f.is_finite() || f.abs() > MAX_EXACT_INTEGER as f64 {
            return None;
        }
        if f != 0.0 && f.abs() < MIN_PLAIN_DECIMAL {
            return None;
        }
        Some(json!(f))
    } else {
        // An integer is a number only while it survives a round trip through a double, which is all
        // an interoperable JSON reader promises. A parse failure here means the literal is past i64
        // too, which is well past this bound.
        let n = trimmed.parse::<i64>().ok()?;
        if n.unsigned_abs() <= MAX_EXACT_INTEGER {
            Some(json!(n))
        } else {
            None
        }
    }
}

/// QMD.md's numeric grammar: an optional `-`, one or more digits, and optionally a `.` followed by
/// one or more digits. Equivalent to `^-?\d+(\.\d+)?$` in the Python and TypeScript parsers.
///
/// Deliberately REJECTS forms the host languages would otherwise accept: `1e5`, `1.5e-3`, `.5`,
/// `5.`, `+1`, `1_000`, `0x10`, `Infinity`, `NaN`. Those are strings.
fn is_integer_or_decimal(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let mut parts = body.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        None => true,
        // A second `.` leaves a non-digit in `frac`, so `1.2.3` is rejected here.
        Some(frac) => !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()),
    }
}

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

    // QMD-71: `null` only. A bare `~` is YAML's null, and QMD.md is deliberately NOT YAML --
    // Python and TypeScript both read it as the one-character string it looks like.
    if trimmed == "null" {
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

    // QMD-71: an integer or a decimal only. `parse::<i64>` accepts a unary plus and
    // `parse::<f64>` accepts exponents and a bare leading or trailing dot, none of which QMD.md
    // defines -- so the grammar is checked FIRST and the parse only runs on a literal that matches.
    // The same grammar lives in Python and TypeScript as the regex `^-?\d+(\.\d+)?$`; see
    // `parse_supported_number` for why this one is hand-written instead.
    if let Some(number) = parse_supported_number(trimmed) {
        return (number, "number");
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
        // QMD-71: a bare `~` is YAML's null, and QMD.md is deliberately not YAML. This assertion
        // used to require `Value::Null`, which is what made Rust disagree with the other two.
        assert_eq!(parse_field_value("~").0, json!("~"));
        assert_eq!(parse_field_value("~").1, "string");
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
