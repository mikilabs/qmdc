//! Reference handling: [[#ref]] extraction and classification

use super::utils::re_double_brackets;

/// Reference found in QMD.md content
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reference {
    pub target: String, // The reference target (id, Kind.id, ns.id, etc.)
    #[serde(rename = "type")]
    pub ref_type: String, // "local", "hash_local", "kind", "namespace", "crossfile"
    pub line: u32,      // 1-based line number
    pub start_col: u32, // 0-based start column
    pub end_col: u32,   // 0-based end column (exclusive)
    pub raw: String,    // Original text including [[...]]
}

/// Classify reference type based on content
pub fn classify_reference(inner: &str) -> &'static str {
    // Handle # prefix
    let content = inner.strip_prefix('#').unwrap_or(inner);

    // Check for crossfile references (contain / or # in middle)
    if content.contains('/') || (inner.len() > 1 && content.contains('#')) {
        return "crossfile";
    }

    // Check for Kind:id or Kind.id format
    if content.contains(':') || content.contains('.') {
        let sep = if content.contains(':') { ':' } else { '.' };
        let parts: Vec<&str> = content.splitn(2, sep).collect();
        if parts.len() == 2 {
            let first = parts[0];
            // If first char is uppercase, assume Kind
            if first
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                return "kind";
            } else {
                return "namespace";
            }
        }
    }

    if inner.starts_with('#') {
        "hash_local"
    } else {
        "local"
    }
}

/// Check if position is inside backticks (inline code)
fn is_inside_backticks(line: &str, pos: usize) -> bool {
    let bytes = line.as_bytes();
    let mut in_backtick = false;
    let mut i = 0;

    while i < bytes.len() && i < pos {
        if bytes[i] == b'`' {
            // Check for triple backticks (code fence) - skip entire line
            if i + 2 < bytes.len() && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                return true; // Treat entire line as code
            }
            in_backtick = !in_backtick;
        }
        i += 1;
    }

    in_backtick
}

/// Extract all [[...]] references from a line with their positions
pub fn extract_references_from_line(line: &str, line_number: u32) -> Vec<Reference> {
    let mut refs = Vec::new();

    for caps in re_double_brackets().captures_iter(line) {
        if let (Some(full_match), Some(inner_match)) = (caps.get(0), caps.get(1)) {
            let inner = inner_match.as_str().trim();
            let raw = full_match.as_str().to_string();
            // Convert byte positions to UTF-16 code unit offsets (for LSP compatibility)
            let start_col = line[..full_match.start()]
                .chars()
                .map(|ch| ch.len_utf16() as u32)
                .sum::<u32>();
            let end_col = line[..full_match.end()]
                .chars()
                .map(|ch| ch.len_utf16() as u32)
                .sum::<u32>();

            // Skip references inside backticks (inline code)
            if is_inside_backticks(line, full_match.start()) {
                continue;
            }

            // Only references start with '#'
            // [[#id]], [[#ns:id]], [[#Kind.field]] - references
            // [[id]], [[id:Kind]], [[field:text]] - definitions (skip)
            if !inner.starts_with('#') {
                continue;
            }

            let ref_type = classify_reference(inner);

            refs.push(Reference {
                target: inner.to_string(),
                ref_type: ref_type.to_string(),
                line: line_number,
                start_col,
                end_col,
                raw,
            });
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_reference() {
        assert_eq!(classify_reference("#local_id"), "hash_local");
        assert_eq!(classify_reference("local_id"), "local");
        assert_eq!(classify_reference("#Kind.field"), "kind");
        assert_eq!(classify_reference("#ns:id"), "namespace");
        assert_eq!(classify_reference("#file/path#id"), "crossfile");
    }

    #[test]
    fn test_extract_references() {
        let refs = extract_references_from_line("See [[#task1]] and [[#User.name]]", 1);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].target, "#task1");
        assert_eq!(refs[0].ref_type, "hash_local");
        assert_eq!(refs[1].target, "#User.name");
        assert_eq!(refs[1].ref_type, "kind");
    }

    #[test]
    fn test_skip_definitions() {
        // Definitions (without #) should be skipped
        let refs = extract_references_from_line("## Task [[task1:Task]]", 1);
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_skip_backtick_refs() {
        let refs = extract_references_from_line("Use `[[#code]]` for inline", 1);
        assert_eq!(refs.len(), 0);
    }
}
