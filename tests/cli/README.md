# CLI conformance corpus

Data-driven CLI tests shared by all three parsers (`qmdc-py`, `qmdc-ts`, `qmdc-rs`).
Each parser has a thin runner that iterates these fixtures, so the `cli` row of the
unified test report reaches parity by construction.

## Case layout

`tests/cli/<NNN-name>/`:

- `cmd` — the CLI arguments on a single line (e.g. `parse -i input.qmd.md --format minimal`).
  The runner shell-splits on spaces; keep args simple (no embedded spaces/quotes).
- `stdin` — optional; piped to the command's standard input.
- any input files referenced by `cmd` (resolved relative to the case dir; the runner
  executes with the case dir as the working directory).
- `expected.json` — optional; the command's stdout parsed as JSON and compared
  structurally (order-sensitive).
- `expected.txt` — optional; the command's stdout compared as trimmed text.
- `exit` — optional; expected process exit code (default `0`).

A case must have exactly one of `expected.json` / `expected.txt`.

## Command coverage

These cases cover the **common** CLI commands (present in all three parsers): `parse`
(stdin/file/`--format`/`--no-pretty`/`--no-comments`), `rebuild`, `query` (via a
`Query`-object reference to avoid spaces in args), and `workspace validate`.

`workspace parse` is intentionally omitted here: its JSON includes an absolute
`root` path (machine-dependent), so it can't be byte-compared in a committed
fixture. It is exercised by the `workspace` conformance suite, which calls
`workspace parse` internally for all 27 fixtures × 5 aspects.

There is no `lint` command — canonical formatting is `qmdc parse … | qmdc rebuild`.
`lint` and `workspace files` (Python/TypeScript-only helpers) are not part of the
shared corpus and are covered by per-parser unit tests.
