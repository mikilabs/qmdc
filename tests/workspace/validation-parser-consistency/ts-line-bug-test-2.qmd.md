# TypeScript Line Bug Test 2

Second file with a duplicate object to verify the line number.

## Complex Object [[complex_object_with_underscores]]

- name: Duplicate Complex Object
- description: This should show line: 5 (not line: 1)

The second `complex_object_with_underscores` object is on line 5 and must raise a `duplicate_id` error with `line: 5` in all parsers.




