# QMD-103: Findings

## Labels skip system headings and are keyed by the unstripped path [[qmd103_finding_cause: Finding]]

`generate_nav()` in `qmdc-mkdocs/qmdc_mkdocs/navigation.py` took page labels only from level-1
objects of non-system kinds, so a readme whose level-1 heading is `__Namespace` or
`__Workspace` had no label and `_derive_page_title()` fell back to the file name. The label map
was keyed by the full workspace path, while a namespace build strips the namespace prefix from
`files`, so no page in a namespace build found its label. The docs site's `docs/nav.yml` had 14
`Readme` entries.

- category: bug
- related_to: [[#qmd103_readme_nav_label]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/navigation.py]
- solution: Include the two system kinds that name a readme, and key labels by the nav path.

## Same rule as the graph sidebar; order left alone [[qmd103_finding_fix: Finding]]

The label query now accepts `__Namespace` and `__Workspace`, the rule `_PAGE_OBJECT_FILTER` in
`graph.py` already uses for the sidebar (#41), and strips the namespace prefix from its keys.
When a file has more than one level-1 heading, the first one names it (the query had no order
before). On the docs site only the 14 readme labels change. Ordering pages by label, part 2 of
the issue, would reorder 4 of the 10 multi-page sections of the docs site and the nav of any
site that numbers its file names to order pages, so it is put to the maintainer instead.

- category: design
- related_to: [[#qmd103_readme_nav_label]]
- solution: Label query in `navigation.py`; nav tests in `tests/test_navigation.py`; `docs/nav.yml` regenerated with `init`.
