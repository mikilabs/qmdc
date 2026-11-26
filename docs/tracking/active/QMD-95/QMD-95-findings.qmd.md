# QMD-95: Findings

## The page label skips the readme's own heading [[qmd95_finding_cause: Finding]]

`_get_file_label()` and the label query in `_get_siblings()` in `qmdc-mkdocs/qmdc_mkdocs/graph.py`
take the first object with `__kind NOT GLOB '__*'`. On a namespace readme that skips the
`__Namespace` heading, so the first child names the page; on the workspace readme nothing is left
and the label falls back to the file name, `Readme`. `templates/partials/toc.html` always rendered
workspace › namespace › page as `<span>`s, so on the namespace's own readme the namespace was
followed by a page item, and no ancestor was a link. The stylesheet already had `.sb-crumb` link
styles that no template used.

- category: bug
- related_to: [[#qmd95_index_breadcrumb]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/graph.py, qmdc-mkdocs/qmdc_mkdocs/converter.py, qmdc-mkdocs/qmdc_mkdocs/templates/partials/toc.html]
- solution: Count `__Namespace`/`__Workspace` as page objects; mark the current level; link ancestors.

## Scope of the fix [[qmd95_finding_fix: Finding]]

Only `__Namespace` and `__Workspace` join the page-label rule, so ordinary pages whose H1 is a
`__Document` keep their current labels. `GraphContext` gains `current_level` and the output paths
of the workspace and namespace index pages; the front matter turns them into `workspace_href` and
`namespace_href`. An ancestor is linked only when its page is in the built site and is not the
current page: in a single-namespace build the workspace root is not part of the site, the
namespace index path loses the prefix like every page path, and an ignored index page is not
linked. The template falls back to the old rendering when the new keys are absent. The optional
short root label suggested in the issue is left out, since it would be a new `__Workspace` field
or setting.

- category: design
- related_to: [[#qmd95_index_breadcrumb]]
- solution: `_PAGE_OBJECT_FILTER`, `_get_index_file` and `current_level` in `graph.py`; hrefs in `_build_front_matter`; links in `toc.html`.
