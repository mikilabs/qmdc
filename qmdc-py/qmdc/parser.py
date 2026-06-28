"""Main QMDC parser."""

import json
import re
from typing import Any, Literal, TypedDict

from markdown_it.token import Token

from .parsers.field import parse_array_items_from_list, parse_fields_from_list
from .parsers.header import HeaderResult, generate_fallback_id, parse_header, set_random_seed
from .tokenizer import tokenize

# Pre-compiled regexes for field detection and mixed_field_keys scanning
_FIELD_RE = re.compile(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$")
_INVALID_FL_RE = re.compile(r"^([^:]+):\s+(.*)$", re.DOTALL)
_VALID_KEY_RE = re.compile(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
_BACKTICK_STRIP_RE = re.compile(r"`[^`]+`")
_BOLD_STRIP_RE = re.compile(r"\*\*([^*]*)\*\*")
_ITALIC_STRIP_RE = re.compile(r"\*([^*]*)\*")
_STRIKETHROUGH_STRIP_RE = re.compile(r"~~([^~]*)~~")
_FIRST_LINE_NORMALIZE_RE = re.compile(r"^([^\n]+)\n")


class BlockTree:
    """
    Minimal BlockTree utilities - Python port from Rust.

    Stores raw source and provides line<->offset conversion for extracting
    raw markdown slices without parsing internal structure.
    """

    def __init__(self, source: str):
        self.source = source
        self._lines = source.split("\n")
        # line_starts[i] = byte offset where line i begins (0-based)
        self._line_starts: list[int] = [0]
        for line in self._lines[:-1]:
            self._line_starts.append(self._line_starts[-1] + len(line) + 1)  # +1 for \n

    def line_to_offset(self, line: int) -> int:
        """Convert 0-based line number to byte offset."""
        if line < 0:
            return 0
        if line >= len(self._line_starts):
            return len(self.source)
        return self._line_starts[line]

    @property
    def line_count(self) -> int:
        """Get the total number of lines in the source."""
        return len(self._lines)

    def get_lines_raw(self, start_line: int, end_line: int) -> str:
        """
        Get raw content from start_line (inclusive) to end_line (exclusive), 0-based.
        Returns the raw slice without any transformation.
        """
        start_offset = self.line_to_offset(start_line)
        end_offset = self.line_to_offset(end_line)
        return self.source[start_offset:end_offset]


class CodeFenceInfo(TypedDict):
    """Metadata for a code fence within __TextBlock."""

    lang: str
    offset_line: int  # 0-based line offset within content
    length_lines: int  # number of lines including ``` markers


# Output features
FEATURE_ID = "id"  # __id even if auto-generated
FEATURE_KIND = "kind"  # __kind even if system type
FEATURE_LABEL = "label"  # __label field
FEATURE_PARENT = "parent"  # __parent, __parent_field, __container
FEATURE_TYPES = "types"  # __types for field type inference
FEATURE_SYNTAX = "syntax"  # __syntax for lossless rebuild (yaml/json/table/headers)
FEATURE_LEVEL = "level"  # __level for heading level (rebuild)
FEATURE_EXPLICIT_ID = "explicit_id"  # __has_explicit_id for rebuild
FEATURE_LINE = "line"  # __line for LSP (line number)
FEATURE_REFERENCES = "references"  # __references for LSP (reference tracking)
FEATURE_POSITIONS = "positions"  # __positions for LSP (field line/col positions)

# Format presets
# minimal: pure data only, no metadata (id/kind only if explicitly set in document)
# standard: supports lossless rebuild
# full: supports LSP (line numbers for go-to-definition, hover, etc.)
FORMATS: dict[str, set[str]] = {
    "minimal": set(),
    "standard": {
        FEATURE_ID,
        FEATURE_KIND,
        FEATURE_LABEL,
        FEATURE_PARENT,
        FEATURE_TYPES,
        FEATURE_SYNTAX,
        FEATURE_LEVEL,
        FEATURE_EXPLICIT_ID,
    },
    "full": {
        FEATURE_ID,
        FEATURE_KIND,
        FEATURE_LABEL,
        FEATURE_PARENT,
        FEATURE_TYPES,
        FEATURE_SYNTAX,
        FEATURE_LEVEL,
        FEATURE_EXPLICIT_ID,
        FEATURE_LINE,
        FEATURE_REFERENCES,
        FEATURE_POSITIONS,
    },
}

# Regex to find [[...]] references
REFERENCE_PATTERN = re.compile(r"\[\[([^\]]+)\]\]")


def classify_reference(inner: str) -> str:
    """Classify reference type based on content."""
    # Handle # prefix
    content = inner[1:] if inner.startswith("#") else inner

    # Check for crossfile references (contain / or # in middle)
    if "/" in content or (not inner.startswith("#") and "#" in content):
        return "crossfile"

    # Check for Kind:id or Kind.id format (first char is uppercase = Kind)
    # Or namespace:id format (first char is lowercase = namespace)
    if ":" in content or "." in content:
        sep = ":" if ":" in content else "."
        parts = content.split(sep, 1)
        if len(parts) == 2:
            first = parts[0]
            # If first char is uppercase, assume Kind
            if first and first[0].isupper():
                return "kind"
            else:
                return "namespace"

    # hash_local vs local
    if inner.startswith("#"):
        return "hash_local"
    return "local"


def is_inside_backticks(text: str, pos: int) -> bool:
    """Check if position is inside backticks (inline code)."""
    in_backtick = False
    i = 0
    while i < len(text) and i < pos:
        if text[i] == "`":
            # Check for triple backticks (code fence) - treat entire line as code
            if i + 2 < len(text) and text[i + 1] == "`" and text[i + 2] == "`":
                return True
            # Check for double backticks (``) - treat as single backtick pair
            if i + 1 < len(text) and text[i + 1] == "`":
                # Skip both backticks and toggle state
                i += 1  # Skip second backtick
                in_backtick = not in_backtick
            else:
                in_backtick = not in_backtick
        i += 1
    return in_backtick


def extract_references_from_text(
    text: str, line_num: int, col_offset: int = 0
) -> list[dict[str, Any]]:
    """Extract references from text, returning list of reference objects."""
    refs = []
    for match in REFERENCE_PATTERN.finditer(text):
        # Skip references inside backticks (inline code)
        if is_inside_backticks(text, match.start()):
            continue

        inner = match.group(1)

        # Only references start with '#'
        # [[#id]], [[#ns:id]], [[#Kind.field]] - references
        # [[id]], [[id:Kind]], [[field:text]] - definitions (skip)
        if not inner.startswith("#"):
            continue

        start_col = col_offset + match.start()
        end_col = col_offset + match.end()
        refs.append(
            {
                "target": inner,
                "type": classify_reference(inner),
                "line": line_num,
                "start_col": start_col,
                "end_col": end_col,
                "raw": match.group(0),
            }
        )
    return refs


def parse(
    markdown: str,
    random_seed: int = 666,
    format: Literal["minimal", "standard", "full"] = "standard",
    features: set[str] | None = None,
) -> list[dict[str, Any]]:
    """
    Parse QMD.md to JSON array of objects.

    Args:
        markdown: QMD.md source string
        random_seed: Seed for deterministic fallback IDs
        format: Output format preset ("minimal", "standard", "full")
        features: Override specific features (if None, uses format preset)

    Returns:
        [{object1}, {object2}, ...]
    """
    if format not in FORMATS:
        raise ValueError(f"Invalid format: {format}. Must be one of {list(FORMATS.keys())}")

    # Use format preset or custom features
    active_features = features if features is not None else FORMATS[format]
    # Set random seed for deterministic fallback IDs
    set_random_seed(random_seed)

    tokens: list[Token] = tokenize(markdown)
    # Stage 1: BlockTree for raw slice extraction (like Rust)
    block_tree = BlockTree(markdown)
    objects: dict[str, dict[str, Any]] = {}
    duplicate_objects: list[dict[str, Any]] = []
    first_seen_lines: dict[str, int] = {}  # Track true first line for each duplicate ID

    def compose_hierarchical_id(parent_id: str, local_id: str) -> str:
        """Compose a hierarchical dot-ID from parent ID and local segment."""
        return parent_id + "." + local_id

    def resolve_child_id(
        parent_id: str, local_id: str, arr_field: str | None = None
    ) -> tuple[str, str | None]:
        """Resolve the composed ID for a child object.

        Returns (composed_id, local_id_or_None).
        - If parent is a system container (__Workspace/__Namespace): returns (local_id, None)
        - Otherwise: returns (hierarchical_id, local_id)

        arr_field: if set, this is an array element and the field name is included in the path.
        When arr_field contains a dot (dot-ID), it's used directly as the path prefix
        (bypassing the structural parent's ID) since it declares its own parentage.
        When arr_field equals parent's __id, the field is the parent itself (top-level array)
        so we skip the extra field name in the path.
        """
        parent_kind = objects[parent_id].get("__kind", "")
        if parent_kind in ("__Workspace", "__Namespace"):
            return (local_id, None)
        parent_full_id = objects[parent_id]["__id"]
        if arr_field:
            if "." in arr_field:
                # Dot-ID in array field: use dot-ID directly as prefix
                composed = arr_field + "." + local_id
            elif arr_field == parent_full_id:
                # Top-level array: parent IS the array, skip extra field name
                composed = compose_hierarchical_id(parent_full_id, local_id)
            else:
                composed = compose_hierarchical_id(parent_full_id, arr_field + "." + local_id)
        else:
            composed = compose_hierarchical_id(parent_full_id, local_id)
        return (composed, local_id)

    # Stack of (object_id, heading_level) for tracking nesting
    object_stack: list[tuple[str, int]] = []

    # Track pending array field from [[field: array]] heading
    pending_array_field: tuple[str, str] | None = None  # (parent_id, field_name)

    # Track pending object array from [[field: [Kind]]] heading
    # (parent_id, field_name, array_kind, level)
    pending_object_array: tuple[str, str, str, int] | None = None

    # Track pending YAML field from [[field: yaml]] heading
    pending_yaml_field: tuple[str, str, str] | None = None  # (parent_id, field_name, label)

    # Track pending JSON field from [[field: json]] heading
    pending_json_field: tuple[str, str, str] | None = None  # (parent_id, field_name, label)

    # Track last anchor for comments (field name or "__self")
    comment_anchor: str = "__self"

    # Track text blocks (headings without [[id]] and without fields)
    text_blocks: list[dict[str, Any]] = []
    text_block_counter: int = 0

    # Track content order for __Document
    content_order: list[str] = []  # List of object IDs and text block IDs in order

    # Track pending text block content
    pending_text_block_content: list[str] = []
    pending_text_block_started: bool = False
    pending_text_block_line: int = 0
    pending_text_block_level: int = 0  # Level of the TextBlock heading
    pending_code_fences: list[CodeFenceInfo] = []

    # Track parsing errors (structured_in_textblock, etc.)
    parsing_errors: list[dict[str, Any]] = []

    def get_current_object_id() -> str | None:
        return object_stack[-1][0] if object_stack else None

    def get_heading_level(tag: str) -> int:
        """Extract heading level from tag like 'h1', 'h2', etc."""
        if tag.startswith("h") and len(tag) >= 2:
            try:
                return int(tag[1:])
            except ValueError:
                return 0
        return 0

    def append_comment(obj_id: str, anchor: str, content: str, merge: bool = False) -> None:
        """
        Append content to object's __comments.

        If merge=True and the last comment has the same `after` value, append content
        with `\n\n` separator. Otherwise, create a new comment entry.
        """
        if obj_id not in objects:
            return
        if "__comments" not in objects[obj_id]:
            objects[obj_id]["__comments"] = []

        comments = objects[obj_id]["__comments"]
        if merge and comments and comments[-1].get("after") == anchor:
            # Merge with existing comment
            comments[-1]["content"] = comments[-1]["content"] + "\n\n" + content
        else:
            # Create new comment entry
            comments.append({"after": anchor, "content": content})

    def has_nested_structured_headings(start_idx: int, current_level: int) -> bool:
        """
        Look-ahead to check if there are nested headings with [[...]] at a deeper level.
        Returns True if any heading at a deeper level contains [[...]] bracket syntax.
        Stops at headings at same or higher level.
        """
        bracket_re = re.compile(r"\[\[[^\]]+\]\]")
        j = start_idx + 3  # Skip heading_open, inline, heading_close
        while j < len(tokens):
            tok = tokens[j]
            if tok.type == "heading_open":
                next_level = get_heading_level(tok.tag)
                if next_level <= current_level:
                    return False
                # Check if the heading text contains [[...]]
                if j + 1 < len(tokens) and tokens[j + 1].type == "inline":
                    heading_content = tokens[j + 1].content or ""
                    if bracket_re.search(heading_content):
                        return True
                j += 3  # Skip heading_open, inline, heading_close
                continue
            j += 1
        return False

    def has_fields_after_heading(start_idx: int, current_level: int) -> bool:
        """
        Look-ahead to check if there are field lists DIRECTLY after a heading.
        Returns True if bullet_list with valid fields found before any other heading.
        Only checks immediate content, not nested sections.

        A valid field list must contain at least one item matching `- key: value`
        where key is a valid QMD.md identifier (starts with letter or _, contains only
        letters, digits, _).
        """
        field_pattern = _FIELD_RE

        j = start_idx + 3  # Skip heading_open, inline, heading_close
        while j < len(tokens):
            tok = tokens[j]
            if tok.type == "heading_open":
                # Any heading ends the search - no direct fields found
                return False
            elif tok.type == "table_open":
                # Tables are just text content, not fields
                # Continue looking for actual field lists
                j += 1
                continue
            elif tok.type == "bullet_list_open":
                # Check if list contains valid fields (- key: value)
                k = j + 1
                has_valid_field = False
                while k < len(tokens) and tokens[k].type != "bullet_list_close":
                    if tokens[k].type == "inline":
                        content = tokens[k].content.strip() if tokens[k].content else ""
                        if field_pattern.match(content):
                            has_valid_field = True
                            break
                    k += 1
                return has_valid_field
            elif tok.type == "fence":
                # Fences are just code blocks in text, not fields.
                # yaml/json fences are only fields when the heading
                # explicitly declares [[field: yaml]] or [[field: json]].
                j += 1
                continue
            j += 1
        return False

    i: int = 0
    while i < len(tokens):
        token: Token = tokens[i]

        if token.type == "heading_open":
            level = get_heading_level(token.tag)
            header: HeaderResult | None = parse_header(tokens, i)

            if header:
                # Pop objects from stack that are at same or deeper level
                while object_stack and object_stack[-1][1] >= level:
                    popped_id, popped_level = object_stack.pop()
                    # Reset comment_anchor to the field that references the popped object
                    # on the new parent (so subsequent comments anchor correctly)
                    if object_stack:
                        new_parent_id = object_stack[-1][0]
                        if new_parent_id in objects:
                            # Find which field on the parent references the popped object
                            for fk, fv in objects[new_parent_id].items():
                                if fk.startswith("__"):
                                    continue
                                if (isinstance(fv, str) and fv == f"[[#{popped_id}]]") or (
                                    isinstance(fv, list) and f"[[#{popped_id}]]" in fv
                                ):
                                    comment_anchor = fk
                                    break

                parent_id = get_current_object_id()
                # Get line number (1-based for LSP)
                line_num = (token.map[0] + 1) if token.map else None

                # Emit multiple_definitions error if heading has 2+ [[...]]
                if header.get("multiple_definitions"):
                    parsing_errors.append(
                        {
                            "__id": f"error_{len(parsing_errors)}",
                            "__kind": "__ParsingError",
                            "type": "multiple_definitions",
                            "definitions": header["multiple_definitions"],
                            "object": f"[[#{header['id']}]]",
                            "line": line_num,
                        }
                    )

                # Clear pending states when encountering a new heading
                if pending_array_field:
                    _, paf_field_name = pending_array_field
                    comment_anchor = paf_field_name
                    pending_array_field = None

                # Check if we're exiting an object array context
                if pending_object_array:
                    arr_parent_id, arr_field, arr_kind, arr_level = pending_object_array
                    if level <= arr_level:
                        # Exiting object array context
                        pending_object_array = None

                # Check if this is a primitive array [[field: array]]
                if header.get("field_type") == "array" and parent_id:
                    # This is a field array, not an object
                    # Mark for next list to be parsed as array items
                    pending_array_field = (parent_id, header["id"])

                # Check if this is an object array [[field: [Kind]]]
                elif header.get("field_type") == "object_array" and parent_id:
                    array_kind = header.get("array_kind", "")
                    # Initialize empty array in parent
                    objects[parent_id][header["id"]] = []
                    # Add __syntax for headers
                    if "__syntax" not in objects[parent_id]:
                        objects[parent_id]["__syntax"] = {}
                    objects[parent_id]["__syntax"][header["id"]] = "headers"
                    # Store label
                    if "__labels" not in objects[parent_id]:
                        objects[parent_id]["__labels"] = {}
                    objects[parent_id]["__labels"][header["id"]] = header["label"]
                    # Mark context for following headings
                    pending_object_array = (parent_id, header["id"], array_kind, level)

                # Top-level object array [[field: [Kind]]] without structural parent
                elif header.get("field_type") == "object_array" and not parent_id:
                    array_kind = header.get("array_kind", "")
                    # Create the heading as a parent object that owns the array
                    obj_id = header["id"]
                    obj = {
                        "__id": obj_id,
                        "__kind": "__Object",
                        "__level": level,
                        "__line": line_num,
                        obj_id: [],  # array field with same name as id
                    }
                    if header["label"]:
                        obj["__label"] = header["label"]
                    if not header.get("has_explicit_id", False):
                        obj["__has_explicit_id"] = False
                    obj["__syntax"] = {obj_id: "headers", "__array_kind": array_kind}
                    obj["__labels"] = {obj_id: header["label"]}
                    objects[obj_id] = obj
                    object_stack.append((obj_id, level))
                    content_order.append(obj_id)
                    # Mark context for following headings
                    pending_object_array = (obj_id, obj_id, array_kind, level)
                    i += 3
                    continue

                # Check if this is a YAML field [[field: yaml]]
                elif header.get("field_type") == "yaml" and parent_id:
                    # Mark for next fence to be parsed as YAML
                    pending_yaml_field = (parent_id, header["id"], header["label"])

                # Check if this is a JSON field [[field: json]]
                elif header.get("field_type") == "json" and parent_id:
                    # Mark for next fence to be parsed as JSON
                    pending_json_field = (parent_id, header["id"], header["label"])

                # Check if this is a text field [[field: text]]
                elif header.get("field_type") == "text" and parent_id:
                    # TWO-STAGE: Extract raw slice for text field content
                    # Find end: next heading of same/higher level
                    content_start_line = token.map[1] if token.map else 0  # Line after heading
                    content_end_line = block_tree.line_count
                    scan_idx = i + 3  # Skip heading tokens

                    while scan_idx < len(tokens):
                        scan_tok = tokens[scan_idx]
                        if scan_tok.type == "heading_open":
                            next_level = get_heading_level(scan_tok.tag)
                            if next_level <= level:
                                content_end_line = (
                                    scan_tok.map[0] if scan_tok.map else content_end_line
                                )
                                break
                        scan_idx += 1

                    # Extract raw slice
                    raw_content = block_tree.get_lines_raw(
                        content_start_line, content_end_line
                    ).strip()

                    # Store in parent object
                    objects[parent_id][header["id"]] = raw_content
                    if "__types" not in objects[parent_id]:
                        objects[parent_id]["__types"] = {}
                    objects[parent_id]["__types"][header["id"]] = "string"
                    if "__syntax" not in objects[parent_id]:
                        objects[parent_id]["__syntax"] = {}
                    objects[parent_id]["__syntax"][header["id"]] = "multiline_text"
                    if "__labels" not in objects[parent_id]:
                        objects[parent_id]["__labels"] = {}
                    objects[parent_id]["__labels"][header["id"]] = header["label"]

                    # Update comment anchor so subsequent comments are "after" this field
                    comment_anchor = header["id"]

                    # Skip to end of content
                    i = scan_idx
                    continue

                # Check if this is a map field [[field: map]]
                elif header.get("field_type") == "map" and parent_id:
                    # Collect - key: value list items into a str→str dict
                    # Reuse field parser, then coerce all values to strings
                    content_start_line = token.map[1] if token.map else 0
                    content_end_line = block_tree.line_count
                    scan_idx = i + 3

                    while scan_idx < len(tokens):
                        scan_tok = tokens[scan_idx]
                        if scan_tok.type == "heading_open":
                            next_level = get_heading_level(scan_tok.tag)
                            if next_level <= level:
                                content_end_line = (
                                    scan_tok.map[0] if scan_tok.map else content_end_line
                                )
                                break
                        scan_idx += 1

                    # Find bullet_list_open between heading and end
                    map_data: dict[str, str] = {}
                    list_scan = i + 3
                    found_list = False
                    while list_scan < scan_idx:
                        if tokens[list_scan].type == "bullet_list_open":
                            if found_list:
                                # Second bullet list — invalid
                                err_line = (
                                    (tokens[list_scan].map[0] + 1) if tokens[list_scan].map else 0
                                )
                                parsing_errors.append(
                                    {
                                        "__id": f"error_{len(parsing_errors)}",
                                        "__kind": "__ParsingError",
                                        "type": "invalid_map_content",
                                        "field": header["id"],
                                        "object": f"[[#{parent_id}]]",
                                        "line": err_line,
                                    }
                                )
                                list_scan += 1
                                continue
                            found_list = True
                            (
                                fields,
                                _field_types,
                                _field_syntax,
                                invalid_items,
                                next_i,
                                _nested_errors,
                            ) = parse_fields_from_list(
                                tokens, list_scan, block_tree, raw_strings=True
                            )
                            map_data.update(fields)
                            # Emit errors for invalid entries
                            for inv in invalid_items:
                                parsing_errors.append(
                                    {
                                        "__id": f"error_{len(parsing_errors)}",
                                        "__kind": "__ParsingError",
                                        "type": "invalid_map_entry",
                                        "field": header["id"],
                                        "object": f"[[#{parent_id}]]",
                                        "line": inv.get("line", 0),
                                    }
                                )
                            list_scan = next_i
                            continue
                        elif tokens[list_scan].type in (
                            "paragraph_open",
                            "fence",
                            "code_block",
                            "ordered_list_open",
                        ):
                            err_line = (
                                (tokens[list_scan].map[0] + 1) if tokens[list_scan].map else 0
                            )
                            parsing_errors.append(
                                {
                                    "__id": f"error_{len(parsing_errors)}",
                                    "__kind": "__ParsingError",
                                    "type": "invalid_map_content",
                                    "field": header["id"],
                                    "object": f"[[#{parent_id}]]",
                                    "line": err_line,
                                }
                            )
                            # Skip past ordered list contents
                            if tokens[list_scan].type == "ordered_list_open":
                                while (
                                    list_scan < scan_idx
                                    and tokens[list_scan].type != "ordered_list_close"
                                ):
                                    list_scan += 1
                        list_scan += 1

                    objects[parent_id][header["id"]] = map_data
                    if "__types" not in objects[parent_id]:
                        objects[parent_id]["__types"] = {}
                    objects[parent_id]["__types"][header["id"]] = "map"
                    if "__syntax" not in objects[parent_id]:
                        objects[parent_id]["__syntax"] = {}
                    objects[parent_id]["__syntax"][header["id"]] = "map"
                    if "__labels" not in objects[parent_id]:
                        objects[parent_id]["__labels"] = {}
                    objects[parent_id]["__labels"][header["id"]] = header["label"]

                    comment_anchor = header["id"]
                    i = scan_idx
                    continue

                # Check if we're inside an object array context
                elif pending_object_array and level > pending_object_array[3]:
                    arr_parent_id, arr_field, arr_kind, arr_level = pending_object_array
                    # This is an element of the object array
                    local_id = header["id"]
                    composed_id, local_id_out = resolve_child_id(arr_parent_id, local_id, arr_field)
                    obj: dict[str, Any] = {
                        "__id": composed_id,
                        "__kind": arr_kind,
                        "__parent": f"[[#{objects[arr_parent_id]['__id']}]]",
                        "__parent_field": arr_field,
                        "__line": line_num,
                    }
                    if local_id_out is not None:
                        obj["__local_id"] = local_id_out
                    if header["label"]:
                        obj["__label"] = header["label"]
                    objects[arr_parent_id][arr_field].append(f"[[#{composed_id}]]")
                    objects[composed_id] = obj
                    object_stack.append((composed_id, level))

                elif parent_id and "kind" not in header and header.get("has_explicit_id", False):
                    # Has [[id]] without kind - field or nested object

                    # BR-16: Dot in nested child's explicit ID is an error
                    if "." in header["id"]:
                        parsing_errors.append(
                            {
                                "__id": header["id"],
                                "__kind": "__ParsingError",
                                "type": "invalid_id_character",
                                "reference": f"[[{header['id']}]]",
                                "line": line_num,
                            }
                        )
                        # Skip heading tokens and content until next heading
                        skip_idx = i + 3
                        while skip_idx < len(tokens):
                            if tokens[skip_idx].type == "heading_open":
                                break
                            skip_idx += 1
                        i = skip_idx
                        continue

                    # Check next token to determine
                    next_idx = i + 3  # After heading_open, inline, heading_close
                    next_token = tokens[next_idx] if next_idx < len(tokens) else None

                    if next_token and next_token.type == "bullet_list_open":
                        # List follows - check if it has fields
                        has_fields = has_fields_after_heading(i, level)
                        if has_fields:
                            # Nested object with fields
                            local_id = header["id"]
                            composed_id, local_id_out = resolve_child_id(parent_id, local_id)
                            parent_full_id = objects[parent_id]["__id"]
                            obj = {
                                "__id": composed_id,
                                "__kind": "__Object",
                                "__level": level,
                                "__line": line_num,
                            }
                            if local_id_out is not None:
                                obj["__local_id"] = local_id_out
                            if header["label"]:
                                obj["__label"] = header["label"]
                            obj["__parent"] = f"[[#{parent_full_id}]]"
                            obj["__parent_field"] = local_id
                            objects[parent_id][local_id] = f"[[#{composed_id}]]"
                            objects[composed_id] = obj
                            object_stack.append((composed_id, level))
                        else:
                            # List without fields - text field: use raw slice
                            content_start_line = token.map[1] if token.map else 0
                            content_end_line = block_tree.line_count
                            scan_idx = i + 3
                            while scan_idx < len(tokens):
                                scan_tok = tokens[scan_idx]
                                if scan_tok.type == "heading_open":
                                    next_level = get_heading_level(scan_tok.tag)
                                    if next_level <= level:
                                        content_end_line = (
                                            scan_tok.map[0] if scan_tok.map else content_end_line
                                        )
                                        break
                                scan_idx += 1
                            raw_content = block_tree.get_lines_raw(
                                content_start_line, content_end_line
                            ).strip()
                            objects[parent_id][header["id"]] = raw_content
                            if "__types" not in objects[parent_id]:
                                objects[parent_id]["__types"] = {}
                            objects[parent_id]["__types"][header["id"]] = "string"
                            if "__syntax" not in objects[parent_id]:
                                objects[parent_id]["__syntax"] = {}
                            objects[parent_id]["__syntax"][header["id"]] = "multiline_text"
                            if "__labels" not in objects[parent_id]:
                                objects[parent_id]["__labels"] = {}
                            objects[parent_id]["__labels"][header["id"]] = header["label"]
                            comment_anchor = header["id"]
                            i = scan_idx
                            continue
                    elif next_token and next_token.type == "heading_open":
                        # Another heading follows - check if it's a child
                        next_level = get_heading_level(next_token.tag)
                        if next_level > level:
                            # Child heading - nested object
                            local_id = header["id"]
                            composed_id, local_id_out = resolve_child_id(parent_id, local_id)
                            parent_full_id = objects[parent_id]["__id"]
                            obj = {
                                "__id": composed_id,
                                "__kind": "__Object",
                                "__level": level,
                                "__line": line_num,
                            }
                            if local_id_out is not None:
                                obj["__local_id"] = local_id_out
                            if header["label"]:
                                obj["__label"] = header["label"]
                            obj["__parent"] = f"[[#{parent_full_id}]]"
                            obj["__parent_field"] = local_id
                            objects[parent_id][local_id] = f"[[#{composed_id}]]"
                            objects[composed_id] = obj
                            object_stack.append((composed_id, level))
                        else:
                            # Same or higher level - empty text field
                            objects[parent_id][header["id"]] = ""
                            if "__types" not in objects[parent_id]:
                                objects[parent_id]["__types"] = {}
                            objects[parent_id]["__types"][header["id"]] = "string"
                            if "__syntax" not in objects[parent_id]:
                                objects[parent_id]["__syntax"] = {}
                            objects[parent_id]["__syntax"][header["id"]] = "multiline_text"
                            if "__labels" not in objects[parent_id]:
                                objects[parent_id]["__labels"] = {}
                            objects[parent_id]["__labels"][header["id"]] = header["label"]
                            comment_anchor = header["id"]
                    else:
                        # Default: text field (paragraph, fence, table, etc.) - use raw slice
                        content_start_line = token.map[1] if token.map else 0
                        content_end_line = block_tree.line_count
                        scan_idx = i + 3
                        while scan_idx < len(tokens):
                            scan_tok = tokens[scan_idx]
                            if scan_tok.type == "heading_open":
                                next_level = get_heading_level(scan_tok.tag)
                                if next_level <= level:
                                    content_end_line = (
                                        scan_tok.map[0] if scan_tok.map else content_end_line
                                    )
                                    break
                            scan_idx += 1
                        raw_content = block_tree.get_lines_raw(
                            content_start_line, content_end_line
                        ).strip()
                        objects[parent_id][header["id"]] = raw_content
                        if "__types" not in objects[parent_id]:
                            objects[parent_id]["__types"] = {}
                        objects[parent_id]["__types"][header["id"]] = "string"
                        if "__syntax" not in objects[parent_id]:
                            objects[parent_id]["__syntax"] = {}
                        objects[parent_id]["__syntax"][header["id"]] = "multiline_text"
                        if "__labels" not in objects[parent_id]:
                            objects[parent_id]["__labels"] = {}
                        objects[parent_id]["__labels"][header["id"]] = header["label"]
                        comment_anchor = header["id"]
                        i = scan_idx
                        continue
                elif parent_id and "kind" not in header and level > object_stack[-1][1]:
                    # No [[id]] inside object AND deeper than parent - this is a COMMENT
                    # (If same or higher level, it's a sibling, not a child comment)
                    #
                    # TWO-STAGE ARCHITECTURE: Extract raw markdown slice instead of
                    # reconstructing from tokens. This preserves original formatting.
                    #
                    # Start line: where this heading starts (token.map[0])
                    # End line: next heading of same/higher level, or heading with Kind
                    start_line = token.map[0] if token.map else 0
                    end_line = block_tree.line_count  # Default: end of file

                    # Skip heading tokens and scan for end boundary
                    j = i + 3  # Skip heading_open, inline, heading_close
                    while j < len(tokens):
                        tok = tokens[j]
                        if tok.type == "heading_open":
                            next_level = get_heading_level(tok.tag)
                            if next_level <= level:
                                # Same or higher level heading - stop here
                                end_line = tok.map[0] if tok.map else end_line
                                break
                            # Deeper heading - check for [[id: Kind]] or [[id]]
                            next_header = parse_header(tokens, j)
                            if next_header:
                                next_has_explicit = next_header.get("has_explicit_id", False)
                                next_has_kind = "kind" in next_header
                                if next_has_explicit and next_has_kind:
                                    # Heading with Kind = nested object - stop before it
                                    end_line = tok.map[0] if tok.map else end_line
                                    break
                                elif next_has_explicit:
                                    # ERROR: structured element inside comment/textblock
                                    error_id = f"error_{len(parsing_errors)}"
                                    if next_header.get("field_type"):
                                        ref_pattern = (
                                            f"[[{next_header['id']}: {next_header['field_type']}]]"
                                        )
                                    else:
                                        ref_pattern = f"[[{next_header['id']}]]"
                                    next_line_num = (tok.map[0] + 1) if tok.map else None
                                    parsing_errors.append(
                                        {
                                            "__id": error_id,
                                            "__kind": "__ParsingError",
                                            "type": "structured_in_textblock",
                                            "reference": ref_pattern,
                                            "line": next_line_num,
                                        }
                                    )
                                    # Continue scanning - error heading is part of comment
                            j += 3  # Skip heading tokens
                        elif tok.type == "bullet_list_open":
                            # Field-like bullet lists inside comment headings are
                            # part of the comment content, not parent object fields.
                            # Don't stop here — include them in the raw slice.
                            j += 1
                        else:
                            j += 1

                    # Extract raw markdown slice and trim
                    raw_comment = block_tree.get_lines_raw(start_line, end_line).strip()

                    # Add to parent's __comments with merging
                    append_comment(parent_id, comment_anchor, raw_comment)

                    # Move index to end of scanned tokens
                    i = j
                    continue
                else:
                    # Check if this should be an object or a TextBlock
                    # Heading without [[id]] and without direct fields = TextBlock
                    # All other headings = objects
                    has_explicit = header.get("has_explicit_id", False)
                    has_kind = "kind" in header
                    has_fields = has_fields_after_heading(i, level)

                    # Also check for nested structured headings (children with [[...]])
                    # Only for H2+ headings — H1 headings are document titles
                    has_structured_children = level >= 2 and has_nested_structured_headings(
                        i, level
                    )

                    # Heading without explicit id and without fields and without kind
                    # AND without structured children = TextBlock
                    is_text_block = (
                        not has_explicit
                        and not has_kind
                        and not has_fields
                        and not has_structured_children
                    )

                    if not is_text_block:
                        # Check for explicit system type error
                        # [[id: __Document]], [[id: __TextBlock]],
                        # [[id: __Object]] are not allowed
                        # But __Workspace and __Namespace are valid kinds
                        _forbidden_explicit = ("__Document", "__TextBlock", "__Object")
                        if has_kind and header["kind"] in _forbidden_explicit:
                            error_id = header["id"]
                            kind_str = header["kind"]
                            ref_pattern = f"[[{header['id']}: {kind_str}]]"
                            parsing_errors.append(
                                {
                                    "__id": error_id,
                                    "__kind": "__ParsingError",
                                    "type": "explicit_system_type",
                                    "reference": ref_pattern,
                                    "line": line_num,
                                }
                            )
                            # Skip heading tokens and non-heading content
                            # (fields, paragraphs etc.) but stop at next heading
                            skip_idx = i + 3  # After heading tokens
                            while skip_idx < len(tokens):
                                if tokens[skip_idx].type == "heading_open":
                                    break
                                skip_idx += 1
                            i = skip_idx
                            continue

                        # Check for structured_in_textblock error
                        # Error occurs when:
                        # 1. TextBlock has content (not just the heading)
                        # 2. New heading is deeper than TextBlock level
                        # 3. TextBlock started at level >= 2 (nested inside document)
                        textblock_has_content = (
                            pending_text_block_started
                            and len(pending_text_block_content) > 1  # More than just the heading
                        )
                        if (
                            textblock_has_content
                            and has_explicit
                            and pending_text_block_level >= 2
                            and level > pending_text_block_level
                        ):
                            # ERROR: Cannot create structured element inside TextBlock
                            # Record error and include heading in text block content
                            error_id = f"error_{len(parsing_errors)}"
                            # Reconstruct the [[...]] pattern
                            if has_kind:
                                ref_pattern = f"[[{header['id']}:{header['kind']}]]"
                            elif header.get("field_type"):
                                ref_pattern = f"[[{header['id']}: {header['field_type']}]]"
                            else:
                                ref_pattern = f"[[{header['id']}]]"
                            parsing_errors.append(
                                {
                                    "__id": error_id,
                                    "__kind": "__ParsingError",
                                    "type": "structured_in_textblock",
                                    "reference": ref_pattern,
                                    "line": line_num,
                                }
                            )
                            # Add heading to text block content and continue
                            heading_text = "#" * level + " " + header["label"]
                            pending_text_block_content.append("")
                            pending_text_block_content.append(heading_text)
                            i += 3  # Skip heading tokens
                            continue

                        # Save any pending text block first
                        if pending_text_block_started and pending_text_block_content:
                            text_block_id = f"text_{text_block_counter}"
                            text_block_counter += 1
                            tb: dict[str, Any] = {
                                "__id": text_block_id,
                                "__kind": "__TextBlock",
                                "content": "\n\n".join(pending_text_block_content),
                                "__line": pending_text_block_line,
                            }
                            if pending_code_fences:
                                tb["__code_fences"] = list(pending_code_fences)
                            text_blocks.append(tb)
                            content_order.append(text_block_id)
                            pending_text_block_content = []
                            pending_text_block_started = False
                            pending_text_block_level = 0
                            pending_code_fences = []

                        # Create object
                        obj = {
                            "__id": header["id"],
                            "__level": level,  # For lossless rebuild
                            "__line": line_num,
                        }
                        if header["label"]:
                            obj["__label"] = header["label"]

                        # Store if [[id]] was explicit (for lossless rebuild)
                        if not has_explicit:
                            obj["__has_explicit_id"] = False

                        if has_kind:
                            obj["__kind"] = header["kind"]
                        else:
                            obj["__kind"] = "__Object"

                        if parent_id:
                            local_id = header["id"]
                            composed_id, local_id_out = resolve_child_id(parent_id, local_id)
                            parent_full_id = objects[parent_id]["__id"]
                            obj["__id"] = composed_id
                            if local_id_out is not None:
                                obj["__local_id"] = local_id_out
                            obj["__parent"] = f"[[#{parent_full_id}]]"
                            obj["__parent_field"] = local_id
                            objects[parent_id][local_id] = f"[[#{composed_id}]]"
                        else:
                            # Top-level object - add to content order
                            content_order.append(header["id"])
                            # Phase 3: Detect dot-ID parent declaration (BR-7)
                            if has_explicit and "." in header["id"]:
                                obj["__local_id"] = header["id"]

                        # Detect true duplicates: if ID already exists
                        # with __line, it's a real duplicate.
                        # During parsing, let the new object overwrite (so fields parse correctly).
                        # Track the true first line for error messages.
                        obj_id_to_store = obj["__id"]
                        if obj_id_to_store in objects and "__line" in objects[obj_id_to_store]:
                            # Get the true first line (from first_seen_lines tracker,
                            # not current occupant)
                            if obj_id_to_store not in first_seen_lines:
                                first_seen_lines[obj_id_to_store] = objects[obj_id_to_store].get(
                                    "__line", 0
                                )
                            first_line = first_seen_lines[obj_id_to_store]
                            # Save the OLD (current occupant) object — it will be output
                            duplicate_objects.append(objects[obj_id_to_store])
                            # Emit __ParsingError for the NEW (incoming) occurrence
                            parsing_errors.append(
                                {
                                    "__id": f"__error_dup_{obj_id_to_store}",
                                    "__kind": "__ParsingError",
                                    "type": "duplicate_id",
                                    "message": (
                                        f"Duplicate ID '{obj_id_to_store}'"
                                        f" (first defined on line"
                                        f" {first_line})"
                                    ),
                                    "object": f"[[#{obj_id_to_store}]]",
                                    "line": line_num,
                                }
                            )
                        objects[obj_id_to_store] = obj
                        object_stack.append((obj["__id"], level))

                        # If heading had field_type but no parent, add __syntax metadata
                        # and capture following content as __comments
                        field_type = header.get("field_type")
                        if field_type and not parent_id:
                            # Emit dangling_field error
                            parsing_errors.append(
                                {
                                    "__id": f"error_{len(parsing_errors)}",
                                    "__kind": "__ParsingError",
                                    "type": "dangling_field",
                                    "field": header["id"],
                                    "field_type": field_type,
                                    "object": f"[[#{header['id']}]]",
                                    "line": line_num,
                                }
                            )
                            if field_type == "text":
                                if "__syntax" not in obj:
                                    obj["__syntax"] = {}
                                obj["__syntax"][header["id"]] = "multiline_text"
                            elif field_type == "array":
                                if "__syntax" not in obj:
                                    obj["__syntax"] = {}
                                obj["__syntax"][header["id"]] = "markdown_list"
                            elif field_type == "map":
                                if "__syntax" not in obj:
                                    obj["__syntax"] = {}
                                obj["__syntax"][header["id"]] = "map"

                            # Capture content after heading as __comments (raw slice)
                            if field_type in ("text", "array", "map"):
                                content_start_line = token.map[1] if token.map else 0
                                content_end_line = block_tree.line_count
                                scan_idx = i + 3
                                while scan_idx < len(tokens):
                                    scan_tok = tokens[scan_idx]
                                    if scan_tok.type == "heading_open":
                                        next_level = get_heading_level(scan_tok.tag)
                                        if next_level <= level:
                                            content_end_line = (
                                                scan_tok.map[0]
                                                if scan_tok.map
                                                else content_end_line
                                            )
                                            break
                                        # Also stop at deeper headings with explicit [[id]]
                                        # (structured objects that should be parsed separately)
                                        # Headings without [[id]] are comment sub-headings
                                        if scan_idx + 1 < len(tokens):
                                            deeper_header = parse_header(tokens, scan_idx)
                                            if deeper_header and deeper_header.get(
                                                "has_explicit_id", False
                                            ):
                                                dh_ft = deeper_header.get("field_type")
                                                # Stop if it has an explicit id and is not
                                                # a text/array field type (those are sub-fields)
                                                if dh_ft not in ("text", "array", "map"):
                                                    content_end_line = (
                                                        scan_tok.map[0]
                                                        if scan_tok.map
                                                        else content_end_line
                                                    )
                                                    break
                                    scan_idx += 1
                                raw_content = block_tree.get_lines_raw(
                                    content_start_line, content_end_line
                                ).strip()
                                if raw_content:
                                    append_comment(header["id"], "__self", raw_content)
                                i = scan_idx
                                continue

                        # If heading had field_type (yaml/json/object_array) but no parent,
                        # capture the following content as __comments
                        field_type = header.get("field_type")
                        if field_type in ("yaml", "json", "object_array") and not parent_id:
                            if "__syntax" not in obj:
                                obj["__syntax"] = {}
                            if field_type in ("yaml", "json"):
                                syntax_name = (
                                    "yaml_object" if field_type == "yaml" else "json_object"
                                )
                                obj["__syntax"][header["id"]] = syntax_name
                            elif field_type == "object_array":
                                obj["__syntax"][header["id"]] = "headers"
                                # Store array kind for heading rebuild
                                array_kind = header.get("array_kind", "")
                                if array_kind:
                                    obj["__syntax"]["__array_kind"] = array_kind

                            # Capture content after heading as __comments (raw slice)
                            content_start_line = token.map[1] if token.map else 0
                            content_end_line = block_tree.line_count
                            scan_idx = i + 3
                            while scan_idx < len(tokens):
                                scan_tok = tokens[scan_idx]
                                if scan_tok.type == "heading_open":
                                    next_level = get_heading_level(scan_tok.tag)
                                    if next_level <= level:
                                        content_end_line = (
                                            scan_tok.map[0] if scan_tok.map else content_end_line
                                        )
                                        break
                                scan_idx += 1
                            raw_content = block_tree.get_lines_raw(
                                content_start_line, content_end_line
                            ).strip()
                            if raw_content:
                                append_comment(header["id"], "__self", raw_content)
                            i = scan_idx
                            continue
                    else:
                        # TextBlock - each heading without [[id]] starts a NEW text block
                        # First, save any pending text block
                        if pending_text_block_started and pending_text_block_content:
                            text_block_id = f"text_{text_block_counter}"
                            text_block_counter += 1
                            tb: dict[str, Any] = {
                                "__id": text_block_id,
                                "__kind": "__TextBlock",
                                "content": "\n\n".join(pending_text_block_content),
                                "__line": pending_text_block_line,
                            }
                            if pending_code_fences:
                                tb["__code_fences"] = list(pending_code_fences)
                            text_blocks.append(tb)
                            content_order.append(text_block_id)
                            pending_text_block_content = []
                            pending_code_fences = []

                        # Start new text block with the heading
                        heading_text = "#" * level + " " + header["label"]
                        pending_text_block_content = [heading_text]
                        pending_text_block_started = True
                        pending_text_block_line = line_num
                        pending_text_block_level = level

                # Reset comment anchor for new object (but not for field-type headings
                # like [[field: array]], [[field: text]], etc. which are fields on parent)
                field_type = header.get("field_type") if header else None
                if not field_type or not parent_id:
                    comment_anchor = "__self"

            i += 3  # Skip heading_open, inline, heading_close
        elif token.type == "bullet_list_open":
            if pending_array_field:
                # This list is for a [[field: array]] section
                parent_id, field_name = pending_array_field
                items, next_i = parse_array_items_from_list(tokens, i)
                objects[parent_id][field_name] = items

                # Add __syntax for markdown_list
                if "__syntax" not in objects[parent_id]:
                    objects[parent_id]["__syntax"] = {}
                objects[parent_id]["__syntax"][field_name] = "markdown_list"

                pending_array_field = None
                comment_anchor = field_name
                i = next_i
            else:
                current_id = get_current_object_id()
                if current_id:
                    # Parse fields from list
                    (
                        fields,
                        field_types,
                        field_syntax,
                        invalid_items,
                        next_i,
                        nested_subitems_errors,
                    ) = parse_fields_from_list(tokens, i, block_tree)

                    # Check if any field keys already exist in the object.
                    # If so, this bullet list is a DUPLICATE — treat it as
                    # comment content instead of overwriting existing fields.
                    has_duplicate_keys = fields and any(
                        k in objects[current_id] for k in fields if not k.startswith("__")
                    )
                    if has_duplicate_keys:
                        # Treat entire bullet list as comment content
                        if token.map:
                            # Find bullet_list_close to get end line
                            scan_j = i + 1
                            while (
                                scan_j < len(tokens) and tokens[scan_j].type != "bullet_list_close"
                            ):
                                scan_j += 1
                            end_line = (
                                tokens[scan_j].map[1]
                                if scan_j < len(tokens) and tokens[scan_j].map
                                else token.map[1]
                            )
                            raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                            if raw_list:
                                append_comment(current_id, comment_anchor, raw_list)
                            i = scan_j + 1
                        else:
                            i = next_i
                        continue

                    # No valid fields but has invalid items — treat entire list
                    # as comment content (e.g. bullet list with colons in prose)
                    if not fields and invalid_items:
                        has_invalid_keys = any(inv.get("key") for inv in invalid_items)
                        if token.map:
                            scan_j = i + 1
                            while (
                                scan_j < len(tokens) and tokens[scan_j].type != "bullet_list_close"
                            ):
                                scan_j += 1
                            end_line = (
                                tokens[scan_j].map[1]
                                if scan_j < len(tokens) and tokens[scan_j].map
                                else token.map[1]
                            )
                            raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                            if raw_list:
                                append_comment(current_id, comment_anchor, raw_list, merge=True)
                            # Emit mixed_field_keys error if items had invalid keys
                            if has_invalid_keys:
                                error_line = next(
                                    (inv.get("line", 0) for inv in invalid_items if inv.get("key")),
                                    invalid_items[0].get("line", 0),
                                )
                                parsing_errors.append(
                                    {
                                        "__id": f"error_{len(parsing_errors)}",
                                        "__kind": "__ParsingError",
                                        "type": "mixed_field_keys",
                                        "object": f"[[#{current_id}]]",
                                        "line": error_line,
                                    }
                                )
                            i = scan_j + 1
                        else:
                            i = next_i
                        continue

                    # No valid fields AND no invalid items — bullets without colons
                    # (e.g. "- **bold** — text"). Treat entire list as comment content.
                    if not fields and not invalid_items:
                        if token.map:
                            scan_j = i + 1
                            while (
                                scan_j < len(tokens) and tokens[scan_j].type != "bullet_list_close"
                            ):
                                scan_j += 1
                            end_line = (
                                tokens[scan_j].map[1]
                                if scan_j < len(tokens) and tokens[scan_j].map
                                else token.map[1]
                            )
                            raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                            if raw_list:
                                append_comment(current_id, comment_anchor, raw_list)
                            i = scan_j + 1
                        else:
                            i = next_i
                        continue

                    objects[current_id].update(fields)

                    # Merge __types (don't overwrite)
                    if field_types:
                        if "__types" not in objects[current_id]:
                            objects[current_id]["__types"] = {}
                        objects[current_id]["__types"].update(field_types)

                    # Merge __syntax (don't overwrite)
                    if field_syntax:
                        if "__syntax" not in objects[current_id]:
                            objects[current_id]["__syntax"] = {}
                        objects[current_id]["__syntax"].update(field_syntax)

                    # Handle invalid field items (e.g. Cyrillic keys) — treat as comment text
                    # Only add if not already captured by the raw comment scanner
                    if invalid_items:
                        if "__comments" not in objects[current_id]:
                            objects[current_id]["__comments"] = []
                        has_new_invalid = False
                        has_invalid_keys = False  # items with actual invalid keys (colon present)
                        grouped: dict[str, list[str]] = {}
                        grouped_has_keys: dict[str, bool] = {}
                        for inv_item in invalid_items:
                            inv_content = inv_item["content"]
                            inv_after = inv_item.get("after", "__self")
                            if inv_item.get("key"):
                                has_invalid_keys = True
                                grouped_has_keys[inv_after] = True
                            # Check if content is already in an existing comment
                            already_captured = any(
                                c.get("content") == inv_content
                                or inv_content in c.get("content", "")
                                for c in objects[current_id].get("__comments", [])
                            )
                            if not already_captured:
                                if inv_after not in grouped:
                                    grouped[inv_after] = []
                                grouped[inv_after].append(inv_content)
                                has_new_invalid = True

                        for anchor, contents in grouped.items():
                            # Invalid-key items (with colon) join with \n\n
                            # Plain non-field items join with \n
                            sep = "\n\n" if grouped_has_keys.get(anchor) else "\n"
                            merged_content = sep.join(contents)
                            append_comment(current_id, anchor, merged_content)

                        # mixed_field_keys error: only when items have actual invalid keys
                        # (colon present but key is invalid, e.g. Cyrillic)
                        # Plain non-field items (no colon) don't trigger this error
                        if fields and has_new_invalid and has_invalid_keys:
                            # Use line of first non-captured invalid item
                            error_line = next(
                                (
                                    inv.get("line", 0)
                                    for inv in invalid_items
                                    if not any(
                                        c.get("content") == inv["content"]
                                        or inv["content"] in c.get("content", "")
                                        for c in objects[current_id].get("__comments", [])
                                        if c.get("after") == "__self"
                                    )
                                ),
                                invalid_items[0].get("line", 0),
                            )
                            parsing_errors.append(
                                {
                                    "__id": f"error_{len(parsing_errors)}",
                                    "__kind": "__ParsingError",
                                    "type": "mixed_field_keys",
                                    "object": f"[[#{current_id}]]",
                                    "line": error_line,
                                }
                            )

                    # nested_subitems errors
                    for ns_err in nested_subitems_errors:
                        parsing_errors.append(
                            {
                                "__id": f"error_{len(parsing_errors)}",
                                "__kind": "__ParsingError",
                                "type": "nested_subitems",
                                "field": ns_err["key"],
                                "object": f"[[#{current_id}]]",
                                "line": ns_err["line"],
                            }
                        )

                    # Update comment anchor to last field
                    if fields:
                        comment_anchor = list(fields.keys())[-1]

                    i = next_i

                    # Content AFTER fields: process each block as separate comment
                    # (similar to content BEFORE fields, but with different anchor)
                    # Note: don't skip i here - let the main loop process blocks
                elif pending_text_block_started:
                    # Collect bullet list as raw markdown text for TextBlock
                    if token.map:
                        # Find bullet_list_close to get end line
                        scan_j = i + 1
                        while scan_j < len(tokens) and tokens[scan_j].type != "bullet_list_close":
                            scan_j += 1
                        end_line = (
                            tokens[scan_j].map[1]
                            if scan_j < len(tokens) and tokens[scan_j].map
                            else token.map[1]
                        )
                        raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                        if raw_list:
                            pending_text_block_content.append(raw_list)
                        i = scan_j + 1
                    else:
                        list_items: list[str] = []
                        i += 1  # Skip bullet_list_open
                        while i < len(tokens) and tokens[i].type != "bullet_list_close":
                            if tokens[i].type == "inline":
                                list_items.append("- " + (tokens[i].content or ""))
                            i += 1
                        if i < len(tokens) and tokens[i].type == "bullet_list_close":
                            i += 1
                        if list_items:
                            pending_text_block_content.append("\n".join(list_items))
                else:
                    i += 1
        elif token.type == "table_open" and pending_text_block_started:
            # Collect table as raw markdown text for TextBlock
            if token.map:
                scan_j = i + 1
                while scan_j < len(tokens) and tokens[scan_j].type != "table_close":
                    scan_j += 1
                end_line = (
                    tokens[scan_j].map[1]
                    if scan_j < len(tokens) and tokens[scan_j].map
                    else token.map[1]
                )
                raw_table = block_tree.get_lines_raw(token.map[0], end_line).strip()
                if raw_table:
                    pending_text_block_content.append(raw_table)
                i = scan_j + 1
            else:
                i += 1
        elif token.type == "table_open" and pending_object_array:
            # Table after [[field: [Kind]]] heading
            arr_parent_id, arr_field, arr_kind, arr_level = pending_object_array

            # Change syntax from "headers" to "table"
            if "__syntax" in objects[arr_parent_id]:
                objects[arr_parent_id]["__syntax"][arr_field] = "table"

            # Add __types for array field
            if "__types" not in objects[arr_parent_id]:
                objects[arr_parent_id]["__types"] = {}
            objects[arr_parent_id]["__types"][arr_field] = "array"

            # Parse table
            column_names: list[str] = []
            rows: list[list[str]] = []
            i += 1  # skip table_open

            # Parse header row (thead)
            while i < len(tokens) and tokens[i].type != "thead_close":
                if tokens[i].type == "inline":
                    column_names.append(tokens[i].content)
                i += 1
            i += 1  # skip thead_close

            # Parse body rows (tbody)
            current_row: list[str] = []
            while i < len(tokens) and tokens[i].type != "table_close":
                if tokens[i].type == "tr_open":
                    current_row = []
                elif tokens[i].type == "inline":
                    current_row.append(tokens[i].content)
                elif tokens[i].type == "tr_close":
                    rows.append(current_row)
                i += 1
            i += 1  # skip table_close

            # Create objects from rows
            for row_idx, row in enumerate(rows):
                local_id = f"{arr_field}_{row_idx}"
                parent_full_id = objects[arr_parent_id]["__id"]
                obj_id, local_id_out = resolve_child_id(arr_parent_id, local_id, arr_field)
                obj: dict[str, Any] = {
                    "__id": obj_id,
                    "__label": "",
                    "__kind": arr_kind,
                    "__parent": f"[[#{parent_full_id}]]",
                    "__parent_field": arr_field,
                }
                if local_id_out is not None:
                    obj["__local_id"] = local_id_out
                field_types: dict[str, str] = {}

                for col_idx, col_name in enumerate(column_names):
                    if col_idx < len(row):
                        value_str = row[col_idx]
                        # Auto-detect type
                        from .parsers.field import parse_field_value

                        value, type_name = parse_field_value(value_str)
                        obj[col_name] = value
                        field_types[col_name] = type_name

                        # Set label from first column
                        if col_idx == 0 and isinstance(value, str):
                            obj["__label"] = value

                if field_types:
                    obj["__types"] = field_types

                objects[obj_id] = obj
                objects[arr_parent_id][arr_field].append(f"[[#{obj_id}]]")

            pending_object_array = None
        elif token.type == "fence" and pending_yaml_field:
            # YAML fence after [[field: yaml]] heading
            yaml_parent_id, yaml_field_name, yaml_label = pending_yaml_field
            yaml_content = token.content

            # Parse YAML
            import yaml

            try:
                yaml_data = yaml.safe_load(yaml_content)
            except yaml.YAMLError:
                yaml_data = yaml_content  # Fallback to raw string

            objects[yaml_parent_id][yaml_field_name] = yaml_data

            # Add __syntax
            if "__syntax" not in objects[yaml_parent_id]:
                objects[yaml_parent_id]["__syntax"] = {}
            objects[yaml_parent_id]["__syntax"][yaml_field_name] = "yaml_object"

            # Add __labels
            if "__labels" not in objects[yaml_parent_id]:
                objects[yaml_parent_id]["__labels"] = {}
            objects[yaml_parent_id]["__labels"][yaml_field_name] = yaml_label

            pending_yaml_field = None
            i += 1
        elif token.type == "fence" and pending_json_field:
            # JSON fence after [[field: json]] heading
            json_parent_id, json_field_name, json_label = pending_json_field
            json_content = token.content

            # Parse JSON
            try:
                json_data = json.loads(json_content)
            except json.JSONDecodeError:
                json_data = json_content  # Fallback to raw string

            objects[json_parent_id][json_field_name] = json_data

            # Add __syntax
            if "__syntax" not in objects[json_parent_id]:
                objects[json_parent_id]["__syntax"] = {}
            objects[json_parent_id]["__syntax"][json_field_name] = "json_object"

            # Add __labels
            if "__labels" not in objects[json_parent_id]:
                objects[json_parent_id]["__labels"] = {}
            objects[json_parent_id]["__labels"][json_field_name] = json_label

            pending_json_field = None
            i += 1
        elif (
            token.type in ("paragraph_open", "blockquote_open", "hr")
            or (
                token.type == "table_open"
                and (
                    comment_anchor != "__self"
                    or (
                        get_current_object_id()
                        and objects.get(get_current_object_id(), {}).get("__kind", "")
                        not in ("__Object", "")
                    )
                )
            )
            or (token.type == "fence" and get_current_object_id())
        ):
            # Block-level content as comment - use raw slice
            # This catches paragraphs, blockquotes, fences, HR, and tables (after fields)
            # Note: table before fields is handled as field, not comment
            current_id = get_current_object_id()
            if current_id:
                # Block-level content as comment - use raw slice
                # If starting with paragraph: stop at SECOND top-level para
                # If starting with non-paragraph (hr, blockquote, fence,
                # table): include all until heading/fields
                para_start_line = token.map[0] if token.map else 0
                current_level = object_stack[-1][1] if object_stack else 0
                started_with_paragraph = token.type == "paragraph_open"

                # Scan forward to find end of this "comment block"
                scan_idx = i
                content_end_line = block_tree.line_count
                para_count = (
                    0  # Count TOP-LEVEL paragraphs seen (only used if started_with_paragraph)
                )
                list_nesting = 0  # Track nesting depth

                while scan_idx < len(tokens):
                    scan_tok = tokens[scan_idx]

                    # Track nesting (lists and blockquotes)
                    if scan_tok.type in (
                        "bullet_list_open",
                        "ordered_list_open",
                        "blockquote_open",
                    ):
                        list_nesting += 1
                    elif scan_tok.type in (
                        "bullet_list_close",
                        "ordered_list_close",
                        "blockquote_close",
                    ):
                        list_nesting -= 1

                    # Stop at heading
                    if scan_tok.type == "heading_open":
                        next_level = get_heading_level(scan_tok.tag)
                        if next_level <= current_level:
                            content_end_line = scan_tok.map[0] if scan_tok.map else content_end_line
                            break
                        # Check if nested heading creates object or field
                        next_header = parse_header(tokens, scan_idx)
                        if next_header:
                            if "kind" in next_header or next_header.get("field_type"):
                                content_end_line = (
                                    scan_tok.map[0] if scan_tok.map else content_end_line
                                )
                                break
                            if next_header.get("has_explicit_id"):
                                content_end_line = (
                                    scan_tok.map[0] if scan_tok.map else content_end_line
                                )
                                break
                        # Nested heading without [[id]] - include in comment

                    # Stop at field list (only at top level)
                    # Check FIRST item - if field, whole list is field list
                    # If first item is NOT field, find where field items start
                    if scan_tok.type == "bullet_list_open" and list_nesting == 1:
                        check_idx = scan_idx + 1
                        first_item_is_field = False
                        first_field_line = None

                        # Scan all items to find first field item
                        # Use proper field_pattern to avoid false positives
                        # from colons inside backtick code spans or prose
                        while (
                            check_idx < len(tokens)
                            and tokens[check_idx].type != "bullet_list_close"
                        ):
                            if tokens[check_idx].type == "list_item_open":
                                item_start_line = (
                                    tokens[check_idx].map[0] if tokens[check_idx].map else None
                                )
                            if tokens[check_idx].type == "inline":
                                list_content = tokens[check_idx].content or ""
                                first_line = list_content.split("\n")[0]
                                if _FIELD_RE.match(first_line):
                                    if first_field_line is None:
                                        first_field_line = item_start_line
                                    if first_item_is_field is False and first_field_line == (
                                        scan_tok.map[0] if scan_tok.map else None
                                    ):
                                        first_item_is_field = True
                            check_idx += 1

                        if first_item_is_field:
                            # Whole list is field list - stop before it
                            content_end_line = scan_tok.map[0] if scan_tok.map else content_end_line
                            list_nesting -= 1
                            break
                        elif first_field_line is not None:
                            # Mixed list - stop at first field item line
                            content_end_line = first_field_line
                            list_nesting -= 1
                            break
                        # else: no field items, continue including this list

                    # Count TOP-LEVEL paragraphs only - stop at SECOND one
                    # Only applies if we started with a paragraph
                    if (
                        started_with_paragraph
                        and scan_tok.type == "paragraph_open"
                        and list_nesting == 0
                    ):
                        para_count += 1
                        if para_count >= 2:
                            # Second top-level paragraph - stop here, it starts a new comment
                            content_end_line = scan_tok.map[0] if scan_tok.map else content_end_line
                            break

                    scan_idx += 1

                # Extract raw slice
                raw_content = block_tree.get_lines_raw(para_start_line, content_end_line).strip()
                if raw_content:
                    # Check if we need to normalize: paragraph directly
                    # followed by list without blank line
                    # Find the first token after paragraph_close
                    needs_normalize = False
                    norm_idx = i + 1  # After paragraph_open
                    while norm_idx < len(tokens) and tokens[norm_idx].type not in (
                        "paragraph_close",
                    ):
                        norm_idx += 1
                    if norm_idx < len(tokens):
                        norm_idx += 1  # Skip paragraph_close
                        if norm_idx < len(tokens) and tokens[norm_idx].type in (
                            "bullet_list_open",
                            "ordered_list_open",
                        ):
                            # Check if there's a blank line between paragraph and list
                            para_end = token.map[1] if token.map else 0
                            list_start = tokens[norm_idx].map[0] if tokens[norm_idx].map else 0
                            if list_start == para_end and list_start < content_end_line:
                                needs_normalize = True

                    if needs_normalize:
                        # Add blank line after first line (paragraph) before list
                        raw_content = _FIRST_LINE_NORMALIZE_RE.sub(r"\1\n\n", raw_content, count=1)

                    append_comment(current_id, comment_anchor, raw_content)

                # Check for mixed_field_keys: bullet list items with invalid field-like keys
                # (e.g. "Custom Views: description" where "Custom Views" is not a valid key)
                # Strip backtick spans before checking — colons inside backticks are clearly
                # escaped content, not ambiguous field syntax.
                if current_id and objects.get(current_id, {}).get("__types"):
                    mixed_scan = i
                    while mixed_scan < scan_idx:
                        mt = tokens[mixed_scan]
                        if (
                            mt.type == "bullet_list_open"
                            and mt.map
                            and mt.map[0] >= para_start_line
                        ):
                            ms = mixed_scan + 1
                            while ms < len(tokens) and tokens[ms].type != "bullet_list_close":
                                if tokens[ms].type == "inline":
                                    mc = (tokens[ms].content or "").strip()
                                    mfl = mc.split("\n")[0]
                                    # Strip inline formatting — colons inside code/bold/italic
                                    # are clearly formatted text, not field syntax
                                    sanitized = _BACKTICK_STRIP_RE.sub("", mfl)
                                    sanitized = _BOLD_STRIP_RE.sub(r"\1", sanitized)
                                    sanitized = _ITALIC_STRIP_RE.sub(r"\1", sanitized)
                                    sanitized = _STRIKETHROUGH_STRIP_RE.sub(r"\1", sanitized)
                                    inv_m = _INVALID_FL_RE.match(sanitized)
                                    if inv_m and not _FIELD_RE.match(sanitized):
                                        pk = inv_m.group(1).strip()
                                        if pk and not _VALID_KEY_RE.match(pk):
                                            err_line = (
                                                (tokens[ms].map[0] + 1) if tokens[ms].map else 0
                                            )
                                            parsing_errors.append(
                                                {
                                                    "__id": f"error_{len(parsing_errors)}",
                                                    "__kind": "__ParsingError",
                                                    "type": "mixed_field_keys",
                                                    "object": f"[[#{current_id}]]",
                                                    "line": err_line,
                                                }
                                            )
                                            break
                                ms += 1
                        mixed_scan += 1

                i = scan_idx
            elif pending_text_block_started:
                # Collect text for pending TextBlock
                i += 1  # skip paragraph_open
                if i < len(tokens) and tokens[i].type == "inline":
                    content = tokens[i].content
                    i += 1  # skip inline
                    if i < len(tokens) and tokens[i].type == "paragraph_close":
                        i += 1  # skip paragraph_close
                    pending_text_block_content.append(content)
            else:
                # Text before any object - start a text block
                para_line = (token.map[0] + 1) if token.map else 1
                i += 1  # skip paragraph_open
                if i < len(tokens) and tokens[i].type == "inline":
                    content = tokens[i].content
                    i += 1  # skip inline
                    if i < len(tokens) and tokens[i].type == "paragraph_close":
                        i += 1  # skip paragraph_close
                    pending_text_block_content.append(content)
                    pending_text_block_started = True
                    pending_text_block_line = para_line
                    pending_text_block_level = 0  # No heading, level 0
        elif token.type == "ordered_list_open":
            # Ordered list for pending array field or as comment/TextBlock
            if pending_array_field:
                # Ordered lists are forbidden in array fields (rule_no_ordered_list_array).
                # Emit error, keep array empty, preserve content in __comments.
                parent_id, field_name = pending_array_field

                # Initialize empty array and syntax (normally done by parse_array_items_from_list)
                objects[parent_id][field_name] = []
                if "__syntax" not in objects[parent_id]:
                    objects[parent_id]["__syntax"] = {}
                objects[parent_id]["__syntax"][field_name] = "markdown_list"

                # Capture raw content as __comments for lossless round-trip
                scan_j = i + 1
                while scan_j < len(tokens) and tokens[scan_j].type != "ordered_list_close":
                    scan_j += 1
                if token.map:
                    raw_end = (
                        tokens[scan_j].map[1]
                        if scan_j < len(tokens) and tokens[scan_j].map
                        else token.map[1]
                    )
                    raw_list = block_tree.get_lines_raw(token.map[0], raw_end).strip()
                    if raw_list:
                        append_comment(parent_id, field_name, raw_list, merge=True)

                # Emit ordered_list_in_array error
                error_line = token.map[0] + 1 if token.map else None
                parsing_errors.append(
                    {
                        "__id": f"error_{len(parsing_errors)}",
                        "__kind": "__ParsingError",
                        "type": "ordered_list_in_array",
                        "field": field_name,
                        "object": f"[[#{parent_id}]]",
                        "line": error_line,
                    }
                )

                pending_array_field = None
                comment_anchor = field_name
                i = scan_j + 1  # skip past ordered_list_close
            else:
                current_id = get_current_object_id()
                if current_id and comment_anchor == "__self":
                    # Ordered list before fields - add as comment
                    list_items: list[str] = []
                    item_num = 1
                    i += 1  # skip ordered_list_open
                    while i < len(tokens) and tokens[i].type != "ordered_list_close":
                        if tokens[i].type == "inline":
                            list_items.append(f"{item_num}. {tokens[i].content}")
                            item_num += 1
                        i += 1
                    i += 1  # skip ordered_list_close
                    if list_items:
                        append_comment(current_id, comment_anchor, "\n".join(list_items))
                elif current_id and comment_anchor and comment_anchor != "__self":
                    # Ordered list after fields (e.g. trailing ordered list
                    # after array) — single merged comment
                    if token.map:
                        scan_j = i + 1
                        while scan_j < len(tokens) and tokens[scan_j].type != "ordered_list_close":
                            scan_j += 1
                        end_line = (
                            tokens[scan_j].map[1]
                            if scan_j < len(tokens) and tokens[scan_j].map
                            else token.map[1]
                        )
                        raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                        if raw_list:
                            append_comment(current_id, comment_anchor, raw_list)
                        i = scan_j + 1
                    else:
                        list_items = []
                        item_num = 1
                        i += 1  # skip ordered_list_open
                        while i < len(tokens) and tokens[i].type != "ordered_list_close":
                            if tokens[i].type == "inline":
                                list_items.append(f"{item_num}. {tokens[i].content}")
                                item_num += 1
                            i += 1
                        i += 1  # skip ordered_list_close
                        if list_items:
                            append_comment(current_id, comment_anchor, "\n".join(list_items))
                elif pending_text_block_started:
                    # Collect ordered list for TextBlock using raw slice
                    if token.map:
                        scan_j = i + 1
                        while scan_j < len(tokens) and tokens[scan_j].type != "ordered_list_close":
                            scan_j += 1
                        end_line = (
                            tokens[scan_j].map[1]
                            if scan_j < len(tokens) and tokens[scan_j].map
                            else token.map[1]
                        )
                        raw_list = block_tree.get_lines_raw(token.map[0], end_line).strip()
                        if raw_list:
                            pending_text_block_content.append(raw_list)
                        i = scan_j + 1
                    else:
                        list_items = []
                        item_num = 1
                        i += 1
                        while i < len(tokens) and tokens[i].type != "ordered_list_close":
                            if tokens[i].type == "inline":
                                list_items.append(f"{item_num}. {tokens[i].content}")
                                item_num += 1
                            i += 1
                        i += 1
                        if list_items:
                            pending_text_block_content.append("\n".join(list_items))
                else:
                    i += 1
        elif token.type == "fence" and not pending_yaml_field and not pending_json_field:
            # Code fence as comment (before fields) or TextBlock
            # Note: fences AFTER fields are collected via raw slice
            current_id = get_current_object_id()
            if current_id and comment_anchor == "__self":
                # Code fence before fields - add as comment
                lang = token.info or ""
                fence_content = (token.content or "").rstrip("\n")
                fence_text = (
                    f"```{lang}\n{fence_content}\n```" if lang else f"```\n{fence_content}\n```"
                )
                append_comment(current_id, comment_anchor, fence_text)
                i += 1
            elif not current_id:
                # Code fence outside of QMD.md object - add to text block
                lang = token.info or ""
                fence_content = (token.content or "").rstrip("\n")
                fence_text = (
                    f"```{lang}\n{fence_content}\n```" if lang else f"```\n{fence_content}\n```"
                )
                fence_lines = fence_text.count("\n") + 1

                # Calculate offset within content (0-based line number)
                existing_content = "\n\n".join(pending_text_block_content)
                offset_line = (
                    0 if not existing_content else existing_content.count("\n") + 2
                )  # +2 for blank line separator

                # Initialize text block if needed
                if not pending_text_block_started:
                    pending_text_block_started = True
                    pending_text_block_line = (token.map[0] + 1) if token.map else 1
                    pending_text_block_level = 0  # No heading, level 0

                # Add code fence metadata
                pending_code_fences.append(
                    {"lang": lang, "offset_line": offset_line, "length_lines": fence_lines}
                )

                # Add code fence text to pending text block
                pending_text_block_content.append(fence_text)
                i += 1
            else:
                i += 1
        else:
            i += 1

    # Handle any remaining pending text block at end of file
    if pending_text_block_started and pending_text_block_content:
        text_block_id = f"text_{text_block_counter}"
        tb: dict[str, Any] = {
            "__id": text_block_id,
            "__kind": "__TextBlock",
            "content": "\n\n".join(pending_text_block_content),
            "__line": pending_text_block_line,
        }
        if pending_code_fences:
            tb["__code_fences"] = list(pending_code_fences)
        text_blocks.append(tb)
        content_order.append(text_block_id)

    # Extract references from all objects (for full mode)
    if FEATURE_REFERENCES in active_features:
        # Need markdown content for line number tracking
        lines = markdown.split("\n")
        _extract_references_for_objects(objects, lines)
        # Also extract references from text blocks
        for tb in text_blocks:
            _extract_references_for_textblock(tb, lines)

    # Extract field positions (for full mode)
    if FEATURE_POSITIONS in active_features:
        lines = markdown.split("\n")
        _extract_field_positions(objects, lines)

    # Build result list
    result: list[dict[str, Any]] = []

    # Check if we need a __Document (if there are text blocks)
    if text_blocks:
        # Generate document ID using stable random suffix (same algorithm as generate_fallback_id)
        fallback = generate_fallback_id()  # returns object_xyz123
        doc_id = f"doc_{fallback[7:]}"  # strip "object_" prefix -> doc_xyz123

        # Build content array with references
        doc_content = [f"[[#{item_id}]]" for item_id in content_order]

        # Create __Document object
        doc_obj: dict[str, Any] = {
            "__id": doc_id,
            "__kind": "__Document",
            "content": doc_content,
        }
        result.append(doc_obj)

        # Add text blocks with __container
        for tb in text_blocks:
            tb["__container"] = f"[[#{doc_id}]]"
            result.append(tb)

        # Add regular objects with __container, then duplicates
        for obj in objects.values():
            obj["__container"] = f"[[#{doc_id}]]"
            result.append(_normalize_field_order(obj))
        for obj in duplicate_objects:
            obj["__container"] = f"[[#{doc_id}]]"
            result.append(_normalize_field_order(obj))
    else:
        # No text blocks - regular objects then duplicates
        result = []
        for obj in objects.values():
            result.append(_normalize_field_order(obj))
        for obj in duplicate_objects:
            result.append(_normalize_field_order(obj))

    # Add parsing errors to result (as __ParsingError objects)
    # Errors appear after all objects, sorted by line number among themselves
    if parsing_errors:
        result.extend(parsing_errors)

        # Sort: objects by __line (stable), errors always after objects (by line among themselves)
        def _sort_key(obj: dict[str, Any]) -> tuple[int, int]:
            if obj.get("__kind") == "__ParsingError":
                # Errors go after all objects (group=1), sorted by line
                return (1, obj.get("line", 0) or 0)
            if obj.get("__kind") == "__Document":
                return (-1, 0)  # __Document always first
            if obj.get("__kind") == "__TextBlock":
                return (0, obj.get("__line", 0) or 0)
            return (0, obj.get("__line", 0) or 0)

        result.sort(key=_sort_key)

    # Filter by active features
    return [_filter_by_features(obj, active_features) for obj in result]


def _extract_references_for_textblock(tb: dict[str, Any], lines: list[str]) -> None:
    """Extract references from __TextBlock content and add __references field."""
    content = tb.get("content", "")
    if "[[" not in content:
        return

    refs: list[dict[str, Any]] = []
    search_start_idx = 0

    # Track if we're inside a code fence
    in_code_fence = False
    example_fence_depth = 0

    for content_line in content.split("\n"):
        # Check for code fence markers
        stripped = content_line.strip()
        if stripped.startswith("```"):
            if example_fence_depth > 0:
                # Inside example fence: track depth to find matching close
                fence_content = stripped[3:]
                if fence_content:  # Opening fence (has content after ```)
                    example_fence_depth += 1
                else:  # Closing fence (bare ```)
                    example_fence_depth -= 1
            else:
                # Check if this is an example code fence
                fence_content = stripped[3:]
                if "example" in fence_content:
                    example_fence_depth = 1
                in_code_fence = not in_code_fence
            continue

        # Skip references inside example code fences only
        # Regular code fences should parse references (for dynamic blocks, etc.)
        if example_fence_depth > 0:
            continue

        if "[[" not in content_line:
            continue
        content_trimmed = stripped
        if not content_trimmed:
            continue
        # Find this line in original markdown
        for line_idx in range(search_start_idx, len(lines)):
            orig_line = lines[line_idx]
            if content_trimmed in orig_line and "[[" in orig_line:
                line_num = line_idx + 1
                col_offset = 0
                refs.extend(extract_references_from_text(orig_line, line_num, col_offset))
                search_start_idx = line_idx + 1
                break

    if refs:
        tb["__references"] = refs


def _extract_references_for_objects(objects: dict[str, dict[str, Any]], lines: list[str]) -> None:
    """
    Extract references from all objects and add __references field.

    References are found in:
    - String field values containing [[...]]
    - Array items containing [[...]]
    - Comment content in __comments
    """
    for _obj_id, obj in objects.items():
        refs: list[dict[str, Any]] = []

        # Get line number for this object (for locating references)
        obj_line = obj.get("__line")
        if obj_line is None:
            continue

        # Track which lines we've processed for this object
        # We need to find references in the markdown source.
        # Search starts at the object's own line (not the file top) and advances
        # monotonically, so a reference value that also appears in an earlier
        # object is attributed to THIS object's actual occurrence, not the first
        # one in the file.
        obj_search_start = (obj_line - 1) if obj_line else 0

        # Process string fields
        for key, value in obj.items():
            if key.startswith("__"):
                # Check comments
                if key == "__comments" and isinstance(value, list):
                    for comment in value:
                        if isinstance(comment, dict):
                            content = comment.get("content", "")
                            if "[[" in content:
                                # Find this comment in source to get line number
                                for line_idx in range(obj_search_start, len(lines)):
                                    line = lines[line_idx]
                                    if content in line:
                                        line_num = line_idx + 1
                                        col_offset = line.find(content)
                                        refs.extend(
                                            extract_references_from_text(
                                                content, line_num, col_offset
                                            )
                                        )
                                        obj_search_start = line_idx + 1
                                        break
                continue

            if isinstance(value, str) and "[[" in value:
                # For multiline text fields, search each line in original markdown
                # Track search position to handle duplicate lines in content
                # Skip lines inside code fences (``` ... ```)
                in_code_fence = False
                example_fence_depth = 0
                for content_line in value.split("\n"):
                    # Track code fence state
                    stripped = content_line.strip()
                    if stripped.startswith("```"):
                        if example_fence_depth > 0:
                            fence_content = stripped[3:]
                            if fence_content:
                                example_fence_depth += 1
                            else:
                                example_fence_depth -= 1
                        else:
                            fence_content = stripped[3:]
                            if "example" in fence_content:
                                example_fence_depth = 1
                            in_code_fence = not in_code_fence
                        continue

                    # Skip references inside example code fences
                    if example_fence_depth > 0:
                        continue

                    if "[[" not in content_line:
                        continue
                    content_trimmed = content_line.strip()
                    if not content_trimmed:
                        continue
                    # Find this line in original markdown, starting from last found position
                    for line_idx in range(obj_search_start, len(lines)):
                        orig_line = lines[line_idx]
                        if content_trimmed in orig_line and "[[" in orig_line:
                            line_num = line_idx + 1
                            col_offset = 0  # Will be calculated in extract_references_from_text
                            refs.extend(
                                extract_references_from_text(orig_line, line_num, col_offset)
                            )
                            # Move search position forward for next content line
                            obj_search_start = line_idx + 1
                            break

            elif isinstance(value, list):
                # Check array items for references
                # Check if this is a YAML array (via __syntax)
                is_yaml_array = obj.get("__syntax", {}).get(key) == "yaml_array"

                if is_yaml_array:
                    # For YAML arrays, extract references from each array element
                    # Find the line containing this field
                    field_line_idx = None
                    field_line = None
                    for line_idx, line in enumerate(lines):
                        if f"{key}:" in line and "[[" in line:
                            field_line_idx = line_idx
                            field_line = line
                            break

                    if field_line:
                        # Find where the value starts (after "key: ")
                        colon_pos = field_line.find(":")
                        if colon_pos >= 0:
                            value_start = colon_pos + 1
                            while value_start < len(field_line) and field_line[value_start] == " ":
                                value_start += 1

                            # Extract value string from markdown
                            value_str = field_line[value_start:].strip()

                            # For each array item, find its position in the value string
                            search_pos = 0
                            for item in value:
                                if isinstance(item, str) and "[[" in item:
                                    # Find this item in the value string
                                    item_pos = value_str[search_pos:].find(item)
                                    if item_pos >= 0:
                                        absolute_pos = value_start + search_pos + item_pos
                                        line_num = field_line_idx + 1
                                        refs.extend(
                                            extract_references_from_text(
                                                item, line_num, absolute_pos
                                            )
                                        )
                                        search_pos += item_pos + len(item)
                else:
                    # For non-YAML arrays (markdown lists), use original logic
                    for item in value:
                        if isinstance(item, str) and "[[" in item:
                            # Find this item in source
                            for line_idx, line in enumerate(lines):
                                if item in line:
                                    line_num = line_idx + 1
                                    col_offset = line.find(item)
                                    refs.extend(
                                        extract_references_from_text(item, line_num, col_offset)
                                    )
                                    break

        if refs:
            # Remove duplicate references (same target, line, start_col, end_col)
            seen = set()
            unique_refs = []
            for ref in refs:
                key = (ref["target"], ref["line"], ref["start_col"], ref["end_col"])
                if key not in seen:
                    seen.add(key)
                    unique_refs.append(ref)
            obj["__references"] = unique_refs


def _extract_field_positions(objects: dict[str, dict[str, Any]], lines: list[str]) -> None:
    """
    Extract field positions and add __positions field.

    Scans markdown to find where each field is defined:
    - `- field: value` style (list item fields)
    - `## Field [[id:text]]` style (heading text fields)
    """
    for _obj_id, obj in objects.items():
        positions: dict[str, dict[str, int]] = {}

        # Get object's starting line
        obj_line = obj.get("__line")
        if obj_line is None:
            continue

        # Get field names (non-meta keys)
        field_names = [k for k in obj if not k.startswith("__")]

        for field_name in field_names:
            # Skip parent fields - these are fields that contain only [[#id]]
            # and have a child object with __parent_field equal to this field name
            field_value = obj.get(field_name)
            if (
                isinstance(field_value, str)
                and field_value.strip().startswith("[[#")
                and field_value.strip().endswith("]]")
            ):
                # Check if any object has __parent_field == field_name
                is_parent_field = any(
                    child.get("__parent_field") == field_name for child in objects.values()
                )
                if is_parent_field:
                    continue

            # Search for field definition in lines starting from object line
            for line_idx in range(obj_line - 1, len(lines)):
                line = lines[line_idx]

                # Check for list item field: `- field: value` or `- **field**: value`
                list_match = re.match(rf"^\s*-\s+\**{re.escape(field_name)}\**\s*:", line)
                if list_match:
                    col_char = line.find(field_name)
                    # Convert character position to byte position
                    col = len(line[:col_char].encode("utf-8"))
                    positions[field_name] = {"line": line_idx + 1, "col": col}
                    break

                # Check for heading text field: `## Label [[id:text]]` or `## Label [[id]]`
                heading_match = re.match(rf"^#+\s+.*\[\[{re.escape(field_name)}(?::.*?)?\]\]", line)
                if heading_match:
                    col_char = line.find(f"[[{field_name}")
                    # Convert character position to byte position
                    col = len(line[:col_char].encode("utf-8"))
                    positions[field_name] = {"line": line_idx + 1, "col": col}
                    break

        if positions:
            obj["__positions"] = positions


def _filter_by_features(obj: dict[str, Any], features: set[str]) -> dict[str, Any]:
    """
    Filter object fields based on active features.
    Preserves original key order from the input object.

    In minimal mode (no FEATURE_ID/KIND):
    - __id only if explicitly set in document
    - __kind only if explicitly set (not system types like __Object)

    Always keeps: __comments, data fields
    Conditional: __id, __kind, __label, __parent/etc, __types, __syntax,
                 __level, __line, __has_explicit_id
    """
    # Define which keys to skip based on features
    skip_keys: set[str] = set()

    # In minimal mode (no FEATURE_ID), skip __id if it was auto-generated
    skip_id = FEATURE_ID not in features and obj.get("__has_explicit_id") is False

    # In minimal mode (no FEATURE_KIND), skip __kind if it's a system type (starts with __)
    kind = obj.get("__kind")
    skip_kind = FEATURE_KIND not in features and (
        not isinstance(kind, str) or kind.startswith("__")
    )

    if FEATURE_LABEL not in features:
        skip_keys.add("__label")

    if FEATURE_PARENT not in features:
        skip_keys.update(["__container", "__parent", "__parent_field"])

    if FEATURE_TYPES not in features:
        skip_keys.add("__types")

    if FEATURE_SYNTAX not in features:
        skip_keys.add("__syntax")

    if FEATURE_LEVEL not in features:
        skip_keys.add("__level")

    if FEATURE_LINE not in features:
        skip_keys.add("__line")
        skip_keys.add("__code_fences")  # Only in full mode

    if FEATURE_EXPLICIT_ID not in features:
        skip_keys.add("__has_explicit_id")

    if FEATURE_REFERENCES not in features:
        skip_keys.add("__references")

    if FEATURE_POSITIONS not in features:
        skip_keys.add("__positions")

    # Copy keys in original order, skipping disabled ones
    result: dict[str, Any] = {}
    for key in obj:
        if key == "__id" and skip_id:
            continue
        if key == "__kind" and skip_kind:
            continue
        if key not in skip_keys:
            result[key] = obj[key]

    return result


def _normalize_field_order(obj: dict[str, Any]) -> dict[str, Any]:
    """
    Normalize field order in object:
    1. __id, __label, __kind, __container, __parent, __parent_field
    2. __comments (before data fields per spec)
    3. Data fields (in original order)
    4. __types, __syntax, __level, __has_explicit_id
    """
    result: dict[str, Any] = {"__id": obj["__id"]}
    if "__local_id" in obj:
        result["__local_id"] = obj["__local_id"]
    if "__label" in obj:
        result["__label"] = obj["__label"]

    # Add identity fields first
    if "__kind" in obj:
        result["__kind"] = obj["__kind"]
    if "__container" in obj:
        result["__container"] = obj["__container"]
    if "__parent" in obj:
        result["__parent"] = obj["__parent"]
    if "__parent_field" in obj:
        result["__parent_field"] = obj["__parent_field"]

    # Add __comments before data fields
    if "__comments" in obj:
        result["__comments"] = obj["__comments"]

    # Add data fields in original order
    for key in obj:
        if not key.startswith("__"):
            result[key] = obj[key]

    # Add metadata fields last
    if "__types" in obj:
        result["__types"] = obj["__types"]
    if "__syntax" in obj:
        result["__syntax"] = obj["__syntax"]
    # Rebuild metadata
    if "__level" in obj:
        result["__level"] = obj["__level"]
    if "__line" in obj:
        result["__line"] = obj["__line"]
    if "__has_explicit_id" in obj:
        result["__has_explicit_id"] = obj["__has_explicit_id"]
    if "__references" in obj:
        result["__references"] = obj["__references"]
    if "__positions" in obj:
        result["__positions"] = obj["__positions"]
    if "__labels" in obj:
        result["__labels"] = obj["__labels"]

    return result


def _rebuild_object_to_lines(
    obj: dict[str, Any],
    objects_by_id: dict[str, dict[str, Any]],
    lines: list[str],
    children_map: dict[str, list[str]] | None = None,
) -> None:
    """Helper to rebuild a single object to lines."""
    if children_map is None:
        children_map = {}

    obj_id = obj.get("__id", "")

    # Use __level from object if present
    actual_level = obj.get("__level", 2)

    # Render heading
    heading = _rebuild_heading(obj, level=actual_level)
    lines.append(heading)

    # Get comments, syntax info, and labels for this object
    obj_comments = obj.get("__comments", [])
    obj_syntax = obj.get("__syntax", {})
    obj_labels = obj.get("__labels", {})

    # Helper to add comment by anchor
    def add_comment_after(anchor: str) -> None:
        for comment in obj_comments:
            if comment.get("after") == anchor:
                lines.append("")
                lines.append(comment["content"])

    # Add comment after __self (before any fields)
    add_comment_after("__self")

    # Single-pass: output fields in insertion order, preserving original field order.
    # Child ref headings encountered during a primitive run are buffered
    # and flushed when the primitive run ends.
    child_ids = children_map.get(obj_id, [])
    obj_types = obj.get("__types", {})
    rendered_children: set[str] = set()
    in_primitive_run = False
    pending_child_headings: list[str] = []

    def flush_pending_child_headings() -> None:
        nonlocal pending_child_headings
        for ref_id in pending_child_headings:
            rendered_children.add(ref_id)
            child_obj = objects_by_id.get(ref_id)
            if child_obj:
                lines.append("")
                _rebuild_object_to_lines(child_obj, objects_by_id, lines, children_map)
        pending_child_headings = []

    for key, value in obj.items():
        if key.startswith("__"):
            continue

        syntax = obj_syntax.get(key, "")
        is_heading_syntax = syntax in (
            "headers",
            "table",
            "markdown_list",
            "multiline_text",
            "yaml_object",
            "json_object",
            "map",
        )

        # Check for single child reference (not heading syntax)
        if (
            not is_heading_syntax
            and isinstance(value, str)
            and value.startswith("[[#")
            and value.endswith("]]")
        ):
            ref_id = value[3:-2]
            if ref_id in child_ids:
                # Output as primitive line if it has a __types entry
                if key in obj_types:
                    if not in_primitive_run:
                        lines.append("")
                        in_primitive_run = True
                    lines.extend(_format_primitive_field(key, value, obj_syntax))
                    add_comment_after(key)
                    # Buffer child heading to render after primitive run
                    pending_child_headings.append(ref_id)
                else:
                    # No __types entry — end primitive run and render child immediately
                    if in_primitive_run:
                        in_primitive_run = False
                        flush_pending_child_headings()
                    rendered_children.add(ref_id)
                    child_obj = objects_by_id.get(ref_id)
                    if child_obj:
                        lines.append("")
                        _rebuild_object_to_lines(child_obj, objects_by_id, lines, children_map)
                    add_comment_after(key)
                continue

        if is_heading_syntax:
            # End primitive run and flush pending child headings
            if in_primitive_run:
                in_primitive_run = False
                flush_pending_child_headings()

            if syntax == "headers" and isinstance(value, list):
                refs = value
                if refs:
                    first_ref_id = refs[0][3:-2]
                    first_obj = objects_by_id.get(first_ref_id)
                    kind = first_obj.get("__kind", "") if first_obj else ""

                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: [{kind}]]]")

                    for ref in refs:
                        ref_id = ref[3:-2]
                        rendered_children.add(ref_id)
                        ref_obj = objects_by_id.get(ref_id)
                        if ref_obj:
                            lines.append("")
                            _rebuild_object_to_lines(ref_obj, objects_by_id, lines, children_map)

            elif syntax == "table" and isinstance(value, list):
                refs = value
                kind = ""
                column_names: list[str] = []
                if refs:
                    first_ref_id = refs[0][3:-2]
                    first_obj = objects_by_id.get(first_ref_id)
                    if first_obj:
                        kind = first_obj.get("__kind", "")
                        for k in first_obj:
                            if not k.startswith("__"):
                                column_names.append(k)

                lines.append("")
                field_label = _get_field_label(key, obj_labels)
                lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: [{kind}]]]")
                lines.append("")

                if column_names:
                    lines.append("| " + " | ".join(column_names) + " |")
                    lines.append("|" + "|".join(["---"] * len(column_names)) + "|")

                    for ref in refs:
                        ref_id = ref[3:-2]
                        rendered_children.add(ref_id)
                        ref_obj = objects_by_id.get(ref_id)
                        if ref_obj:
                            row_values = [
                                _format_value(ref_obj.get(col, "")) for col in column_names
                            ]
                            lines.append("| " + " | ".join(row_values) + " |")

            elif syntax == "markdown_list" and isinstance(value, list):
                lines.append("")
                field_label = _get_field_label(key, obj_labels)
                lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: array]]")
                lines.append("")
                for item in value:
                    lines.append(f"- {_format_value(item)}")
                add_comment_after(key)

            elif syntax == "multiline_text" and isinstance(value, str):
                lines.append("")
                field_label = _get_field_label(key, obj_labels)
                lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: text]]")
                lines.append("")
                if value:
                    lines.append(value)
                add_comment_after(key)

            elif syntax == "yaml_object" and isinstance(value, dict):
                import yaml

                lines.append("")
                field_label = _get_field_label(key, obj_labels)
                lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: yaml]]")
                lines.append("")
                lines.append("```yaml")
                yaml_str = yaml.dump(
                    value, default_flow_style=False, allow_unicode=True, sort_keys=False
                )
                lines.append(yaml_str.rstrip())
                lines.append("```")

            elif syntax == "json_object":
                lines.append("")
                field_label = _get_field_label(key, obj_labels)
                lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: json]]")
                lines.append("")
                lines.append("```json")
                json_str = json.dumps(value, indent=2, ensure_ascii=False)
                lines.append(json_str)
                lines.append("```")

            # TODO: deduplicate map rebuild with rebuild()
            elif syntax == "map" and isinstance(value, dict):
                lines.extend(_rebuild_map_lines(key, value, actual_level + 1, obj_labels))
                add_comment_after(key)

        else:
            # Primitive field
            if not in_primitive_run:
                lines.append("")
                in_primitive_run = True
            lines.extend(_format_primitive_field(key, value, obj_syntax))
            add_comment_after(key)

    # Flush any remaining pending child headings
    flush_pending_child_headings()

    # Render remaining children not referenced by fields
    for child_id in child_ids:
        if child_id not in rendered_children:
            child_obj = objects_by_id.get(child_id)
            if child_obj:
                lines.append("")
                _rebuild_object_to_lines(child_obj, objects_by_id, lines, children_map)


