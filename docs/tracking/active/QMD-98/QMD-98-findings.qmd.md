# QMD-98: Findings

## Marker text is inside the heading MkDocs reads [[qmd98_finding_cause: Finding]]

`_replace_def_with_kind()` and `_replace_bare_id()` in `qmdc-mkdocs/qmdc_mkdocs/syntax.py` put the
marker (`[[question: text]]`, hidden by CSS) and the kind badge text inside the heading. MkDocs'
`toc` extension computes the slug, and the search plugin the title, from that text. The clean id
existed only on the inner span, so the `¶` permalink used the polluted slug. Field headings took
the same path through `_transform_remaining_heading_markers()` in `converter.py`.

- category: bug
- related_to: [[#qmd98_heading_marker_leak]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/syntax.py, qmdc-mkdocs/qmdc_mkdocs/converter.py]
- solution: Give the heading its id with `attr_list`; draw the badge from an attribute.

## Heading id via attr_list, badge via CSS [[qmd98_finding_fix: Finding]]

A test build with plain MkDocs and Material confirmed the issue's suggestion. Markers are removed
from the heading text, the first marker's id becomes the heading's `{ #id }` (`attr_list`, which the
plugin already enables), and the kind badge is an empty `<span class="qmdc-kind"
data-qmdc-kind="…">` whose text CSS draws with `::after { content: attr(data-qmdc-kind) }`.
Ids with `:`, `-`, `_` and `.` work in `{ #id }`; an id that syntax cannot carry keeps an empty
anchor span. Two code paths read the old span shape and were adjusted: hiding content-generator
target headings in `_collapse_content_generators` and placing hint icons in `on_post_page`, which
now also matches an id on the heading tag. `data-pagefind-filter` keeps the kind as an explicit
value. Old slug anchors such as `#question-question-texttext` no longer exist; the object anchors
internal links use are unchanged. The legacy `transform_qmdc_syntax()` is not used by the build
and is unchanged. Headings inside `markdown example` blocks are rendered from example text, are
not parsed objects, and keep their markers.

- category: design
- related_to: [[#qmd98_heading_marker_leak]]
- solution: `_transform_heading_line` / `_kind_badge` in `syntax.py`, one CSS rule, two matching adjustments.
