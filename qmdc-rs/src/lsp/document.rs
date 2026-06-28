use std::collections::HashMap;

/// Reference info extracted from parsed objects
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedReference {
    pub target: String,
    pub ref_type: String, // "local", "hash_local", "kind", "namespace", "crossfile"
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub content: String,
    pub objects: Vec<serde_json::Value>,
    /// All references extracted from __references fields
    pub references: Vec<ParsedReference>,
    /// Map from id -> object index
    pub id_to_object: HashMap<String, usize>,
    /// Which workspace this document belongs to (if any)
    #[allow(dead_code)]
    pub workspace_id: Option<String>,
}
