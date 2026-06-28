"""Field parser - extracts fields from list items."""

import re
from typing import Any

from markdown_it.token import Token

# Pre-compiled regexes for field parsing (compiled once at module load)
_FIELD_PATTERN = re.compile(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$", re.DOTALL)
_INVALID_FIELD_LIKE_PATTERN = re.compile(r"^([^:]+):\s+(.*)$", re.DOTALL)
_VALID_KEY_PATTERN = re.compile(r"^[a-zA-Z_][a-zA-Z0-9_]*$")


def parse_yaml_array(value_str: str) -> tuple[list[Any], dict[str, str]]:
    """
    Parse YAML-style array like [a, b, c] or [1, 2, 3].

    Returns: (items_list, item_types)
    """
    # Remove brackets
    inner = value_str[1:-1].strip()

    if not inner:
        return [], {}

    items: list[Any] = []
    types: dict[str, str] = {}

    # Split by comma, handle quoted strings
    parts = _split_yaml_array(inner)

    for idx, part in enumerate(parts):
        val, type_name = parse_field_value(part)
        items.append(val)
        types[str(idx)] = type_name

    return items, types


def _split_yaml_array(s: str) -> list[str]:
    """Split YAML array content by commas, respecting quotes."""
    result: list[str] = []
    current = ""
    in_quotes = False
    quote_char = ""

    for char in s:
        if char in ('"', "'") and not in_quotes:
            in_quotes = True
            quote_char = char
            current += char
        elif char == quote_char and in_quotes:
            in_quotes = False
            current += char
            quote_char = ""
        elif char == "," and not in_quotes:
            result.append(current.strip())
            current = ""
        else:
            current += char

    if current.strip():
        result.append(current.strip())

    return result


def parse_field_value(value_str: str) -> tuple[Any, str]:
    """
    Parse field value and auto-detect type.

    Returns: (value, type_name)

    Types:
    - array → (list, "array")
    - null → (None, "null")
    - true/false → (bool, "boolean")
    - number → (int/float, "number")
    - string → (str, "string")
    """
    value = value_str.strip()

    # Empty array
    if value == "[]":
        return [], "array"

    # Multiple comma-separated references: [[#a]], [[#b]], [[#c]]
    # NOT a YAML array (which would be [[[#a]], [[#b]]])
    if value.startswith("[[") and not value.startswith("[[[") and "]], [[" in value:
        items = []
        for part in value.split("]], [["):
            part = part.strip()
            # Restore brackets
            if not part.startswith("[["):
                part = "[[" + part
            if not part.endswith("]]"):
                part = part + "]]"
            items.append(part)
        return items, "ref_array"

    # YAML array [a, b, c] - but not single references [[#id]]
    # Array of refs [[[#id1]], [[#id2]]] should be parsed as array
    is_array_bracket = value.startswith("[") and value.endswith("]")
    is_single_ref = value.startswith("[[") and not value.startswith("[[[")
    if is_array_bracket and not is_single_ref:
        items, _ = parse_yaml_array(value)
        return items, "array"

    # null
    if value == "null":
        return None, "null"

    # boolean
    if value == "true":
        return True, "boolean"
    if value == "false":
        return False, "boolean"

    # number (int or float)
    try:
        # Try int first
        if "." not in value:
            return int(value), "number"
        # Then float
        return float(value), "number"
    except ValueError:
        pass

    # string (default) - remove quotes if present
    if (value.startswith('"') and value.endswith('"')) or (
        value.startswith("'") and value.endswith("'")
    ):
        return value[1:-1], "string"

    return value, "string"


def parse_fields_from_list(
    tokens: list[Token], start_idx: int, block_tree: Any = None, *, raw_strings: bool = False
) -> tuple[
    dict[str, Any], dict[str, str], dict[str, str], list[dict[str, Any]], int, list[dict[str, Any]]
]:
    """
    Parse fields from markdown list starting at start_idx.

    Returns:
        (fields_dict, types_dict, syntax_dict, invalid_items, next_index, nested_subitems_errors)

    invalid_items: list of {"key": str, "content": str, "line": int, "after": str} for items
                   that look like fields but have invalid keys (e.g. Cyrillic).
                   "after" is the last valid field key before this item, or "__self".
    nested_subitems_errors: list of {"key": str, "line": int} for fields with nested sub-items
                           (pattern `- key:\n  - item` which is forbidden).
    """
    fields: dict[str, Any] = {}
    types: dict[str, str] = {}
    syntax: dict[str, str] = {}
    invalid_items: list[dict[str, Any]] = []
    nested_subitems_errors: list[dict[str, Any]] = []
    i = start_idx

    # Pattern: `- key: value` or `- key:value` (with DOTALL for multiline values)
    field_pattern = _FIELD_PATTERN
    # Pattern for invalid field-like items (any text with colon and space)
    invalid_field_like_pattern = _INVALID_FIELD_LIKE_PATTERN
    # Valid key pattern
    valid_key_pattern = _VALID_KEY_PATTERN

    current_line = 0
    last_valid_field: str = "__self"

    while i < len(tokens):
        token = tokens[i]

        if token.type == "bullet_list_open":
            i += 1
            continue

        if token.type == "bullet_list_close":
            i += 1
            break

        if token.type == "list_item_open":
            # Track line number for error reporting
            if token.map:
                current_line = token.map[0] + 1  # 1-based
            i += 1
            continue

        if token.type == "list_item_close":
            i += 1
            continue

        if token.type == "paragraph_open":
            i += 1
            continue

        if token.type == "paragraph_close":
            i += 1
            continue

        if token.type == "inline":
            # Parse field from inline content
            content = token.content.strip()
            # Get line from token map if available
            if token.map:
                current_line = token.map[0] + 1
            first_line = content.split("\n")[0]
            match = field_pattern.match(first_line)

            if match:
                key = match.group(1)
                # For multiline content, get value from full content
                colon_pos = content.find(":")
                value_str = content[colon_pos + 1 :].strip() if colon_pos >= 0 else match.group(2)

                # Check for YAML multiline: `field: |`
                # markdown-it may merge `field: |` with next line into one inline token,
                # so value_str can be `"|"` or `"|\n  next line..."`.
                if value_str.strip() == "|" or value_str.strip().startswith("|\n"):
                    # Use raw-slice from BlockTree when available (preserves numbering, formatting)
                    if block_tree and token.map:
                        # Find the OUTER list_item_close (track nesting to skip inner ones)
                        scan = i + 1
                        nesting = 0
                        while scan < len(tokens):
                            st = tokens[scan].type
                            if st in ("ordered_list_open", "bullet_list_open"):
                                nesting += 1
                            elif st in ("ordered_list_close", "bullet_list_close"):
                                nesting -= 1
                            elif st == "list_item_close" and nesting == 0:
                                break
                            scan += 1
                        pipe_line = token.map[0]  # line of "- field: |"
                        # End line: next list_item_open or bullet_list_close
                        end_line = None
                        for j in range(scan, min(scan + 3, len(tokens))):
                            if j < len(tokens) and tokens[j].type == "list_item_open":
                                if tokens[j].map:
                                    end_line = tokens[j].map[0]
                                break
                            if j < len(tokens) and tokens[j].type == "bullet_list_close":
                                break
                        if end_line is None:
                            # Last item — find bullet_list_close
                            for j in range(scan, min(scan + 3, len(tokens))):
                                if j < len(tokens) and tokens[j].type == "bullet_list_close":
                                    if tokens[j].map:
                                        end_line = tokens[j].map[0]
                                    break
                        if end_line is None:
                            end_line = block_tree.line_count

                        # Extract raw content after the pipe line
                        raw = block_tree.get_lines_raw(pipe_line + 1, end_line)
                        # Strip trailing blank lines
                        raw_stripped = raw.rstrip("\n")
                        # Dedent: remove common leading whitespace
                        raw_lines = raw_stripped.split("\n")
                        if raw_lines:
                            indents = [len(ln) - len(ln.lstrip()) for ln in raw_lines if ln.strip()]
                            min_indent = min(indents) if indents else 0
                            raw_lines = [ln[min_indent:] for ln in raw_lines]
                        value = "\n".join(raw_lines)

                        fields[key] = value
                        types[key] = "string"
                        syntax[key] = "yaml_multiline"
                        last_valid_field = key
                        # Skip to list_item_close
                        i = scan
                        continue
                    else:
                        # Fallback: collect from tokens
                        multiline_parts: list[str] = []
                        if value_str.strip().startswith("|\n"):
                            after_pipe = value_str.strip()[2:]
                            lines = after_pipe.split("\n")
                            if lines:
                                indents = [len(ln) - len(ln.lstrip()) for ln in lines if ln.strip()]
                                min_indent = min(indents) if indents else 0
                                lines = [
                                    ln[min_indent:] if len(ln) >= min_indent else ln for ln in lines
                                ]
                            multiline_parts.append("\n".join(lines))

                        i += 1
                        nesting = 0
                        while i < len(tokens) and tokens[i].type != "list_item_close":
                            t = tokens[i]
                            if t.type == "fence":
                                lang = t.info or ""
                                fence_content = t.content.rstrip("\n")
                                if lang:
                                    multiline_parts.append(f"```{lang}\n{fence_content}\n```")
                                else:
                                    multiline_parts.append(f"```\n{fence_content}\n```")
                            elif t.type == "code_block":
                                multiline_parts.append(t.content.rstrip("\n"))
                            elif t.type == "inline":
                                multiline_parts.append(t.content)
                            elif t.type in (
                                "ordered_list_open",
                                "bullet_list_open",
                            ):
                                nesting += 1
                            elif t.type in (
                                "ordered_list_close",
                                "bullet_list_close",
                            ):
                                nesting -= 1
                            elif t.type in (
                                "paragraph_open",
                                "paragraph_close",
                                "list_item_open",
                                "list_item_close",
                            ):
                                pass
                            i += 1

                        value = "\n".join(multiline_parts) if multiline_parts else ""
                        fields[key] = value
                        types[key] = "string"
                        syntax[key] = "yaml_multiline"
                        last_valid_field = key
                        continue

                if raw_strings:
                    fields[key] = value_str
                    types[key] = "string"
                    last_valid_field = key
                else:
                    value, type_name = parse_field_value(value_str)
                    fields[key] = value
                    types[key] = "array" if type_name == "ref_array" else type_name
                    last_valid_field = key

                    # Track syntax for arrays
                    if type_name == "ref_array":
                        syntax[key] = "comma_refs"
                    elif type_name == "array":
                        # Detect multiline array: value spans multiple lines
                        if "\n" in value_str and value_str.strip().startswith("["):
                            syntax[key] = "yaml_multiline_array"
                        else:
                            syntax[key] = "yaml_array"

                # Detect nested sub-items: field with empty value followed by nested list
                # e.g. `- affected_files:\n  - item1\n  - item2`
                # This is a syntax error (nested_subitems) per spec rule_no_nested_subitems
                if value_str == "" and i + 1 < len(tokens):
                    # Look ahead for nested list (bullet or ordered)
                    # before list_item_close
                    lookahead = i + 1
                    while lookahead < len(tokens) and tokens[lookahead].type in (
                        "paragraph_close",
                    ):
                        lookahead += 1
                    if lookahead < len(tokens) and tokens[lookahead].type in (
                        "bullet_list_open",
                        "ordered_list_open",
                    ):
                        nested_list_type = tokens[lookahead].type
                        nested_list_close = nested_list_type.replace("_open", "_close")
                        # Skip past the nested list
                        lookahead += 1  # skip list_open
                        while (
                            lookahead < len(tokens) and tokens[lookahead].type != nested_list_close
                        ):
                            lookahead += 1
                        if lookahead < len(tokens):
                            lookahead += 1  # skip list_close
                        # Record as nested_subitems error
                        nested_subitems_errors.append(
                            {
                                "key": key,
                                "line": current_line,
                            }
                        )
                        # Remove the field we just added (it has empty value)
                        del fields[key]
                        if key in types:
                            del types[key]
                        i = lookahead
                        continue
            else:
                # Not a valid field - check if it looks like a field with invalid key
                invalid_match = invalid_field_like_pattern.match(first_line)
                if invalid_match:
                    potential_key = invalid_match.group(1).strip()
                    if potential_key and not valid_key_pattern.match(potential_key):
                        # Invalid field key (e.g. Cyrillic)
                        invalid_items.append(
                            {
                                "key": potential_key,
                                "content": f"- {content}",
                                "line": current_line,
                                "after": last_valid_field,
                            }
                        )
                elif last_valid_field != "__self" and content:
                    # Plain list item after a valid field — not a field at all.
                    # Save as invalid item so it can be preserved in __comments.
                    invalid_items.append(
                        {
                            "key": "",
                            "content": f"- {content}",
                            "line": current_line,
                            "after": last_valid_field,
                        }
                    )

            i += 1
            continue

        # Unknown token, skip
        i += 1

    return fields, types, syntax, invalid_items, i, nested_subitems_errors


def parse_array_items_from_list(tokens: list[Token], start_idx: int) -> tuple[list[Any], int]:
    """
    Parse list items as array elements (no key: prefix).

    Used for [[field: array]] sections where list items are plain values.
    Only bullet lists reach this function — ordered lists are intercepted
    by the parser and emitted as ordered_list_in_array errors.

    Returns:
        (items_list, next_index)
    """
    items: list[Any] = []
    i = start_idx
    nesting = 0

    while i < len(tokens):
        token = tokens[i]
        if token.type in ("bullet_list_open", "ordered_list_open"):
            nesting += 1
            i += 1
            continue
        if token.type in ("bullet_list_close", "ordered_list_close"):
            nesting -= 1
            if nesting <= 0:
                i += 1
                break
            i += 1
            continue
        if token.type == "list_item_open":
            i += 1
            continue
        if token.type == "list_item_close":
            i += 1
            continue
        if token.type == "paragraph_open":
            i += 1
            continue
        if token.type == "paragraph_close":
            i += 1
            continue
        if token.type == "inline":
            content = token.content.strip()
            value, _ = parse_field_value(content)
            items.append(value)
            i += 1
            continue
        i += 1

    return items, i
