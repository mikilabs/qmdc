# Document with example code blocks

## Object [[test_obj]]

Some text with a real reference: [[#real_ref]]

### Description [[description: text]]

This is a text field with example code blocks:

```json example
{
  "id": "example",
  "reference": [[#fake_ref_in_example]]
}
```

Regular code block (should parse references):

```json
{
  "id": "regular",
  "reference": [[#real_ref]]
}
```

Another example block:

```markdown example
## User [[user]]
- name: Alice
- profile: [[#user_profile]]
```

More text with real reference: [[#real_ref]]

## Real Reference [[real_ref]]

This is a real object.


