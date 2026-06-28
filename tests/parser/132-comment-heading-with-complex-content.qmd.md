## Solution [[solution1: Finding]]

- status: confirmed

### Part 1: Comment handling

Comments go to `__comments`, fields below go to parent.

Affects all three parsers.

### Part 2: Error handling

Add error type for structured content in textblock.

```json
{
  "type": "structured_in_textblock",
  "line": 17
}
```

Logic:
1. If pending text block is active
2. And TextBlock has content
3. Generate error

### Part 3: Testing

Run all microtests after changes.

- severity: high
