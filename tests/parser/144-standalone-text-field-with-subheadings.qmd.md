# Doc Title

## Result [[result1: Result]]

- task: [[#some_task]]
- completed: 2024-12-26

## Summary [[summary: text]]

Fixed multiple issues in the parser.

### Fixed Problems:

1. Code blocks in text fields now preserved
2. Ordered lists no longer converted to bullet lists
3. Links preserved in Rust parser

### Improvements:

- Added support for markdown_list
- Nested objects now output inline

## Files Changed [[files_changed: array]]

- src/parser.py
- src/rebuild.py
