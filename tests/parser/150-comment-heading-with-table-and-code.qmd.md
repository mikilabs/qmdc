# Doc

## Analysis [[analysis1: Finding]]

Found several issues.

- category: parser
- related_to: [[#task1]]

### Current Behavior

When running the parser:
- Python shows real line numbers
- TypeScript always shows line 1

Example output:
```
duplicate_id|file.qmd.md:3|obj1  # Python
duplicate_id|file.qmd.md:1|obj1  # TypeScript
```

### Root Cause

The function tries to find the line by regex pattern:

```typescript
function getLineNumber(content: string): number {
  return 1; // Default
}
```

### Solution

1. Use obj.__line directly
2. Only use getLineNumber() as fallback
3. Improve regex patterns

### Affected Files

- `src/workspace.ts` (lines 228-250)
- `src/parser.ts` (lines 100-120)
