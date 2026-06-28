# qmdc-mkdocs

Build documentation sites from [QMDC](https://qmdc.mikilabs.io/) workspaces using the
[MkDocs Material](https://squidfunk.github.io/mkdocs-material/) theme.

`qmdc-mkdocs` pre-renders QMD.md files (`.qmd.md`) into standard Markdown, then runs
MkDocs to produce a deployable static site. On top of Material it adds QMDC-specific
features:

- **Graph sidebar** — per-page breadcrumb, "links to", "linked from", and sibling
  navigation derived from the workspace reference graph.
- **Semantic hints** — popovers on object headings showing similar objects, from
  pre-computed embeddings in `.qmdc-semantic/hints.json`.
- **Dynamic SQL blocks** — ` ```table ` fenced blocks are executed against the
  workspace database and rendered as Markdown tables.
- **Reference links** — `[[#id]]` references become clickable links to the target
  object's page and anchor.
- **Mermaid diagrams** — ` ```mermaid ` fences render at natural (readable) size
  and get a zoom/pan viewport with a toolbar (fit-to-width, actual size, zoom
  in/out; Ctrl/Cmd+wheel and drag to pan). Enabled automatically by the plugin —
  no `markdown_extensions` config required. Mermaid is **self-hosted** (vendored
  into the site, no third-party CDN), so diagrams render offline / behind a CSP,
  and the theme follows Material's light/dark palette. The renderer core is
  shared verbatim with the VS Code preview (see `make mermaid-sync`).

Full-text search is provided by MkDocs Material's built-in search plugin.

## How it works

The tool uses the `qmdc` Python parser **as a library** (no `qmdc` binary): it parses
the workspace with `qmdc.workspace.parse_workspace`, loads objects and edges into an
in-memory SQLite database (`qmdc.db.QmdcDatabase`), and queries that locally for
reference resolution, the graph sidebar, and SQL blocks.

A build runs entirely in a temporary directory and leaves only the final HTML:

```text
init → convert (internal) → mkdocs build → move HTML to _site/ → clean up tmp
```

The only file added to your workspace is `mkdocs.yml` (generated once, never
overwritten) plus the output directory.

## Install

```bash
# From the monorepo root, with uv:
uv pip install -e ./qmdc-mkdocs

# Dev dependencies (pytest, ruff):
cd qmdc-mkdocs && uv sync --extra dev
```

Requires Python 3.12+. Runtime dependencies: `qmdc`, `mkdocs>=1.6`,
`mkdocs-material>=9.5`, `click`, `pyyaml`.

## CLI

```bash
qmdc-mkdocs -w <workspace> <command>
```

`--workspace/-w` (default `.`) is the QMDC workspace root (a directory whose
`readme.qmd.md` declares `[[id: __Workspace]]`), or a namespace directory within
one — in which case the root is found automatically and only that namespace is built.
`--output/-o` (default `<workspace>/_site`) sets the output directory.

| Command | What it does |
|---------|--------------|
| `init` | Generate `mkdocs.yml` (only if absent) and write a reference `nav.yml` to the workspace root. |
| `build` | Full pipeline: init → convert → `mkdocs build`, then move the HTML into the output directory. |
| `serve` | Same as build but runs `mkdocs serve` with live reload. `--port` (default 8000), `--host` (default `127.0.0.1`). |
| `regenerate` | Regenerate `ContentGenerator` targets whose source objects changed. |

Examples:

```bash
qmdc-mkdocs -w ./docs build              # build ./docs → ./docs/_site
qmdc-mkdocs -w ./docs -o /tmp/site build # custom output dir
qmdc-mkdocs -w ./docs serve --port 8800  # live preview
qmdc-mkdocs -w ./docs/storage build      # build only the `storage` namespace
```

From the monorepo `Makefile`:

```bash
make mkdocs WS=./docs          # build
make mkdocs-serve WS=./docs    # serve (PORT=8800 by default)
make mkdocs-test                # run this package's test suite
```

## Plugin

The MkDocs plugin is registered as `qmdc`. To enable QMDC features in your own
`mkdocs.yml`, just add it to `plugins`:

```yaml
plugins:
  - search
  - qmdc
```

The plugin injects its CSS/JS automatically (via `on_config`) and passes the graph
sidebar and semantic-hint data to the templates — you do not need to add anything to
`extra_css`/`extra_javascript`. All other build-time settings (`docs_dir`,
`site_dir`, `theme.custom_dir`) are injected automatically during `build`/`serve`.

### `hidden_kinds`

To hide objects of certain kinds from the graph sidebar edges, add to the qmdc plugin
config in `mkdocs.yml`:

```yaml
plugins:
  - qmdc:
      hidden_kinds: [Note, Draft]
```

## Excluding files

Create a `.qmdc-mkdocs.ignore` file (gitignore-style, one pattern per line) in the
workspace root or any parent directory to exclude files from the site:

```gitignore
# Exclude an entire namespace
tracking/**
# Exclude SOP files everywhere
*.sop.qmd.md
```

Ignored files are excluded both as pages and from graph-sidebar edges.

## Custom pages

- A plain `readme.md` next to a `readme.qmd.md` takes priority and is copied
  unchanged as the directory index (no QMDC transformation).
- Files under a `_pages/` directory are copied into the build unchanged, so you can
  reference them from your `nav:`.

## Custom theme / branding

Two layers, in increasing order of effort:

1. **`mkdocs.yml`** — the workspace `mkdocs.yml` is passed through verbatim, so any
   [MkDocs Material](https://squidfunk.github.io/mkdocs-material/setup/changing-the-colors/)
   theming works out of the box: `theme.palette` (including `primary: custom`),
   `theme.font`, `theme.logo`, `theme.favicon`, light/dark toggle, `extra_css`,
   `extra_javascript`. This covers most branding (colors, fonts, logo).

2. **`.mkdocs_theme/`** — an optional directory in the workspace root whose contents
   are overlaid onto the theme `custom_dir` at build time. Use it to ship the asset
   files your `mkdocs.yml` references, or to add extra partials/icons:

   ```text
   <workspace>/
     .mkdocs_theme/
       css/brand.css        # then: extra_css: [css/brand.css] in mkdocs.yml
       js/brand.js          # then: extra_javascript: [js/brand.js]
       .icons/mybrand/…      # custom icons for theme.logo / theme.icon
   mkdocs.yml
   ```

   **Safe precedence:** the plugin's own templates (`main.html`, `partials/`,
   `css/qmdc-*`, `js/`) are always applied *last* and win on name conflicts. You can
   freely **add** files, but you cannot replace the plugin's own files — so QMDC
   features (graph sidebar, Mermaid, hint popovers) can never be broken by a theme
   override. The directory is dot-prefixed, so it is never picked up as site content.

## Development

```bash
cd qmdc-mkdocs
uv run --extra dev pytest -q      # tests (also: `make mkdocs-test`)
uv run --extra dev ruff check .   # lint
```

## License

[AGPL-3.0-or-later](LICENSE) © mikilabs
