"""Chunking algorithm for QMD.md objects.

Based on Finding 3: Hierarchical chunking with parent/child structure.

Improvements:
- Adds object ID to chunk text for FTS discovery
- Better handling of __TextBlock (uses content directly)
- Adds namespace/parent context for short objects
"""

import hashlib
from pathlib import Path
from typing import Any

from .config import ChunkingConfig


def compute_global_id(obj: dict[str, Any]) -> str:
    """Compute __global_id from object fields.

    Format matches Rust parser: workspace:namespace:id or workspace::id

    Args:
        obj: QMD.md object with __workspace, __namespace, __id fields.

    Returns:
        Global ID string in format "workspace:namespace:id" or "workspace::id".
    """
    workspace = obj.get("__workspace", "")
    namespace = obj.get("__namespace", "")
    obj_id = obj.get("__id", "")

    if not obj_id:
        return ""

    if namespace:
        return f"{workspace}:{namespace}:{obj_id}"
    else:
        return f"{workspace}::{obj_id}"


def enrich_objects_with_global_id(objects: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Add __global_id to objects that don't have it.

    This enriches Python parser output to match Rust parser format.

    Args:
        objects: List of QMD.md objects from parse_workspace.

    Returns:
        Same objects with __global_id added.
    """
    for obj in objects:
        if "__global_id" not in obj:
            obj["__global_id"] = compute_global_id(obj)
    return objects


def _compute_hash(text: str) -> str:
    """Compute SHA256 hash of text, truncated to 16 chars."""
    return hashlib.sha256(text.encode()).hexdigest()[:16]


def _collect_text_fields(obj: dict[str, Any]) -> dict[str, str]:
    """Collect text fields from object data, including __comments content.

    Comments (markdown descriptions, code blocks, tables) are stored in __comments
    with an `after` key indicating which field they follow. We merge comment content
    into the corresponding field or create a synthetic "__description" field for
    comments after "__self" (the object heading).
    """
    text_fields = {}
    for key, value in obj.items():
        if key.startswith("__"):
            continue
        if isinstance(value, str) and value.strip():
            text_fields[key] = value

    # Merge __comments content into fields
    comments = obj.get("__comments", [])
    if not comments:
        return text_fields

    # Group comments by their anchor field
    comment_groups: dict[str, list[str]] = {}
    for comment in comments:
        if not isinstance(comment, dict):
            continue
        anchor = comment.get("after", "")
        content = comment.get("content", "")
        if content.strip():
            comment_groups.setdefault(anchor, []).append(content)

    for anchor, contents in comment_groups.items():
        joined = "\n\n".join(contents)
        if anchor == "__self":
            # Text after the object heading — store as description
            if "__description" in text_fields:
                text_fields["__description"] += "\n\n" + joined
            else:
                text_fields["__description"] = joined
        elif anchor in text_fields:
            # Append comment to existing field value
            text_fields[anchor] += "\n\n" + joined
        else:
            # Comment after a field that has no string value (e.g. array field)
            # — store under the anchor name
            text_fields[anchor] = joined

    return text_fields


def _extract_id_variants(global_id: str, obj_id: str) -> str:
    """Extract ID variants for better FTS matching.

    For "qmd41" returns "qmd41 QMD-41 QMDC 41" for FTS matching.
    """
    import re

    variants = [obj_id]

    # Add hyphenated variant: qmd41 -> QMD-41
    match = re.match(r"^([a-zA-Z]+)(\d+)$", obj_id)
    if match:
        prefix, num = match.groups()
        variants.append(f"{prefix.upper()}-{num}")

    return " ".join(variants)


def _create_chunk(
    chunk_id: str,
    object_id: str,
    object_kind: str,
    chunk_type: str,
    source_file: str,
    text: str,
    parent_chunk_id: str | None = None,
) -> dict[str, Any]:
    """Create a chunk dict."""
    return {
        "chunk_id": chunk_id,
        "object_id": object_id,
        "object_kind": object_kind,
        "chunk_type": chunk_type,
        "source_file": source_file,
        "text": text,
        "text_hash": _compute_hash(text),
        "parent_chunk_id": parent_chunk_id,
    }


def _split_text_by_sections(text: str, max_size: int) -> list[tuple[str, str]]:
    """Split long text into sections at logical boundaries.

    Detects section headers in the text (lines ending with colon, markdown-style
    headers, or labeled paragraphs) and splits at those boundaries.

    Returns list of (section_label, section_text) tuples.
    The section_label is a slug derived from the section header for use in chunk IDs.
    """
    import re

    if len(text) <= max_size:
        return [("", text)]

    # Detect section boundaries: lines that look like headers
    # Patterns: "Section name:", "## Section", "**Section**:", or ALL-CAPS lines
    section_pattern = re.compile(
        r"^("
        r"(?:#{1,4}\s+.+)|"  # Markdown headers
        r"(?:[A-Z][A-Za-z /()-]+:$)|"  # "Section Name:" at line start
        r"(?:\*\*[^*]+\*\*:?$)|"  # **Bold section**:
        r"(?:[A-Z][a-z]+ [a-z]+ \([^)]+\):$)"  # "Label (context):"
        r")",
        re.MULTILINE,
    )

    # Find all section boundaries
    boundaries = []
    for match in section_pattern.finditer(text):
        boundaries.append(match.start())

    if not boundaries:
        # No sections detected — fall back to paragraph splitting
        return _split_text_by_paragraphs(text, max_size)

    # Split at section boundaries
    sections = []
    for i, start in enumerate(boundaries):
        end = boundaries[i + 1] if i + 1 < len(boundaries) else len(text)
        section_text = text[start:end].strip()
        if section_text:
            # Extract label from first line
            first_line = section_text.split("\n")[0].strip()
            label = _slugify_section_label(first_line)
            sections.append((label, section_text))

    # Handle text before first section
    if boundaries[0] > 0:
        preamble = text[: boundaries[0]].strip()
        if preamble:
            sections.insert(0, ("preamble", preamble))

    # Merge small adjacent sections to avoid too-tiny chunks
    merged = _merge_small_sections(sections, max_size)

    # If any section is still too large, split by paragraphs
    final = []
    for label, section_text in merged:
        if len(section_text) > max_size:
            sub_parts = _split_text_by_paragraphs(section_text, max_size)
            for i, (_, part_text) in enumerate(sub_parts):
                part_label = f"{label}_{i}" if i > 0 else label
                final.append((part_label, part_text))
        else:
            final.append((label, section_text))

    return final


def _split_text_by_paragraphs(text: str, max_size: int) -> list[tuple[str, str]]:
    """Split text at double-newline paragraph boundaries.

    Falls back to hard split if paragraphs are too large.
    """
    paragraphs = text.split("\n\n")
    chunks = []
    current_chunk = ""
    chunk_idx = 0

    for para in paragraphs:
        if current_chunk and len(current_chunk) + len(para) + 2 > max_size:
            # Flush current chunk
            chunks.append((f"part_{chunk_idx}", current_chunk.strip()))
            chunk_idx += 1
            current_chunk = para
        else:
            current_chunk = current_chunk + "\n\n" + para if current_chunk else para

    if current_chunk.strip():
        chunks.append((f"part_{chunk_idx}", current_chunk.strip()))

    # Handle case where a single paragraph exceeds max_size
    final = []
    for label, chunk_text in chunks:
        if len(chunk_text) > max_size:
            # Hard split at newline boundaries
            lines = chunk_text.split("\n")
            sub_chunk = ""
            sub_idx = 0
            for line in lines:
                if sub_chunk and len(sub_chunk) + len(line) + 1 > max_size:
                    final.append((f"{label}_{sub_idx}", sub_chunk.strip()))
                    sub_idx += 1
                    sub_chunk = line
                else:
                    sub_chunk = sub_chunk + "\n" + line if sub_chunk else line
            if sub_chunk.strip():
                final.append((f"{label}_{sub_idx}", sub_chunk.strip()))
        else:
            final.append((label, chunk_text))

    return final


def _merge_small_sections(sections: list[tuple[str, str]], max_size: int) -> list[tuple[str, str]]:
    """Merge adjacent small sections to avoid too-tiny chunks.

    Sections smaller than max_size/4 are merged with the next section.
    """
    min_section_size = max_size // 4
    merged = []
    current_label = ""
    current_text = ""

    for label, text in sections:
        if not current_text:
            current_label = label
            current_text = text
        elif len(current_text) < min_section_size and len(current_text) + len(text) + 2 <= max_size:
            # Merge small section with current
            current_text = current_text + "\n\n" + text
        else:
            merged.append((current_label, current_text))
            current_label = label
            current_text = text

    if current_text:
        merged.append((current_label, current_text))

    return merged


def _slugify_section_label(header: str) -> str:
    """Convert a section header to a slug for use in chunk IDs."""
    import re

    # Remove markdown formatting
    label = re.sub(r"[#*`]", "", header)
    # Remove trailing colon
    label = label.rstrip(":")
    # Remove parenthetical content
    label = re.sub(r"\([^)]*\)", "", label)
    # Lowercase, replace spaces/special chars with underscore
    label = re.sub(r"[^a-z0-9]+", "_", label.lower().strip())
    # Trim underscores and truncate
    label = label.strip("_")[:40]
    return label or "section"


def _build_header(obj_kind: str, obj_label: str, obj_id: str, global_id: str) -> str:
    """Build chunk header with kind, label and ID variants.

    For internal kinds (__TextBlock, __Document, etc.) - skip kind prefix.
    For normal objects - include kind and ID variants for FTS.
    """
    # Skip kind prefix for internal types
    if obj_kind.startswith("__"):
        # For __TextBlock just use the content, no header
        return ""

    # Build header with ID variants for better FTS matching
    id_variants = _extract_id_variants(global_id, obj_id)

    header = f"{obj_kind}: {obj_label}" if obj_label else obj_kind
    header += f"\nID: {id_variants}"

    return header


def _get_namespace_context(global_id: str) -> str:
    """Extract namespace path as context.

    For "ns1::ns2::obj" returns "ns1 > ns2" for context.
    """
    parts = global_id.split("::")
    if len(parts) > 1:
        # Skip last part (object itself)
        ns_parts = [p for p in parts[:-1] if p]
        if ns_parts:
            return f"[{' > '.join(ns_parts)}]"
    return ""


def extract_object_chunks(
    obj: dict[str, Any],
    config: ChunkingConfig,
) -> list[dict[str, Any]]:
    """Extract chunks from a single QMD.md object.

    Algorithm:
    1. Collect text fields (excluding __ metadata)
    2. Split into long (>=threshold) and short (<threshold) fields
    3. If long fields exist:
       - Each long field -> child chunk
       - Create parent chunk with metadata
    4. If only short fields:
       - Create single combined chunk
    5. Add namespace context and ID variants for discoverability

    Args:
        obj: QMD.md object dict with __id, __kind, __label, __file, etc.
        config: Chunking configuration.

    Returns:
        List of chunk dicts.
    """
    obj_id = obj.get("__id", "")
    obj_kind = obj.get("__kind", "")
    obj_label = obj.get("__label", "")
    source_file = obj.get("__file", "")
    global_id = obj.get("__global_id", f"::{obj_id}")

    # For system types with auto-generated IDs (text_0, doc_xxx), the global_id
    # can collide across files in the same namespace. Include the file path
    # to make chunk IDs unique.
    if obj_kind.startswith("__") and source_file:
        file_slug = source_file.replace("/", "_").replace(".qmd.md", "")
        chunk_base_id = f"{global_id}@{file_slug}"
    else:
        chunk_base_id = global_id

    text_fields = _collect_text_fields(obj)
    if not text_fields:
        return []

    # Get context parts
    header = _build_header(obj_kind, obj_label, obj_id, global_id)
    ns_context = _get_namespace_context(global_id)

    # Split by field length
    long_fields = {k: v for k, v in text_fields.items() if len(v) >= config.long_field_threshold}
    short_fields = {k: v for k, v in text_fields.items() if len(v) < config.long_field_threshold}

    chunks = []

    if long_fields:
        # Create child chunks for long fields
        child_ids = []
        for field_name, field_value in long_fields.items():
            child_id = f"{chunk_base_id}:{field_name}"

            # For content field (especially in __TextBlock), use value directly
            child_text = (
                field_value
                if field_name in ("content", "__description")
                else f"{field_name}: {field_value}"
            )

            # Add context for better discoverability
            if ns_context:
                child_text = f"{ns_context}\n{child_text}"

            if len(child_text) < config.min_text_length:
                continue

            # Split large child chunks at section boundaries
            if len(child_text) > config.max_chunk_size:
                sections = _split_text_by_sections(child_text, config.max_chunk_size)
                if len(sections) > 1:
                    for section_label, section_text in sections:
                        section_chunk_id = (
                            f"{child_id}:{section_label}" if section_label else child_id
                        )
                        if len(section_text) >= config.min_text_length:
                            chunks.append(
                                _create_chunk(
                                    chunk_id=section_chunk_id,
                                    object_id=global_id,
                                    object_kind=obj_kind,
                                    chunk_type="child",
                                    source_file=source_file,
                                    text=section_text,
                                    parent_chunk_id=chunk_base_id,
                                )
                            )
                            child_ids.append(section_chunk_id)
                else:
                    # Splitting didn't help — store as-is
                    chunks.append(
                        _create_chunk(
                            chunk_id=child_id,
                            object_id=global_id,
                            object_kind=obj_kind,
                            chunk_type="child",
                            source_file=source_file,
                            text=child_text,
                            parent_chunk_id=chunk_base_id,
                        )
                    )
                    child_ids.append(child_id)
            else:
                chunks.append(
                    _create_chunk(
                        chunk_id=child_id,
                        object_id=global_id,
                        object_kind=obj_kind,
                        chunk_type="child",
                        source_file=source_file,
                        text=child_text,
                        parent_chunk_id=chunk_base_id,
                    )
                )
                child_ids.append(child_id)

        # Create parent chunk with summary
        parent_parts = []
        if header:
            parent_parts.append(header)
        if ns_context:
            parent_parts.append(ns_context)
        if short_fields:
            parent_parts.append("\n".join(f"{k}: {v}" for k, v in short_fields.items()))

        parent_text = "\n".join(parent_parts)

        if len(parent_text) >= config.min_text_length:
            chunks.append(
                _create_chunk(
                    chunk_id=chunk_base_id,
                    object_id=global_id,
                    object_kind=obj_kind,
                    chunk_type="parent",
                    source_file=source_file,
                    text=parent_text,
                )
            )
    else:
        # Only short fields - create combined chunk
        combined_parts = []
        if header:
            combined_parts.append(header)
        if ns_context:
            combined_parts.append(ns_context)
        if short_fields:
            # For __TextBlock with only "content" field - use it directly
            if obj_kind == "__TextBlock" and list(short_fields.keys()) == ["content"]:
                combined_parts.append(short_fields["content"])
            else:
                combined_parts.append("\n".join(f"{k}: {v}" for k, v in short_fields.items()))

        combined_text = "\n".join(combined_parts)

        if len(combined_text) >= config.min_text_length:
            chunks.append(
                _create_chunk(
                    chunk_id=chunk_base_id,
                    object_id=global_id,
                    object_kind=obj_kind,
                    chunk_type="combined",
                    source_file=source_file,
                    text=combined_text,
                )
            )

    return chunks


def extract_chunks(
    workspace_path: Path | str,
    config: ChunkingConfig,
) -> list[dict[str, Any]]:
    """Extract all chunks from a QMDC workspace.

    Args:
        workspace_path: Path to workspace directory.
        config: Chunking configuration.

    Returns:
        List of all chunk dicts from the workspace.
    """
    # Import here to avoid circular dependency
    from qmdc import parse_workspace

    workspace_path = Path(workspace_path)
    result = parse_workspace(str(workspace_path))

    # Enrich objects with __global_id (Python parser doesn't add it)
    objects = enrich_objects_with_global_id(result.objects)

    all_chunks = []
    for obj in objects:
        obj_chunks = extract_object_chunks(obj, config)
        all_chunks.extend(obj_chunks)

    return all_chunks
