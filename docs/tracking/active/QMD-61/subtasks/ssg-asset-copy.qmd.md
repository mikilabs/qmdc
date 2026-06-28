# QMD-61 · Subtask: SSG asset-copy (images on the built site)

## SSG asset-copy [[qmd61_ssg_asset_copy: Feature]]

Teach the `qmdc-mkdocs` converter to copy static media (images, etc.) from the
workspace into the generated site, so `![](assets/…)` references resolve on
`qmdc.mikilabs.io` instead of 404ing. Blocks the docs media pass tracked in
`docs/tracking/screenshots-temp.qmd.md` — the in-editor VS Code preview already
renders images (done), but the published site does not yet ship them.

- status: done
- priority: medium
- category: docs
- parent_task: [[#qmd61]]

### Problem [[qmd61_ssg_asset_problem: text]]

`qmdc_mkdocs/converter.py::convert_workspace` only emitted three kinds of output
into `tmpdir/docs/`: each `*.qmd.md` → `.md` (transformed), plain `readme.md` →
`index.md`, and `_pages/*.md`. Referenced media were never copied, so an embedded
`![](…/hero.png)` 404'd on the built site. Worse, the real screenshots live in a
**hidden** `docs/.assets/` dir (and may live anywhere — outside the workspace, an
absolute path), which MkDocs ignores (dotfiles) and cannot serve, so in-place
serving is impossible: the file must be copied AND the reference rewritten.

### Fix (implemented) [[qmd61_ssg_asset_fix: text]]

Reference-driven, NOT an extension sweep — only media a page actually references
is shipped. New `_copy_and_rewrite_media` pass in `convert_workspace`, run per page
after the content transforms:

- Scan page content for media refs — Markdown `![alt](url "title")` and HTML
  `<img>`/`<video>`/`<source>` `src="…"` — skipping fenced code so examples aren't touched.
- Skip non-local targets: `http(s)://`, protocol-relative `//`, `data:`, `mailto:`, anchors.
- Resolve each target to an absolute path (`~` expanded; relative to the referencing
  file's dir; absolute paths honoured — so sources OUTSIDE the workspace work).
- **Minimal intervention**: a file already servable in place (inside the workspace,
  no dot-prefixed path part) is left untouched — MkDocs serves it as-is.
- Otherwise copy to `docs/assets/qmdc/<sha1(abs_path)[:12]>/<basename>` (non-dot dir →
  shipped; per-source-path hash subdir → collision-proof; basename preserved for
  readability) and rewrite the reference to that location, relative to the page's
  output (`_compute_relative_path`). De-duped via a shared cache; not counted as pages.
- A referenced file that does NOT exist → `WARN` to stderr, reference left as-is; the
  build proceeds (the intentionally-missing `quickstart-terminal.png` slot does not break it).

### Image sizing + zoom (no source-markdown pollution) [[qmd61_ssg_asset_zoom: text]]

Screenshots are full-resolution, so they're capped on-page and made click-to-enlarge —
without adding any `{ width=… }` attributes to the `.qmd.md` sources (kept portable):

- **Zoom**: the `mkdocs-glightbox` plugin (Material's recommended lightbox) — added to
  `qmdc-mkdocs` deps and enabled by default in generated configs + this repo's
  `docs/mkdocs.yml`. Auto-wraps content images in `<a class="glightbox">`; emoji and
  Mermaid (inline `<svg>`) are unaffected.
- **On-page size**: a CSS rule injected via the SSG (`templates/css/qmdc-extra.css`),
  scoped to `.md-typeset a.glightbox img` (exactly the zoomable screenshots). Sizing
  lives in SSG CSS, never in the source Markdown.

### Tests [[qmd61_ssg_asset_tests: text]]

`qmdc-mkdocs/tests/test_converter.py::TestAssetCopy` (all green):

- image in a hidden `.assets/` dir → copied under `assets/qmdc/<hash>/` and the ref rewritten off the dot-path;
- absolute path OUTSIDE the workspace → copied in and rewritten;
- missing referenced file → no copy, no exception, stderr `WARN` (build proceeds);
- remote `http(s)://` / `data:` URIs → untouched, no copy;
- in-place servable image (non-dot, inside workspace) → left untouched (no needless copy).

Verified end-to-end with a real `make site WS=./docs`: a hidden-dir `hero.png` lands at
`assets/qmdc/<hash>/hero.png`, the `<img>` is rewritten + glightbox-wrapped, and the
intentionally-missing slot only warns. `mkdocs` suite green; `validate docs` stays `[]`.

### Files [[qmd61_ssg_asset_files: text]]

- `qmdc-mkdocs/qmdc_mkdocs/converter.py` — `_copy_and_rewrite_media` + `_resolve_copy_rewrite`, wired into `convert_workspace`.
- `qmdc-mkdocs/qmdc_mkdocs/config.py` — `glightbox` in default plugins.
- `qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css` — screenshot sizing rule.
- `qmdc-mkdocs/pyproject.toml` — `mkdocs-glightbox` dependency.
- `docs/mkdocs.yml` — `glightbox` plugin enabled.
- `qmdc-mkdocs/tests/test_converter.py` — `TestAssetCopy` coverage.
