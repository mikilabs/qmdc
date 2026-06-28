# Parent Object [[parent_obj: Container]]

A main object with a Kind that contains a comment.

- name: Parent

## Comment Section

This is a comment without [[id]] inside the parent_obj object.
A comment is created for a level-2 heading without an explicit ID.

### Nested Object [[nested_step: Step]]

This is an attempt to create an object with a Kind (Step) inside a comment.
Question: should this be an error or a valid object?

- order: 1
- description: Nested step description

### Another Nested [[nested_item: Item]]

Another object with a Kind inside a comment.

- value: 42




