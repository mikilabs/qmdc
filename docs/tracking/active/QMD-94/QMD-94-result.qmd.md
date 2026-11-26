# QMD-94: Result

## In-workspace media is copied into the built site [[qmd94_result: Result]]

A referenced image or video inside the workspace under a non-dot path is copied to the same
relative path under `docs_dir`, with the namespace prefix stripped in a single-namespace build.
The reference stays as written, so the URL is unchanged.

- feature: [[#qmd94_inplace_media_missing]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/converter.py]
- tests_changed: [qmdc-mkdocs/tests/test_converter.py]

### Verification [[qmd94_result_verification: text]]

- about: [[#qmd94_result]]

- `test_inplace_servable_asset_left_untouched` (now also checks the file is in `docs/`),
  `test_sibling_image_copied_next_to_page` and `test_inplace_asset_in_namespace_build` fail on
  the old code (file not found in `docs/`) and pass after the fix.
- The issue's reproduction with `qmdc-mkdocs init` and `build`: `_site/wide.png` exists,
  `figure/index.html` has `<img src="../wide.png">`, and MkDocs prints no missing-target warning.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