def rebuild(data: list[dict[str, Any]]) -> str:
    """
    Rebuild QMD.md from JSON array of objects.

    Uses __parent to determine document structure and heading levels.
    Handles __Document and __TextBlock system types.

    Args:
        data: List of objects

    Returns:
        QMD.md source string
    """
    lines: list[str] = []

    # Build index of objects by ID
    objects_by_id: dict[str, dict[str, Any]] = {obj["__id"]: obj for obj in data if "__id" in obj}

    # Check for __Document (first element with __kind == "__Document")
    doc_obj = None
    for obj in data:
        if obj.get("__kind") == "__Document":
            doc_obj = obj
            break

    # If we have a __Document, use its content order
    if doc_obj:
        # Build children map for regular objects
        children_map: dict[str, list[str]] = {}
        for obj in data:
            kind = obj.get("__kind", "")
            # Skip system types except __Object
            if "__id" not in obj or (kind.startswith("__") and kind != "__Object"):
                continue
            parent_ref = obj.get("__parent")
            if parent_ref and isinstance(parent_ref, str):
                parent_id = parent_ref[3:-2]
                if parent_id not in children_map:
                    children_map[parent_id] = []
                children_map[parent_id].append(obj["__id"])

        content_refs = doc_obj.get("content", [])
        for ref in content_refs:
            ref_id = ref[3:-2]  # Extract ID from [[#id]]
            item = objects_by_id.get(ref_id)
            if not item:
                continue

            if item.get("__kind") == "__TextBlock":
                # Output text block content as-is
                if lines:
                    lines.append("")
                lines.append(item.get("content", ""))
            else:
                # Output regular object
                if lines:
                    lines.append("")
                _rebuild_object_to_lines(item, objects_by_id, lines, children_map)

        # Remove trailing empty lines, ensure exactly one trailing newline
        while lines and lines[-1] == "":
            lines.pop()

        return "\n".join(lines) + "\n"

    # No __Document - use old logic for backward compatibility
    # Filter out system types (except __Object, __Workspace, __Namespace)
    def is_system_type(kind: str) -> bool:
        return kind.startswith("__") and kind not in ("__Object", "__Workspace", "__Namespace")

    actual_objects = [obj for obj in data if not is_system_type(obj.get("__kind", ""))]

    # Build parent->children map preserving order from data
    children_map: dict[str, list[str]] = {}  # parent_id -> [child_ids]
    # Track order of objects in original data
    object_order: dict[str, int] = {}  # obj_id -> index in data
    for idx, obj in enumerate(data):
        if "__id" in obj:
            object_order[obj["__id"]] = idx

    for obj in actual_objects:
        if "__id" not in obj:
            continue
        parent_ref = obj.get("__parent")
        if parent_ref and isinstance(parent_ref, str):
            parent_id = parent_ref[3:-2]  # Extract from [[#id]]
            if parent_id not in children_map:
                children_map[parent_id] = []
            children_map[parent_id].append(obj["__id"])

    # Sort children by their order in original data
    for parent_id in children_map:
        children_map[parent_id].sort(key=lambda x: object_order.get(x, 999999))

    # Find root objects (no __parent and no __container), preserving order from data
    root_objects = [obj for obj in actual_objects if "__parent" not in obj and "__id" in obj]
    root_ids = [
        obj["__id"]
        for obj in sorted(root_objects, key=lambda x: object_order.get(x["__id"], 999999))
    ]

    def rebuild_object(obj_id: str, level: int) -> None:
        """Recursively rebuild object and its children."""
        obj = objects_by_id.get(obj_id)
        if not obj:
            return

        # Use __level from object if present
        actual_level = obj.get("__level", level)

        # Render heading
        heading = _rebuild_heading(obj, level=actual_level)
        lines.append(heading)

        # Get comments, syntax info, and labels for this object
        obj_comments = obj.get("__comments", [])
        obj_syntax = obj.get("__syntax", {})
        obj_labels = obj.get("__labels", {})

        # Helper to add comment by anchor
        def add_comment_after(anchor: str) -> None:
            for comment in obj_comments:
                if comment.get("after") == anchor:
                    lines.append("")
                    lines.append(comment["content"])

        # Add comment after __self (before any fields)
        add_comment_after("__self")

        # Collect child IDs and track which fields are object arrays/tables
        child_ids = children_map.get(obj_id, [])
        object_array_fields: dict[str, list[str]] = {}  # field_name -> [ref_ids]
        table_fields: dict[str, list[str]] = {}  # field_name -> [ref_ids]

        for key, value in obj.items():
            if key.startswith("__"):
                continue
            if isinstance(value, list) and obj_syntax.get(key) == "headers":
                object_array_fields[key] = value
            elif isinstance(value, list) and obj_syntax.get(key) == "table":
                table_fields[key] = value

        # Track rendered children (from tables)
        rendered_children = set()
        inline_rendered_children = set()  # Children fully rendered inline (headers/tables)
        for refs in table_fields.values():
            for ref in refs:
                rendered_children.add(ref[3:-2])
                inline_rendered_children.add(ref[3:-2])

        # Single-pass: output fields in insertion order with buffered child headings.
        obj_types = obj.get("__types", {})
        in_primitive_run = False
        pending_child_headings: list[str] = []

        def flush_pending_child_headings() -> None:
            nonlocal pending_child_headings
            for ref_id in pending_child_headings:
                rendered_children.add(ref_id)
                child_obj = objects_by_id.get(ref_id)
                if child_obj:
                    lines.append("")
                    rebuild_object(ref_id, actual_level + 1)
            pending_child_headings = []

        for key, value in obj.items():
            if key.startswith("__"):
                continue

            syntax = obj_syntax.get(key, "")
            is_heading_syntax = syntax in (
                "headers",
                "table",
                "markdown_list",
                "multiline_text",
                "yaml_object",
                "json_object",
                "map",
            )

            # Check for single child reference (not heading syntax)
            if (
                not is_heading_syntax
                and isinstance(value, str)
                and value.startswith("[[#")
                and value.endswith("]]")
            ):
                ref_id = value[3:-2]
                if ref_id in child_ids:
                    if key in obj_types:
                        if not in_primitive_run:
                            lines.append("")
                            in_primitive_run = True
                        lines.extend(_format_primitive_field(key, value, obj_syntax))
                        add_comment_after(key)
                        pending_child_headings.append(ref_id)
                    else:
                        if in_primitive_run:
                            in_primitive_run = False
                            flush_pending_child_headings()
                        rendered_children.add(ref_id)
                        child_obj = objects_by_id.get(ref_id)
                        if child_obj:
                            lines.append("")
                            rebuild_object(ref_id, actual_level + 1)
                        add_comment_after(key)
                    continue

            if is_heading_syntax:
                if in_primitive_run:
                    in_primitive_run = False
                    flush_pending_child_headings()

                if syntax == "headers" and isinstance(value, list):
                    refs = object_array_fields.get(key, [])
                    if refs:
                        first_ref_id = refs[0][3:-2]
                        first_obj = objects_by_id.get(first_ref_id)
                        kind = first_obj.get("__kind", "") if first_obj else ""

                        # Self-array pattern: field key == object ID means the
                        # heading already includes [Kind] annotation — skip sub-heading
                        is_self_array = key == obj_id
                        if not is_self_array:
                            lines.append("")
                            field_label = _get_field_label(key, obj_labels)
                            lines.append(
                                f"{'#' * (actual_level + 1)} {field_label} [[{key}: [{kind}]]]"
                            )

                        for ref in refs:
                            ref_id = ref[3:-2]
                            rendered_children.add(ref_id)
                            inline_rendered_children.add(ref_id)
                            lines.append("")
                            child_level = actual_level + 1 if is_self_array else actual_level + 2
                            rebuild_object(ref_id, child_level)

                elif syntax == "table":
                    refs = table_fields.get(key, [])
                    kind = ""
                    column_names: list[str] = []
                    if refs:
                        first_ref_id = refs[0][3:-2]
                        first_obj = objects_by_id.get(first_ref_id)
                        if first_obj:
                            kind = first_obj.get("__kind", "")
                            for k in first_obj:
                                if not k.startswith("__"):
                                    column_names.append(k)

                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: [{kind}]]]")
                    lines.append("")

                    if column_names:
                        lines.append("| " + " | ".join(column_names) + " |")
                        lines.append("|" + "|".join(["---"] * len(column_names)) + "|")
                        for ref in refs:
                            ref_id = ref[3:-2]
                            ref_obj = objects_by_id.get(ref_id)
                            if ref_obj:
                                row_values = [
                                    _format_value(ref_obj.get(col, "")) for col in column_names
                                ]
                                lines.append("| " + " | ".join(row_values) + " |")

                elif syntax == "markdown_list" and isinstance(value, list):
                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: array]]")
                    lines.append("")
                    for item in value:
                        lines.append(f"- {_format_value(item)}")
                    add_comment_after(key)

                elif syntax == "multiline_text" and isinstance(value, str):
                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: text]]")
                    lines.append("")
                    if value:
                        lines.append(value)
                    add_comment_after(key)

                elif syntax == "yaml_object" and isinstance(value, dict):
                    import yaml

                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: yaml]]")
                    lines.append("")
                    lines.append("```yaml")
                    yaml_str = yaml.dump(
                        value, default_flow_style=False, allow_unicode=True, sort_keys=False
                    )
                    lines.append(yaml_str.rstrip())
                    lines.append("```")

                elif syntax == "json_object":
                    lines.append("")
                    field_label = _get_field_label(key, obj_labels)
                    lines.append(f"{'#' * (actual_level + 1)} {field_label} [[{key}: json]]")
                    lines.append("")
                    lines.append("```json")
                    json_str = json.dumps(value, indent=2, ensure_ascii=False)
                    lines.append(json_str)
                    lines.append("```")

                # TODO: deduplicate map rebuild with _rebuild_object_to_lines()
                elif syntax == "map" and isinstance(value, dict):
                    lines.extend(_rebuild_map_lines(key, value, actual_level + 1, obj_labels))
                    add_comment_after(key)

            else:
                if not in_primitive_run:
                    lines.append("")
                    in_primitive_run = True
                lines.extend(_format_primitive_field(key, value, obj_syntax))
                add_comment_after(key)

        # Flush any remaining pending child headings
        flush_pending_child_headings()

        # Render remaining children not referenced by fields
        for child_id in child_ids:
            if child_id in rendered_children:
                continue
            child_obj = objects_by_id.get(child_id)
            if child_obj:
                lines.append("")
                rebuild_object(child_id, actual_level + 1)

    # Rebuild all root objects
    for root_id in root_ids:
        rebuild_object(root_id, level=1)
        lines.append("")

    # Remove trailing empty lines, ensure exactly one trailing newline
    while lines and lines[-1] == "":
        lines.pop()

    return "\n".join(lines) + "\n"


