# UI Test Plan: qmdc-mkdocs

Agent-executable test plan for verifying the rendered site using Playwright.

**Prerequisites:**
- Build the site: `uv run qmdc-mkdocs --workspace ../docs build`
- Serve it: `python -m http.server 8111` from `docs/_site/`
- Or use: `uv run qmdc-mkdocs --workspace ../docs serve --port 8111`
- Base URL: `http://localhost:8111`

---

## 1. Site Structure & Navigation

### 1.1 Homepage loads
- Navigate to `/` (root)
- Page title contains "QMD Documentation v2"
- Left nav sidebar is visible with section groups
- The workspace `readme.qmd.md` is rendered as the homepage (`index.html`)

### 1.2 Left navigation has correct sections
- Sections present: Architecture, VS Code Extension, QMD Format, Guides, Ideas, LSP, Parsers, Semantic, Testing
- NO duplicate section names (e.g. no multiple "Tracking" entries)
- Section titles are clean (no raw `[[id: Kind]]` markers)
- Each section is expandable (click to reveal children)

### 1.3 Navigation links work
- Click "Algorithms" under Architecture → navigates to `/architecture/algorithms/`
- Click "References" under QMD Format → navigates to `/format/references/`
- Click "Rust" under Parsers → navigates to `/parsers/rust-parser/`
- All links produce 200 (no 404s)

### 1.4 Readme pages are section index
- `/architecture/` loads the namespace readme (index.html)
- `/format/` loads the format namespace readme
- `/parsers/` loads the parsers namespace readme
- Root `/` loads the workspace readme

### 1.5 Plain readme.md priority
- If a directory has both `readme.md` and `readme.qmd.md`, the plain `readme.md` is used as `index.md` without QMD transformation
- The `readme.qmd.md` is skipped (not rendered)

### 1.6 .qmdc-mkdocs.ignore respected
- Files matching patterns in `.qmdc-mkdocs.ignore` are NOT converted
- Ignored files do NOT appear in the nav
- Ignored files do NOT appear in graph sidebar (links-to, linked-from, siblings)
- Example: `tracking/**` excludes all tracking pages

---

## 2. Graph Sidebar (Right)

### 2.1 Sidebar renders in correct position
- On `/architecture/algorithms/`: `.md-sidebar--secondary` contains `.qmd-sidebar`
- The sidebar is on the RIGHT side of the content (not inside left nav on desktop)
- Sidebar has `data-pagefind-ignore` attribute

### 2.2 Breadcrumb trail
- On `/architecture/algorithms/`: breadcrumb shows "QMD Documentation v2 › Architecture › Parsing Algorithm"
- On `/format/references/`: breadcrumb shows "QMD Documentation v2 › QMD Format Specification › Reference"
- On `/readme/`: breadcrumb shows "QMD Documentation v2 › [file label]" (no namespace segment)

### 2.3 Page TOC section
- On `/architecture/algorithms/`: "On this page" section lists headings: Parsing Algorithm, Rebuild Algorithm, Workspace Indexing, SQLite Mapping, Reference Resolution
- TOC links have `href="#heading_id"` format
- Clicking a TOC link scrolls to the heading

### 2.4 Links-to section
- On `/architecture/algorithms/`: "Links to" section shows outgoing edges (Parse, Query, Rebuild, Rust Parser, Workspace Parse)
- Each edge item shows label + kind badge (e.g. "Parse Command")
- Edge links use directory-style URLs (e.g. `../../parsers/commands/#cmd_parse` — no `.md` extension)
- Clicking edge links navigates to the correct page (no 404)

### 2.5 Linked-from section
- On `/format/references/`: "Linked from" section shows incoming edges
- Verify at least one incoming edge exists (e.g. from Validation Errors or Workspace)
- NO edges from ignored files (e.g. no tracking items if tracking is in .qmdc-mkdocs.ignore)

### 2.6 Siblings section
- On `/architecture/algorithms/`: "Siblings" shows other files in same directory (Components, Readme)
- Current page is highlighted (bold or different style)
- Sibling links navigate to correct pages

### 2.7 Empty sidebar graceful
- On a page with no outgoing/incoming edges: "Links to" and "Linked from" sections are absent (not empty divs)

---

## 3. Reference Links

### 3.1 Resolved references render as links
- On `/architecture/algorithms/`: field `used_in: [[#parsers:cmd_parse]]` renders as clickable link "Parse"
- The link href points to `../../parsers/commands/#cmd_parse` (or similar relative path)
- Clicking the link navigates to the target page

