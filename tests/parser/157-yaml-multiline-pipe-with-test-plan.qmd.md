## Finding [[finding3: Finding]]

- category: testing
- solution: |
    Step 1: Run the parser on input
    Step 2: Compare output with expected
    Step 3: Verify round-trip
- test_plan: |
    **Existing tests:** microtests cover basic parsing
    
    **New tests needed:**
    1. SQL test for code block in text field
    2. Round-trip test for yaml multiline
- severity: high