def _get_field_label(key: str, labels: dict[str, str]) -> str:
    """Get field label from __labels or generate from key."""
    return labels.get(key, key.replace("_", " ").title())


def _rebuild_map_lines(
    key: str, value: dict[str, Any], level: int, labels: dict[str, str]
) -> list[str]:
    """Rebuild a map field as QMD.md lines: heading + bullet list of key: value pairs."""
    lines = [
        "",
        f"{'#' * level} {_get_field_label(key, labels)} [[{key}: map]]",
    ]
    if value:
        lines.append("")
        for mk, mv in value.items():
            mv_str = str(mv)
            if "\n" in mv_str:
                lines.append(f"- {mk}: |")
                for ml in mv_str.split("\n"):
                    lines.append(f"    {ml}")
            else:
                lines.append(f"- {mk}: {mv_str}")
    return lines


def _format_primitive_field(key: str, value: Any, syntax: dict[str, str]) -> list[str]:
    """Format a primitive field for output in QMD.md, handling YAML multiline syntax."""
    # Check if it's a YAML multiline field
    if syntax.get(key) == "yaml_multiline" and isinstance(value, str):
        lines = [f"- {key}: |"]
        # Indent each line with 4 spaces
        for line in value.split("\n"):
            lines.append(f"    {line}")
        return lines
    # yaml_multiline_array: multiline bracket array with indentation
    if syntax.get(key) == "yaml_multiline_array" and isinstance(value, list):
        lines = [f"- {key}: ["]
        for idx, item in enumerate(value):
            formatted = _format_value(item)
            if idx < len(value) - 1:
                lines.append(f"    {formatted},")
            else:
                lines.append(f"    {formatted}")
        lines.append("  ]")
        return lines
    # comma_refs: comma-separated references without outer brackets
    if syntax.get(key) == "comma_refs" and isinstance(value, list):
        formatted_items = [_format_value(item) for item in value]
        return [f"- {key}: {', '.join(formatted_items)}"]
    return [f"- {key}: {_format_value(value)}"]


