"""Markdown tokenizer wrapper using markdown-it-py."""

from markdown_it import MarkdownIt
from markdown_it.token import Token


def create_tokenizer() -> MarkdownIt:
    """Create and configure markdown-it tokenizer."""
    md: MarkdownIt = MarkdownIt()
    md.enable("table")
    md.options["html"] = True
    return md


def tokenize(markdown: str) -> list[Token]:
    """Tokenize markdown content."""
    md: MarkdownIt = create_tokenizer()
    return md.parse(markdown)
