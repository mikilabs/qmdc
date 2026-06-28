# Pull Request

## What & why

Briefly describe what this PR changes and the motivation.

Closes #NNN (if applicable).

## Changes

- Describe each notable change here.

## How verified

- [ ] `make test` is green (full sequential suite)
- [ ] `make lint` is clean
- [ ] Docs updated (if behavior or CLI changed)
- [ ] Fixtures added/updated under `tests/` (if behavior changed)

## Parity checklist

For changes to the QMD.md format or parser behavior:

- [ ] Implemented in all three parsers (Python, Rust, TypeScript)
- [ ] Output is byte-identical across implementations
- [ ] Expected files were regenerated from corrected output, not hand-edited to pass
- [ ] Conformance parity holds (the `make test` matrix shows equal per-suite counts)

## Notes for reviewers

Anything that needs special attention, trade-offs, or follow-ups.
