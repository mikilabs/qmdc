## Bug [[bug1: Bug]]

Description of the bug with details.

**Problem:**
- Parser correctly recognizes YAML array `["[[#library:simple_zoe]]"]`
- But extraction calls `extract_references("[[[#library:simple_zoe]]]")`
- Regex finds only outer match `[[[#library:simple_zoe]]]`
- LSP cannot find reference at position 36

**Reproduction:**
1. Create file with field
2. Run LSP and try to find definition

