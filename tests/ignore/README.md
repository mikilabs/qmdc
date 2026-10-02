# `.qmdcignore` acceptance data

`gitignore-matrix.json` pins the meaning of `.qmdcignore` for all three parsers. The spec says
the file works like `.gitignore`, so the expected answers are not typed by hand: they are what
git itself ignores for the same content.

- `tree` — the files that exist in the test tree.
- `cases[].content` — the exact `.qmdcignore` text, byte for byte (CRLF, a byte-order mark and
  trailing spaces are cases of their own).
- `cases[].ignored` — the paths of `tree` that git ignores for that content.

Each parser feeds `content` to its own matcher (`qmdc-rs/src/ignore.rs`,
`qmdc-py/qmdc/ignore.py`, `qmdc-ts/src/ignore.ts`) and must ignore exactly `ignored`. The tests
are `qmdc-rs/tests/ignore_matrix.rs`, `qmdc-py/tests/test_ignore.py` and
`qmdc-ts/tests/test-ignore.ts`; the report counts them as the `ignore` suite, with parity
enforced.

To add a case, append it to `CASES` in `gen_matrix.py` and regenerate from the repository root:

```bash
uv run --no-project python tests/ignore/gen_matrix.py
```

The end-to-end shapes, where the ignore file steers a real workspace scan, live in
`tests/workspace/qmdcignore-gitignore/`.
