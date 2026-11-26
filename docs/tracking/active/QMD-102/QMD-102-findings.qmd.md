# QMD-102: Findings

## The TOC was built from typed objects and rendered flat [[qmd102_finding_cause: Finding]]

`_get_toc()` in `qmdc-mkdocs/qmdc_mkdocs/graph.py` built the TOC from parser objects of
non-system kinds at level 2 and deeper, so text fields and plain headings were never listed.
`templates/partials/toc.html` rendered that list flat; the stylesheet's `.sb-toc-indent` rule
was never used. On the docs site, 13 of the 145 entries on `main` pointed at an anchor the page
does not have (ContentGenerator headings, which the build turns into a collapsible).

- category: bug
- related_to: [[#qmd102_page_toc]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/graph.py, qmdc-mkdocs/qmdc_mkdocs/templates/partials/toc.html]
- solution: Render the section from MkDocs' page.toc.

## page.toc, with the title skipped and later h1 sections kept [[qmd102_finding_fix: Finding]]

The partial now walks `page.toc` recursively, so entries carry the real anchor, title and
nesting of every rendered heading, and Material's scroll spy marks the current one. As in
Material, the first h1 (the page title) is not listed. Material drops every heading after a
second h1; a rendered example adds one on `format/validation-errors`, so its children are kept
here. Nested lists indent by `.qmdc-sidebar ul.md-nav__list.sb-toc-sub`, which has to beat
the sidebar's `!important` list reset. `_get_toc()`, `GraphContext.toc` and the front-matter
`toc` key had no other reader and were removed with their tests. An object-only TOC mode would
be a new option and is not added. Duplicate heading ids (five `Description` text fields on one
page share `id="description"`) predate this change and are not addressed here.

- category: design
- related_to: [[#qmd102_page_toc]]
- solution: Recursive page.toc loop in `partials/toc.html`, one CSS rule, dead TOC data removed; build and template tests in `tests/test_graph.py`.
