# Edge Cases

## No Preamble [[no_preamble: Case]]

- related: [[#auth]]

### Notes [[no_preamble_notes: text]]

This text field has no preamble at all.
It just references [[#authentication]] inline.

## Invalid Preamble [[invalid_preamble: Case]]

- related: [[#auth]]

### Notes [[invalid_preamble_notes: text]]

- not a reference: just plain text
- about: [[#authentication]]

This has an invalid preamble because the first item is not a reference.
All-or-nothing rule means no typed edges are extracted.
