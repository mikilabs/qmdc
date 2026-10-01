# QMD-79: Findings

## Measured on the QMD-77 build [[qmd79_finding_measurement: Finding]]

Measured by running all three parsers on one input, an object with a field followed by `Prose one.`,
an HTML comment and `Prose two.` as separate paragraphs.

| parser | `__comments` |
|---|---|
| Rust | `Prose one.` and `Prose two.` |
| Python | `Prose one.\n\n<!-- hidden -->` and `Prose two.` |
| TypeScript | same as Python |

When the HTML comment is the only text after the fields, none of the three emits a comment, so the
split is only between prose and a comment that follows it. Inside a `__TextBlock` the comment is kept
verbatim by all three since QMD-77, because a text block's content is its source region; whether
`__comments` should follow the spec's "ignored" or the text block's verbatim rule is the decision to
make first.

- category: parser
- related_to: [[#qmd79_html_comment_in_comments]]
- solution: Decide between the spec's "ignored" and verbatim; then align the two parsers that differ.
