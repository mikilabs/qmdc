## Blocks [[blocks1]]

### Syntax [[blocks_syntax: text]]

Blocks support a scope parameter.

With workspace filter:
```table
sql: SELECT __id FROM objects
scope: workspace
```

Without filter:
```table
sql: SELECT __id FROM objects
scope: all
```

### Renderers

| Type | Description |
|------|-------------|
| table | HTML table |
| chart | Data visualization |

- related: [[#workspace]]
