use indexmap::IndexMap;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser as MdParser, Tag, TagEnd};
use regex::Regex;
use serde_json::{json, Value};

// Import utilities from parser_modules
use crate::parser_modules::{
    build_block_tree_from_events, build_from_map, extract_references_from_line, parse_field_value,
    parse_header, re_double_brackets, re_field_check, re_field_kv, Reference, SimpleRng,
};

// Re-export OutputFormat for backward compatibility
pub use crate::parser_modules::OutputFormat;

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub random_seed: Option<u64>,
    pub format: OutputFormat,
}

pub type QmdcObject = IndexMap<String, Value>;

/// Code fence metadata for __TextBlock
#[derive(Debug, Clone)]
struct CodeFenceInfo {
    lang: String,
    offset_line: usize,  // 0-based line offset within content
    length_lines: usize, // number of lines including ``` markers
}

/// Current object being built during parsing
struct CurrentObject {
    id: String,
    local_id: Option<String>, // Set when id is hierarchical (child of non-system parent)
    label: String,
    kind: Option<String>,
    level: u8,
    line: u32,
    fields: IndexMap<String, Value>,
    types: IndexMap<String, String>,
    syntax: IndexMap<String, String>,
    comments: Vec<IndexMap<String, String>>,
    has_explicit_id: bool,
    parent: Option<String>,
    parent_field: Option<String>,
    comment_anchor: String,
    is_array_element: bool, // True if this is an element of [[field: [Kind]]] array
    references: Vec<Reference>, // All [[...]] references in this object's content
    positions: IndexMap<String, (u32, u32)>, // field_name -> (line, col) for LSP
}

