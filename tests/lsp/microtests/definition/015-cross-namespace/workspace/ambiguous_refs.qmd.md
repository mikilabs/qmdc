# Ambiguous References Test [[ambiguous_refs_test]]

Test for detecting ambiguous references.

This file at the workspace root (no namespace) references objects that exist in multiple namespaces.

## Consumer [[consumer:Service]]

- name: Consumer Service
- users_table: [[#users]]

The reference `[[#users]]` is ambiguous — there are `alpha:Table:users` and `beta:Table:users`.

## Correct Consumer [[correct_consumer:Service]]

- name: Correct Consumer
- users_table: [[#alpha:users]]

The reference `[[#alpha:users]]` is unambiguous — a namespace is specified.

## Expected Errors

```json
[
  {
    "type": "ambiguous_reference",
    "file": "ambiguous_refs.qmd.md",
    "object": "consumer",
    "field": "users_table",
    "reference": "[[#users]]",
    "candidates": ["alpha:Table:users", "beta:Table:users"],
    "message": "Ambiguous reference, specify namespace or Kind",
    "severity": "error"
  }
]
```

