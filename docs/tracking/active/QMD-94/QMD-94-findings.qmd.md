# QMD-94: Findings

## MkDocs builds from a temporary docs dir [[qmd94_finding_cause: Finding]]

`build` and `serve` run MkDocs with `docs_dir` set to `tmpdir/docs` (`generate_build_config` in
`config.py`), which `convert_workspace()` fills with converted pages. `_resolve_copy_rewrite()` in
`converter.py` left any file inside the workspace under a non-dot path alone, on the assumption
that MkDocs serves it in place, so it was never copied anywhere. The existing test
`test_inplace_servable_asset_left_untouched` checked only that the reference text was unchanged,
which is why it passed while the image was missing.

- category: bug
- related_to: [[#qmd94_inplace_media_missing]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/converter.py]
- solution: Copy such files into `docs_dir` at the same relative path.

## Same relative path, reference unchanged [[qmd94_finding_fix: Finding]]

Of the issue's two options, copying to the same relative path keeps the reference text and the
URL as written, so pages and readers see no change. The other option, copying every file to
`assets/qmdc/<hash>/`, would rewrite every in-place reference. A single-namespace build strips
the namespace prefix from page paths, so the destination is stripped the same way; a file outside
the namespace being built has no such path and is copied to `assets/qmdc/<hash>/` like dot-path
media. Plain `readme.md` and `_pages/*.md` keep their un-stripped paths, and their media pass
keeps them too.

- category: design
- related_to: [[#qmd94_inplace_media_missing]]
- solution: `namespace_prefix` passed to `_copy_and_rewrite_media` for converted pages; in-place copy in `_resolve_copy_rewrite`.
