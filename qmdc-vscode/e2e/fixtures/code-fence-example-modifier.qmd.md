# Example Code Blocks

## Regular content

- ref: [[#some_object]]

## Code Examples

```markdown example
## User [[alice: User]]

- name: Alice
- profile: [[#user_profile]]
```

```json example
{
  "__id": "user",
  "name": "Alice",
  "profile": "[[#user_profile]]"
}
```

```sql example
SELECT * FROM objects WHERE __kind = 'Table'
```
