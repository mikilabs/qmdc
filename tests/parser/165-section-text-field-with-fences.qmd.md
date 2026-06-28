# Spec [[spec1: Section]]

- source: docs

## Definition [[def1: text]]

Dynamic blocks are fenced code blocks with type `table`.

## Syntax [[syntax1: text]]

### Reference

Example:

```markdown
```table
query: [[#example]]
```
```

### Inline

```markdown
```table
sql: SELECT __id FROM objects
```
```

### Scope [[scope1: text]]

Blocks support `scope` parameter:

```table
sql: SELECT __id FROM objects
scope: workspace
```

Without filter:

```table
sql: SELECT __id FROM objects
scope: all
```

## Renderers [[renderers1: text]]

| Type | Description |
|------|-------------|
| `table` | HTML table |
| `diagram` | D2/Mermaid |

Unknown type returns raw YAML.

## Contract [[contract1: text]]

Fenced code blocks go into `__TextBlock.content`.

- related: [[#spec1]]