fn heading_level_to_u8(level: &HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Resolve the composed ID for a child object.
/// Returns (composed_id, Option<local_id>).
/// - If parent is a system container (__Workspace/__Namespace): returns (local_id, None)
/// - Otherwise: returns (parent_full_id.arr_field.local_id, Some(local_id)) for array elements
///   or (parent_full_id.local_id, Some(local_id)) for single children
///
/// When arr_field contains a dot (dot-ID), it's used directly as the path prefix.
/// When arr_field equals parent's __id, the field is the parent itself (top-level array).
fn resolve_child_id(
    objects_map: &IndexMap<String, IndexMap<String, Value>>,
    parent_id: &str,
    local_id: &str,
    arr_field: Option<&str>,
) -> (String, Option<String>) {
    let parent_kind = objects_map
        .get(parent_id)
        .and_then(|m| m.get("__kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if parent_kind == "__Workspace" || parent_kind == "__Namespace" {
        return (local_id.to_string(), None);
    }
    let parent_full_id = objects_map
        .get(parent_id)
        .and_then(|m| m.get("__id"))
        .and_then(|v| v.as_str())
        .unwrap_or(parent_id);
    let composed = if let Some(field) = arr_field {
        if field.contains('.') {
            // Dot-ID in array field: use dot-ID directly as prefix
            format!("{}.{}", field, local_id)
        } else if field == parent_full_id {
            // Top-level array: parent IS the array, skip extra field name
            format!("{}.{}", parent_full_id, local_id)
        } else {
            format!("{}.{}.{}", parent_full_id, field, local_id)
        }
    } else {
        format!("{}.{}", parent_full_id, local_id)
    };
    (composed, Some(local_id.to_string()))
}

/// Extract a markdown table (or any block) verbatim from the source by byte
/// range, trimmed. Used for tables in text fields / comments / text blocks so
/// the original separators and cell spacing are preserved (reconstructing the
/// table would normalize the separator row and diverge from Python/TS).
/// Panic-safe: `str::get` returns `None` for out-of-range or non-char-boundary
/// indices, and the table range endpoints land on ASCII `|`/`\n`.
fn raw_table_slice(source: &str, start: usize, end: usize) -> String {
    source.get(start..end).unwrap_or("").trim().to_string()
}

/// Create child objects from a markdown table inside an object array context.
/// Returns the created objects as (id, element) pairs.
fn create_table_child_objects(
    table_rows: &[Vec<String>],
    arr_parent_id: &str,
    arr_field: &str,
    arr_kind: &str,
    objects_map: &IndexMap<String, IndexMap<String, Value>>,
) -> Vec<(String, IndexMap<String, Value>)> {
    if table_rows.is_empty() {
        return Vec::new();
    }

    let column_names = &table_rows[0];
    let data_rows = &table_rows[1..];
    let mut result = Vec::new();

    // Get parent's full ID for hierarchical composition
    let parent_full_id = objects_map
        .get(arr_parent_id)
        .and_then(|m| m.get("__id"))
        .and_then(|v| v.as_str())
        .unwrap_or(arr_parent_id);

    // Check if parent is a system container
    let parent_kind = objects_map
        .get(arr_parent_id)
        .and_then(|m| m.get("__kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_system_parent = parent_kind == "__Workspace" || parent_kind == "__Namespace";

    for (row_idx, row) in data_rows.iter().enumerate() {
        let local_id = format!("{}_{}", arr_field, row_idx);
        let (obj_id, local_id_out) = if is_system_parent {
            (format!("{}_{}_{}", arr_parent_id, arr_field, row_idx), None)
        } else {
            let composed = format!("{}.{}.{}", parent_full_id, arr_field, &local_id);
            (composed, Some(local_id.clone()))
        };
        let mut element = IndexMap::new();
        element.insert("__id".to_string(), json!(&obj_id));

        let mut label_set = false;
        let mut field_types: IndexMap<String, Value> = IndexMap::new();

        for (col_idx, col_name) in column_names.iter().enumerate() {
            if col_idx < row.len() {
                let (value, type_name) = parse_field_value(&row[col_idx]);
                if !label_set {
                    if let Some(s) = value.as_str() {
                        element.insert("__label".to_string(), json!(s));
                    }
                    label_set = true;
                }
                element.insert(col_name.clone(), value);
                field_types.insert(col_name.clone(), json!(type_name));
            }
        }

        element.insert("__kind".to_string(), json!(arr_kind));
        if let Some(ref lid) = local_id_out {
            element.insert("__local_id".to_string(), json!(lid));
        }
        element.insert(
            "__parent".to_string(),
            json!(format!("[[#{}]]", parent_full_id)),
        );
        element.insert("__parent_field".to_string(), json!(arr_field));

        if !field_types.is_empty() {
            let types_obj: serde_json::Map<String, Value> = field_types.into_iter().collect();
            element.insert("__types".to_string(), json!(types_obj));
        }

        result.push((obj_id, element));
    }

    result
}

/// Main parse function
pub fn parse(markdown: &str, options: ParseOptions) -> Vec<Value> {
    let seed = options.random_seed.unwrap_or(666);
    let format = options.format;
    let mut rng = SimpleRng::new(seed);

    // Split markdown into lines for position tracking
    let lines: Vec<&str> = markdown.lines().collect();

    let mut all_objects: Vec<Value> = Vec::new();

    // Track objects by id for building relationships
    let mut objects_map: IndexMap<String, IndexMap<String, Value>> = IndexMap::new();

    // Track duplicate objects (same __id appears more than once)
    // These are stored separately so they appear in output but don't interfere with parent lookups
    let mut duplicate_objects: Vec<IndexMap<String, Value>> = Vec::new();

    // Track the true first line for each duplicate ID (for error messages with 3+ duplicates)
    let mut first_seen_lines: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    // Track text blocks
    let mut text_blocks: Vec<(String, String, usize, Vec<CodeFenceInfo>)> = Vec::new(); // (id, content, line, code_fences)
    let mut text_block_counter = 0;
    let mut pending_text_block: Option<Vec<String>> = None;
    let mut pending_text_block_line: usize = 0;
    let mut pending_text_block_level: u8 = 0; // Track level of pending TextBlock for structured_in_textblock check
    let mut pending_code_fences: Vec<CodeFenceInfo> = Vec::new();

    // Track parsing errors (structured_in_textblock, etc.)
    let mut parsing_errors: Vec<IndexMap<String, Value>> = Vec::new();
    let mut parsing_error_counter = 0;

    // Track content order for __Document
    let mut content_order: Vec<String> = Vec::new();

    // Current object being parsed
    let mut current_obj: Option<CurrentObject> = None;

    // Object stack for nesting (id, level)
    let mut object_stack: Vec<(String, u8)> = Vec::new();

    // Pending states
    let mut pending_text_field: Option<(String, String, u8, String)> = None; // (parent_id, field_name, level, field_type)
    let mut pending_object_array: Option<(String, String, String, u8)> = None; // (parent_id, field_name, kind, level)

    // Parser state
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut heading_level: u8 = 0;
    let mut heading_line: u32 = 0;
    let mut heading_start_offset: usize = 0;

    let mut in_list_item = false;
    let mut list_item_text = String::new();
    let mut list_item_start: Option<usize> = None; // Start offset of current list item
    let mut in_text_field_list = false; // Track if we're in a list inside text field
    let mut current_list_order: Option<u64> = None; // None = unordered, Some(n) = ordered starting at n
    let mut current_list_item_num: u64 = 1; // Current item number for ordered lists
    let mut comment_list_items: Vec<String> = Vec::new(); // Accumulate list items for comments
    let mut list_nesting_level: usize = 0; // Track nesting level for comments
    let mut list_order_stack: Vec<Option<u64>> = Vec::new(); // Stack of list orders for nesting
    let mut list_comment_anchor: Option<String> = None; // Anchor at the start of list (for correct comment placement)
    let mut comment_list_raw_start: Option<usize> = None; // Raw slice start for comment lists
    let mut comment_list_raw_end: Option<usize> = None; // Raw slice end for comment lists (avoid swallowing fields)
    let mut list_has_duplicate_keys: bool = false; // Whether current list has items that are duplicate field keys
    let mut list_inserted_field_keys: Vec<String> = Vec::new(); // Field keys inserted during current list (for rollback on duplicate)
    let mut ordered_list_in_array_error: bool = false; // Track ordered list under array field (forbidden)
    let mut ordered_list_error_line: Option<u32> = None; // Line number for ordered_list_in_array error (None = trailing comment, no error)
    let mut list_raw_start: Option<usize> = None; // Raw start offset of the entire current outermost list
    let mut pending_multiline_list_field: Option<String> = None; // Field name waiting for nested sub-items
    let mut pending_yaml_multiline_pipe_field: Option<String> = None; // Field name for yaml_multiline pipe with nested list
    let mut pending_yaml_multiline_pipe_start: Option<usize> = None; // Saved list_item_start for the parent pipe field item
    let mut multiline_list_items: Vec<String> = Vec::new(); // Accumulated sub-items for yaml_multiline_list
    let mut mixed_field_invalid_line: Option<u32> = None; // Line of first invalid field-like item in current list
    let mut mixed_field_has_invalid: bool = false; // Whether current list has invalid field-like items not already captured
    let mut textblock_list_raw_start: Option<usize> = None; // Raw slice start for lists inside TextBlock
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_table_row: Vec<String> = Vec::new();
    let mut table_cell_text = String::new();
    let mut in_table_cell = false;
    let mut table_start_offset: usize = 0;

    // Track labels for text fields (object_id -> field_name -> label)
    let mut text_field_labels: IndexMap<String, IndexMap<String, String>> = IndexMap::new();

    let mut in_paragraph = false;
    let mut paragraph_text = String::new();
    let mut paragraph_start_offset: usize = 0;

    let mut in_code_block = false; // Track if we're inside a fenced code block
    let mut code_block_content = String::new(); // Content of current code block
    let mut code_block_lang = String::new(); // Language of current code block
    let mut code_block_start_line: usize = 0; // Line where code block starts (for __code_fences)
    let mut code_block_start_offset: usize = 0; // Byte offset where fenced code block starts

    // Track blockquote state
    let mut in_blockquote = false;
    let mut blockquote_lines: Vec<String> = Vec::new();

    // Track if last comment was a block element (blockquote, rule, code block, table)
    // Used to merge following paragraphs with the block
    let mut last_comment_was_block = false;

    let mut in_link = false;
    let mut link_url = String::new();
    let mut link_text = String::new();

    let field_re = re_field_kv();

    // Use all options EXCEPT smart punctuation (which converts quotes to curly quotes)
    let md_options = Options::all() - Options::ENABLE_SMART_PUNCTUATION;
    let parser = MdParser::new_ext(markdown, md_options);

    // Collect events with source positions
    let events: Vec<(Event, std::ops::Range<usize>)> = parser.into_offset_iter().collect();

    // Stage 2 (BlockTree): build from existing Stage 1 event stream.
    // For now we only use it for offset->line conversion, keeping parse() semantics unchanged.
    let block_tree = build_block_tree_from_events(markdown, &events);

    // Calculate line number from byte offset
    let get_line = |offset: usize| -> u32 { block_tree.offset_to_line(offset) };

    // Helper to check if there's a table right after heading (for table fields)
    let has_table_after = |start_idx: usize, events: &[(Event, std::ops::Range<usize>)]| -> bool {
        for (event, _) in events.iter().skip(start_idx) {
            match event {
                Event::Start(Tag::Heading { .. }) => return false,
                Event::Start(Tag::Table(_)) => return true,
                Event::Start(Tag::List(_)) => return false,
                Event::Start(Tag::Paragraph) => {} // Skip paragraphs
                Event::End(TagEnd::Paragraph) => {}
                Event::Text(_) => {}
                _ => {}
            }
        }
        false
    };

    // Regex for field detection in has_fields_after (compiled once, outside the loop)
    let field_check_re = re_field_check();

    // Regexes for stripping inline formatting before colon detection (compiled once, outside the loop)
    let backtick_re_strip = Regex::new(r"`[^`]+`").unwrap();
    let bold_re_strip = Regex::new(r"\*\*([^*]*)\*\*").unwrap();
    let italic_re_strip = Regex::new(r"\*([^*]*)\*").unwrap();
    let strike_re_strip = Regex::new(r"~~([^~]*)~~").unwrap();

    // Helper: check if a list starting at `start_idx` contains ANY valid QMD.md field.
    // Scans all items (not just the first), used for boundary detection in comment scanning.
    let _list_has_any_field = |start_idx: usize,
                               events: &[(Event, std::ops::Range<usize>)],
                               markdown: &str,
                               re: &Regex|
     -> bool {
        let mut item_start: Option<usize> = None;
        let mut in_list = false;

        for (event, range) in events.iter().skip(start_idx) {
            match event {
                Event::Start(Tag::List(_)) => {
                    in_list = true;
                }
                Event::End(TagEnd::List(_)) => {
                    return false; // Reached end of list without finding a field
                }
                Event::Start(Tag::Item) => {
                    if in_list {
                        item_start = Some(range.start);
                    }
                }
                Event::End(TagEnd::Item) => {
                    if let Some(start) = item_start {
                        let item_text = &markdown[start..range.end];
                        if re.is_match(item_text) {
                            return true;
                        }
                    }
                    item_start = None;
                }
                _ => {}
            }
        }
        false
    };

    // Helper to check if there are QMD.md fields (- key: value) after a heading
    let has_fields_after = |start_idx: usize,
                            events: &[(Event, std::ops::Range<usize>)],
                            markdown: &str,
                            re: &Regex|
     -> bool {
        let mut in_list = false;
        let mut item_start: Option<usize> = None;

        for (event, range) in events.iter().skip(start_idx) {
            match event {
                Event::Start(Tag::Heading { .. }) => return false,
                Event::Start(Tag::Table(_)) => {
                    // Tables are just text content, not fields
                    // Continue looking for actual field lists
                    continue;
                }
                Event::Start(Tag::List(_)) => {
                    in_list = true;
                }
                Event::End(TagEnd::List(_)) => {
                    in_list = false;
                }
                Event::Start(Tag::Item) => {
                    item_start = Some(range.start);
                }
                Event::End(TagEnd::Item) => {
                    // Check if this item looks like a field (starts with word:)
                    if let Some(start) = item_start {
                        let item_text = &markdown[start..range.end];
                        // Check for pattern: - word: or - word :
                        if re.is_match(item_text) {
                            return true;
                        }
                    }
                    item_start = None;
                    // If we're in a list but first item isn't a field, it's not an object
                    if in_list {
                        return false;
                    }
                }
                _ => {}
            }
        }
        false
    };

    // Helper to check if there are nested headings with [[...]] at a deeper level.
    // Returns true if any heading at a deeper level contains [[...]] bracket syntax.
    // Stops at headings at same or higher level.
    let has_nested_structured_headings =
        |start_idx: usize, current_level: u8, events: &[(Event, std::ops::Range<usize>)]| -> bool {
            let bracket_re = Regex::new(r"\[\[[^\]]+\]\]").unwrap();

            for (event, range) in events.iter().skip(start_idx) {
                if let Event::Start(Tag::Heading { level, .. }) = event {
                    let next_level = heading_level_to_u8(level);
                    if next_level <= current_level {
                        return false;
                    }
                    let heading_text = block_tree.source.get(range.clone()).unwrap_or("");
                    if bracket_re.is_match(heading_text) {
                        return true;
                    }
                }
            }
            false
        };

    let mut i = 0;
    while i < events.len() {
        let (event, range) = &events[i];

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_text.clear();
                heading_start_offset = range.start;
                heading_level = heading_level_to_u8(level);
                heading_line = get_line(range.start);
            }

            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;

                // Check if this heading is inside a text field
                if let Some((ref parent_id, ref field_name, text_field_level, _)) =
                    pending_text_field.clone()
                {
                    if heading_level > text_field_level {
                        // This heading is part of the text field content
                        let heading_md = format!(
                            "{} {}",
                            "#".repeat(heading_level as usize),
                            heading_text.trim()
                        );

                        // Add to parent's text field
                        if let Some(parent) = objects_map.get_mut(parent_id) {
                            let existing = parent
                                .get(field_name)
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let new_val = if existing.is_empty() {
                                heading_md
                            } else {
                                format!("{}\n\n{}", existing, heading_md)
                            };
                            parent.insert(field_name.clone(), json!(new_val));
                        }

                        i += 1;
                        continue;
                    } else {
                        // Exiting text field context - finalize it
                        if let Some(parent) = objects_map.get_mut(parent_id) {
                            // Check if this is an array field (don't set __types for arrays)
                            let is_array = parent
                                .get(field_name)
                                .map(|v| v.is_array())
                                .unwrap_or(false);

                            // Check if this field was parsed as yaml_object or json_object
                            let is_object_syntax = parent
                                .get("__syntax")
                                .and_then(|s| s.as_object())
                                .and_then(|obj| obj.get(field_name))
                                .and_then(|v| v.as_str())
                                .map(|s| s == "yaml_object" || s == "json_object")
                                .unwrap_or(false);

                            if !is_array && !is_object_syntax {
                                // Add __types and __syntax if not already set
                                if let Some(types) = parent.get_mut("__types") {
                                    if let Some(obj) = types.as_object_mut() {
                                        if !obj.contains_key(field_name) {
                                            obj.insert(field_name.clone(), json!("string"));
                                        }
                                    }
                                } else {
                                    let mut types_map = serde_json::Map::new();
                                    types_map.insert(field_name.clone(), json!("string"));
                                    parent.insert("__types".to_string(), json!(types_map));
                                }

                                if let Some(syntax) = parent.get_mut("__syntax") {
                                    if let Some(obj) = syntax.as_object_mut() {
                                        if !obj.contains_key(field_name) {
                                            obj.insert(field_name.clone(), json!("multiline_text"));
                                        }
                                    }
                                } else {
                                    let mut syntax_map = serde_json::Map::new();
                                    syntax_map.insert(field_name.clone(), json!("multiline_text"));
                                    parent.insert("__syntax".to_string(), json!(syntax_map));
                                }
                            }
                        }
                        pending_text_field = None;
                    }
                }

                let header = parse_header(&heading_text, &mut rng);

                // Pop objects from stack at same or deeper level
                while !object_stack.is_empty() && object_stack.last().unwrap().1 >= heading_level {
                    let popped = object_stack.pop().unwrap();
                    let popped_id = popped.0.clone();

                    // If the popped object is current_obj, finalize it
                    let is_current = current_obj
                        .as_ref()
                        .map(|o| o.id == popped_id)
                        .unwrap_or(false);
                    if is_current {
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }
                    }
                }

                let parent_id = object_stack.last().map(|(id, _)| id.clone());

                // Emit multiple_definitions error if heading has 2+ [[...]]
                if let Some(ref defs) = header.multiple_definitions {
                    let error_id = format!("error_{}", parsing_errors.len());
                    let mut error = IndexMap::new();
                    error.insert("__id".to_string(), json!(error_id));
                    error.insert("__kind".to_string(), json!("__ParsingError"));
                    error.insert("type".to_string(), json!("multiple_definitions"));
                    error.insert("definitions".to_string(), json!(defs));
                    error.insert("object".to_string(), json!(format!("[[#{}]]", header.id)));
                    error.insert("line".to_string(), json!(heading_line));
                    parsing_errors.push(error);
                }

                // Check if this is an array field: [[field: array]]
                if header.field_type.as_deref() == Some("array") {
                    if let Some(ref pid) = parent_id {
                        let parent_is_current =
                            current_obj.as_ref().map(|o| o.id == *pid).unwrap_or(false);

                        if parent_is_current {
                            // Parent is current_obj — write array field setup directly.
                            // No finalize needed; the list handler will write to current_obj.
                            if let Some(ref mut obj) = current_obj {
                                obj.fields.insert(header.id.clone(), json!([]));
                                obj.syntax
                                    .insert(header.id.clone(), "markdown_list".to_string());
                                obj.types.shift_remove(&header.id);

                                let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                                let col =
                                    line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                                obj.positions.insert(header.id.clone(), (heading_line, col));

                                // Update comment_anchor so that comments after this array
                                // heading (e.g. raw ordered list content) use field_name as anchor
                                obj.comment_anchor = header.id.clone();
                            }
                        } else {
                            // Parent is already in objects_map (current_obj is a sibling).
                            // Finalize current_obj, then write to objects_map.
                            if let Some(obj) = current_obj.take() {
                                finalize_object(
                                    &mut objects_map,
                                    &mut duplicate_objects,
                                    &mut parsing_errors,
                                    &mut first_seen_lines,
                                    obj,
                                );
                            }

                            if let Some(parent) = objects_map.get_mut(pid) {
                                parent.insert(header.id.clone(), json!([]));

                                let syntax = parent
                                    .entry("__syntax".to_string())
                                    .or_insert_with(|| json!({}));
                                if let Some(obj) = syntax.as_object_mut() {
                                    obj.insert(header.id.clone(), json!("markdown_list"));
                                }

                                if let Some(types) = parent.get_mut("__types") {
                                    if let Some(obj) = types.as_object_mut() {
                                        obj.remove(&header.id);
                                    }
                                }

                                let positions = parent
                                    .entry("__positions".to_string())
                                    .or_insert_with(|| json!({}));
                                if let Some(pos_obj) = positions.as_object_mut() {
                                    let line_text =
                                        lines.get(heading_line as usize - 1).unwrap_or(&"");
                                    let col =
                                        line_text.find(&format!("[[{}", header.id)).unwrap_or(0)
                                            as u32;
                                    pos_obj.insert(
                                        header.id.clone(),
                                        json!({"line": heading_line, "col": col}),
                                    );
                                }
                            }
                        }

                        pending_text_field = Some((
                            pid.clone(),
                            header.id.clone(),
                            heading_level,
                            "array".to_string(),
                        ));
                        i += 1;
                        continue;
                    }
                }

                // Check if this is a json/yaml field header (NOT text - text uses the unified path below)
                let is_content_field =
                    matches!(header.field_type.as_deref(), Some("json") | Some("yaml"));
                if is_content_field {
                    if let Some(ref pid) = parent_id {
                        // Finalize current object (the parent) before setting up text field
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }

                        // Initialize empty string in parent
                        if let Some(parent) = objects_map.get_mut(pid) {
                            parent.insert(header.id.clone(), json!(""));

                            // Save label for text field (for rebuild)
                            text_field_labels
                                .entry(pid.clone())
                                .or_default()
                                .insert(header.id.clone(), header.label.clone());

                            // Track field position for LSP (heading-defined text field)
                            let positions = parent
                                .entry("__positions".to_string())
                                .or_insert_with(|| json!({}));
                            if let Some(pos_obj) = positions.as_object_mut() {
                                let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                                let col =
                                    line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                                pos_obj.insert(
                                    header.id.clone(),
                                    json!({"line": heading_line, "col": col}),
                                );
                            }
                        }

                        let ft = header
                            .field_type
                            .clone()
                            .unwrap_or_else(|| "text".to_string());
                        pending_text_field =
                            Some((pid.clone(), header.id.clone(), heading_level, ft));
                        i += 1;
                        continue;
                    }
                }

                // Check if this is a table field: [[id]] (no Kind) followed by table
                // This handles patterns like ### Statuses [[task_statuses]] with a table below
                if header.has_explicit_id
                    && header.kind.is_none()
                    && header.field_type.is_none()
                    && has_table_after(i + 1, &events)
                {
                    if let Some(ref pid) = parent_id {
                        // This is a table field of the parent object
                        // Finalize current object (the parent) before setting up table field
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }

                        // Initialize empty array in parent for table rows
                        if let Some(parent) = objects_map.get_mut(pid) {
                            parent.insert(header.id.clone(), json!([]));

                            // Add __syntax for table
                            let syntax = parent
                                .entry("__syntax".to_string())
                                .or_insert_with(|| json!({}));
                            if let Some(obj) = syntax.as_object_mut() {
                                obj.insert(header.id.clone(), json!("table"));
                            }

                            // Track field position for LSP (heading-defined table field)
                            let positions = parent
                                .entry("__positions".to_string())
                                .or_insert_with(|| json!({}));
                            if let Some(pos_obj) = positions.as_object_mut() {
                                let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                                let col =
                                    line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                                pos_obj.insert(
                                    header.id.clone(),
                                    json!({"line": heading_line, "col": col}),
                                );
                            }
                        }

                        // Set pending_text_field to collect table content
                        pending_text_field = Some((
                            pid.clone(),
                            header.id.clone(),
                            heading_level,
                            "table".to_string(),
                        ));
                        i += 1;
                        continue;
                    }
                }

                // Check if this is an object array header [[field: [Kind]]]
                if header.field_type.as_deref() == Some("object_array") {
                    if let Some(ref pid) = parent_id {
                        // Finalize current object (the parent) before setting up array
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }

                        // Initialize empty array in parent
                        if let Some(parent) = objects_map.get_mut(pid) {
                            parent.insert(header.id.clone(), json!([]));

                            // Add __syntax
                            let syntax = parent
                                .entry("__syntax".to_string())
                                .or_insert_with(|| json!({}));
                            if let Some(obj) = syntax.as_object_mut() {
                                obj.insert(header.id.clone(), json!("headers"));
                            }

                            // Track field position for LSP (heading-defined field)
                            let positions = parent
                                .entry("__positions".to_string())
                                .or_insert_with(|| json!({}));
                            if let Some(pos_obj) = positions.as_object_mut() {
                                let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                                let col =
                                    line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                                pos_obj.insert(
                                    header.id.clone(),
                                    json!({"line": heading_line, "col": col}),
                                );
                            }
                        }

                        // Save label for object_array field (for rebuild)
                        text_field_labels
                            .entry(pid.clone())
                            .or_default()
                            .insert(header.id.clone(), header.label.clone());

                        pending_object_array = Some((
                            pid.clone(),
                            header.id.clone(),
                            header.array_kind.clone().unwrap_or_default(),
                            heading_level,
                        ));
                        i += 1;
                        continue;
                    } else {
                        // Top-level object array without structural parent
                        // Create the heading as a parent object that owns the array
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }

                        let obj_id = header.id.clone();
                        let array_kind = header.array_kind.clone().unwrap_or_default();
                        let mut element = IndexMap::new();
                        element.insert("__id".to_string(), json!(&obj_id));
                        if !header.label.is_empty() {
                            element.insert("__label".to_string(), json!(&header.label));
                        }
                        element.insert("__kind".to_string(), json!("__Object"));
                        element.insert(obj_id.clone(), json!([]));
                        element.insert(
                            "__syntax".to_string(),
                            json!({&obj_id: "headers", "__array_kind": &array_kind}),
                        );
                        element.insert("__labels".to_string(), json!({&obj_id: &header.label}));
                        element.insert("__level".to_string(), json!(heading_level));
                        element.insert("__line".to_string(), json!(heading_line));

                        // Save label
                        text_field_labels
                            .entry(obj_id.clone())
                            .or_default()
                            .insert(obj_id.clone(), header.label.clone());

                        objects_map.insert(obj_id.clone(), element);
                        object_stack.push((obj_id.clone(), heading_level));
                        content_order.push(obj_id.clone());

                        pending_object_array =
                            Some((obj_id.clone(), obj_id, array_kind, heading_level));
                        i += 1;
                        continue;
                    }
                }

                // Check if inside an object array context
                if let Some((ref arr_parent_id, ref arr_field, ref arr_kind, arr_level)) =
                    pending_object_array.clone()
                {
                    // Skip if this is a text field - it should be handled as a field of current element
                    let is_text_field = header.field_type.as_deref() == Some("text");
                    if heading_level > arr_level && !is_text_field {
                        // Finalize previous array element if any
                        if let Some(obj) = current_obj.take() {
                            finalize_object(
                                &mut objects_map,
                                &mut duplicate_objects,
                                &mut parsing_errors,
                                &mut first_seen_lines,
                                obj,
                            );
                        }

                        // This is an element of the object array - resolve hierarchical ID
                        let local_id = header.id.clone();
                        let (composed_id, local_id_out) = resolve_child_id(
                            &objects_map,
                            arr_parent_id,
                            &local_id,
                            Some(arr_field.as_str()),
                        );
                        let parent_full_id = objects_map
                            .get(arr_parent_id.as_str())
                            .and_then(|m| m.get("__id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(arr_parent_id)
                            .to_string();

                        let mut element = IndexMap::new();
                        element.insert("__id".to_string(), json!(&composed_id));
                        if let Some(ref lid) = local_id_out {
                            element.insert("__local_id".to_string(), json!(lid));
                        }
                        if !header.label.is_empty() {
                            element.insert("__label".to_string(), json!(&header.label));
                        }
                        element.insert("__kind".to_string(), json!(arr_kind));
                        element.insert(
                            "__parent".to_string(),
                            json!(format!("[[#{}]]", parent_full_id)),
                        );
                        element.insert("__parent_field".to_string(), json!(arr_field));

                        // Add reference to parent's array
                        if let Some(parent) = objects_map.get_mut(arr_parent_id.as_str()) {
                            if let Some(arr) = parent.get_mut(arr_field.as_str()) {
                                if let Some(arr_vec) = arr.as_array_mut() {
                                    arr_vec.push(json!(format!("[[#{}]]", composed_id)));
                                }
                            }
                        }

                        objects_map.insert(composed_id.clone(), element);
                        object_stack.push((composed_id.clone(), heading_level));

                        current_obj = Some(CurrentObject {
                            id: composed_id.clone(),
                            local_id: local_id_out,
                            label: header.label.clone(),
                            kind: Some(arr_kind.clone()),
                            level: heading_level,
                            line: heading_line,
                            fields: IndexMap::new(),
                            types: IndexMap::new(),
                            syntax: IndexMap::new(),
                            comments: Vec::new(),
                            has_explicit_id: header.has_explicit_id,
                            parent: Some(format!("[[#{}]]", parent_full_id)),
                            parent_field: Some(arr_field.clone()),
                            comment_anchor: "__self".to_string(),
                            is_array_element: true,
                            references: Vec::new(),
                            positions: IndexMap::new(),
                        });

                        i += 1;
                        continue;
                    } else if !is_text_field {
                        // Exiting object array context (but not for text fields)
                        pending_object_array = None;
                    }
                    // If is_text_field, continue to text field handling below
                }

                // Check if this is a comment heading (no [[id]] inside an object)
                if parent_id.is_some() && !header.has_explicit_id {
                    // Heading WITHOUT [[id]] inside object = COMMENT (always)
                    // New architecture step: do NOT reconstruct comment markdown from events.
                    // Store raw markdown slice for this comment section.
                    //
                    // Find end boundary: next heading of same/higher level, OR a nested object heading with Kind.
                    //
                    // NOTE: we still detect `structured_in_textblock` errors for nested headings
                    // with `[[id]]` but without Kind (legacy validation behavior), without
                    // reconstructing the markdown.
                    let mut end_offset = markdown.len();
                    let mut j = i + 1;
                    while j < events.len() {
                        let (evt, evt_range) = &events[j];
                        match evt {
                            Event::Start(Tag::Heading {
                                level: next_lvl, ..
                            }) => {
                                let next_level = *next_lvl as u8;

                                if next_level <= heading_level {
                                    end_offset = evt_range.start;
                                    break;
                                }

                                // Nested heading: if it declares an object (has Kind), stop before it.
                                let mut nested_heading_text = String::new();
                                let mut k = j + 1;
                                while k < events.len() {
                                    match &events[k].0 {
                                        Event::Text(txt) => nested_heading_text.push_str(txt),
                                        Event::Code(code) => {
                                            nested_heading_text.push('`');
                                            nested_heading_text.push_str(code);
                                            nested_heading_text.push('`');
                                        }
                                        Event::End(TagEnd::Heading(_)) => break,
                                        _ => {}
                                    }
                                    k += 1;
                                }

                                let nested_header = parse_header(&nested_heading_text, &mut rng);
                                if nested_header.has_explicit_id && nested_header.kind.is_none() {
                                    let bracket_re = re_double_brackets();
                                    if let Some(caps) = bracket_re.captures(&nested_heading_text) {
                                        let ref_pattern = caps
                                            .get(0)
                                            .map(|m| m.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        if !ref_pattern.is_empty() {
                                            let nested_heading_line = get_line(evt_range.start);
                                            let error_id =
                                                format!("parsing_error_{}", parsing_error_counter);
                                            parsing_error_counter += 1;

                                            let mut error = IndexMap::new();
                                            error.insert("__id".to_string(), json!(error_id));
                                            error.insert(
                                                "__kind".to_string(),
                                                json!("__ParsingError"),
                                            );
                                            error.insert(
                                                "type".to_string(),
                                                json!("structured_in_textblock"),
                                            );
                                            error.insert(
                                                "reference".to_string(),
                                                json!(ref_pattern),
                                            );
                                            error.insert(
                                                "line".to_string(),
                                                json!(nested_heading_line),
                                            );
                                            parsing_errors.push(error);
                                        }
                                    }
                                }
                                if nested_header.kind.is_some() {
                                    end_offset = evt_range.start;
                                    break;
                                }
                            }
                            Event::Start(Tag::List(_)) => {
                                // Field-like bullet lists inside comment headings are part of
                                // the comment content, not parent object fields (matching Python).
                                // Do NOT stop at them.
                            }
                            _ => {}
                        }
                        j += 1;
                    }

                    let raw_comment = block_tree
                        .source
                        .get(heading_start_offset..end_offset)
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    // Get the comment anchor from current_obj or determine from parent
                    let comment_anchor = if let Some(ref obj) = current_obj {
                        obj.comment_anchor.clone()
                    } else if let Some(ref pid) = parent_id {
                        // current_obj was finalized (child popped from stack).
                        // Find which field on the parent references the last popped child
                        // to use as the comment anchor.
                        let mut anchor = "__self".to_string();
                        if let Some(parent_map) = objects_map.get(pid) {
                            // Find the last non-__ field that is a child reference
                            for (fk, fv) in parent_map.iter().rev() {
                                if fk.starts_with("__") {
                                    continue;
                                }
                                let is_ref = match fv {
                                    Value::String(s) => s.starts_with("[[#") && s.ends_with("]]"),
                                    _ => false,
                                };
                                if is_ref {
                                    anchor = fk.clone();
                                    break;
                                }
                            }
                            // If no child ref found, use the last non-__ field
                            if anchor == "__self" {
                                for (fk, _) in parent_map.iter().rev() {
                                    if !fk.starts_with("__") {
                                        anchor = fk.clone();
                                        break;
                                    }
                                }
                            }
                        }
                        anchor
                    } else {
                        "__self".to_string()
                    };

                    // Add comment to current_obj
                    if let Some(ref mut obj) = current_obj {
                        let mut comment_map = IndexMap::new();
                        comment_map.insert("after".to_string(), comment_anchor);
                        comment_map.insert("content".to_string(), raw_comment);
                        obj.comments.push(comment_map);
                        i = j;
                        continue;
                    } else if let Some(ref pid) = parent_id {
                        // current_obj is None but parent_id exists
                        // Try to add to the parent in objects_map
                        if let Some(parent) = objects_map.get_mut(pid) {
                            let comments = parent
                                .entry("__comments".to_string())
                                .or_insert_with(|| json!([]));
                            if let Some(arr) = comments.as_array_mut() {
                                arr.push(json!({
                                    "after": comment_anchor,
                                    "content": raw_comment
                                }));
                            }
                        }
                        i = j;
                        continue;
                    }
                }

                // Determine if this heading creates an object or a text block
                let has_fields = has_fields_after(i + 1, &events, markdown, field_check_re);

                // Check if this is a map field [[field: map]]
                if header.field_type.as_deref() == Some("map") && parent_id.is_some() {
                    let parent = parent_id.clone().unwrap();
                    let content_start = range.end;
                    let mut end_offset = markdown.len();
                    let mut j = i + 1;
                    while j < events.len() {
                        let (evt, evt_range) = &events[j];
                        if let Event::Start(Tag::Heading {
                            level: next_lvl, ..
                        }) = evt
                        {
                            let next_level = *next_lvl as u8;
                            if next_level <= heading_level {
                                end_offset = evt_range.start;
                                break;
                            }
                        }
                        j += 1;
                    }

                    let raw = block_tree
                        .source
                        .get(content_start..end_offset)
                        .unwrap_or("")
                        .trim();
                    let mut map_data = serde_json::Map::new();
                    // Compute line number of first non-blank line in raw slice
                    let raw_start_offset = if !raw.is_empty() {
                        block_tree.source[content_start..end_offset]
                            .find(raw)
                            .map(|p| content_start + p)
                            .unwrap_or(content_start)
                    } else {
                        content_start
                    };
                    let base_line = get_line(raw_start_offset);
                    let raw_lines: Vec<&str> = raw.lines().collect();
                    let is_valid_key = |k: &str| -> bool {
                        !k.is_empty()
                            && k.chars()
                                .next()
                                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    };
                    let mut idx = 0;
                    while idx < raw_lines.len() {
                        let stripped = raw_lines[idx].trim();
                        if let Some(item) = stripped.strip_prefix("- ") {
                            if let Some(colon_pos) = item.find(':') {
                                let k = item[..colon_pos].trim();
                                if !is_valid_key(k) {
                                    // Invalid key (e.g. **bold**)
                                    let line = base_line + idx as u32;
                                    let error_id = format!("error_{}", parsing_errors.len());
                                    let mut error = IndexMap::new();
                                    error.insert("__id".to_string(), json!(error_id));
                                    error.insert("__kind".to_string(), json!("__ParsingError"));
                                    error.insert("type".to_string(), json!("invalid_map_entry"));
                                    error.insert("field".to_string(), json!(header.id));
                                    error.insert(
                                        "object".to_string(),
                                        json!(format!("[[#{}]]", parent)),
                                    );
                                    error.insert("line".to_string(), json!(line));
                                    parsing_errors.push(error);
                                    idx += 1;
                                    continue;
                                }
                                let v = item[colon_pos + 1..].trim();
                                if v == "|" {
                                    // Multiline value
                                    let mut ml_lines: Vec<&str> = Vec::new();
                                    idx += 1;
                                    while idx < raw_lines.len() {
                                        let ln = raw_lines[idx];
                                        if ln.trim().is_empty()
                                            || ln.starts_with(' ')
                                            || ln.starts_with('\t')
                                        {
                                            ml_lines.push(ln);
                                            idx += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    // Dedent
                                    let min_indent = ml_lines
                                        .iter()
                                        .filter(|l| !l.trim().is_empty())
                                        .map(|l| l.len() - l.trim_start().len())
                                        .min()
                                        .unwrap_or(0);
                                    let dedented: Vec<&str> = ml_lines
                                        .iter()
                                        .map(|l| {
                                            if l.len() >= min_indent {
                                                &l[min_indent..]
                                            } else {
                                                l
                                            }
                                        })
                                        .collect();
                                    // Strip trailing blank lines
                                    let mut end = dedented.len();
                                    while end > 0 && dedented[end - 1].trim().is_empty() {
                                        end -= 1;
                                    }
                                    map_data
                                        .insert(k.to_string(), json!(dedented[..end].join("\n")));
                                    continue;
                                } else {
                                    map_data.insert(k.to_string(), json!(v));
                                }
                            } else {
                                // No colon — not a valid map entry
                                let line = base_line + idx as u32;
                                let error_id = format!("error_{}", parsing_errors.len());
                                let mut error = IndexMap::new();
                                error.insert("__id".to_string(), json!(error_id));
                                error.insert("__kind".to_string(), json!("__ParsingError"));
                                error.insert("type".to_string(), json!("invalid_map_entry"));
                                error.insert("field".to_string(), json!(header.id));
                                error.insert(
                                    "object".to_string(),
                                    json!(format!("[[#{}]]", parent)),
                                );
                                error.insert("line".to_string(), json!(line));
                                parsing_errors.push(error);
                            }
                        } else if !stripped.is_empty() {
                            // Non-list content (paragraph, code fence, numbered list, etc.)
                            let line = base_line + idx as u32;
                            let error_id = format!("error_{}", parsing_errors.len());
                            let mut error = IndexMap::new();
                            error.insert("__id".to_string(), json!(error_id));
                            error.insert("__kind".to_string(), json!("__ParsingError"));
                            error.insert("type".to_string(), json!("invalid_map_content"));
                            error.insert("field".to_string(), json!(header.id));
                            error.insert("object".to_string(), json!(format!("[[#{}]]", parent)));
                            error.insert("line".to_string(), json!(line));
                            parsing_errors.push(error);
                            // Skip code fence contents
                            if stripped.starts_with("```") {
                                idx += 1;
                                while idx < raw_lines.len() {
                                    if raw_lines[idx].trim().starts_with("```") {
                                        break;
                                    }
                                    idx += 1;
                                }
                            }
                        }
                        idx += 1;
                    }

                    if let Some(ref mut obj) = current_obj {
                        obj.fields
                            .insert(header.id.clone(), Value::Object(map_data));
                        obj.types.insert(header.id.clone(), "map".to_string());
                        obj.syntax.insert(header.id.clone(), "map".to_string());
                        obj.comment_anchor = header.id.clone();
                        text_field_labels
                            .entry(obj.id.clone())
                            .or_default()
                            .insert(header.id.clone(), header.label.clone());
                    } else if let Some(parent_obj) = objects_map.get_mut(&parent) {
                        parent_obj.insert(header.id.clone(), Value::Object(map_data));
                        if let Some(types) = parent_obj.get_mut("__types") {
                            if let Some(types_map) = types.as_object_mut() {
                                types_map.insert(header.id.clone(), json!("map"));
                            }
                        }
                        if let Some(syntax) = parent_obj.get_mut("__syntax") {
                            if let Some(syntax_map) = syntax.as_object_mut() {
                                syntax_map.insert(header.id.clone(), json!("map"));
                            }
                        }
                        text_field_labels
                            .entry(parent.clone())
                            .or_default()
                            .insert(header.id.clone(), header.label.clone());
                    }

                    i = j;
                    continue;
                }

                // Check if this is a text field:
                // 1. [[id: text]] - explicit text type (always text field, ignore nested structure)
                // 2. [[id]] without kind, inside object, no fields after, no nested structured headings
                let has_nested_structure =
                    has_nested_structured_headings(i + 1, heading_level, &events);
                let is_explicit_text = header.field_type.as_deref() == Some("text");
                let is_implicit_text =
                    header.kind.is_none() && !has_fields && !has_nested_structure;
                if parent_id.is_some()
                    && header.has_explicit_id
                    && (is_explicit_text || is_implicit_text)
                {
                    // This is a text field, not a new object.
                    //
                    // New architecture step: do NOT parse the inside of the field (tables/lists/inline).
                    // Instead, store a raw markdown slice between field heading and the next heading
                    // of the same or higher level.
                    let parent = parent_id.clone().unwrap();

                    let content_start = range.end;
                    let mut end_offset = markdown.len();
                    let mut j = i + 1;
                    while j < events.len() {
                        let (evt, evt_range) = &events[j];
                        if let Event::Start(Tag::Heading {
                            level: next_lvl, ..
                        }) = evt
                        {
                            let next_level = *next_lvl as u8;
                            if next_level <= heading_level {
                                end_offset = evt_range.start;
                                break;
                            }
                        }
                        j += 1;
                    }

                    let raw_value = block_tree
                        .source
                        .get(content_start..end_offset)
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let field_value = raw_value;
                    if let Some(ref mut obj) = current_obj {
                        obj.fields.insert(header.id.clone(), json!(field_value));
                        obj.types.insert(header.id.clone(), "string".to_string());
                        obj.syntax
                            .insert(header.id.clone(), "multiline_text".to_string());
                        obj.comment_anchor = header.id.clone();

                        // Add position for text field (for LSP outline)
                        let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                        let col = line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                        obj.positions.insert(header.id.clone(), (heading_line, col));

                        // Save label for text field (for rebuild)
                        text_field_labels
                            .entry(obj.id.clone())
                            .or_default()
                            .insert(header.id.clone(), header.label.clone());
                    } else if let Some(parent_obj) = objects_map.get_mut(&parent) {
                        parent_obj.insert(header.id.clone(), json!(field_value));
                        if let Some(types) = parent_obj.get_mut("__types") {
                            if let Some(types_map) = types.as_object_mut() {
                                types_map.insert(header.id.clone(), json!("string"));
                            }
                        }
                        if let Some(syntax) = parent_obj.get_mut("__syntax") {
                            if let Some(syntax_map) = syntax.as_object_mut() {
                                syntax_map.insert(header.id.clone(), json!("multiline_text"));
                            }
                        }

                        // Add position for text field (for LSP outline)
                        let line_text = lines.get(heading_line as usize - 1).unwrap_or(&"");
                        let col = line_text.find(&format!("[[{}", header.id)).unwrap_or(0) as u32;
                        let positions = parent_obj
                            .entry("__positions".to_string())
                            .or_insert_with(|| json!({}));
                        if let Some(pos_obj) = positions.as_object_mut() {
                            pos_obj.insert(
                                header.id.clone(),
                                json!({"line": heading_line, "col": col}),
                            );
                        }

                        // Save label for text field (for rebuild)
                        text_field_labels
                            .entry(parent.clone())
                            .or_default()
                            .insert(header.id.clone(), header.label.clone());
                    }

                    i = j;
                    continue;
                }

                // Check for explicit system type error
                // [[id: __Document]], [[id: __TextBlock]], [[id: __Object]] are not allowed
                // But __Workspace and __Namespace are valid kinds (explicit declaration in anchor files)
                if let Some(ref kind) = header.kind {
                    if kind == "__Document" || kind == "__TextBlock" || kind == "__Object" {
                        let ref_pattern = format!("[[{}: {}]]", header.id, kind);
                        let mut error = IndexMap::new();
                        error.insert("__id".to_string(), json!(&header.id));
                        error.insert("__kind".to_string(), json!("__ParsingError"));
                        error.insert("type".to_string(), json!("explicit_system_type"));
                        error.insert("reference".to_string(), json!(ref_pattern));
                        error.insert("line".to_string(), json!(heading_line));
                        parsing_errors.push(error);

                        // Skip to next heading
                        i += 1;
                        while i < events.len() {
                            if matches!(events[i].0, Event::Start(Tag::Heading { .. })) {
                                break;
                            }
                            i += 1;
                        }
                        continue;
                    }
                }

                // Check for structured_in_textblock error: [[id]] inside TextBlock
                // Only error if:
                // 1. No Kind specified (e.g., [[id]] or [[id: text]], not [[id: Object]])
                // 2. TextBlock has actual content (not just an empty heading)
                // 3. TextBlock level >= 2 (started at ## or deeper)
                // 4. New heading is DEEPER than TextBlock heading
                let is_valid_new_object = header.kind.is_some();
                let textblock_has_content = pending_text_block
                    .as_ref()
                    .map(|parts| {
                        // TextBlock has content if there's more than just the heading
                        // (heading + blank + content, or heading + content in next parts)
                        parts.len() > 1
                            || parts
                                .first()
                                .map(|s| s.lines().count() > 1)
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                // Only error if new heading is deeper than TextBlock and TextBlock is level 2+
                // (same or higher level heading starts new object, not error)
                let is_deeper_than_textblock = heading_level > pending_text_block_level;
                let textblock_is_level2_or_deeper = pending_text_block_level >= 2;
                if pending_text_block.is_some()
                    && header.has_explicit_id
                    && !is_valid_new_object
                    && textblock_has_content
                    && is_deeper_than_textblock
                    && textblock_is_level2_or_deeper
                {
                    // Generate error
                    let error_id = format!("parsing_error_{}", parsing_error_counter);
                    parsing_error_counter += 1;

                    // Build reference pattern (e.g., "[[invalid_field: text]]" or "[[another_invalid]]")
                    let ref_pattern = if let Some(ref ft) = header.field_type {
                        format!("[[{}: {}]]", header.id, ft)
                    } else if let Some(ref kind) = header.kind {
                        format!("[[{}: {}]]", header.id, kind)
                    } else {
                        format!("[[{}]]", header.id)
                    };

                    let mut error = IndexMap::new();
                    error.insert("__id".to_string(), json!(error_id));
                    error.insert("__kind".to_string(), json!("__ParsingError"));
                    error.insert("type".to_string(), json!("structured_in_textblock"));
                    error.insert("reference".to_string(), json!(ref_pattern));
                    error.insert("line".to_string(), json!(heading_line));
                    parsing_errors.push(error);

                    // Add heading text to TextBlock content and continue (don't create object)
                    if let Some(ref mut parts) = pending_text_block {
                        let heading_md =
                            format!("{} {}", "#".repeat(heading_level as usize), header.label);
                        parts.push(String::new());
                        parts.push(heading_md);
                    }

                    i += 1;
                    continue;
                }

                let has_structured_children = if heading_level >= 2 {
                    has_nested_structured_headings(i + 1, heading_level, &events)
                } else {
                    false
                };
                let is_text_block = !header.has_explicit_id
                    && header.kind.is_none()
                    && !has_fields
                    && !has_structured_children;

                if is_text_block {
                    // Save any pending text block first
                    if let Some(content_parts) = pending_text_block.take() {
                        let tb_id = format!("text_{}", text_block_counter);
                        text_block_counter += 1;
                        let fences = std::mem::take(&mut pending_code_fences);
                        text_blocks.push((
                            tb_id.clone(),
                            content_parts.join("\n\n"),
                            pending_text_block_line,
                            fences,
                        ));
                        content_order.push(tb_id);
                    }

                    // Start new text block with heading
                    let heading_md =
                        format!("{} {}", "#".repeat(heading_level as usize), header.label);
                    pending_text_block = Some(vec![heading_md]);
                    pending_text_block_line = heading_line as usize;
                    pending_text_block_level = heading_level;

                    // Clear current_obj so list items go to TextBlock, not object
                    if let Some(obj) = current_obj.take() {
                        finalize_object(
                            &mut objects_map,
                            &mut duplicate_objects,
                            &mut parsing_errors,
                            &mut first_seen_lines,
                            obj,
                        );
                    }
                } else {
                    // Save any pending text block
                    if let Some(content_parts) = pending_text_block.take() {
                        let tb_id = format!("text_{}", text_block_counter);
                        text_block_counter += 1;
                        let fences = std::mem::take(&mut pending_code_fences);
                        text_blocks.push((
                            tb_id.clone(),
                            content_parts.join("\n\n"),
                            pending_text_block_line,
                            fences,
                        ));
                        content_order.push(tb_id);
                        pending_text_block_level = 0;
                    }

                    // Finalize current object
                    if let Some(obj) = current_obj.take() {
                        finalize_object(
                            &mut objects_map,
                            &mut duplicate_objects,
                            &mut parsing_errors,
                            &mut first_seen_lines,
                            obj,
                        );
                    }

                    // BR-16: Dot in nested child's explicit ID is an error
                    if parent_id.is_some() && header.has_explicit_id && header.id.contains('.') {
                        let error_id = header.id.clone();
                        let mut error = IndexMap::new();
                        error.insert("__id".to_string(), json!(&error_id));
                        error.insert("__kind".to_string(), json!("__ParsingError"));
                        error.insert("type".to_string(), json!("invalid_id_character"));
                        error.insert("reference".to_string(), json!(format!("[[{}]]", header.id)));
                        error.insert("line".to_string(), json!(heading_line));
                        parsing_errors.push(error);

                        // Skip heading tokens and content until next heading
                        i += 1;
                        while i < events.len() {
                            if matches!(events[i].0, Event::Start(Tag::Heading { .. })) {
                                break;
                            }
                            i += 1;
                        }
                        continue;
                    }

                    // Resolve hierarchical ID for child objects
                    let (resolved_id, local_id_out, parent_ref) = if let Some(ref pid) = parent_id {
                        let local_id = header.id.clone();
                        let (composed_id, lid) =
                            resolve_child_id(&objects_map, pid, &local_id, None);
                        let parent_full_id = objects_map
                            .get(pid.as_str())
                            .and_then(|m| m.get("__id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(pid)
                            .to_string();
                        (composed_id, lid, Some(format!("[[#{}]]", parent_full_id)))
                    } else {
                        // Phase 3: Detect dot-ID parent declaration (BR-7)
                        let lid = if header.has_explicit_id && header.id.contains('.') {
                            Some(header.id.clone())
                        } else {
                            None
                        };
                        (header.id.clone(), lid, None)
                    };

                    // Create new object
                    let new_obj = CurrentObject {
                        id: resolved_id.clone(),
                        local_id: local_id_out.clone(),
                        label: header.label.clone(),
                        kind: header.kind.clone(),
                        level: heading_level,
                        line: heading_line,
                        fields: IndexMap::new(),
                        types: IndexMap::new(),
                        syntax: IndexMap::new(),
                        comments: Vec::new(),
                        has_explicit_id: header.has_explicit_id,
                        parent: parent_ref.clone(),
                        parent_field: if parent_id.is_some() {
                            Some(header.id.clone())
                        } else {
                            None
                        },
                        comment_anchor: "__self".to_string(),
                        is_array_element: false,
                        references: Vec::new(),
                        positions: IndexMap::new(),
                    };

                    // Add reference to parent
                    if let Some(ref pid) = parent_id {
                        if let Some(parent) = objects_map.get_mut(pid.as_str()) {
                            parent
                                .insert(header.id.clone(), json!(format!("[[#{}]]", resolved_id)));
                        }
                    }

                    // Add to content order if top-level
                    if new_obj.parent.is_none() {
                        content_order.push(resolved_id.clone());
                    }

                    object_stack.push((resolved_id.clone(), heading_level));
                    current_obj = Some(new_obj);

                    // If heading had field_type but no parent, add __syntax metadata
                    // and capture following content as __comments
                    // But NOT if the object was inside a TextBlock context
                    if let Some(ref ft) = header.field_type {
                        if parent_id.is_none() {
                            // Emit dangling_field error
                            let error_id = format!("error_{}", parsing_errors.len());
                            let mut error = IndexMap::new();
                            error.insert("__id".to_string(), json!(error_id));
                            error.insert("__kind".to_string(), json!("__ParsingError"));
                            error.insert("type".to_string(), json!("dangling_field"));
                            error.insert("field".to_string(), json!(header.id));
                            error.insert("field_type".to_string(), json!(ft));
                            error
                                .insert("object".to_string(), json!(format!("[[#{}]]", header.id)));
                            error.insert("line".to_string(), json!(heading_line));
                            parsing_errors.push(error);

                            let syntax_value = match ft.as_str() {
                                "text" => Some("multiline_text"),
                                "array" => Some("markdown_list"),
                                "yaml" => Some("yaml_object"),
                                "json" => Some("json_object"),
                                "object_array" => Some("headers"),
                                "map" => Some("map"),
                                _ => None,
                            };
                            if let Some(sv) = syntax_value {
                                if let Some(ref mut obj) = current_obj {
                                    obj.syntax.insert(header.id.clone(), sv.to_string());
                                    // For object_array types, store __array_kind
                                    if ft == "object_array" {
                                        if let Some(ref ak) = header.array_kind {
                                            obj.syntax
                                                .insert("__array_kind".to_string(), ak.clone());
                                        }
                                    }
                                }

                                // Capture content after heading as __comments (raw slice)
                                let content_start_offset = events[i].1.end;
                                let mut content_end_offset = markdown.len();
                                let mut scan_idx = i + 1;
                                while scan_idx < events.len() {
                                    let (ref ev, ref range) = events[scan_idx];
                                    if let Event::Start(Tag::Heading {
                                        level: next_lvl, ..
                                    }) = ev
                                    {
                                        let next_level = *next_lvl as u8;
                                        if next_level <= heading_level {
                                            content_end_offset = range.start;
                                            break;
                                        }
                                        // For object_array, capture ALL deeper content as comments
                                        // (don't stop at child headings)
                                        if ft == "object_array" {
                                            scan_idx += 1;
                                            continue;
                                        }
                                        // Also stop at deeper headings with non-field-type kind
                                        // (structured objects that should be parsed separately)
                                        let next_heading_text = {
                                            let mut txt = String::new();
                                            let mut j = scan_idx + 1;
                                            while j < events.len() {
                                                match &events[j].0 {
                                                    Event::Text(t) => txt.push_str(t),
                                                    Event::Code(c) => {
                                                        txt.push('`');
                                                        txt.push_str(c);
                                                        txt.push('`');
                                                    }
                                                    Event::End(TagEnd::Heading(_)) => break,
                                                    _ => {}
                                                }
                                                j += 1;
                                            }
                                            txt
                                        };
                                        let next_header =
                                            parse_header(&next_heading_text, &mut rng);
                                        // Only stop at deeper headings with explicit [[id]]
                                        // that create structure. Headings without [[id]] are
                                        // just markdown content (matching Python/TS behavior).
                                        if next_header.has_explicit_id {
                                            let nh_ft = next_header.field_type.as_deref();
                                            if nh_ft != Some("text") && nh_ft != Some("array") {
                                                content_end_offset = range.start;
                                                break;
                                            }
                                        }
                                    }
                                    scan_idx += 1;
                                }

                                let raw_content =
                                    markdown[content_start_offset..content_end_offset].trim();
                                if !raw_content.is_empty() {
                                    if let Some(ref mut obj) = current_obj {
                                        let mut comment_map = IndexMap::new();
                                        comment_map
                                            .insert("after".to_string(), "__self".to_string());
                                        comment_map
                                            .insert("content".to_string(), raw_content.to_string());
                                        obj.comments.push(comment_map);
                                    }
                                }

                                // Finalize and skip to scan position
                                if let Some(obj) = current_obj.take() {
                                    finalize_object(
                                        &mut objects_map,
                                        &mut duplicate_objects,
                                        &mut parsing_errors,
                                        &mut first_seen_lines,
                                        obj,
                                    );
                                }
                                i = scan_idx;
                                continue;
                            }
                        }
                    }
                }
            }

            Event::Text(text) => {
                if in_code_block {
                    // Collect code block content for YAML multiline support
                    code_block_content.push_str(text);
                } else if in_heading {
                    heading_text.push_str(text);
                } else if in_table_cell {
                    table_cell_text.push_str(text);
                } else if in_link {
                    // Collect link text (will be formatted as [text](url) when link ends)
                    link_text.push_str(text);
                } else if in_list_item {
                    list_item_text.push_str(text);
                } else if in_paragraph {
                    paragraph_text.push_str(text);
                } else if let Some(ref mut parts) = pending_text_block {
                    // Collect text for text block
                    if !parts.is_empty() {
                        let last = parts.last_mut().unwrap();
                        if !last.ends_with('\n') {
                            last.push_str(text);
                        } else {
                            parts.push(text.to_string());
                        }
                    }
                }
            }

            Event::Code(_code) => {
                // Use raw markdown slice to preserve original backtick count and spacing
                let raw_code = &markdown[range.start..range.end];

                if in_heading {
                    heading_text.push_str(raw_code);
                } else if in_table_cell {
                    table_cell_text.push_str(raw_code);
                } else if in_list_item {
                    list_item_text.push_str(raw_code);
                } else if in_paragraph {
                    paragraph_text.push_str(raw_code);
                }
            }

            Event::Start(Tag::Strong) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push_str("**");
                } else if in_paragraph {
                    paragraph_text.push_str("**");
                }
            }

            Event::End(TagEnd::Strong) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push_str("**");
                } else if in_paragraph {
                    paragraph_text.push_str("**");
                }
            }

            Event::Start(Tag::Emphasis) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push('*');
                } else if in_paragraph {
                    paragraph_text.push('*');
                }
            }

            Event::End(TagEnd::Emphasis) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push('*');
                } else if in_paragraph {
                    paragraph_text.push('*');
                }
            }

            Event::Start(Tag::Strikethrough) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push_str("~~");
                } else if in_paragraph {
                    paragraph_text.push_str("~~");
                }
            }

            Event::End(TagEnd::Strikethrough) => {
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push_str("~~");
                } else if in_paragraph {
                    paragraph_text.push_str("~~");
                }
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = true;
                link_url = dest_url.to_string();
                link_text.clear();
            }

            Event::End(TagEnd::Link) => {
                in_link = false;
                let formatted = format!("[{}]({})", link_text, link_url);
                // Check in_list_item FIRST (same order as Event::Text)
                if in_list_item {
                    list_item_text.push_str(&formatted);
                } else if in_paragraph {
                    paragraph_text.push_str(&formatted);
                }
                link_text.clear();
                link_url.clear();
            }

            Event::SoftBreak | Event::HardBreak => {
                if in_list_item {
                    list_item_text.push(' ');
                } else if in_paragraph {
                    paragraph_text.push('\n');
                }
            }

            Event::TaskListMarker(checked) => {
                // Add checkbox marker to list item text
                if in_list_item {
                    let marker = if *checked { "[x] " } else { "[ ] " };
                    list_item_text.insert_str(0, marker);
                }
            }

            Event::Start(Tag::List(order)) => {
                // When inside text field and list starts, mark beginning of list
                if pending_text_field.is_some() && !in_text_field_list {
                    in_text_field_list = true;
                }

                // Track raw start for TextBlock list extraction
                if pending_text_block.is_some() && current_obj.is_none() && list_nesting_level == 0
                {
                    textblock_list_raw_start = Some(range.start);
                }

                // Save comment anchor at the start of outermost list
                if list_nesting_level == 0 {
                    list_raw_start = Some(range.start);
                    if let Some(ref obj) = current_obj {
                        list_comment_anchor = Some(obj.comment_anchor.clone());
                    }
                }

                // If we're inside a list item and starting a nested list,
                // save the current list item text as a parent item
                if in_list_item && !list_item_text.trim().is_empty() && current_obj.is_some() {
                    let trimmed = list_item_text.trim();
                    // Check if it's not a field
                    // IMPORTANT: Only bullet list items can be fields.
                    // Ordered list items (1. 2. 3.) are always comment content.
                    // Items inside a yaml_multiline pipe field are raw content, not fields.
                    let first_line = trimmed.lines().next().unwrap_or(trimmed);
                    if current_list_order.is_none()
                        && pending_yaml_multiline_pipe_field.is_none()
                        && field_re.is_match(first_line)
                    {
                        // Field with empty value followed by nested list → yaml_multiline_list
                        // OR field with pipe value followed by nested list → yaml_multiline
                        if let Some(caps) = field_re.captures(first_line) {
                            let fname = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                            let fval = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
                            // Check for pipe: either fval is exactly "|" (simple case)
                            // or fval starts with "| " (pulldown-cmark merged paragraphs after pipe)
                            // In both cases, verify against raw source that it's truly "field: |\n"
                            let is_pipe_field =
                                if !fname.is_empty() && (fval == "|" || fval.starts_with("| ")) {
                                    // Verify in raw source
                                    if let Some(start_offset) = list_item_start {
                                        let raw_item =
                                            block_tree.source.get(start_offset..).unwrap_or("");
                                        let pattern = format!("{}: |", fname);
                                        if let Some(pipe_pos) = raw_item.find(&pattern) {
                                            let after = &raw_item[pipe_pos + pattern.len()..];
                                            after.starts_with('\n') || after.starts_with("\r\n")
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };
                            if is_pipe_field {
                                // yaml_multiline pipe: use raw-slice to extract content
                                // The nested list content is the multiline value
                                pending_yaml_multiline_pipe_field = Some(fname.to_string());
                                pending_yaml_multiline_pipe_start = list_item_start;
                                if let Some(ref mut obj) = current_obj {
                                    // Defensive: flush any accumulated non-field items.
                                    // In practice this is unreachable — comment_list_items
                                    // are flushed at list boundaries before nesting increases,
                                    // and pipe fields are detected at nesting level 1.
                                    if !comment_list_items.is_empty() {
                                        let list_content = if !obj.fields.is_empty() {
                                            comment_list_items.join("\n\n")
                                        } else {
                                            comment_list_items.join("\n")
                                        };
                                        let anchor = obj.comment_anchor.clone();
                                        let should_merge = obj
                                            .comments
                                            .last()
                                            .map(|c| c.get("after") == Some(&anchor))
                                            .unwrap_or(false);
                                        if should_merge {
                                            if let Some(last_comment) = obj.comments.last_mut() {
                                                if let Some(existing) =
                                                    last_comment.get_mut("content")
                                                {
                                                    *existing =
                                                        format!("{}\n\n{}", existing, list_content);
                                                }
                                            }
                                        } else {
                                            let mut comment = IndexMap::new();
                                            comment.insert("after".to_string(), anchor);
                                            comment.insert("content".to_string(), list_content);
                                            obj.comments.push(comment);
                                        }
                                        comment_list_items.clear();
                                        comment_list_raw_start = None;
                                        comment_list_raw_end = None;
                                    }
                                    obj.comment_anchor = fname.to_string();
                                    if let Some(start_offset) = list_item_start {
                                        let item_line = get_line(start_offset);
                                        let line_text =
                                            lines.get(item_line as usize - 1).unwrap_or(&"");
                                        let col = line_text.find(fname).unwrap_or(0) as u32;
                                        obj.positions.insert(fname.to_string(), (item_line, col));
                                    }
                                }
                            } else if fval.is_empty() && !fname.is_empty() {
                                pending_multiline_list_field = Some(fname.to_string());
                                // Store the field with null value for now; will be replaced
                                // when the nested list ends
                                if let Some(ref mut obj) = current_obj {
                                    // Flush any accumulated non-field items first
                                    if !comment_list_items.is_empty() {
                                        let list_content = if !obj.fields.is_empty() {
                                            comment_list_items.join("\n\n")
                                        } else {
                                            comment_list_items.join("\n")
                                        };
                                        let anchor = obj.comment_anchor.clone();
                                        let should_merge = obj
                                            .comments
                                            .last()
                                            .map(|c| c.get("after") == Some(&anchor))
                                            .unwrap_or(false);
                                        if should_merge {
                                            if let Some(last_comment) = obj.comments.last_mut() {
                                                if let Some(existing) =
                                                    last_comment.get_mut("content")
                                                {
                                                    *existing =
                                                        format!("{}\n\n{}", existing, list_content);
                                                }
                                            }
                                        } else {
                                            let mut comment = IndexMap::new();
                                            comment.insert("after".to_string(), anchor);
                                            comment.insert("content".to_string(), list_content);
                                            obj.comments.push(comment);
                                        }
                                        comment_list_items.clear();
                                        comment_list_raw_start = None;
                                        comment_list_raw_end = None;
                                    }
                                    obj.comment_anchor = fname.to_string();
                                    // Track field position
                                    if let Some(start_offset) = list_item_start {
                                        let item_line = get_line(start_offset);
                                        let line_text =
                                            lines.get(item_line as usize - 1).unwrap_or(&"");
                                        let col = line_text.find(fname).unwrap_or(0) as u32;
                                        obj.positions.insert(fname.to_string(), (item_line, col));
                                    }
                                }
                            }
                        }
                    } else {
                        if comment_list_raw_start.is_none() {
                            comment_list_raw_start = list_item_start;
                        }
                        let item_prefix = if current_list_order.is_some() {
                            format!("{}.", current_list_item_num)
                        } else {
                            "-".to_string()
                        };
                        // Add indentation for nested lists (3 spaces for proper markdown nesting)
                        let indent = if list_nesting_level > 1 {
                            "   ".repeat(list_nesting_level - 1)
                        } else {
                            String::new()
                        };
                        comment_list_items.push(format!("{}{} {}", indent, item_prefix, trimmed));
                    }
                    list_item_text.clear();
                }

                // Track nesting level for comments
                list_order_stack.push(*order);
                list_nesting_level = list_order_stack.len();
                // Track whether this is an ordered list (Some(start_num)) or unordered (None)
                current_list_order = *order;
                current_list_item_num = order.unwrap_or(1);

                // Detect ordered list under array field — forbidden per rule_no_ordered_list_array
                // Only trigger if the array is still empty (no bullet list consumed yet)
                // If array already has items, this ordered list is a trailing comment
                if list_nesting_level == 1 && order.is_some() {
                    if let Some((ref parent_id, ref field_name, _, ref field_type)) =
                        pending_text_field
                    {
                        if field_type == "array" {
                            let array_is_empty = current_obj
                                .as_ref()
                                .and_then(|obj| obj.fields.get(field_name))
                                .and_then(|v| v.as_array())
                                .map(|a| a.is_empty())
                                .unwrap_or_else(|| {
                                    objects_map
                                        .get(parent_id)
                                        .and_then(|p| p.get(field_name))
                                        .and_then(|v| v.as_array())
                                        .map(|a| a.is_empty())
                                        .unwrap_or(true)
                                });
                            if array_is_empty {
                                ordered_list_in_array_error = true;
                                ordered_list_error_line = Some(get_line(range.start));
                            } else {
                                // Array already populated — ordered list is a trailing comment
                                // Set flag to prevent items from being pushed into array
                                ordered_list_in_array_error = true;
                                ordered_list_error_line = None; // trailing comment, no error to emit
                            }
                        }
                    }
                }
            }

            Event::End(TagEnd::List(_)) => {
                in_text_field_list = false;

                // Pop from nesting stack
                list_order_stack.pop();
                list_nesting_level = list_order_stack.len();
                current_list_order = list_order_stack.last().copied().flatten();

                // Finalize nested_subitems error when exiting nested list
                // Pattern `- key:\n  - item` is forbidden per rule_no_nested_subitems
                if list_nesting_level == 1 && !multiline_list_items.is_empty() {
                    if let Some(ref field_name) = pending_multiline_list_field {
                        if let Some(ref mut obj) = current_obj {
                            // Remove the field (it was stored with null value)
                            obj.fields.shift_remove(field_name);
                            obj.types.shift_remove(field_name);

                            // Get line number for error
                            let error_line =
                                obj.positions.get(field_name).map(|(l, _)| *l).unwrap_or(0);
                            obj.positions.shift_remove(field_name);

                            // Generate nested_subitems parsing error
                            let error_id = format!("error_{}", parsing_error_counter);
                            parsing_error_counter += 1;
                            let mut error = IndexMap::new();
                            error.insert("__id".to_string(), json!(error_id));
                            error.insert("__kind".to_string(), json!("__ParsingError"));
                            error.insert("type".to_string(), json!("nested_subitems"));
                            error.insert("field".to_string(), json!(field_name));
                            error.insert("object".to_string(), json!(format!("[[#{}]]", obj.id)));
                            error.insert("line".to_string(), json!(error_line));
                            parsing_errors.push(error);
                        }
                    }
                    multiline_list_items.clear();
                    pending_multiline_list_field = None;
                }

                // Finalize yaml_multiline pipe field when exiting nested list
                // NOTE: Don't extract here — the pipe field content may span multiple
                // nested lists within the same list item. Extraction happens at End(Item)
                // when the pipe field's own list item ends.

                // TextBlock: use raw slice extraction for outermost list
                if list_nesting_level == 0 && pending_text_block.is_some() && current_obj.is_none()
                {
                    if let Some(start) = textblock_list_raw_start.take() {
                        let raw_list = markdown[start..range.end].trim().to_string();
                        if !raw_list.is_empty() {
                            if let Some(ref mut parts) = pending_text_block {
                                parts.push(raw_list);
                            }
                        }
                        // Clear any accumulated list items since we used raw slice
                        comment_list_items.clear();
                        list_comment_anchor = None;
                        comment_list_raw_start = None;
                        comment_list_raw_end = None;
                        list_has_duplicate_keys = false;
                        list_inserted_field_keys.clear();
                        list_raw_start = None;
                    }
                }

                // Ordered list for [[field: array]] heading: capture raw content as __comments
                // with after=field_name for lossless rebuild (matching Python/TS behavior).
                if list_nesting_level == 0
                    && (!comment_list_items.is_empty() || ordered_list_in_array_error)
                {
                    if let Some((ref _parent_id, ref field_name, _, ref field_type)) =
                        pending_text_field
                    {
                        if field_type == "array" {
                            if let Some(ref mut obj) = current_obj {
                                // Use raw slice of the entire list
                                let raw_list = if let Some(start) = list_raw_start {
                                    block_tree
                                        .source
                                        .get(start..range.end)
                                        .unwrap_or("")
                                        .trim()
                                        .to_string()
                                } else {
                                    comment_list_items.join("\n")
                                };
                                if !raw_list.is_empty() {
                                    let mut comment = IndexMap::new();
                                    comment.insert("after".to_string(), field_name.clone());
                                    comment.insert("content".to_string(), raw_list);
                                    obj.comments.push(comment);
                                }
                            }
                            // Clear comment items — we've handled them
                            comment_list_items.clear();
                            list_comment_anchor = None;
                            comment_list_raw_start = None;
                            comment_list_raw_end = None;
                            list_has_duplicate_keys = false;
                            list_inserted_field_keys.clear();
                            list_raw_start = None;
                        }
                    }
                }

                // Emit ordered_list_in_array error when exiting the list
                if list_nesting_level == 0 && ordered_list_in_array_error {
                    // Only emit error if this was a real error (not trailing comment after populated array)
                    if let Some(err_line) = ordered_list_error_line {
                        if let Some((ref parent_id, ref field_name, _, _)) = pending_text_field {
                            let error_id = format!("error_{}", parsing_error_counter);
                            parsing_error_counter += 1;
                            let mut error = IndexMap::new();
                            error.insert("__id".to_string(), json!(error_id));
                            error.insert("__kind".to_string(), json!("__ParsingError"));
                            error.insert("type".to_string(), json!("ordered_list_in_array"));
                            error.insert("field".to_string(), json!(field_name));
                            error
                                .insert("object".to_string(), json!(format!("[[#{}]]", parent_id)));
                            error.insert("line".to_string(), json!(err_line));
                            parsing_errors.push(error);
                        }
                    }
                    ordered_list_in_array_error = false;
                    ordered_list_error_line = None;
                }

                // Save whether the current list inserted valid fields (before clearing)
                let current_list_had_valid_fields =
                    list_nesting_level == 0 && !list_inserted_field_keys.is_empty();

                // When a list has duplicate keys, rollback any fields inserted during this list
                // and treat the entire list as comment content (matching Python/TS behavior).
                if list_nesting_level == 0
                    && list_has_duplicate_keys
                    && !list_inserted_field_keys.is_empty()
                {
                    if let Some(ref mut obj) = current_obj {
                        // Remove fields that were inserted during this list
                        for key in &list_inserted_field_keys {
                            obj.fields.shift_remove(key.as_str());
                            obj.types.shift_remove(key.as_str());
                            obj.syntax.shift_remove(key.as_str());
                            obj.positions.shift_remove(key.as_str());
                        }
                        // Restore comment_anchor to what it was before the list
                        if let Some(ref anchor) = list_comment_anchor {
                            obj.comment_anchor = anchor.clone();
                        }
                        // Use raw slice of the entire list as comment content
                        if let Some(start) = list_raw_start {
                            let raw_list = block_tree
                                .source
                                .get(start..range.end)
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            if !raw_list.is_empty() {
                                let anchor = list_comment_anchor
                                    .clone()
                                    .unwrap_or_else(|| obj.comment_anchor.clone());
                                let mut comment = IndexMap::new();
                                comment.insert("after".to_string(), anchor);
                                comment.insert("content".to_string(), raw_list);
                                obj.comments.push(comment);
                            }
                        }
                    }
                    // Clear everything — we've handled the entire list
                    comment_list_items.clear();
                    list_comment_anchor = None;
                    comment_list_raw_start = None;
                    comment_list_raw_end = None;
                    list_has_duplicate_keys = false;
                    list_inserted_field_keys.clear();
                    list_raw_start = None;
                }

                // Only flush to comment when we exit the outermost list
                if list_nesting_level == 0 && !comment_list_items.is_empty() {
                    if let Some(ref mut obj) = current_obj {
                        // Preserve original markdown list indentation and inline code markers
                        let list_content = if let Some(start) = comment_list_raw_start {
                            let end = comment_list_raw_end.unwrap_or(range.end);
                            block_tree
                                .source
                                .get(start..end)
                                .unwrap_or("")
                                .trim()
                                .to_string()
                        } else {
                            comment_list_items.join("\n")
                        };

                        // Use the anchor saved at list start, UNLESS valid fields were parsed
                        // in this list (then use current anchor which tracks last valid field)
                        let anchor = if !obj.fields.is_empty() {
                            obj.comment_anchor.clone()
                        } else {
                            list_comment_anchor
                                .clone()
                                .unwrap_or_else(|| obj.comment_anchor.clone())
                        };

                        // Check if we can append to the last comment with the same anchor
                        // BUT: don't append if the list contains duplicate keys —
                        // duplicate-key lists should be separate comments (matching Python/TS).
                        let should_append = !list_has_duplicate_keys
                            && obj
                                .comments
                                .last()
                                .map(|c| c.get("after") == Some(&anchor))
                                .unwrap_or(false);

                        if should_append {
                            // Append list to existing comment
                            if let Some(last_comment) = obj.comments.last_mut() {
                                if let Some(existing) = last_comment.get_mut("content") {
                                    *existing = format!("{}\n\n{}", existing, list_content);
                                }
                            }
                        } else {
                            // Create new comment with list
                            let mut comment = IndexMap::new();
                            comment.insert("after".to_string(), anchor);
                            comment.insert("content".to_string(), list_content);
                            obj.comments.push(comment);
                        }
                    }
                    comment_list_items.clear();
                    list_comment_anchor = None;
                    comment_list_raw_start = None;
                    comment_list_raw_end = None;
                    list_has_duplicate_keys = false;
                    list_inserted_field_keys.clear();
                    list_raw_start = None;
                }

                // Emit mixed_field_keys error when outermost list ends
                if list_nesting_level == 0 && mixed_field_has_invalid {
                    if let Some(ref obj) = current_obj {
                        // Only emit if THIS list had valid fields (truly mixed list)
                        // or if the object has fields and this list had invalid field-like items
                        if current_list_had_valid_fields || !obj.fields.is_empty() {
                            let error_id = format!("error_{}", parsing_error_counter);
                            parsing_error_counter += 1;
                            let mut error = IndexMap::new();
                            error.insert("__id".to_string(), json!(error_id));
                            error.insert("__kind".to_string(), json!("__ParsingError"));
                            error.insert("type".to_string(), json!("mixed_field_keys"));
                            error.insert("object".to_string(), json!(format!("[[#{}]]", obj.id)));
                            error.insert(
                                "line".to_string(),
                                json!(mixed_field_invalid_line.unwrap_or(0)),
                            );
                            parsing_errors.push(error);
                        }
                    }
                    mixed_field_has_invalid = false;
                    mixed_field_invalid_line = None;
                }

                // Always clear per-list tracking when outermost list ends
                if list_nesting_level == 0 {
                    list_inserted_field_keys.clear();
                    list_raw_start = None;
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_content.clear();
                code_block_start_line = get_line(range.start) as usize;
                code_block_start_offset = range.start;
                code_block_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
            }

            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let fence = if code_block_lang.is_empty() {
                    "```".to_string()
                } else {
                    format!("```{}", code_block_lang)
                };
                let code_text = format!("{}\n{}\n```", fence, code_block_content.trim_end());
                let code_lines = code_text.lines().count();

                // If inside list item, add code block as text (for YAML multiline support)
                if in_list_item {
                    if !list_item_text.is_empty() {
                        list_item_text.push('\n');
                    }
                    // Add indentation for code blocks inside list items
                    // Each nesting level needs 3 spaces (for "1. " or "- " alignment)
                    let indent = "   ".repeat(list_nesting_level.max(1));
                    let indented_code = code_text
                        .lines()
                        .map(|line| format!("{}{}", indent, line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    list_item_text.push_str(&indented_code);
                } else if let Some((ref parent_id, ref field_name, _, ref field_type)) =
                    pending_text_field
                {
                    // Code block inside text/yaml/json field
                    if let Some(parent) = objects_map.get_mut(parent_id) {
                        // Check if this is an array field
                        let is_array_field = parent
                            .get(field_name)
                            .map(|v| v.is_array())
                            .unwrap_or(false);

                        if !is_array_field {
                            if field_type == "yaml" {
                                // Parse YAML content
                                if let Ok(yaml_value) =
                                    serde_yaml::from_str::<serde_json::Value>(&code_block_content)
                                {
                                    parent.insert(field_name.clone(), yaml_value);

                                    // Set __syntax to yaml_object
                                    let syntax = parent
                                        .entry("__syntax".to_string())
                                        .or_insert_with(|| json!({}));
                                    if let Some(obj) = syntax.as_object_mut() {
                                        obj.insert(field_name.clone(), json!("yaml_object"));
                                    }

                                    // Remove from __types (it's an object, not a string)
                                    if let Some(types) = parent.get_mut("__types") {
                                        if let Some(obj) = types.as_object_mut() {
                                            obj.remove(field_name);
                                        }
                                    }
                                } else {
                                    // Fall back to string if YAML parsing fails
                                    parent.insert(field_name.clone(), json!(code_text.clone()));
                                }
                            } else if field_type == "json" {
                                // Parse JSON content
                                if let Ok(json_value) =
                                    serde_json::from_str::<serde_json::Value>(&code_block_content)
                                {
                                    parent.insert(field_name.clone(), json_value);

                                    // Set __syntax to json_object
                                    let syntax = parent
                                        .entry("__syntax".to_string())
                                        .or_insert_with(|| json!({}));
                                    if let Some(obj) = syntax.as_object_mut() {
                                        obj.insert(field_name.clone(), json!("json_object"));
                                    }

                                    // Remove from __types (it's an object, not a string)
                                    if let Some(types) = parent.get_mut("__types") {
                                        if let Some(obj) = types.as_object_mut() {
                                            obj.remove(field_name);
                                        }
                                    }
                                } else {
                                    // Fall back to string if JSON parsing fails
                                    parent.insert(field_name.clone(), json!(code_text.clone()));
                                }
                            } else {
                                // Regular text field - append code block
                                let existing = parent
                                    .get(field_name)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let new_val = if existing.is_empty() {
                                    code_text.clone()
                                } else {
                                    format!("{}\n\n{}", existing, code_text)
                                };
                                parent.insert(field_name.clone(), json!(new_val));

                                // Ensure __types and __syntax
                                let types = parent
                                    .entry("__types".to_string())
                                    .or_insert_with(|| json!({}));
                                if let Some(obj) = types.as_object_mut() {
                                    if !obj.contains_key(field_name) {
                                        obj.insert(field_name.clone(), json!("string"));
                                    }
                                }

                                let syntax = parent
                                    .entry("__syntax".to_string())
                                    .or_insert_with(|| json!({}));
                                if let Some(obj) = syntax.as_object_mut() {
                                    if !obj.contains_key(field_name) {
                                        obj.insert(field_name.clone(), json!("multiline_text"));
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(ref mut obj) = current_obj {
                    // Code block as comment content
                    // Capture if object has fields, prior comments, or explicit Kind
                    let has_same_anchor_comment = obj
                        .comments
                        .last()
                        .map(|c| c.get("after") == Some(&obj.comment_anchor))
                        .unwrap_or(false);
                    let has_explicit_kind = obj.kind.is_some();

                    if !obj.fields.is_empty() || has_same_anchor_comment || has_explicit_kind {
                        // Preserve original markdown fences (``` vs ```` etc.)
                        let raw_code_text = block_tree
                            .source
                            .get(code_block_start_offset..range.end)
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        // Check if we can append to the last comment with the same anchor
                        let should_append = obj
                            .comments
                            .last()
                            .map(|c| c.get("after") == Some(&obj.comment_anchor))
                            .unwrap_or(false);

                        if should_append {
                            // Append code block to existing comment
                            if let Some(last_comment) = obj.comments.last_mut() {
                                if let Some(existing) = last_comment.get_mut("content") {
                                    *existing = format!("{}\n\n{}", existing, raw_code_text);
                                }
                            }
                        } else {
                            // Create new comment with code block
                            let mut comment = IndexMap::new();
                            comment.insert("after".to_string(), obj.comment_anchor.clone());
                            comment.insert("content".to_string(), raw_code_text);
                            obj.comments.push(comment);
                        }
                    }
                } else if pending_text_block.is_some() {
                    // Add code block to text block (not inside QMD.md object)
                    // Calculate offset within content (0-based line number)
                    let offset_line = if let Some(ref parts) = pending_text_block {
                        // Count lines in existing content + 1 for the blank line separator (\n\n)
                        let existing_content = parts.join("\n\n");
                        if existing_content.is_empty() {
                            0
                        } else {
                            existing_content.lines().count() + 1
                        }
                    } else {
                        0
                    };

                    // Add code fence metadata
                    pending_code_fences.push(CodeFenceInfo {
                        lang: code_block_lang.clone(),
                        offset_line,
                        length_lines: code_lines,
                    });

                    // Add code block text to pending text block
                    if let Some(ref mut parts) = pending_text_block {
                        parts.push(code_text);
                    }
                } else {
                    // Code block outside any context - create text block
                    pending_text_block = Some(vec![code_text]);
                    pending_text_block_line = code_block_start_line;
                    pending_text_block_level = 0;

                    // Add code fence metadata
                    pending_code_fences.push(CodeFenceInfo {
                        lang: code_block_lang.clone(),
                        offset_line: 0,
                        length_lines: code_lines,
                    });
                }
                code_block_content.clear();
                code_block_lang.clear();
            }

            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_start_offset = range.start;
                table_rows.clear();
                current_table_row.clear();
            }

            Event::End(TagEnd::Table) => {
                in_table = false;

                // Table inside TextBlock — use raw-slice extraction
                if pending_text_block.is_some()
                    && current_obj.is_none()
                    && pending_text_field.is_none()
                {
                    let raw_table =
                        raw_table_slice(&block_tree.source, table_start_offset, range.end);
                    if !raw_table.is_empty() {
                        if let Some(ref mut parts) = pending_text_block {
                            parts.push(raw_table);
                        }
                    }
                    table_rows.clear();
                    i += 1;
                    continue;
                }

                // Table inside object array context — parse into child objects
                if let Some((ref arr_parent_id, ref arr_field, ref arr_kind, _arr_level)) =
                    pending_object_array
                {
                    if !table_rows.is_empty() {
                        // Update parent syntax and types
                        if let Some(parent) = objects_map.get_mut(arr_parent_id) {
                            if let Some(syntax_obj) = parent.get_mut("__syntax") {
                                if let Some(syntax_map) = syntax_obj.as_object_mut() {
                                    syntax_map.insert(arr_field.clone(), json!("table"));
                                }
                            }
                            if let Some(types_obj) = parent.get_mut("__types") {
                                if let Some(types_map) = types_obj.as_object_mut() {
                                    types_map.insert(arr_field.clone(), json!("array"));
                                }
                            } else {
                                let mut types_map = serde_json::Map::new();
                                types_map.insert(arr_field.clone(), json!("array"));
                                parent.insert("__types".to_string(), json!(types_map));
                            }
                        }

                        // Create child objects and wire them to parent
                        let children = create_table_child_objects(
                            &table_rows,
                            arr_parent_id,
                            arr_field,
                            arr_kind,
                            &objects_map,
                        );
                        for (obj_id, element) in children {
                            if let Some(parent) = objects_map.get_mut(arr_parent_id) {
                                if let Some(arr) = parent.get_mut(arr_field) {
                                    if let Some(arr_vec) = arr.as_array_mut() {
                                        arr_vec.push(json!(format!("[[#{}]]", obj_id)));
                                    }
                                }
                            }
                            objects_map.insert(obj_id, element);
                        }
                    }
                    table_rows.clear();
                    i += 1;
                    continue;
                }

                // Convert table to markdown and add to text field
                if let Some((ref parent_id, ref field_name, _, _)) = pending_text_field {
                    if !table_rows.is_empty() {
                        // Preserve the original markdown verbatim (separators, cell
                        // spacing) by slicing the raw source, instead of
                        // reconstructing the table — reconstruction would normalize
                        // the separator row (e.g. `|--------|` -> `|---|`) and diverge
                        // from the Python/TS parsers.
                        let table_md =
                            raw_table_slice(&block_tree.source, table_start_offset, range.end);

                        // Add to text field
                        if let Some(parent) = objects_map.get_mut(parent_id) {
                            let existing = parent
                                .get(field_name)
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let new_val = if existing.is_empty() {
                                table_md.trim().to_string()
                            } else {
                                format!("{}\n\n{}", existing, table_md.trim())
                            };
                            parent.insert(field_name.clone(), json!(new_val));
                        }
                    }
                } else if pending_object_array.is_none() {
                    // Table as comment content (only if not in object array context)
                    if let Some(ref mut obj) = current_obj {
                        // Add table to comments if:
                        // 1. Object has fields (table is supplementary content), OR
                        // 2. There's a comment with the same anchor (table follows text), OR
                        // 3. Object has explicit Kind (like Section) — table is always content
                        let has_same_anchor_comment = obj
                            .comments
                            .last()
                            .map(|c| c.get("after") == Some(&obj.comment_anchor))
                            .unwrap_or(false);
                        let has_explicit_kind = obj.kind.is_some();

                        if (!obj.fields.is_empty() || has_same_anchor_comment || has_explicit_kind)
                            && !table_rows.is_empty()
                        {
                            // Preserve original markdown separators and spacing
                            let table_content =
                                raw_table_slice(&block_tree.source, table_start_offset, range.end);

                            // Check if we can append to the last comment with the same anchor
                            let should_append = obj
                                .comments
                                .last()
                                .map(|c| c.get("after") == Some(&obj.comment_anchor))
                                .unwrap_or(false);

                            if should_append {
                                // Append table to existing comment
                                if let Some(last_comment) = obj.comments.last_mut() {
                                    if let Some(existing) = last_comment.get_mut("content") {
                                        *existing = format!("{}\n\n{}", existing, table_content);
                                    }
                                }
                            } else {
                                // Create new comment with table
                                let mut comment = IndexMap::new();
                                comment.insert("after".to_string(), obj.comment_anchor.clone());
                                comment.insert("content".to_string(), table_content);
                                obj.comments.push(comment);
                            }
                        }
                    }
                }

                table_rows.clear();
            }

            Event::Start(Tag::TableHead) => {}
            Event::End(TagEnd::TableHead) => {
                // Header row is complete, add it
                if in_table && !current_table_row.is_empty() {
                    table_rows.push(current_table_row.clone());
                    current_table_row.clear();
                }
            }
            Event::Start(Tag::TableRow) => {
                current_table_row.clear();
            }

            Event::End(TagEnd::TableRow) => {
                if in_table {
                    table_rows.push(current_table_row.clone());
                }
            }

            Event::Start(Tag::TableCell) => {
                in_table_cell = true;
                table_cell_text.clear();
            }

            Event::End(TagEnd::TableCell) => {
                in_table_cell = false;
                if in_table {
                    current_table_row.push(table_cell_text.trim().to_string());
                }
            }

            Event::Start(Tag::Item) => {
                in_list_item = true;
                list_item_text.clear();
                list_item_start = Some(range.start);
            }

            Event::End(TagEnd::Item) => {
                // Finalize yaml_multiline pipe field when its list item ends
                // Pattern `- field: |\n  content\n  - nested list\n  more content`
                // The pipe field content spans the entire list item, which may contain
                // multiple nested lists, paragraphs, code fences, etc.
                if list_nesting_level == 1 {
                    if let Some(field_name) = pending_yaml_multiline_pipe_field.take() {
                        if let Some(ref mut obj) = current_obj {
                            if let Some(start_offset) = pending_yaml_multiline_pipe_start.take() {
                                let raw_item =
                                    block_tree.source.get(start_offset..range.end).unwrap_or("");
                                let pattern = format!("{}: |", field_name);
                                if let Some(pipe_pos) = raw_item.find(&pattern) {
                                    let after_pattern = &raw_item[pipe_pos + pattern.len()..];
                                    if after_pattern.starts_with('\n')
                                        || after_pattern.starts_with("\r\n")
                                    {
                                        let after_pipe =
                                            after_pattern.trim_start_matches(['\n', '\r']);
                                        let content_lines: Vec<&str> = after_pipe.lines().collect();
                                        if !content_lines.is_empty() {
                                            let min_indent = content_lines
                                                .iter()
                                                .filter(|l| !l.trim().is_empty())
                                                .map(|l| l.len() - l.trim_start().len())
                                                .min()
                                                .unwrap_or(0);
                                            let dedented: Vec<&str> = content_lines
                                                .iter()
                                                .map(|l| {
                                                    if l.len() >= min_indent {
                                                        &l[min_indent..]
                                                    } else {
                                                        *l
                                                    }
                                                })
                                                .collect();
                                            let value = dedented.join("\n").trim().to_string();
                                            obj.fields.insert(field_name.clone(), json!(value));
                                            obj.types
                                                .insert(field_name.clone(), "string".to_string());
                                            obj.syntax
                                                .insert(field_name, "yaml_multiline".to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                in_list_item = false;
                let trimmed = list_item_text.trim();

                // Handle text field collection
                if let Some((ref parent_id, ref field_name, _, _)) = pending_text_field {
                    // Check if this is a heading-level array field (parent is still current_obj)
                    let is_array_on_current = current_obj
                        .as_ref()
                        .map(|o| {
                            o.id == *parent_id
                                && o.syntax
                                    .get(field_name)
                                    .map(|s| s == "markdown_list")
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);

                    if is_array_on_current {
                        // Skip pushing items if this is an ordered list in array (forbidden)
                        if !ordered_list_in_array_error {
                            // Write array items directly to current_obj
                            if let Some(ref mut obj) = current_obj {
                                if let Some(arr) = obj.fields.get_mut(field_name) {
                                    if let Some(arr_vec) = arr.as_array_mut() {
                                        arr_vec.push(json!(trimmed));

                                        // Extract references from list item
                                        if let Some(start_offset) = list_item_start {
                                            let item_line = get_line(start_offset);
                                            let raw_item = block_tree
                                                .source
                                                .get(start_offset..range.end)
                                                .unwrap_or("");
                                            for line_text in raw_item.lines() {
                                                for r in extract_references_from_line(
                                                    line_text, item_line,
                                                ) {
                                                    obj.references.push(r);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(parent) = objects_map.get_mut(parent_id) {
                        // Non-array text field — parent was finalized into objects_map
                        let is_array_field = parent
                            .get("__syntax")
                            .and_then(|s| s.as_object())
                            .and_then(|obj| obj.get(field_name))
                            .and_then(|v| v.as_str())
                            .map(|s| s == "markdown_list")
                            .unwrap_or(false);

                        if is_array_field && !ordered_list_in_array_error {
                            // Fallback: array field on a finalized parent (e.g. inline array)
                            if let Some(arr) = parent.get_mut(field_name) {
                                if let Some(arr_vec) = arr.as_array_mut() {
                                    arr_vec.push(json!(trimmed));

                                    if let Some(start_offset) = list_item_start {
                                        let item_line = get_line(start_offset);
                                        let raw_item = block_tree
                                            .source
                                            .get(start_offset..range.end)
                                            .unwrap_or("");
                                        for line_text in raw_item.lines() {
                                            for r in
                                                extract_references_from_line(line_text, item_line)
                                            {
                                                if let Some(refs) = parent.get_mut("__references") {
                                                    if let Some(refs_arr) = refs.as_array_mut() {
                                                        refs_arr.push(
                                                            serde_json::to_value(&r).unwrap(),
                                                        );
                                                    }
                                                } else {
                                                    let refs_vec =
                                                        vec![serde_json::to_value(&r).unwrap()];
                                                    parent.insert(
                                                        "__references".to_string(),
                                                        json!(refs_vec),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Collect list items as text (existing behavior)
                            let existing = parent
                                .get(field_name)
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            // Check if we're already in a list (existing ends with list item content)
                            // List items look like "- content" or "1. content" with optional trailing newlines
                            let already_in_list = existing.contains("\n- ")
                                || existing.starts_with("- ")
                                || existing
                                    .lines()
                                    .last()
                                    .map(|l| {
                                        l.chars()
                                            .next()
                                            .map(|c| c.is_ascii_digit())
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                            let separator = if existing.is_empty() {
                                ""
                            } else if already_in_list {
                                // Continuing a list - single newline
                                "\n"
                            } else {
                                // First list item after paragraph - double newline (blank line in markdown)
                                "\n\n"
                            };
                            // Format based on list type
                            let item_prefix = if current_list_order.is_some() {
                                format!("{}.", current_list_item_num)
                            } else {
                                "-".to_string()
                            };
                            let new_val =
                                format!("{}{}{} {}", existing, separator, item_prefix, trimmed);
                            parent.insert(field_name.clone(), json!(new_val));
                            // Increment item number for ordered lists
                            current_list_item_num += 1;
                        }
                    }
                } else if let Some(ref mut obj) = current_obj {
                    // Parse as field - only match first line for field detection
                    // IMPORTANT: Only bullet list items can be fields.
                    // Ordered list items (1. 2. 3.) are always comment content.
                    // Items inside a yaml_multiline pipe field are raw content, not fields.
                    let first_line = trimmed.lines().next().unwrap_or(trimmed);
                    if current_list_order.is_none()
                        && pending_yaml_multiline_pipe_field.is_none()
                        && field_re.is_match(first_line)
                    {
                        let caps = field_re.captures(first_line).unwrap();
                        let field_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        let field_value_str = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                        // Check if this field key already exists in the object,
                        // OR if a duplicate was already found in this list.
                        // If so, treat as comment content — the entire list with
                        // any duplicate key becomes comment (matching TS/Python).
                        if obj.fields.contains_key(field_name) || list_has_duplicate_keys {
                            // Treat as comment text (same as non-field item)
                            let item_text = format!("- {}", trimmed);
                            comment_list_items.push(item_text);
                            if comment_list_raw_start.is_none() {
                                comment_list_raw_start = list_item_start;
                            }
                            comment_list_raw_end = Some(range.end);
                            if obj.fields.contains_key(field_name) {
                                list_has_duplicate_keys = true;
                            }
                        } else {
                            // Handle YAML multiline: `field: |` followed by indented content
                            let mut yaml_multiline_value: Option<String> = None;
                            if let Some(start_offset) = list_item_start {
                                // Get raw markdown for this list item
                                let raw_item =
                                    block_tree.source.get(start_offset..range.end).unwrap_or("");
                                // Check if it's YAML multiline: "field: |" followed by newline
                                let pattern = format!("{}: |", field_name);
                                if let Some(pipe_pos) = raw_item.find(&pattern) {
                                    let after_pattern = &raw_item[pipe_pos + pattern.len()..];
                                    // Must have newline after pipe (YAML multiline indicator)
                                    if after_pattern.starts_with('\n')
                                        || after_pattern.starts_with("\r\n")
                                    {
                                        let after_pipe =
                                            after_pattern.trim_start_matches(['\n', '\r']);
                                        // Remove leading newline and dedent
                                        let content_lines: Vec<&str> = after_pipe.lines().collect();
                                        if !content_lines.is_empty() {
                                            // Find minimum indentation (skip empty lines)
                                            let min_indent = content_lines
                                                .iter()
                                                .filter(|l| !l.trim().is_empty())
                                                .map(|l| l.len() - l.trim_start().len())
                                                .min()
                                                .unwrap_or(0);
                                            // Dedent all lines
                                            let dedented: Vec<&str> = content_lines
                                                .iter()
                                                .map(|l| {
                                                    if l.len() >= min_indent {
                                                        &l[min_indent..]
                                                    } else {
                                                        *l
                                                    }
                                                })
                                                .collect();
                                            yaml_multiline_value =
                                                Some(dedented.join("\n").trim().to_string());
                                        }
                                    }
                                }
                            }

                            let (value, type_name) =
                                if let Some(ref ml_value) = yaml_multiline_value {
                                    (json!(ml_value), "string")
                                } else {
                                    parse_field_value(field_value_str)
                                };

                            // Track syntax for YAML multiline
                            if yaml_multiline_value.is_some() {
                                obj.syntax
                                    .insert(field_name.to_string(), "yaml_multiline".to_string());
                            }

                            // Check if it's a YAML array (but not a single reference [[#...]])
                            let fv = field_value_str.trim();
                            let is_single_ref = fv.starts_with("[[")
                                && fv.ends_with("]]")
                                && fv[2..fv.len() - 2].starts_with('#')
                                && !fv[2..fv.len() - 2].contains(',');
                            let is_yaml_array =
                                fv.starts_with('[') && fv.ends_with(']') && !is_single_ref;
                            // Detect multiline array: check raw source for newlines inside [...]
                            let is_multiline_array = if is_yaml_array {
                                if let Some(start) = list_item_start {
                                    let raw_item =
                                        block_tree.source.get(start..range.end).unwrap_or("");
                                    if let Some(colon_pos) = raw_item.find(':') {
                                        let after_colon = raw_item[colon_pos + 1..].trim();
                                        after_colon.starts_with('[') && after_colon.contains('\n')
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                            if is_yaml_array {
                                let syntax_name = if is_multiline_array {
                                    "yaml_multiline_array"
                                } else {
                                    "yaml_array"
                                };
                                obj.syntax
                                    .insert(field_name.to_string(), syntax_name.to_string());
                            }

                            // Check if it's a comma-separated reference array
                            if type_name == "ref_array" {
                                obj.syntax
                                    .insert(field_name.to_string(), "comma_refs".to_string());
                            }

                            // Collect references from field value (for Full format) BEFORE moving value
                            if let Some(start_offset) = list_item_start {
                                let item_line = get_line(start_offset);
                                // Get original line from markdown
                                let line_text =
                                    block_tree.source.get(start_offset..range.end).unwrap_or("");
                                // Find position of field value in the line
                                // We need to find where the actual value starts (after "key: ")
                                if let Some(colon_pos) = line_text.find(':') {
                                    // Calculate value offset: position after colon + any whitespace
                                    let after_colon = &line_text[colon_pos + 1..];
                                    let trimmed_len = after_colon.trim_start().len();
                                    let whitespace_after_colon = after_colon.len() - trimmed_len;
                                    let value_offset =
                                        start_offset + colon_pos + 1 + whitespace_after_colon;

                                    if is_yaml_array {
                                        // For YAML arrays, extract references from each array element
                                        if let Some(arr) = value.as_array() {
                                            let value_str = block_tree
                                                .source
                                                .get(value_offset..range.end)
                                                .unwrap_or("");
                                            let mut search_pos = 0;

                                            for item in arr.iter() {
                                                if let Some(item_str) = item.as_str() {
                                                    // Find this item in the original markdown value string
                                                    if let Some(item_pos) =
                                                        value_str[search_pos..].find(item_str)
                                                    {
                                                        let absolute_item_pos =
                                                            value_offset + search_pos + item_pos;

                                                        // Extract references from this array element
                                                        for mut r in extract_references_from_line(
                                                            item_str, item_line,
                                                        ) {
                                                            // Adjust column positions to be relative to line start
                                                            let line_start_offset = block_tree
                                                                .line_start_offset(start_offset);
                                                            let item_col_offset = markdown
                                                                [line_start_offset
                                                                    ..absolute_item_pos]
                                                                .chars()
                                                                .map(|ch| ch.len_utf16() as u32)
                                                                .sum::<u32>();

                                                            r.start_col += item_col_offset;
                                                            r.end_col += item_col_offset;
                                                            obj.references.push(r);
                                                        }

                                                        // Move search position forward
                                                        search_pos += item_pos + item_str.len();
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // For non-array values, use the original logic
                                        for mut r in
                                            extract_references_from_line(field_value_str, item_line)
                                        {
                                            // Adjust column positions to be relative to line start
                                            let line_start_offset =
                                                block_tree.line_start_offset(start_offset);
                                            // Convert byte offset to UTF-16 offset
                                            let col_offset = markdown
                                                [line_start_offset..value_offset]
                                                .chars()
                                                .map(|ch| ch.len_utf16() as u32)
                                                .sum::<u32>();
                                            r.start_col += col_offset;
                                            r.end_col += col_offset;
                                            obj.references.push(r);
                                        }
                                    }
                                }
                            }

                            let is_null_value =
                                value == Value::Null && yaml_multiline_value.is_none();

                            // Flush any accumulated non-field items BEFORE inserting the field
                            if !comment_list_items.is_empty() {
                                // When fields already exist, these are invalid field-like items
                                // between valid fields — use \n\n join (matches Python behavior).
                                // When no fields yet, use \n join for tight list formatting.
                                let list_content = if !obj.fields.is_empty() {
                                    comment_list_items.join("\n\n")
                                } else {
                                    comment_list_items.join("\n")
                                };
                                let anchor = obj.comment_anchor.clone();

                                // Check if we can merge with the last comment (same anchor)
                                let should_merge = obj
                                    .comments
                                    .last()
                                    .map(|c| c.get("after") == Some(&anchor))
                                    .unwrap_or(false);

                                if should_merge {
                                    if let Some(last_comment) = obj.comments.last_mut() {
                                        if let Some(existing) = last_comment.get_mut("content") {
                                            *existing = format!("{}\n\n{}", existing, list_content);
                                        }
                                    }
                                } else {
                                    let mut comment = IndexMap::new();
                                    comment.insert("after".to_string(), anchor);
                                    comment.insert("content".to_string(), list_content);
                                    obj.comments.push(comment);
                                }
                                comment_list_items.clear();
                                comment_list_raw_start = None;
                                comment_list_raw_end = None;
                            }

                            obj.fields.insert(field_name.to_string(), value);
                            // Track this field key for potential rollback if a duplicate is found later in this list
                            if list_nesting_level == 1 {
                                list_inserted_field_keys.push(field_name.to_string());
                            }
                            let stored_type = if type_name == "ref_array" {
                                "array"
                            } else {
                                type_name
                            };
                            obj.types
                                .insert(field_name.to_string(), stored_type.to_string());

                            obj.comment_anchor = field_name.to_string();

                            // If field value is null (empty after colon), it might have nested sub-items
                            if is_null_value {
                                pending_multiline_list_field = Some(field_name.to_string());
                            } else {
                                pending_multiline_list_field = None;
                            }

                            // Track field position for LSP
                            if let Some(start_offset) = list_item_start {
                                let item_line = get_line(start_offset);
                                let line_text = lines.get(item_line as usize - 1).unwrap_or(&"");
                                let col = line_text.find(field_name).unwrap_or(0) as u32;
                                obj.positions
                                    .insert(field_name.to_string(), (item_line, col));
                            }
                        } // end else (not duplicate key)
                    } else if !trimmed.is_empty() && pending_object_array.is_none() {
                        // Check if this is a nested sub-item for a yaml_multiline_list field
                        if list_nesting_level > 1 && pending_multiline_list_field.is_some() {
                            multiline_list_items.push(trimmed.to_string());
                        } else if pending_yaml_multiline_pipe_field.is_some() {
                            // Skip — these items belong to the pipe field and will be
                            // extracted via raw-slice when the nested list ends
                        } else {
                            // List item is not a field — treat as comment text
                            // (invalid keys like cyrillic, backticks, bold, spaces are just markdown)

                            // Accumulate for comment (skip empty items)
                            // Capture as comment if:
                            // 1. Item contains a colon (looks like markdown text with colon, not array), OR
                            // 2. Object already has fields (so this list is clearly not array content), OR
                            // 3. There's a comment with the same anchor (list follows text)
                            let anchor_to_check =
                                list_comment_anchor.as_ref().unwrap_or(&obj.comment_anchor);
                            let has_same_anchor_comment = obj
                                .comments
                                .last()
                                .and_then(|c| c.get("after"))
                                .map(|s| s == anchor_to_check)
                                .unwrap_or(false);
                            // Strip backtick spans before checking for colons —
                            // colons inside code are clearly not field syntax
                            let sanitized_line = backtick_re_strip.replace_all(first_line, "");
                            let sanitized_line = bold_re_strip.replace_all(&sanitized_line, "$1");
                            let sanitized_line = italic_re_strip.replace_all(&sanitized_line, "$1");
                            let sanitized_line = strike_re_strip.replace_all(&sanitized_line, "$1");
                            let item_has_colon = sanitized_line.contains(':');

                            // Track invalid field-like items for mixed_field_keys error
                            // An item is "invalid field-like" if it has colon pattern but invalid key
                            // Only applies to bullet lists — ordered list items are always comments
                            // Only applies at outermost nesting level — nested items are comment content
                            if item_has_colon
                                && !obj.fields.is_empty()
                                && current_list_order.is_none()
                                && list_nesting_level <= 1
                            {
                                // Check if this looks like a field with invalid key
                                // (has colon-space pattern but key doesn't match [a-zA-Z_][a-zA-Z0-9_]*)
                                // Must have ": " (colon + space) like Python's _INVALID_FL_RE pattern
                                if let Some(colon_pos) = sanitized_line.find(": ") {
                                    let potential_key = sanitized_line[..colon_pos].trim();
                                    if !potential_key.is_empty()
                                        && !field_re.is_match(&format!("{}: x", potential_key))
                                    {
                                        // Check if this content is NOT already captured by raw comment scanner
                                        let already_captured = obj.comments.iter().any(|c| {
                                            if let Some(content) = c.get("content") {
                                                if c.get("after")
                                                    .map(|a| a == "__self")
                                                    .unwrap_or(false)
                                                {
                                                    let item_text = format!("- {}", trimmed);
                                                    content.contains(&item_text)
                                                        || content.contains(trimmed)
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        });
                                        if !already_captured {
                                            mixed_field_has_invalid = true;
                                            if mixed_field_invalid_line.is_none() {
                                                if let Some(start_offset) = list_item_start {
                                                    mixed_field_invalid_line =
                                                        Some(get_line(start_offset));
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if item_has_colon
                                || !obj.fields.is_empty()
                                || has_same_anchor_comment
                                || list_nesting_level >= 1
                            {
                                if comment_list_raw_start.is_none() {
                                    comment_list_raw_start = list_item_start;
                                }
                                comment_list_raw_end = Some(range.end);
                                let item_prefix = if current_list_order.is_some() {
                                    format!("{}.", current_list_item_num)
                                } else {
                                    "-".to_string()
                                };
                                // Add indentation for nested lists (3 spaces for proper markdown nesting)
                                let indent = if list_nesting_level > 1 {
                                    "   ".repeat(list_nesting_level - 1)
                                } else {
                                    String::new()
                                };
                                comment_list_items
                                    .push(format!("{}{} {}", indent, item_prefix, trimmed));
                                current_list_item_num += 1;
                            }
                        } // end else (not yaml_multiline_list sub-item)
                    }
                } else if pending_text_block.is_some()
                    && current_obj.is_none()
                    && textblock_list_raw_start.is_none()
                {
                    // Collect list items as text for TextBlock (only if not using raw slice extraction)
                    if let Some(ref mut parts) = pending_text_block {
                        // Format based on list type
                        let item_prefix = if current_list_order.is_some() {
                            format!("{}.", current_list_item_num)
                        } else {
                            "-".to_string()
                        };

                        // Check if last part is already a list item
                        let already_in_list = parts
                            .last()
                            .map(|s| {
                                s.starts_with("- ")
                                    || s.contains("\n- ")
                                    || s.chars()
                                        .next()
                                        .map(|c| c.is_ascii_digit())
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);

                        if already_in_list {
                            // Append to last part with single newline
                            if let Some(last) = parts.last_mut() {
                                last.push_str(&format!("\n{} {}", item_prefix, trimmed));
                            }
                        } else {
                            // Start new list
                            parts.push(format!("{} {}", item_prefix, trimmed));
                        }
                        current_list_item_num += 1;
                    }
                }

                list_item_text.clear();
                list_item_start = None;
            }

            Event::Start(Tag::Paragraph) => {
                in_paragraph = true;
                paragraph_text.clear();
                paragraph_start_offset = range.start;
            }

            Event::End(TagEnd::Paragraph) => {
                in_paragraph = false;
                let text = paragraph_text.trim().to_string();

                if !text.is_empty() {
                    // Handle blockquote - collect lines for later formatting
                    if in_blockquote {
                        blockquote_lines.push(text.clone());
                    }
                    // Handle text field
                    else if let Some((ref parent_id, ref field_name, _, _)) = pending_text_field {
                        // Check if this is a heading-level array field (parent is still current_obj)
                        let is_array_on_current = current_obj
                            .as_ref()
                            .map(|o| {
                                o.id == *parent_id
                                    && o.fields
                                        .get(field_name)
                                        .map(|v| v.is_array())
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);

                        if is_array_on_current {
                            // Array field is done — text after list is a comment on current_obj
                            if let Some(ref mut obj) = current_obj {
                                let mut cm = IndexMap::new();
                                cm.insert("after".to_string(), field_name.clone());
                                cm.insert("content".to_string(), text.clone());
                                obj.comments.push(cm);
                                obj.comment_anchor = field_name.clone();
                            }
                            pending_text_field = None;
                        } else if let Some(parent) = objects_map.get_mut(parent_id) {
                            // Non-array text field — parent was finalized into objects_map
                            let is_array_field = parent
                                .get(field_name)
                                .map(|v| v.is_array())
                                .unwrap_or(false);

                            if !is_array_field {
                                let existing = parent
                                    .get(field_name)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let new_val = if existing.is_empty() {
                                    text.clone()
                                } else {
                                    format!("{}\n\n{}", existing, text)
                                };
                                parent.insert(field_name.clone(), json!(new_val));
                            } else {
                                // Array field on finalized parent — text after list is a comment
                                let comments = parent
                                    .entry("__comments".to_string())
                                    .or_insert_with(|| json!([]));
                                if let Some(arr) = comments.as_array_mut() {
                                    arr.push(json!({
                                        "after": field_name.clone(),
                                        "content": text.clone()
                                    }));
                                }
                                // Can't restore current_obj here (parent already finalized),
                                // just clear pending so subsequent text becomes comments on
                                // whatever object is current.
                                pending_text_field = None;
                            }
                        }
                    }
                    // Handle text block
                    else if let Some(ref mut parts) = pending_text_block {
                        parts.push(text.clone());
                    }
                    // Handle comment for current object
                    else if let Some(ref mut obj) = current_obj {
                        // Check if we should merge with previous block comment
                        let should_merge = last_comment_was_block
                            && obj
                                .comments
                                .last()
                                .map(|c| c.get("after") == Some(&obj.comment_anchor))
                                .unwrap_or(false);

                        if should_merge {
                            // Merge with previous block comment
                            if let Some(last_comment) = obj.comments.last_mut() {
                                if let Some(existing) = last_comment.get_mut("content") {
                                    *existing = format!("{}\n\n{}", existing, text);
                                }
                            }
                            // Keep last_comment_was_block = true to continue merging
                        } else {
                            // Create new comment
                            let mut comment = IndexMap::new();
                            comment.insert("after".to_string(), obj.comment_anchor.clone());
                            comment.insert("content".to_string(), text.clone());
                            obj.comments.push(comment);
                            // Reset block flag for new comment
                            last_comment_was_block = false;
                        }

                        // Extract references from paragraph text
                        let start_offset = paragraph_start_offset;
                        let para_line = get_line(start_offset);
                        // Get original paragraph from markdown
                        let para_text =
                            block_tree.source.get(start_offset..range.end).unwrap_or("");

                        // Calculate byte offset for each line in the paragraph
                        let mut line_offset = start_offset;

                        // Process each line of the paragraph
                        for (line_idx, line_text) in para_text.lines().enumerate() {
                            let line_num = para_line + line_idx as u32;

                            // Extract references from this line
                            for mut r in extract_references_from_line(line_text, line_num) {
                                // Adjust column positions to be relative to line start in original markdown
                                let line_start_offset = block_tree.line_start_offset(line_offset);
                                // Convert byte offset to UTF-16 offset
                                let col_offset = markdown[line_start_offset..line_offset]
                                    .chars()
                                    .map(|ch| ch.len_utf16() as u32)
                                    .sum::<u32>();

                                r.start_col += col_offset;
                                r.end_col += col_offset;
                                obj.references.push(r);
                            }

                            // Move to next line (+1 for newline)
                            line_offset += line_text.len() + 1;
                        }
                    }
                }

                paragraph_text.clear();
            }

            Event::Rule => {
                // Horizontal rule (---) - add to current object's comments
                if let Some(ref mut obj) = current_obj {
                    // Check if we can append to the last comment with the same anchor
                    let should_append = obj
                        .comments
                        .last()
                        .map(|c| c.get("after") == Some(&obj.comment_anchor))
                        .unwrap_or(false);

                    if should_append {
                        // Append --- to existing comment
                        if let Some(last_comment) = obj.comments.last_mut() {
                            if let Some(existing) = last_comment.get_mut("content") {
                                *existing = format!("{}\n\n---", existing);
                            }
                        }
                    } else {
                        // Create new comment with just ---
                        let mut comment = IndexMap::new();
                        comment.insert("after".to_string(), obj.comment_anchor.clone());
                        comment.insert("content".to_string(), "---".to_string());
                        obj.comments.push(comment);
                    }
                    // Mark that last comment was a block element
                    last_comment_was_block = true;
                }
            }

            Event::Start(Tag::BlockQuote) => {
                in_blockquote = true;
                blockquote_lines.clear();
            }

            Event::End(TagEnd::BlockQuote) => {
                in_blockquote = false;

                if !blockquote_lines.is_empty() {
                    // Format blockquote with > prefix for each line
                    let blockquote_content = blockquote_lines
                        .iter()
                        .flat_map(|para| para.lines())
                        .map(|line| format!("> {}", line))
                        .collect::<Vec<_>>()
                        .join("\n");

                    if let Some(ref mut obj) = current_obj {
                        // Check if we can append to the last comment with the same anchor
                        let should_append = obj
                            .comments
                            .last()
                            .map(|c| c.get("after") == Some(&obj.comment_anchor))
                            .unwrap_or(false);

                        if should_append {
                            // Append blockquote to existing comment
                            if let Some(last_comment) = obj.comments.last_mut() {
                                if let Some(existing) = last_comment.get_mut("content") {
                                    *existing = format!("{}\n\n{}", existing, blockquote_content);
                                }
                            }
                        } else {
                            // Create new comment with blockquote
                            let mut comment = IndexMap::new();
                            comment.insert("after".to_string(), obj.comment_anchor.clone());
                            comment.insert("content".to_string(), blockquote_content);
                            obj.comments.push(comment);
                        }
                        // Mark that last comment was a block element
                        last_comment_was_block = true;
                    }
                }
                blockquote_lines.clear();
            }

            _ => {}
        }

        i += 1;
    }

    // Finalize pending text field
    if let Some((parent_id, field_name, _, _)) = pending_text_field.take() {
        // Check if parent is still current_obj (heading-level array field case)
        let handled_on_current = current_obj
            .as_ref()
            .map(|o| {
                o.id == parent_id
                    && o.syntax
                        .get(&field_name)
                        .map(|s| s == "markdown_list")
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        // If parent is current_obj and it's an array field, nothing to do —
        // array items are already in current_obj.fields, it will be finalized below.

        if !handled_on_current {
            if let Some(parent) = objects_map.get_mut(&parent_id) {
                // Check if this is an array field
                let is_array_field = parent
                    .get("__syntax")
                    .and_then(|s| s.as_object())
                    .and_then(|obj| obj.get(&field_name))
                    .and_then(|v| v.as_str())
                    .map(|s| s == "markdown_list")
                    .unwrap_or(false);

                if !is_array_field {
                    if !parent.contains_key(&field_name) {
                        parent.insert(field_name.clone(), json!(""));
                    }

                    // Check if this field was parsed as yaml_object or json_object
                    let is_object_syntax = parent
                        .get("__syntax")
                        .and_then(|s| s.as_object())
                        .and_then(|obj| obj.get(&field_name))
                        .and_then(|v| v.as_str())
                        .map(|s| s == "yaml_object" || s == "json_object")
                        .unwrap_or(false);

                    // Only add to __types if not an object (objects don't go in __types)
                    if !is_object_syntax {
                        let types = parent
                            .entry("__types".to_string())
                            .or_insert_with(|| json!({}));
                        if let Some(obj) = types.as_object_mut() {
                            // Only set type if not already set
                            if !obj.contains_key(&field_name) {
                                obj.insert(field_name.clone(), json!("string"));
                            }
                        }
                    }

                    let syntax = parent
                        .entry("__syntax".to_string())
                        .or_insert_with(|| json!({}));
                    if let Some(obj) = syntax.as_object_mut() {
                        // Only set syntax if not already set
                        if !obj.contains_key(&field_name) {
                            obj.insert(field_name.clone(), json!("multiline_text"));
                        }
                    }
                }
            }
        }
    }

    // Finalize pending text block
    if let Some(content_parts) = pending_text_block.take() {
        let tb_id = format!("text_{}", text_block_counter);
        let fences = std::mem::take(&mut pending_code_fences);
        text_blocks.push((
            tb_id.clone(),
            content_parts.join("\n\n"),
            pending_text_block_line,
            fences,
        ));
        content_order.push(tb_id);
    }

    // Finalize current object
    if let Some(obj) = current_obj.take() {
        finalize_object(
            &mut objects_map,
            &mut duplicate_objects,
            &mut parsing_errors,
            &mut first_seen_lines,
            obj,
        );
    }

    // Add __labels to objects from text_field_labels
    for (obj_id, labels) in text_field_labels {
        if let Some(obj) = objects_map.get_mut(&obj_id) {
            let labels_map: IndexMap<String, Value> =
                labels.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            obj.insert("__labels".to_string(), json!(labels_map));
        }
    }

    // Build result
    // Check if there's a __Workspace object - if so, don't create __Document
    let has_workspace = objects_map.values().any(|obj| {
        obj.get("__kind")
            .and_then(|v| v.as_str())
            .map(|k| k == "__Workspace")
            .unwrap_or(false)
    });

    if !text_blocks.is_empty() && !has_workspace {
        // Need __Document
        let doc_id = rng.gen_doc_id();

        // Build content array
        let content_refs: Vec<Value> = content_order
            .iter()
            .map(|id| json!(format!("[[#{}]]", id)))
            .collect();

        // Create __Document
        let mut doc = IndexMap::new();
        doc.insert("__id".to_string(), json!(&doc_id));
        doc.insert("__kind".to_string(), json!("__Document"));
        doc.insert("content".to_string(), json!(content_refs));
        all_objects.push(json!(doc));

        // Add text blocks
        for (tb_id, content, tb_line, code_fences) in &text_blocks {
            let mut tb = IndexMap::new();
            tb.insert("__id".to_string(), json!(tb_id));
            tb.insert("__kind".to_string(), json!("__TextBlock"));
            tb.insert("content".to_string(), json!(content));
            if format == OutputFormat::Full {
                tb.insert("__line".to_string(), json!(tb_line));
            }
            tb.insert("__container".to_string(), json!(format!("[[#{}]]", doc_id)));
            // Add __code_fences in Full mode
            if format == OutputFormat::Full && !code_fences.is_empty() {
                let fences_json: Vec<Value> = code_fences
                    .iter()
                    .map(|f| {
                        json!({
                            "lang": f.lang,
                            "offset_line": f.offset_line,
                            "length_lines": f.length_lines
                        })
                    })
                    .collect();
                tb.insert("__code_fences".to_string(), json!(fences_json));
            }

            // Extract references from TextBlock content (skip code fences)
            if format == OutputFormat::Full && content.contains("[[") {
                let mut refs = Vec::new();
                let mut in_code_fence = false;
                let mut example_fence_depth: i32 = 0;

                for (line_idx, line) in content.lines().enumerate() {
                    let stripped = line.trim();
                    // Check for code fence markers
                    if stripped.starts_with("```") {
                        if example_fence_depth > 0 {
                            let fence_content = stripped.strip_prefix("```").unwrap_or("");
                            if !fence_content.is_empty() {
                                example_fence_depth += 1;
                            } else {
                                example_fence_depth -= 1;
                            }
                        } else {
                            let fence_content = stripped.strip_prefix("```").unwrap_or("");
                            if fence_content.contains("example") {
                                example_fence_depth = 1;
                            }
                            in_code_fence = !in_code_fence;
                        }
                        continue;
                    }

                    // Skip references inside example code fences only
                    // (non-example fences like ```table may contain valid refs)
                    if example_fence_depth > 0 {
                        continue;
                    }

                    if line.contains("[[") {
                        let line_num = *tb_line as u32 + line_idx as u32;
                        for r in extract_references_from_line(line, line_num) {
                            refs.push(r);
                        }
                    }
                }

                if !refs.is_empty() {
                    let refs_json: Vec<Value> = refs
                        .iter()
                        .map(|r| {
                            json!({
                                "target": r.target,
                                "type": r.ref_type,
                                "line": r.line,
                                "start_col": r.start_col,
                                "end_col": r.end_col,
                                "raw": r.raw,
                            })
                        })
                        .collect();
                    tb.insert("__references".to_string(), json!(refs_json));
                }
            }

            all_objects.push(json!(tb));
        }

        // Add regular objects with __container, children interleaved after parents
        let child_ids_tb: Vec<String> = objects_map
            .keys()
            .filter(|id| !content_order.contains(id))
            .cloned()
            .collect();

        let mut added_ids_tb: std::collections::HashSet<String> = std::collections::HashSet::new();

        fn add_with_children_container(
            id: &str,
            objects_map: &IndexMap<String, IndexMap<String, Value>>,
            child_ids: &[String],
            all_objects: &mut Vec<Value>,
            added_ids: &mut std::collections::HashSet<String>,
            format: OutputFormat,
            doc_id: &str,
        ) {
            if added_ids.contains(id) {
                return;
            }
            if let Some(obj_map) = objects_map.get(id) {
                let mut obj = obj_map.clone();
                obj.insert("__container".to_string(), json!(format!("[[#{}]]", doc_id)));
                all_objects.push(build_from_map(&obj, format));
                added_ids.insert(id.to_string());
            }
            for cid in child_ids {
                if added_ids.contains(cid.as_str()) {
                    continue;
                }
                if let Some(obj_map) = objects_map.get(cid.as_str()) {
                    let is_child = obj_map
                        .get("__parent")
                        .and_then(|v| v.as_str())
                        .map(|p| p == format!("[[#{}]]", id))
                        .unwrap_or(false);
                    if is_child {
                        add_with_children_container(
                            cid,
                            objects_map,
                            child_ids,
                            all_objects,
                            added_ids,
                            format,
                            doc_id,
                        );
                    }
                }
            }
        }

        for id in &content_order {
            if !id.starts_with("text_") {
                add_with_children_container(
                    id,
                    &objects_map,
                    &child_ids_tb,
                    &mut all_objects,
                    &mut added_ids_tb,
                    format,
                    &doc_id,
                );
            }
        }

        // Add any remaining objects not reachable from content_order
        for (id, obj_map) in &objects_map {
            if !added_ids_tb.contains(id.as_str()) {
                let mut obj = obj_map.clone();
                obj.insert("__container".to_string(), json!(format!("[[#{}]]", doc_id)));
                all_objects.push(build_from_map(&obj, format));
            }
        }

        // Add duplicate objects (same __id, different heading) so LSP can detect duplicates
        for dup_map in &duplicate_objects {
            let mut obj = dup_map.clone();
            obj.insert("__container".to_string(), json!(format!("[[#{}]]", doc_id)));
            all_objects.push(build_from_map(&obj, format));
        }
    } else {
        // No text blocks - just objects
        // Add top-level objects with their children interleaved (depth-first)
        // so that child objects appear right after their parent.
        let child_ids: Vec<String> = objects_map
            .keys()
            .filter(|id| !content_order.contains(id))
            .cloned()
            .collect();

        let mut added_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        fn add_with_children(
            id: &str,
            objects_map: &IndexMap<String, IndexMap<String, Value>>,
            child_ids: &[String],
            all_objects: &mut Vec<Value>,
            added_ids: &mut std::collections::HashSet<String>,
            format: OutputFormat,
        ) {
            if added_ids.contains(id) {
                return;
            }
            if let Some(obj_map) = objects_map.get(id) {
                all_objects.push(build_from_map(obj_map, format));
                added_ids.insert(id.to_string());
            }
            // Add children of this object
            for cid in child_ids {
                if added_ids.contains(cid.as_str()) {
                    continue;
                }
                if let Some(obj_map) = objects_map.get(cid.as_str()) {
                    let is_child = obj_map
                        .get("__parent")
                        .and_then(|v| v.as_str())
                        .map(|p| p == format!("[[#{}]]", id))
                        .unwrap_or(false);
                    if is_child {
                        add_with_children(
                            cid,
                            objects_map,
                            child_ids,
                            all_objects,
                            added_ids,
                            format,
                        );
                    }
                }
            }
        }

        for id in &content_order {
            add_with_children(
                id,
                &objects_map,
                &child_ids,
                &mut all_objects,
                &mut added_ids,
                format,
            );
        }

        // Add any remaining objects not reachable from content_order
        for (id, obj_map) in &objects_map {
            if !added_ids.contains(id.as_str()) {
                all_objects.push(build_from_map(obj_map, format));
            }
        }

        // Add duplicate objects (same __id, different heading) so LSP can detect duplicates
        for dup_map in &duplicate_objects {
            all_objects.push(build_from_map(dup_map, format));
        }
    }

    // Post-process: extract references from string fields (for text fields with [[...]])
    // Search in original markdown lines to get correct line numbers
    if format == OutputFormat::Full {
        for obj in &mut all_objects {
            if let Some(obj_map) = obj.as_object_mut() {
                let mut all_refs: Vec<Value> = Vec::new();

                // Get existing references
                if let Some(existing) = obj_map.get("__references").and_then(|v| v.as_array()) {
                    all_refs.extend(existing.iter().cloned());
                }

                // Scan multiline_text and simple ref string fields for [[...]] references
                // Skip __TextBlock content field (handled separately)
                let is_text_block = obj_map
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .map(|k| k == "__TextBlock")
                    .unwrap_or(false);

                // Check if this is a __Document object
                let is_document = obj_map
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .map(|k| k == "__Document")
                    .unwrap_or(false);

                let syntax = obj_map.get("__syntax").and_then(|v| v.as_object());
                let field_names: Vec<String> = obj_map
                    .keys()
                    .filter(|k| !k.starts_with("__"))
                    .filter(|k| {
                        // Skip content field of __TextBlock
                        if is_text_block && *k == "content" {
                            return false;
                        }
                        // For __Document: include all string fields with [[
                        // (they don't have __syntax entries but need reference extraction)
                        if is_document {
                            return obj_map
                                .get(*k)
                                .and_then(|v| v.as_str())
                                .map(|s| s.contains("[["))
                                .unwrap_or(false);
                        }
                        // For other objects: only include multiline_text fields
                        // Simple fields are handled during main parsing
                        syntax
                            .map(|s| s.get(*k).and_then(|v| v.as_str()) == Some("multiline_text"))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                // Get object's starting line to limit search scope
                let obj_start_line =
                    obj_map.get("__line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

                for field_name in field_names {
                    if let Some(text) = obj_map.get(&field_name).and_then(|v| v.as_str()) {
                        if text.contains("[[") {
                            // Start search from object's line (0-indexed)
                            let mut search_start_idx = obj_start_line.saturating_sub(1);

                            // Track code fence state for this field
                            let mut in_code_fence = false;
                            let mut example_fence_depth: i32 = 0;

                            // For each line in the field content, find it in original markdown
                            for content_line in text.lines() {
                                let stripped = content_line.trim();

                                // Check for code fence markers
                                if stripped.starts_with("```") {
                                    if example_fence_depth > 0 {
                                        let fence_content =
                                            stripped.strip_prefix("```").unwrap_or("");
                                        if !fence_content.is_empty() {
                                            example_fence_depth += 1;
                                        } else {
                                            example_fence_depth -= 1;
                                        }
                                    } else {
                                        let fence_content =
                                            stripped.strip_prefix("```").unwrap_or("");
                                        if fence_content.contains("example") {
                                            example_fence_depth = 1;
                                        }
                                        in_code_fence = !in_code_fence;
                                    }
                                    continue;
                                }

                                // Skip references inside example code fences
                                if example_fence_depth > 0 {
                                    continue;
                                }

                                if !content_line.contains("[[") {
                                    continue;
                                }

                                // Find this line in original markdown, starting from last found position
                                for (line_idx, orig_line) in
                                    lines.iter().enumerate().skip(search_start_idx)
                                {
                                    if orig_line.contains(content_line.trim())
                                        && orig_line.contains("[[")
                                    {
                                        let line_num = (line_idx + 1) as u32;

                                        // Extract references from this line
                                        for r in extract_references_from_line(orig_line, line_num) {
                                            // Avoid duplicates
                                            let is_dup = all_refs.iter().any(|existing| {
                                                existing.get("line").and_then(|l| l.as_u64())
                                                    == Some(r.line as u64)
                                                    && existing
                                                        .get("start_col")
                                                        .and_then(|c| c.as_u64())
                                                        == Some(r.start_col as u64)
                                            });
                                            if !is_dup {
                                                all_refs.push(json!({
                                                    "target": r.target,
                                                    "type": r.ref_type,
                                                    "line": r.line,
                                                    "start_col": r.start_col,
                                                    "end_col": r.end_col,
                                                    "raw": r.raw,
                                                }));
                                            }
                                        }
                                        // Move search position forward for next content line
                                        search_start_idx = line_idx + 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Update __references if we found any
                if !all_refs.is_empty() {
                    obj_map.insert("__references".to_string(), json!(all_refs));

                    // Re-insert __positions and __labels after __references to maintain consistent order
                    if let Some(positions) = obj_map.remove("__positions") {
                        obj_map.insert("__positions".to_string(), positions);
                    }
                    if let Some(labels) = obj_map.remove("__labels") {
                        obj_map.insert("__labels".to_string(), labels);
                    }
                }
            }
        }
    }

    // Add parsing errors as __ParsingError objects
    // Errors appear after all objects, sorted by line number among themselves
    if !parsing_errors.is_empty() {
        // Inject __line into all objects for sorting (standard format strips it during build_from_map)
        // We use the raw objects_map and duplicate_objects to recover line numbers.
        let mut line_by_label: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        for obj_map in objects_map.values() {
            if let (Some(id), Some(label), Some(line)) = (
                obj_map.get("__id").and_then(|v| v.as_str()),
                obj_map.get("__label").and_then(|v| v.as_str()),
                obj_map.get("__line").and_then(|v| v.as_i64()),
            ) {
                line_by_label.insert((id.to_string(), label.to_string()), line);
            }
        }
        for dup_map in &duplicate_objects {
            if let (Some(id), Some(label), Some(line)) = (
                dup_map.get("__id").and_then(|v| v.as_str()),
                dup_map.get("__label").and_then(|v| v.as_str()),
                dup_map.get("__line").and_then(|v| v.as_i64()),
            ) {
                line_by_label.insert((id.to_string(), label.to_string()), line);
            }
        }
        // Also build simple id->line map for children and other objects
        let mut line_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for (id, obj_map) in &objects_map {
            if let Some(line) = obj_map.get("__line").and_then(|v| v.as_i64()) {
                line_map.insert(id.clone(), line);
            }
        }
        for error in parsing_errors {
            all_objects.push(json!(error));
        }
        // Sort: objects by line (group 0), errors after all objects (group 1)
        all_objects.sort_by_key(|obj| {
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "__ParsingError" {
                (1i64, obj.get("line").and_then(|v| v.as_i64()).unwrap_or(0))
            } else if kind == "__Document" {
                (-1, 0) // __Document always first
            } else {
                // Try __line first (Full format), then look up from line_by_label, then line_map
                let line = obj
                    .get("__line")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_else(|| {
                        let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
                        let label = obj.get("__label").and_then(|v| v.as_str()).unwrap_or("");
                        // Try (id, label) pair first (disambiguates duplicates)
                        if let Some(&line) = line_by_label.get(&(id.to_string(), label.to_string()))
                        {
                            return line;
                        }
                        line_map.get(id).copied().unwrap_or(0)
                    });
                (0, line)
            }
        });
    }

    all_objects
}

fn finalize_object(
    objects_map: &mut IndexMap<String, IndexMap<String, Value>>,
    duplicate_objects: &mut Vec<IndexMap<String, Value>>,
    parsing_errors: &mut Vec<IndexMap<String, Value>>,
    first_seen_lines: &mut std::collections::HashMap<String, u64>,
    obj: CurrentObject,
) {
    let mut map = IndexMap::new();

    map.insert("__id".to_string(), json!(&obj.id));
    if let Some(ref lid) = obj.local_id {
        map.insert("__local_id".to_string(), json!(lid));
    }
    if !obj.label.is_empty() {
        map.insert("__label".to_string(), json!(&obj.label));
    }
    map.insert(
        "__kind".to_string(),
        json!(obj.kind.as_deref().unwrap_or("__Object")),
    );

    if let Some(ref p) = obj.parent {
        map.insert("__parent".to_string(), json!(p));
    }
    if let Some(ref pf) = obj.parent_field {
        map.insert("__parent_field".to_string(), json!(pf));
    }

    if !obj.comments.is_empty() {
        let comments_json: Vec<Value> = obj.comments.iter().map(|c| json!(c)).collect();
        map.insert("__comments".to_string(), json!(comments_json));
    }

    for (k, v) in &obj.fields {
        map.insert(k.clone(), v.clone());
    }

    if !obj.types.is_empty() {
        let types_map: IndexMap<String, Value> = obj
            .types
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        map.insert("__types".to_string(), json!(types_map));
    }

    if !obj.syntax.is_empty() {
        let syntax_map: IndexMap<String, Value> = obj
            .syntax
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        map.insert("__syntax".to_string(), json!(syntax_map));
    }

    map.insert("__level".to_string(), json!(obj.level));
    map.insert("__line".to_string(), json!(obj.line));

    if !obj.has_explicit_id {
        map.insert("__has_explicit_id".to_string(), json!(false));
    }

    if obj.is_array_element {
        map.insert("__is_array_element".to_string(), json!(true));
    }

    // References from simple fields are already extracted during parsing.
    // References from multiline_text fields are handled separately in parse()
    // after all objects are finalized, where we have access to original lines.

    // Store references for Full format
    if !obj.references.is_empty() {
        let refs_json: Vec<Value> = obj
            .references
            .iter()
            .map(|r| {
                json!({
                    "target": r.target,
                    "type": r.ref_type,
                    "line": r.line,
                    "start_col": r.start_col,
                    "end_col": r.end_col,
                    "raw": r.raw,
                })
            })
            .collect();
        map.insert("__references".to_string(), json!(refs_json));
    }

    // Store field positions for Full format
    if !obj.positions.is_empty() {
        let positions_json: IndexMap<String, Value> = obj
            .positions
            .iter()
            .map(|(k, (line, col))| (k.clone(), json!({"line": line, "col": col})))
            .collect();
        map.insert("__positions".to_string(), json!(positions_json));
    }

    // Detect true duplicates: if the existing entry was already finalized
    // (has __line field from a previous finalize_object call), it's a real duplicate.
    // If it exists but without __line, it's a skeleton placeholder that should be overwritten.
    //
    // Semantics: last-wins for the map occupant (new object overwrites), old goes to duplicates.
    // This matches Python/TS behavior: children after the last duplicate attach to it.
    if let Some(existing) = objects_map.get(&obj.id) {
        if existing.contains_key("__line") {
            // True duplicate — different heading with same [[id]]
            // Track the true first line (from first occurrence, not current occupant)
            let first_line = *first_seen_lines
                .entry(obj.id.clone())
                .or_insert_with(|| existing.get("__line").and_then(|v| v.as_u64()).unwrap_or(0));

            // Push the OLD (current occupant) to duplicates
            duplicate_objects.push(existing.clone());

            // Generate __ParsingError for the NEW (incoming) occurrence
            let error_id = format!("__error_dup_{}", obj.id);
            let mut error = IndexMap::new();
            error.insert("__id".to_string(), json!(error_id));
            error.insert("__kind".to_string(), json!("__ParsingError"));
            error.insert("type".to_string(), json!("duplicate_id"));
            error.insert(
                "message".to_string(),
                json!(format!(
                    "Duplicate ID '{}' (first defined on line {})",
                    obj.id, first_line
                )),
            );
            error.insert("object".to_string(), json!(format!("[[#{}]]", obj.id)));
            error.insert("line".to_string(), json!(obj.line));
            parsing_errors.push(error);
        }
    }
    objects_map.insert(obj.id.clone(), map);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_matches_python() {
        let mut rng = SimpleRng::new(666);
        let id = rng.gen_doc_id();
        // Python/TS with seed 666 produces "doc_ry4ljv"
        assert_eq!(id, "doc_ry4ljv");
    }

    #[test]
    fn test_basic_object() {
        let md = "## User [[user:Person]]\n\n- name: Alice\n- age: 30\n";
        let result = parse(md, ParseOptions::default());

        assert_eq!(result.len(), 1);
        let obj = result[0].as_object().unwrap();
        assert_eq!(obj.get("__id").unwrap(), "user");
        assert_eq!(obj.get("__kind").unwrap(), "Person");
        assert_eq!(obj.get("name").unwrap(), "Alice");
        assert_eq!(obj.get("age").unwrap(), 30);
    }
}
