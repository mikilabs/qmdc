# QMD-94: In-workspace images are missing from the built site

## Media under a non-dot path is never copied into docs_dir [[qmd94_inplace_media_missing: Bug]]

GitHub [#46](https://github.com/mikilabs/qmdc/issues/46). An image stored next to its page, e.g.
`![chart](wide.png)`, is missing from the site built by `qmdc-mkdocs build`. MkDocs warns that
the target is not found and the page shows a broken image. The same file under a dot directory
works, because that path is copied to `assets/qmdc/`.

- status: done_review
- priority: high
- category: mkdocs
- findings: [[#qmd94_finding_cause]], [[#qmd94_finding_fix]]
- result: [[#qmd94_result]]

### Goals [[goals: [Goal]]]

#### A1: Tests reproduce the missing file [[qmd94_goal_a1]]

Converter tests with an image beside the page, in a subdirectory, and in a single-namespace
build fail on the current code: the file is not in the built `docs/`.

- group: A_tests
- done: true

#### B1: In-workspace media reaches docs_dir with its reference unchanged [[qmd94_goal_b1]]

The file is copied to the same relative path under `docs_dir`, so the reference and the URL stay
as written. The issue's reproduction builds with the image present and no MkDocs warning.

- group: B_fix
- done: true
