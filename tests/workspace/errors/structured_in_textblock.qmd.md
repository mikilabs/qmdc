# Structured in TextBlock Test [[structured_in_textblock_test]]

Test for detecting an error: a structural element `[[id]]` inside a TextBlock.

## Valid Object [[valid_object: Section]]

- name: Valid

### Valid Field [[valid_field: text]]

This is fine — parent is an object.

## TextBlock Without ID

This is a TextBlock (no `[[id]]`, no fields).

### Invalid Field [[invalid_field: text]]

ERROR: Cannot create structured element inside TextBlock!

## Another TextBlock

Another textblock.

### Another Invalid [[another_invalid]]

- key: value

ERROR: This should also be an error.





