# Doc

## Quick Wins [[phase1: Finding]]

Three trivial bugs to fix.

- category: parser
- related_to: [[#task1]]

### Bug 1: heading space

- affected_files: [src/parser.py]
- affected_functions: [rebuild_heading]
- severity: low
- fix_size: trivial

Root cause is a missing space in the format string.

### test_plan

- Existing tests: make test covers all parsers
- New tests needed: no
- How to verify: make test and roundtrip check
