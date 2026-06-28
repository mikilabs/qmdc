## Algorithm Documentation [[algorithm_doc:Algorithm]]

- description: Describes algorithm structure

### Syntax Examples [[syntax:text]]

Here are examples of QMD syntax without backticks:

#### Object Definition

Use [[id:Kind]] to define objects with types.

Example: [[user:User]] creates User object with kind User.
Another example: [[config:Config]] creates Config object.

#### Field Definition

Use [[field_id:text]] for text fields.

Example: [[description:text]] creates multiline text field.
Another: [[notes:text]] creates another text field.

#### References (the correct way)

To reference an object, use hash sign.

Example: [[#user]] is reference to user object.
Another: [[#config]] is reference to config object.

Summary: [[id:Kind]] without hash is NOT a reference!
