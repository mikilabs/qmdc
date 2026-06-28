---
name: Bug report
about: Report incorrect behavior in a parser, the CLI, or a package
title: "[bug] "
labels: bug
---

## Summary

A clear, one-sentence description of the bug.

## Affected component

- [ ] `qmdc` CLI / parser (which implementation: Python / Rust / TypeScript / all)
- [ ] `qmdc-semantic`
- [ ] `qmdc-mkdocs`
- [ ] `qmdc-vscode`

## Minimal reproduction

A minimal `.qmd.md` fixture and the exact command(s):

```qmd.md
## Example [[example]]

- field: value
```

```bash
qmdc parse -i example.qmd.md
```

## Expected behavior

What you expected to happen.

## Actual behavior

What actually happened. Include the full output / error.

## Environment

- Package version(s):
- OS:
- Python / Node / Rust version (if relevant):

## Parity note

If this is a parser bug, does it reproduce in all three implementations or only one? (If known.)