def _format_value(value: Any) -> str:
    """Format a value for output in QMD.md."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "null"
    if isinstance(value, list):
        # Format as YAML array
        formatted_items = [_format_value(item) for item in value]
        return "[" + ", ".join(formatted_items) + "]"
    if isinstance(value, str):
        # Quote strings with leading/trailing whitespace to preserve them
        if value != value.strip():
            escaped = value.replace('"', '\\"')
            return f'"{escaped}"'
        return value
    return str(value)


def _rebuild_heading(obj: dict[str, Any], level: int = 2) -> str:
    """Reconstruct heading from object metadata."""
    label: str = obj.get("__label", "")
    obj_id: str = obj.get("__id", "")
    kind: str | None = obj.get("__kind")
    has_explicit_id: bool = obj.get("__has_explicit_id", True)  # Default: explicit
    obj_syntax: dict[str, str] = obj.get("__syntax", {})

    # BR-12: Use __local_id for heading reconstruction when present
    heading_id: str = obj.get("__local_id", obj_id)

    # __Object is the default kind - don't output it
    if kind == "__Object":
        kind = None

    # Check if this standalone object has a field type hint for its own ID
    # e.g. __syntax: {summary: "multiline_text"} → [[summary: text]]
    field_type_hint = None
    if heading_id in obj_syntax and not kind:
        syntax_val = obj_syntax[heading_id]
        if syntax_val == "multiline_text":
            field_type_hint = "text"
        elif syntax_val == "markdown_list":
            field_type_hint = "array"
        elif syntax_val == "yaml_object":
            field_type_hint = "yaml"
        elif syntax_val == "json_object":
            field_type_hint = "json"
        elif syntax_val == "map":
            field_type_hint = "map"
        elif syntax_val == "headers":
            # Object array: [[id: [Kind]]]
            array_kind = obj_syntax.get("__array_kind", "")
            if array_kind:
                field_type_hint = f"[{array_kind}]"

    # Use __level from object if present, otherwise use computed level
    actual_level = obj.get("__level", level)

    parts: list[str] = []

    if label:
        parts.append(label)

    # Build the ID part (only if has explicit ID or has kind)
    if kind and not label:
        # Pattern: [[:Kind]] (no label, no explicit ID)
        parts.append(f"[[:{kind}]]")
    elif kind and heading_id:
        # Pattern: [[id:Kind]] or Label [[id:Kind]]
        parts.append(f"[[{heading_id}: {kind}]]")
    elif field_type_hint and heading_id:
        # Pattern: [[id: type]] for standalone field-type objects
        parts.append(f"[[{heading_id}: {field_type_hint}]]")
    elif has_explicit_id and heading_id:
        # Pattern: [[id]] or Label [[id]] (only if explicit)
        parts.append(f"[[{heading_id}]]")
    # else: no [[id]] - heading without explicit ID

    heading_prefix = "#" * actual_level
    return f"{heading_prefix} {' '.join(parts)}"
