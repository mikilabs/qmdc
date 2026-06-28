//! Header parsing: [[id:Kind]] extraction from headings

use super::utils::{re_double_brackets, snake_case, SimpleRng};
use regex::Regex;

/// Header parsing result
#[derive(Debug, Clone, Default)]
pub struct HeaderResult {
    pub id: String,
    pub label: String,
    pub kind: Option<String>,
    pub field_type: Option<String>, // "array", "text", "yaml", "json", "object_array"
    pub array_kind: Option<String>, // Kind for [[field: [Kind]]]
    pub has_explicit_id: bool,
    pub multiple_definitions: Option<Vec<String>>, // Raw [[...]] strings when 2+ definitions
}

/// Parse header text to extract id, label, kind
pub fn parse_header(text: &str, rng: &mut SimpleRng) -> HeaderResult {
    let content = text.trim();

    // Pattern: [[...]] with balanced matching for nested brackets
    let bracket_re = re_double_brackets();

    // Strip backtick-escaped content before matching [[...]] patterns
    // so that `[[id]]` inside backticks is not treated as a definition
    let backtick_re = Regex::new(r"`[^`]+`").unwrap();
    let search_content = backtick_re.replace_all(content, |m: &regex::Captures| {
        " ".repeat(m.get(0).unwrap().as_str().len())
    });
    let matches: Vec<_> = bracket_re.find_iter(&search_content).collect();

    let mut result = HeaderResult::default();

    if !matches.is_empty() {
        // Detect multiple definitions
        if matches.len() > 1 {
            result.multiple_definitions = Some(
                matches
                    .iter()
                    .map(|m| content[m.start()..m.end()].to_string())
                    .collect(),
            );
        }

        // Remove all [[...]] from content to get label (use positions from search_content on original)
        let mut label = content.to_string();
        for m in matches.iter().rev() {
            label = format!("{}{}", &label[..m.start()], &label[m.end()..]);
        }
        // Clean up multiple spaces and trim
        let label: String = label.split_whitespace().collect::<Vec<_>>().join(" ");
        result.label = label.clone();

        // Parse first [[...]] — extract bracket content from original content using positions
        let first_match = &matches[0];
        let bracket_content = content[first_match.start() + 2..first_match.end() - 2].trim();

        if bracket_content.contains(':') {
            let parts: Vec<&str> = bracket_content.splitn(2, ':').collect();
            let left = parts[0].trim();
            let right = parts.get(1).map(|s| s.trim()).unwrap_or("");

            let right_lower = right.to_lowercase();

            if right_lower == "array" {
                // [[field: array]] - primitive array
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("array".to_string());
                result.has_explicit_id = true;
            } else if right_lower == "yaml" {
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("yaml".to_string());
                result.has_explicit_id = true;
            } else if right_lower == "json" {
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("json".to_string());
                result.has_explicit_id = true;
            } else if right_lower == "text" {
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("text".to_string());
                result.has_explicit_id = true;
            } else if right_lower == "map" {
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("map".to_string());
                result.has_explicit_id = true;
            } else if right.starts_with('[') && right.ends_with(']') {
                // [[field: [Kind]]] - object array
                let kind_name = right[1..right.len() - 1].trim();
                result.id = if left.is_empty() {
                    rng.gen_id()
                } else {
                    left.to_string()
                };
                result.field_type = Some("object_array".to_string());
                result.array_kind = Some(kind_name.to_string());
                result.has_explicit_id = true;
            } else if !left.is_empty() {
                // [[id: Kind]]
                result.id = left.to_string();
                result.kind = Some(right.to_string());
                result.has_explicit_id = true;
            } else {
                // [[:Kind]]
                result.kind = Some(right.to_string());
                result.id = if !label.is_empty() {
                    snake_case(&label, rng)
                } else {
                    rng.gen_id()
                };
                result.has_explicit_id = true;
            }
        } else {
            // [[id]]
            result.id = if bracket_content.is_empty() {
                rng.gen_id()
            } else {
                bracket_content.to_string()
            };
            result.has_explicit_id = true;
        }
    } else {
        // No [[...]], just plain text
        result.label = content.to_string();
        result.id = snake_case(content, rng);
        result.has_explicit_id = false;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_with_id_and_kind() {
        let mut rng = SimpleRng::new(42);
        let result = parse_header("User [[user:Person]]", &mut rng);
        assert_eq!(result.id, "user");
        assert_eq!(result.kind, Some("Person".to_string()));
        assert_eq!(result.label, "User");
        assert!(result.has_explicit_id);
    }

    #[test]
    fn test_parse_header_id_only() {
        let mut rng = SimpleRng::new(42);
        let result = parse_header("Task [[task1]]", &mut rng);
        assert_eq!(result.id, "task1");
        assert_eq!(result.kind, None);
        assert_eq!(result.label, "Task");
        assert!(result.has_explicit_id);
    }

    #[test]
    fn test_parse_header_no_brackets() {
        let mut rng = SimpleRng::new(42);
        let result = parse_header("Simple Title", &mut rng);
        assert_eq!(result.id, "simple_title");
        assert_eq!(result.label, "Simple Title");
        assert!(!result.has_explicit_id);
    }

    #[test]
    fn test_parse_header_field_types() {
        let mut rng = SimpleRng::new(42);

        let result = parse_header("[[items: array]]", &mut rng);
        assert_eq!(result.id, "items");
        assert_eq!(result.field_type, Some("array".to_string()));

        let result = parse_header("[[content: text]]", &mut rng);
        assert_eq!(result.id, "content");
        assert_eq!(result.field_type, Some("text".to_string()));

        let result = parse_header("[[tasks: [Task]]]", &mut rng);
        assert_eq!(result.id, "tasks");
        assert_eq!(result.field_type, Some("object_array".to_string()));
        assert_eq!(result.array_kind, Some("Task".to_string()));
    }
}
