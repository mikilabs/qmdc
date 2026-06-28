# Static Site Generator — Documentation Website Builder [[ssg:__Namespace]]

- commands: [[#ssg_cmd_init]], [[#ssg_cmd_build]], [[#ssg_cmd_serve]], [[#ssg_cmd_regenerate]]

`qmdc-mkdocs` builds documentation sites from QMDC workspaces using the MkDocs Material theme. It pre-renders QMD.md files (`.qmd.md`) into standard Markdown, then runs MkDocs to produce a deployable static site — a documentation website you can host anywhere — adding QMDC-specific features on top of Material.

## Overview [[ssg_overview: NarrativeDoc]]

- about: [[#ssg_cmd_build]]

### How It Works [[ssg_how_it_works: text]]

The tool uses the `qmdc` Python parser as a library — it parses the workspace with `parse_workspace`, loads objects and edges into an in-memory SQLite database (`QmdcDatabase`), and queries that locally for reference resolution, the graph sidebar, and dynamic SQL blocks. There is no dependency on a `qmdc` binary.

A build runs entirely in a temporary directory and leaves only the final HTML: `init` → convert (internal stage) → `mkdocs build` → move HTML to `_site/` → clean up. The only file added to the workspace is `mkdocs.yml` (generated once, never overwritten) plus the output directory.

### Features [[ssg_features: text]]

- **Graph sidebar** — per-page breadcrumb, "links to", "linked from", and sibling navigation derived from the workspace reference graph (replaces Material's right-hand TOC).
- **Semantic hints** — popovers on object headings showing similar objects, from pre-computed embeddings in `.qmdc-semantic/hints.json`.
- **Dynamic SQL blocks** — ` ```table ` fenced blocks are executed against the workspace database and rendered as Markdown tables.
- **Reference links** — `[[#id]]` references become clickable links to the target object's page and anchor.
- **Full-text search** — provided by MkDocs Material's built-in search plugin.

### Plugin [[ssg_plugin: text]]

The MkDocs plugin is registered as `qmdc`. Enable QMDC features by adding it to `plugins` in `mkdocs.yml`:

```yaml
plugins:
  - search
  - qmdc
```

The plugin injects its CSS/JS automatically (`on_config`) and passes the graph sidebar and semantic-hint data to the templates. Build-time settings (`docs_dir`, `site_dir`, `theme.custom_dir`) are injected automatically during `build`/`serve`.

### Custom Theme [[ssg_custom_theme: text]]

You can brand the generated site without forking the theme, on two levels.

**1. `mkdocs.yml`** — the workspace `mkdocs.yml` is passed through verbatim, so any MkDocs Material theming works out of the box: `theme.palette` (including `primary: custom` / `accent: custom`), `theme.font`, `theme.logo`, `theme.favicon`, light/dark toggle, `extra_css`, and `extra_javascript`. This already covers most branding — colors, fonts, and logo.

**2. `.mkdocs_theme/`** — an optional directory in the workspace root whose contents are overlaid onto the theme `custom_dir` at build time. Use it to ship the asset files your `mkdocs.yml` references (e.g. a `css/brand.css` for `extra_css`), or to add extra partials and icons:

```text
<workspace>/
  .mkdocs_theme/
    css/brand.css        # then: extra_css: [css/brand.css] in mkdocs.yml
    js/brand.js          # then: extra_javascript: [js/brand.js]
    .icons/mybrand/…      # custom icons for theme.logo / theme.icon
  mkdocs.yml
```

**Safe precedence:** the plugin's own templates (`main.html`, `partials/`, `css/qmdc-*`, `js/`) are always applied last and win on name conflicts. You can freely add files, but you cannot replace the plugin's own files — so QMDC features (graph sidebar, Mermaid, hint popovers) can never be broken by a theme override. The directory is dot-prefixed, so it is never picked up as site content.

A common branding recipe — a custom accent color defined against Material's color variables, placed in `.mkdocs_theme/css/brand.css` and wired up via `extra_css` plus `palette.primary: custom` / `accent: custom`.

## init [[ssg_cmd_init: Command]]

- usage: `qmdc-mkdocs -w <workspace> init`

Generate `mkdocs.yml` (only if absent) and write a reference `nav.yml` to the workspace root.

## build [[ssg_cmd_build: Command]]

- usage: `qmdc-mkdocs -w <workspace> build`

Full pipeline: init → convert → `mkdocs build`, then move the HTML into the output directory (`--output`, default `<workspace>/_site`).

## serve [[ssg_cmd_serve: Command]]

- usage: `qmdc-mkdocs -w <workspace> serve --port 8000`

Same as build but runs `mkdocs serve` with live reload. `--host` defaults to `127.0.0.1`.

## regenerate [[ssg_cmd_regenerate: Command]]

- usage: `qmdc-mkdocs -w <workspace> regenerate`

Regenerate `ContentGenerator` targets (agent-generated content config) whose source objects changed. This is an authoring tool, not part of a normal site build.

## Excluding Files [[ssg_ignore: text]]

- about: [[#ssg_cmd_build]]

Create a `.qmdc-mkdocs.ignore` file (gitignore-style, one pattern per line) in the workspace root or any parent directory to exclude files from the site. Ignored files are excluded both as pages and from graph-sidebar edges.

```gitignore
tracking/**
*.sop.qmd.md
```

## Namespace Builds [[ssg_namespace_builds: text]]

- about: [[#ssg_cmd_build]]

Pointing `--workspace` at a namespace directory within a workspace (rather than the root) builds only that namespace. The workspace root is found automatically so the full reference graph is still available for resolution.
