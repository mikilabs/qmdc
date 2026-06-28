# Broken Link Inline Code Test

Test to verify that references inside inline code (backticks) are not validated as `broken_link`.

## Documentation [[doc_refs]]

Description of reference formats:

**Reference formats:**

1. ``[[#id]]`` - local reference (current namespace)
2. ``[[#Kind:id]]`` - with a Kind (to resolve collisions)
3. ``[[#namespace:id]]`` - another namespace

References inside backticks (`` `[[#id]]` ``) are code examples and must NOT be validated as `broken_link`.

## Valid Reference [[valid_ref]]

- reference: [[#doc_refs]]

This reference is valid and must not raise an error.

