"""Pygments lexer for QMD.md format.

Patterns are derived from the TextMate grammar (qmdc-vscode/syntaxes/qmdc.tmLanguage.json)
to maintain a single source of truth for QMD.md syntax highlighting.

The lexer highlights:
- Headings with [[id: Kind]] definitions
- Field definitions (- key: value)
- References ([[#id]])
- YAML values (booleans, numbers, null)
- Code fences (delegated to sub-lexers)
"""

from __future__ import annotations

from pygments.lexer import RegexLexer, bygroups
from pygments.token import (
    Comment,
    Generic,
    Keyword,
    Name,
    Number,
    Punctuation,
    String,
    Text,
)


class QmdcLexer(RegexLexer):
    """Pygments lexer for QMDC (structured data in Markdown)."""

    name = "QMD.md"
    aliases = ["qmd.md", "qmdmd"]
    filenames = ["*.qmd.md"]
    mimetypes = ["text/x-qmdmd"]

    tokens = {
        "root": [
            # HTML comments
            (r"<!--[\s\S]*?-->", Comment.Multiline),
            # Code fences (must come before headings to avoid matching # inside code)
            (r"^(\s*)(```+)(\w*)\s*$", bygroups(Text, Punctuation, Name.Label), "code-fence"),
            # Headings with [[id: Kind]] or [[id]] or [[:Kind]]
            (
                r"^(#{1,6}\s+)(.*?)(\[\[)(:?)([^\]:#]*)(:?\s*[^\]]*?)(\]\])(.*?)$",
                bygroups(
                    Generic.Heading,       # ##
                    Generic.Heading,       # title text before [[
                    Punctuation,           # [[
                    Keyword,               # : (auto-id)
                    Name.Tag,              # id
                    Name.Class,            # :Kind
                    Punctuation,           # ]]
                    Generic.Heading,       # trailing text
                ),
            ),
            # Plain headings (no [[]])
            (r"^(#{1,6}\s+)(.+)$", bygroups(Generic.Heading, Generic.Heading)),
            # References [[#id]] or [[#ns:Kind:id]]
            (
                r"(\[\[)(#)([^\]]+)(\]\])",
                bygroups(Punctuation, Keyword, Name.Function, Punctuation),
            ),
            # Field definitions: - key: value
            (
                r"^(\s*-\s+)([a-zA-Z_][a-zA-Z0-9_]*)(:)\s*",
                bygroups(Punctuation, Keyword.Declaration, Punctuation),
                "field-value",
            ),
            # Plain list items (no key:)
            (r"^(\s*-\s+)(.*)", bygroups(Punctuation, Text)),
            # Everything else
            (r".", Text),
            (r"\n", Text),
        ],
        "field-value": [
            # References in values
            (
                r"(\[\[)(#)([^\]]+)(\]\])",
                bygroups(Punctuation, Keyword, Name.Function, Punctuation),
            ),
            # YAML array
            (r"\[", Punctuation, "yaml-array"),
            # Boolean
            (r"\b(true|false)\b", Keyword.Constant),
            # Null
            (r"\bnull\b", Keyword.Constant),
            # Numbers
            (r"-?\b\d+(\.\d+)?([eE][+-]?\d+)?\b", Number),
            # Quoted strings
            (r'"[^"]*"', String.Double),
            (r"'[^']*'", String.Single),
            # Pipe for multiline
            (r"\|", Punctuation),
            # Rest of line
            (r"[^\n\[\]]+", String),
            (r"\n", Text, "#pop"),
        ],
        "yaml-array": [
            (r"\]", Punctuation, "#pop"),
            (r",", Punctuation),
            (
                r"(\[\[)(#)([^\]]+)(\]\])",
                bygroups(Punctuation, Keyword, Name.Function, Punctuation),
            ),
            (r"\b(true|false|null)\b", Keyword.Constant),
            (r"-?\b\d+(\.\d+)?\b", Number),
            (r'"[^"]*"', String.Double),
            (r"'[^']*'", String.Single),
            (r"[^\],\[\]]+", String),
        ],
        "code-fence": [
            (r"^(\s*)(```+)\s*$", bygroups(Text, Punctuation), "#pop"),
            (r".*\n", Text),
        ],
    }