### 3.2 Cross-namespace references work
- On `/api/endpoints/` (if exists): references to `[[#storage:users]]` render as links to storage pages
- The relative path correctly traverses directories (../storage/...)

### 3.3 Broken / dead references render as spans
- If any broken references exist: they render as `<span class="broken-link">` with the original text
- References whose target object exists but whose page is excluded from the site
  (`.qmdc-mkdocs.ignore`) render as `<span class="broken-link">` with the target
  LABEL (a dead link — no href, no navigation, no MkDocs broken-link warning)
- No Markdown link syntax visible for broken/dead refs

---

## 4. QMD Syntax Transformation

### 4.1 Object headings have ID anchors
- On `/architecture/algorithms/`: heading "Parsing Algorithm" has an element with `id="parsing_algorithm"`
- The ID is inside a `<span class="qmd-id">` (hidden via CSS)

### 4.2 Kind badges render
- On `/architecture/algorithms/`: "Algorithm" badge appears next to "Parsing Algorithm" heading
- Badge has class `qmd-kind` and attribute `data-pagefind-filter="kind"`

### 4.3 System types have no badge
- On `/readme/`: the `__Workspace` heading does NOT show a kind badge
- On `/architecture/readme/`: the `__Namespace` heading does NOT show a kind badge

### 4.4 Code fences are not transformed
- On any page with code examples: `[[id]]` patterns inside ``` blocks remain as plain text
- No `<span class="qmd-id">` inside code blocks

### 4.5 Text field headings
- On `/architecture/algorithms/`: headings like `### Description [[description: text]]` should be transformed the same way as object headings
- The `[[description: text]]` marker should be hidden (inside a `<span class="qmd-id">`)
- The kind badge "text" should appear (since `text` doesn't start with `__`)
- NO raw `[[description: text]]` visible in the rendered heading text

---

## 5. Semantic Hints (Inline)

### 5.1 Hint icons appear inline with headings
- On `/architecture/algorithms/`: 💡 icon appears on the same line as "Workspace Indexing" heading
- Icon is INSIDE the `<h2>` tag (not below it as a separate block)
- Icon is small, subtle, and doesn't disrupt heading readability

### 5.2 Hint popover opens on click
- Click the 💡 icon → a dropdown popover appears below/near the icon
- Popover shows list of similar objects with: label, kind badge, score percentage
- Popover items are clickable links (navigate to target page)

### 5.3 Hint popover closes
- Click outside the popover → it closes
- Click a different 💡 icon → previous popover closes, new one opens

### 5.4 Only objects with hints get icons
- Headings without hint data do NOT have a 💡 icon
- On `/architecture/algorithms/`: "Parsing Algorithm" may or may not have hints (depends on hints.json data)

### 5.5 Hints respect .qmdc-mkdocs.ignore
- Hint targets pointing to ignored files are NOT shown in the popover
- No dead links in hint popovers

---

## 6. Dynamic SQL Blocks

### 6.1 Table blocks render as tables
- On a page with ` ```table ` blocks: they render as HTML tables
- Tables have header row, separator, and data rows
- No raw ` ```table ` fence visible in output
- Note: if tracking is ignored, test on a non-tracking page that has table blocks

### 6.2 Query object references resolve
- If a ` ```table ` block uses `query: [[#query_id]]`: the referenced Query object's SQL is executed
- Result renders as a table

### 6.3 Empty results show message
- If a query returns no rows: "*No results*" is displayed (rendered as italic by MkDocs)

### 6.4 SQL errors show error div
- If a query has invalid SQL: `<div class="sql-error">` is rendered with error message

---

## 7. Mermaid Diagrams

### 7.1 Mermaid renders to SVG
- On `/learn/markdown-to-graph/`: the ` ```mermaid ` block renders to an `<svg>`
- The container is `<div class="mermaid">` (NOT `<pre class="mermaid">`)
- No raw `graph LR ...` source text visible
- Mermaid is self-hosted: the network tab shows `js/mermaid.min.js` loaded from
  the site origin, NOT from a third-party CDN (e.g. unpkg)
- Diagrams still render with the network offline (after first load) / behind a CSP

### 7.2 Zoom/pan viewport + toolbar
- The diagram is wrapped in `.mermaid-viewport`
- A `.mermaid-toolbar` is present with zoom out / label / zoom in / fit / 1:1 buttons
- Toolbar is faint by default, becomes opaque on hover/focus
- `container.dataset.zoomReady === "1"` after render

### 7.3 Zoom/pan interactions
- Clicking `+` / `−` zooms; the percentage label updates
- "Fit to width" scales the diagram to the column; "1:1" shows actual size
- Ctrl/Cmd + wheel zooms toward the cursor (plain wheel scrolls the page)
- When zoomed wider than the column, the viewport gains `.mermaid-pannable` and
  drag / Arrow keys pan horizontally

### 7.4 Follows the Material light/dark palette
- Toggle the Material palette (light ↔ slate/dark): the diagram re-renders with a
  matching mermaid theme (`default` in light, `dark` in slate)
- No light-on-dark (or dark-on-light) diagram after toggling

### 7.4 No console errors
- Rendering a page with a diagram produces no console errors

## 8. Search

### 8.1 Material search works

---

## 9. Styling & Responsiveness

### 9.1 Material design tokens used
- Inspect `.qmd-kind` element: color uses `var(--md-default-fg-color--light)` (no hardcoded hex)
- Inspect `.sb-section-title`: uses Material CSS variables
- No hardcoded `#hex` or `rgb()` values in computed styles of QMD elements

### 9.2 Dark theme compatibility
- If palette toggle exists: switch to light theme
- All QMD elements (sidebar, badges, links) adapt colors automatically
- No invisible text or broken contrast

### 9.3 Responsive layout
- Resize viewport to < 900px width
- Right sidebar disappears (Material hides `.md-sidebar--secondary`)
- Graph sidebar content accessible via left nav drawer (Material's mobile behavior)
- Content area takes full width

### 9.4 Kind badge styling
- `.qmd-kind` renders as small text badge (not a block element)
- Badge is adjacent to the heading text (same line)
- Badge has reduced opacity/lighter color

### 9.5 Broken / dead link styling
- `.broken-link` elements render as a red, dashed-underline link-like span
- They carry NO href and are inert (cursor: not-allowed) — clicking does nothing
- Used for genuinely broken refs AND refs whose target page is excluded from the
  site (e.g. via `.qmdc-mkdocs.ignore`)
- Visually distinct from regular (navigable) links

---

## 10. Page Content Rendering

### 10.1 Markdown renders correctly
- Headings (h1-h6) render with correct hierarchy
- Code blocks have syntax highlighting (Material's highlight.js)
- Lists (ordered and unordered) render correctly
- Bold, italic, inline code render correctly

### 10.2 Front matter not visible
- No `---` YAML front matter block visible at top of any page
- `_graph_sidebar` and `_semantic_hints` metadata not shown in content

### 10.3 File extension mapping
- All internal links use `.md` extension (not `.qmd.md`)
- MkDocs converts `.md` links to proper URLs (directory-style)

---

## 11. Edge Cases

### 11.1 Pages with many objects
- `/format/references/` or `/format/fields/`: pages with many objects render without errors
- All headings have proper ID anchors
- TOC in sidebar lists all headings

### 11.2 Pages with no references
- Pages that have no `[[#ref]]` patterns: render cleanly without broken-link spans
- No empty "Links to" or "Linked from" sections in sidebar

### 11.3 Unicode content
- Pages with Cyrillic text (tracking tasks): render correctly
- No encoding issues in nav titles or page content

### 11.4 Deep nesting
- Pages in deeply nested directories (e.g. `tracking/done/QMD-42/...`): load correctly
- Relative links from deep pages navigate correctly

---

## 12. CLI Behavior

### 12.1 Build produces correct output
- `docs/_site/` directory exists after build
- Contains `assets/` (Material's CSS/JS)
- Contains `css/qmd-extra.css`, `css/qmd-mermaid.css`, `js/qmd-extra.js`, `js/qmd-mermaid.js`
- Contains the self-hosted Mermaid assets: `js/mermaid.min.js` (vendored library)
  and `js/qmd-mermaid-core.js` (shared renderer), loaded by the bootstrap
- Contains `search/` directory (Material's search index)

### 12.2 No intermediate files left
- No `docs/` directory in workspace after build
- No `overrides/` directory in workspace after build
- Only `_site/` and `mkdocs.yml` and `nav.yml` added to workspace

### 12.3 Init idempotent
- Running `init` twice: `mkdocs.yml` not overwritten
- `nav.yml` is updated (reference file, always regenerated)

---

## Execution Notes

- Use Playwright `browser_navigate`, `browser_snapshot`, `browser_evaluate` for verification
- For CSS checks: use `window.getComputedStyle(element)` via `browser_evaluate`
- For element presence: use `document.querySelector` via `browser_evaluate`
- For content checks: use `element.textContent` or `element.innerHTML`
- Take screenshots at key checkpoints for visual verification
- Report PASS/FAIL for each test with brief explanation if FAIL
