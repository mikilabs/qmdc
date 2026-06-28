use serde_json::Value;

/// Rebuild QMD.md from JSON objects
pub fn rebuild(objects: &[Value]) -> String {
    // Build object map by ID
    let obj_map: std::collections::HashMap<String, &Value> = objects
        .iter()
        .filter_map(|obj| {
            if let Value::Object(map) = obj {
                map.get("__id")
                    .and_then(|v| v.as_str())
                    .map(|id| (id.to_string(), obj))
            } else {
                None
            }
        })
        .collect();

    // Build children map: parent_id -> [child_ids]
    let mut children_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for obj in objects {
        if let Value::Object(map) = obj {
            if let Some(parent_ref) = map.get("__parent").and_then(|v| v.as_str()) {
                if parent_ref.starts_with("[[#") && parent_ref.ends_with("]]") {
                    let parent_id = &parent_ref[3..parent_ref.len() - 2];
                    if let Some(child_id) = map.get("__id").and_then(|v| v.as_str()) {
                        children_map
                            .entry(parent_id.to_string())
                            .or_default()
                            .push(child_id.to_string());
                    }
                }
            }
        }
    }

    // Find __Document to get content order for top-level elements
    let ordered_ids: Vec<String> = objects
        .iter()
        .find_map(|obj| {
            if let Value::Object(map) = obj {
                if map.get("__kind").and_then(|v| v.as_str()) == Some("__Document") {
                    return map.get("content").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                item.as_str().and_then(|s| {
                                    if s.starts_with("[[#") && s.ends_with("]]") {
                                        Some(s[3..s.len() - 2].to_string())
                                    } else {
                                        None
                                    }
                                })
                            })
                            .collect()
                    });
                }
            }
            None
        })
        .unwrap_or_else(|| {
            // No __Document, use original order but only non-nested
            objects
                .iter()
                .filter_map(|obj| {
                    if let Value::Object(map) = obj {
                        if !map.contains_key("__parent") {
                            return map.get("__id").and_then(|v| v.as_str()).map(String::from);
                        }
                    }
                    None
                })
                .collect()
        });

    let mut output = String::new();

    // Output objects recursively
    fn output_obj_recursive(
        id: &str,
        obj_map: &std::collections::HashMap<String, &Value>,
        children_map: &std::collections::HashMap<String, Vec<String>>,
        output: &mut String,
        parent_level: Option<usize>,
        rendered_children: &mut std::collections::HashSet<String>,
    ) {
        // Skip if already rendered
        if rendered_children.contains(id) {
            return;
        }
        rendered_children.insert(id.to_string());

        let Some(obj) = obj_map.get(id) else { return };
        let Some(map) = obj.as_object() else { return };

        let kind = map
            .get("__kind")
            .and_then(|v| v.as_str())
            .unwrap_or("__Object");

        // Skip __Document
        if kind == "__Document" {
            return;
        }

        // Skip __ParsingError - these are validation errors, not content
        if kind == "__ParsingError" {
            return;
        }

        // Handle __TextBlock - output content as-is
        if kind == "__TextBlock" {
            if let Some(content) = map.get("content").and_then(|v| v.as_str()) {
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(content);
                output.push_str("\n\n");
            }
            return;
        }

        // Get metadata
        let obj_id = map.get("__id").and_then(|v| v.as_str()).unwrap_or("");
        // Use __local_id for heading reconstruction when present
        let heading_id = map
            .get("__local_id")
            .and_then(|v| v.as_str())
            .unwrap_or(obj_id);
        let label = map.get("__label").and_then(|v| v.as_str()).unwrap_or("");
        // Use __level if present, otherwise compute from parent
        let level = map
            .get("__level")
            .and_then(|v| v.as_u64())
            .map(|l| l as usize)
            .or(parent_level.map(|p| p + 2))
            .unwrap_or(2);
        let syntax = map.get("__syntax").and_then(|v| v.as_object());
        let has_explicit_id = map
            .get("__has_explicit_id")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Build heading
        let hashes = "#".repeat(level);

        // Add blank line before heading if needed (but not if already ends with \n\n)
        if !output.is_empty() && !output.ends_with("\n\n") {
            output.push('\n');
        }

        // Only output [[id]] if explicit
        if has_explicit_id {
            // Check for standalone field type hint (e.g. __syntax: {summary: "multiline_text"})
            let field_type_hint = if kind == "__Object" {
                syntax
                    .and_then(|s| s.get(heading_id))
                    .and_then(|v| v.as_str())
                    .and_then(|sv| match sv {
                        "multiline_text" => Some("text".to_string()),
                        "markdown_list" => Some("array".to_string()),
                        "yaml_object" => Some("yaml".to_string()),
                        "json_object" => Some("json".to_string()),
                        "map" => Some("map".to_string()),
                        "headers" => {
                            // Object array — get kind from __array_kind
                            syntax
                                .and_then(|s| s.get("__array_kind"))
                                .and_then(|v| v.as_str())
                                .map(|k| format!("[{}]", k))
                        }
                        _ => None,
                    })
            } else {
                None
            };

            let kind_suffix = if let Some(ref fth) = field_type_hint {
                format!(": {}", fth)
            } else if kind != "__Object" {
                // Don't include kind for array elements (kind is already in parent's [[field: [Kind]]] heading)
                let is_array_element = map
                    .get("__parent_field")
                    .and_then(|v| v.as_str())
                    .and_then(|pf| {
                        // Check if parent's syntax for this field is "headers" or "table"
                        let parent_ref = map.get("__parent").and_then(|v| v.as_str())?;
                        if parent_ref.starts_with("[[#") && parent_ref.ends_with("]]") {
                            let parent_id = &parent_ref[3..parent_ref.len() - 2];
                            let parent_obj = obj_map.get(parent_id)?;
                            let parent_map = parent_obj.as_object()?;
                            let parent_syntax = parent_map.get("__syntax")?.as_object()?;
                            let field_syntax = parent_syntax.get(pf)?.as_str()?;
                            if field_syntax == "headers" || field_syntax == "table" {
                                Some(())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .is_some();
                if is_array_element {
                    String::new()
                } else {
                    format!(": {}", kind)
                }
            } else {
                String::new()
            };
            output.push_str(&format!(
                "{} {} [[{}{}]]\n",
                hashes, label, heading_id, kind_suffix
            ));
        } else {
            output.push_str(&format!("{} {}\n", hashes, label));
        }

        // Output comments with "after": "__self" first (before fields)
        if let Some(comments) = map.get("__comments").and_then(|v| v.as_array()) {
            for comment in comments {
                if let Some(after) = comment.get("after").and_then(|v| v.as_str()) {
                    if after == "__self" {
                        if let Some(content) = comment.get("content").and_then(|v| v.as_str()) {
                            // Ensure exactly one blank line before comment content
                            if !output.ends_with("\n\n") {
                                output.push('\n');
                            }
                            output.push_str(content);
                            output.push('\n');
                        }
                    }
                }
            }
        }
        // Ensure blank line after __self comments (before fields)
        if !output.ends_with("\n\n") {
            output.push('\n');
        }

        // Build comments map by "after" field
        let comments_map: std::collections::HashMap<String, Vec<String>> = map
            .get("__comments")
            .and_then(|v| v.as_array())
            .map(|comments| {
                let mut m: std::collections::HashMap<String, Vec<String>> =
                    std::collections::HashMap::new();
                for comment in comments {
                    let after = comment
                        .get("after")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if after != "__self" {
                        if let Some(content) = comment.get("content").and_then(|v| v.as_str()) {
                            m.entry(after).or_default().push(content.to_string());
                        }
                    }
                }
                m
            })
            .unwrap_or_default();

        // Single-pass: output fields in insertion order, preserving original field order.
        // Heading-syntax fields and child ref headings are output inline.
        // Child ref headings encountered during a primitive run are buffered
        // and flushed when the primitive run ends.
        let types_map = map.get("__types").and_then(|v| v.as_object());
        let labels = map.get("__labels").and_then(|l| l.as_object());

        // Collect child IDs for this object
        let child_ids: Vec<String> = children_map.get(obj_id).cloned().unwrap_or_default();

        let mut in_primitive_run = false;
        let mut pending_child_headings: Vec<String> = Vec::new();

        // Helper: check if a ref_id is a child of this object with matching parent_field
        let is_child_ref = |ref_id: &str, field_key: &str| -> bool {
            obj_map
                .get(ref_id)
                .and_then(|o| o.as_object())
                .is_some_and(|ref_map| {
                    let is_child_of_this = ref_map
                        .get("__parent")
                        .and_then(|v| v.as_str())
                        .map(|p| {
                            if p.starts_with("[[#") && p.ends_with("]]") {
                                &p[3..p.len() - 2] == obj_id
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    let parent_field_matches = ref_map
                        .get("__parent_field")
                        .and_then(|v| v.as_str())
                        .map(|f| f == field_key)
                        .unwrap_or(false);
                    is_child_of_this && parent_field_matches
                })
        };

        for (key, value) in map.iter() {
            if key.starts_with("__") {
                continue;
            }

            let field_syntax = syntax
                .and_then(|s| s.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let is_heading_syntax = matches!(
                field_syntax,
                "headers"
                    | "table"
                    | "markdown_list"
                    | "multiline_text"
                    | "yaml_object"
                    | "json_object"
                    | "map"
            );

            // Check for single child reference (not heading syntax)
            if !is_heading_syntax {
                if let Value::String(s) = value {
                    if s.starts_with("[[#") && s.ends_with("]]") {
                        let ref_id = &s[3..s.len() - 2];
                        if is_child_ref(ref_id, key) {
                            // Output as primitive line if it has a __types entry
                            if types_map.is_some_and(|t| t.contains_key(key.as_str())) {
                                if !in_primitive_run {
                                    in_primitive_run = true;
                                }
                                output.push_str(&format!("- {}: {}\n", key, format_value(value)));
                                if let Some(field_comments) = comments_map.get(key.as_str()) {
                                    for content in field_comments {
                                        output.push('\n');
                                        output.push_str(content);
                                        output.push('\n');
                                    }
                                }
                                // Buffer child heading to render after primitive run
                                pending_child_headings.push(ref_id.to_string());
                            } else {
                                // No __types entry — end primitive run and render child immediately
                                if in_primitive_run {
                                    in_primitive_run = false;
                                    for pending_id in pending_child_headings.drain(..) {
                                        output_obj_recursive(
                                            &pending_id,
                                            obj_map,
                                            children_map,
                                            output,
                                            Some(level),
                                            rendered_children,
                                        );
                                    }
                                }
                                output_obj_recursive(
                                    ref_id,
                                    obj_map,
                                    children_map,
                                    output,
                                    Some(level),
                                    rendered_children,
                                );
                                if let Some(field_comments) = comments_map.get(key.as_str()) {
                                    for content in field_comments {
                                        output.push('\n');
                                        output.push_str(content);
                                        output.push('\n');
                                    }
                                }
                            }
                            continue;
                        }
                    }
                }
            }

            // Check for child array (all items are child refs) — not heading syntax
            if !is_heading_syntax {
                if let Value::Array(arr) = value {
                    let children_to_render: Vec<String> = arr
                        .iter()
                        .filter_map(|item| {
                            if let Value::String(s) = item {
                                if s.starts_with("[[#") && s.ends_with("]]") {
                                    let ref_id = &s[3..s.len() - 2];
                                    if is_child_ref(ref_id, key) {
                                        return Some(ref_id.to_string());
                                    }
                                }
                            }
                            None
                        })
                        .collect();

                    if !children_to_render.is_empty() && children_to_render.len() == arr.len() {
                        // End primitive run before child array headings
                        if in_primitive_run {
                            in_primitive_run = false;
                            for pending_id in pending_child_headings.drain(..) {
                                output_obj_recursive(
                                    &pending_id,
                                    obj_map,
                                    children_map,
                                    output,
                                    Some(level),
                                    rendered_children,
                                );
                            }
                        }
                        for child_id in &children_to_render {
                            output_obj_recursive(
                                child_id,
                                obj_map,
                                children_map,
                                output,
                                Some(level),
                                rendered_children,
                            );
                        }
                        continue;
                    }
                }
            }

            if is_heading_syntax {
                // End primitive run and flush pending child headings
                if in_primitive_run {
                    in_primitive_run = false;
                    for pending_id in pending_child_headings.drain(..) {
                        output_obj_recursive(
                            &pending_id,
                            obj_map,
                            children_map,
                            output,
                            Some(level),
                            rendered_children,
                        );
                    }
                }

                match field_syntax {
                    "multiline_text" => {
                        let field_label = labels
                            .and_then(|l| l.get(key.as_str()))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| titlecase_field(key));

                        output.push_str(&format!(
                            "\n{} {} [[{}: text]]\n\n",
                            "#".repeat(level + 1),
                            field_label,
                            key
                        ));

                        if let Some(text) = value.as_str() {
                            if !text.is_empty() {
                                output.push_str(text);
                                output.push_str("\n\n");
                            }
                        }

                        if let Some(field_comments) = comments_map.get(key.as_str()) {
                            for content in field_comments {
                                output.push_str(content);
                                output.push_str("\n\n");
                            }
                        }
                    }
                    "markdown_list" => {
                        let field_label = labels
                            .and_then(|l| l.get(key.as_str()))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| titlecase_field(key));

                        output.push_str(&format!(
                            "\n{} {} [[{}: array]]\n\n",
                            "#".repeat(level + 1),
                            field_label,
                            key
                        ));

                        if let Value::Array(arr) = value {
                            for item in arr {
                                let item_str = match item {
                                    Value::String(s) => s.clone(),
                                    Value::Null => "null".to_string(),
                                    Value::Bool(b) => b.to_string(),
                                    Value::Number(n) => n.to_string(),
                                    _ => format_value(item),
                                };
                                output.push_str(&format!("- {}\n", item_str));
                            }
                        }
                        output.push('\n');

                        if let Some(field_comments) = comments_map.get(key.as_str()) {
                            for content in field_comments {
                                output.push_str(content);
                                output.push_str("\n\n");
                            }
                        }
                    }
                    "yaml_object" => {
                        let field_label = labels
                            .and_then(|l| l.get(key.as_str()))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| titlecase_field(key));

                        output.push_str(&format!(
                            "\n{} {} [[{}: yaml]]\n\n",
                            "#".repeat(level + 1),
                            field_label,
                            key
                        ));
                        output.push_str("```yaml\n");
                        if let Some(obj) = value.as_object() {
                            for (k, v) in obj {
                                output.push_str(&format_yaml_value(k, v, 0));
                            }
                        }
                        output.push_str("```\n\n");
                    }
                    "json_object" => {
                        let field_label = labels
                            .and_then(|l| l.get(key.as_str()))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| titlecase_field(key));

                        output.push_str(&format!(
                            "\n{} {} [[{}: json]]\n\n",
                            "#".repeat(level + 1),
                            field_label,
                            key
                        ));
                        output.push_str("```json\n");
                        output.push_str(&serde_json::to_string_pretty(value).unwrap_or_default());
                        output.push_str("\n```\n\n");
                    }
                    "map" => {
                        let field_label = labels
                            .and_then(|l| l.get(key.as_str()))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| titlecase_field(key));

                        output.push_str(&format!(
                            "\n{} {} [[{}: map]]",
                            "#".repeat(level + 1),
                            field_label,
                            key
                        ));
                        if let Value::Object(map) = value {
                            if !map.is_empty() {
                                output.push('\n');
                                for (mk, mv) in map {
                                    let mv_str = mv.as_str().unwrap_or("");
                                    if mv_str.contains('\n') {
                                        output.push_str(&format!("\n- {}: |", mk));
                                        for ml in mv_str.split('\n') {
                                            output.push_str(&format!("\n    {}", ml));
                                        }
                                    } else {
                                        output.push_str(&format!("\n- {}: {}", mk, mv_str));
                                    }
                                }
                            }
                        }
                        output.push('\n');
                    }
                    "headers" => {
                        let elem_type = if let Value::Array(arr) = value {
                            arr.first()
                                .and_then(|v| v.as_str())
                                .and_then(|s| {
                                    if s.starts_with("[[#") && s.ends_with("]]") {
                                        let child_id = &s[3..s.len() - 2];
                                        obj_map
                                            .get(child_id)
                                            .and_then(|o| o.as_object())
                                            .and_then(|m| m.get("__kind"))
                                            .and_then(|k| k.as_str())
                                            .filter(|k| *k != "__Object")
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or("Item")
                        } else {
                            "Item"
                        };

                        // Self-array pattern: field key == object ID means the
                        // heading already includes [Kind] annotation — skip sub-heading
                        let is_self_array = key == obj_id;
                        if !is_self_array {
                            let field_label = labels
                                .and_then(|l| l.get(key.as_str()))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| titlecase_field(key));

                            output.push_str(&format!(
                                "\n{} {} [[{}: [{}]]]\n\n",
                                "#".repeat(level + 1),
                                field_label,
                                key,
                                elem_type
                            ));
                        }

                        if let Value::Array(refs) = value {
                            for ref_val in refs {
                                if let Some(ref_str) = ref_val.as_str() {
                                    if ref_str.starts_with("[[#") && ref_str.ends_with("]]") {
                                        let child_id = &ref_str[3..ref_str.len() - 2];
                                        // For self-array, children are one level below parent
                                        // For normal arrays, children are two levels below (after array heading)
                                        let child_parent_level = if is_self_array {
                                            Some(level - 1)
                                        } else {
                                            Some(level)
                                        };
                                        output_obj_recursive(
                                            child_id,
                                            obj_map,
                                            children_map,
                                            output,
                                            child_parent_level,
                                            rendered_children,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    "table" => {
                        if let Value::Array(refs) = value {
                            let kind = refs
                                .first()
                                .and_then(|v| v.as_str())
                                .and_then(|s| {
                                    if s.starts_with("[[#") && s.ends_with("]]") {
                                        let child_id = &s[3..s.len() - 2];
                                        obj_map
                                            .get(child_id)
                                            .and_then(|o| o.as_object())
                                            .and_then(|m| m.get("__kind"))
                                            .and_then(|k| k.as_str())
                                            .filter(|k| *k != "__Object")
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or("Item");

                            let field_label = labels
                                .and_then(|l| l.get(key.as_str()))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| titlecase_field(key));
                            output.push_str(&format!(
                                "\n{} {} [[{}: [{}]]]\n\n",
                                "#".repeat(level + 1),
                                field_label,
                                key,
                                kind
                            ));

                            let column_names: Vec<String> = refs
                                .first()
                                .and_then(|v| v.as_str())
                                .and_then(|s| {
                                    if s.starts_with("[[#") && s.ends_with("]]") {
                                        let child_id = &s[3..s.len() - 2];
                                        obj_map.get(child_id).and_then(|o| o.as_object())
                                    } else {
                                        None
                                    }
                                })
                                .map(|m| {
                                    m.keys().filter(|k| !k.starts_with("__")).cloned().collect()
                                })
                                .unwrap_or_default();

                            if !column_names.is_empty() {
                                output.push_str("| ");
                                output.push_str(&column_names.join(" | "));
                                output.push_str(" |\n|");
                                for _ in &column_names {
                                    output.push_str("---|");
                                }
                                output.push('\n');

                                for ref_val in refs {
                                    if let Some(ref_str) = ref_val.as_str() {
                                        if ref_str.starts_with("[[#") && ref_str.ends_with("]]") {
                                            let child_id = &ref_str[3..ref_str.len() - 2];
                                            rendered_children.insert(child_id.to_string());
                                            if let Some(child_obj) = obj_map.get(child_id) {
                                                if let Some(child_map) = child_obj.as_object() {
                                                    output.push_str("| ");
                                                    let vals: Vec<String> = column_names
                                                        .iter()
                                                        .map(|col| {
                                                            child_map
                                                                .get(col)
                                                                .map(format_value)
                                                                .unwrap_or_default()
                                                        })
                                                        .collect();
                                                    output.push_str(&vals.join(" | "));
                                                    output.push_str(" |\n");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            output.push('\n');
                        }
                    }
                    _ => {}
                }
            } else {
                // Primitive field
                if !in_primitive_run {
                    in_primitive_run = true;
                }

                let field_syntax_str = field_syntax;
                if field_syntax_str == "yaml_multiline" {
                    output.push_str(&format!("- {}: |\n", key));
                    if let Some(text) = value.as_str() {
                        for line in text.lines() {
                            output.push_str(&format!("    {}\n", line));
                        }
                    }
                } else if field_syntax_str == "yaml_multiline_array" {
                    if let Value::Array(arr) = value {
                        output.push_str(&format!("- {}: [\n", key));
                        for (idx, item) in arr.iter().enumerate() {
                            let formatted = format_value(item);
                            if idx < arr.len() - 1 {
                                output.push_str(&format!("    {},\n", formatted));
                            } else {
                                output.push_str(&format!("    {}\n", formatted));
                            }
                        }
                        output.push_str("  ]\n");
                    } else {
                        output.push_str(&format!("- {}: {}\n", key, format_value(value)));
                    }
                } else if field_syntax_str == "comma_refs" {
                    output.push_str(&format!("- {}: {}\n", key, format_comma_refs(value)));
                } else {
                    output.push_str(&format!("- {}: {}\n", key, format_value(value)));
                }

                if let Some(field_comments) = comments_map.get(key.as_str()) {
                    for content in field_comments {
                        output.push('\n');
                        output.push_str(content);
                        output.push('\n');
                    }
                }
            }
        }

        // Flush any remaining pending child headings
        for pending_id in pending_child_headings.drain(..) {
            output_obj_recursive(
                &pending_id,
                obj_map,
                children_map,
                output,
                Some(level),
                rendered_children,
            );
        }

        // Render remaining children not referenced by fields
        for child_id in &child_ids {
            if !rendered_children.contains(child_id.as_str()) {
                output_obj_recursive(
                    child_id,
                    obj_map,
                    children_map,
                    output,
                    Some(level),
                    rendered_children,
                );
            }
        }
    }

    let mut rendered_children = std::collections::HashSet::new();
    for id in &ordered_ids {
        output_obj_recursive(
            id,
            &obj_map,
            &children_map,
            &mut output,
            None,
            &mut rendered_children,
        );
    }

    // Ensure exactly one trailing newline
    let trimmed = output.trim_end_matches('\n');
    let mut result = trimmed.to_string();
    result.push('\n');
    result
}

fn titlecase_field(key: &str) -> String {
    key.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_yaml_value(key: &str, value: &Value, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    match value {
        Value::Object(obj) => {
            let mut result = format!("{}{}:\n", prefix, key);
            for (k, v) in obj {
                result.push_str(&format_yaml_value(k, v, indent + 1));
            }
            result
        }
        Value::Array(arr) => {
            let mut result = format!("{}{}:\n", prefix, key);
            for item in arr {
                match item {
                    Value::String(s) => {
                        result.push_str(&format!("{}- {}\n", "  ".repeat(indent + 1), s))
                    }
                    _ => result.push_str(&format!(
                        "{}- {}\n",
                        "  ".repeat(indent + 1),
                        format_value(item)
                    )),
                }
            }
            result
        }
        Value::String(s) => format!("{}{}: {}\n", prefix, key, s),
        Value::Number(n) => format!("{}{}: {}\n", prefix, key, n),
        Value::Bool(b) => format!("{}{}: {}\n", prefix, key, b),
        Value::Null => format!("{}{}: null\n", prefix, key),
    }
}

fn format_value(value: &Value) -> String {
    format_value_inner(value, false)
}

fn format_comma_refs(value: &Value) -> String {
    if let Value::Array(arr) = value {
        let items: Vec<String> = arr
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        items.join(", ")
    } else {
        format_value(value)
    }
}

fn format_value_inner(value: &Value, in_array: bool) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // Empty strings must be quoted to distinguish from null
            if s.is_empty() {
                return "\"\"".to_string();
            }
            // Strings that look like incomplete references need quotes
            if s.starts_with("[[") && !s.ends_with("]]") {
                return format!("\"{}\"", s);
            }
            // Strings that look like list items need quotes
            if s.starts_with("- ") {
                return format!("\"{}\"", s);
            }
            // In YAML arrays, quote strings with spaces or special chars
            if in_array {
                let needs_quotes = s.contains(' ')
                    || s.contains(',')
                    || (s.starts_with('[') && !s.starts_with("[["));

                if needs_quotes {
                    format!("\"{}\"", s)
                } else {
                    s.clone()
                }
            } else {
                // For list items (- key: value), no quotes needed
                s.clone()
            }
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|v| format_value_inner(v, true)).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_rebuild_basic() {
        let objects = vec![json!({
            "__id": "user",
            "__label": "User",
            "__kind": "Person",
            "__level": 2,
            "name": "Alice",
            "age": 30
        })];

        let md = rebuild(&objects);
        assert!(md.contains("## User [[user: Person]]"));
        assert!(md.contains("- name: Alice"));
        assert!(md.contains("- age: 30"));
    }
}
