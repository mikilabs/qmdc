"""Header parser - extracts __id, __label, __kind from headings."""

import re
import string
from typing import TypedDict

from markdown_it.token import Token


class HeaderResult(TypedDict, total=False):
    """Result of header parsing."""

    id: str
    label: str
    kind: str
    field_type: str  # "array" for [[field: array]]
    array_kind: str  # Kind for [[field: [Kind]]] - object array
    has_explicit_id: bool
    multiple_definitions: list[str]  # Raw [[...]] strings when 2+ definitions found


# Linear congruential generator for deterministic fallback IDs
_rng_state = 666


def set_random_seed(seed: int) -> None:
    """Set random seed for deterministic fallback ID generation."""
    global _rng_state
    _rng_state = seed


def _lcg_next() -> float:
    """Linear congruential generator - same algorithm as TypeScript."""
    global _rng_state
    _rng_state = (_rng_state * 1664525 + 1013904223) % 4294967296
    return _rng_state / 4294967296


def generate_fallback_id() -> str:
    """Generate fallback ID when snake_case returns empty string."""
    # Generate 6-character alphanumeric string using LCG
    chars = string.ascii_lowercase + string.digits
    suffix = "".join(chars[int(_lcg_next() * len(chars))] for _ in range(6))
    return f"object_{suffix}"


def parse_header(tokens: list[Token], start_idx: int = 0) -> HeaderResult | None:
    """
    Parse heading tokens to extract object metadata.

    Patterns:
    - [[id]]                    -> __id=id, __label from text
    - [[id: Kind]]              -> __id=id, __kind=Kind, __label from text
    - [[:Kind]]                 -> __kind=Kind, __id from snake_case(label)
    - Label [[id]]              -> __id=id, __label=Label
    - Label                     -> __id from snake_case(Label), __label=Label

    Returns:
        HeaderResult dict with keys: id, label, kind (optional), or None if parsing fails
    """
    # Find inline token (contains heading content)
    inline_token: Token | None = None

    for i in range(start_idx, min(start_idx + 3, len(tokens))):
        if tokens[i].type == "inline":
            inline_token = tokens[i]
            break

    if not inline_token:
        return None

    content: str = inline_token.content.strip()

    # Extract [[...]] patterns
    # Pattern: [[id]], [[id: Kind]], [[:Kind]], [[]], [[field: [Kind]]]
    # Use balanced matching for nested brackets
    bracket_pattern: str = r"\[\[((?:[^\[\]]|\[[^\]]*\])*)\]\]"

    # Strip backtick-escaped content before matching [[...]] patterns
    # so that `[[id]]` inside backticks is not treated as a definition
    search_content = re.sub(r"`[^`]+`", lambda m: " " * len(m.group(0)), content)
    matches: list[re.Match[str]] = list(re.finditer(bracket_pattern, search_content))

    result: HeaderResult = {}

    if matches:
        # Detect multiple definitions
        if len(matches) > 1:
            raw_defs = [content[m.start() : m.end()] for m in matches]
            result["multiple_definitions"] = raw_defs

        # Remove [[...]] from content to get label (use match positions from search_content)
        label: str = content
        for match in reversed(matches):
            label = label[: match.start()] + label[match.end() :]
        # Clean up multiple spaces and trim
        label = " ".join(label.split()).strip()

        # Parse first [[...]] (extract bracket content from original content using positions)
        bracket_content: str = content[matches[0].start() + 2 : matches[0].end() - 2].strip()

        if ":" in bracket_content:
            # [[id: Kind]] or [[:Kind]] or [[field: array]]
            parts: list[str] = bracket_content.split(":", 1)
            left: str = parts[0].strip()
            right: str = parts[1].strip()

            if right.lower() == "array":
                # [[field: array]] - primitive array
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "array"
            elif right.lower() == "yaml":
                # [[field: yaml]] - YAML block
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "yaml"
            elif right.lower() == "json":
                # [[field: json]] - JSON block
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "json"
            elif right.lower() == "text":
                # [[field: text]] - multiline text field
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "text"
            elif right.lower() == "map":
                # [[field: map]] - key-value map field (str→str)
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "map"
            elif right.startswith("[") and right.endswith("]"):
                # [[field: [Kind]]] - object array
                kind_name = right[1:-1].strip()
                result["id"] = left if left else generate_fallback_id()
                result["field_type"] = "object_array"
                result["array_kind"] = kind_name
            elif left:
                # [[id: Kind]]
                result["id"] = left
                result["kind"] = right
            else:
                # [[:Kind]]
                result["kind"] = right
                # Generate ID from label
                result["id"] = snake_case(label) if label else generate_fallback_id()
        else:
            # [[id]]
            result["id"] = bracket_content if bracket_content else generate_fallback_id()

        # For [[...]] patterns, use label as-is (may be empty)
        result["label"] = label
        result["has_explicit_id"] = True  # [[...]] was present
    else:
        # No [[...]], just plain text - use content for both label and id
        result["label"] = content
        result["id"] = snake_case(content)
        result["has_explicit_id"] = False  # No [[...]] - ID was auto-generated

    return result


def snake_case(text: str) -> str:
    """Convert text to snake_case for auto-generated IDs."""
    # Remove special chars, replace spaces with underscore
    cleaned: str = re.sub(r"[^\w\s-]", "", text)
    cleaned = re.sub(r"[\s-]+", "_", cleaned)
    result: str = cleaned.lower().strip("_")
    # Return fallback ID if result is empty (e.g., heading with only special chars)
    return result if result else generate_fallback_id()
