## Docs [[docs1: Section]]

- version: 1

### Structure

The project has this layout:

```
docs/
├── readme.qmd.md
├── done/
└── active/
```

### Templates

Use these templates:

| Template | Usage |
|----------|-------|
| Feature | Task description |
| Finding | Technical finding |

### Queries

All features query:

```sql
SELECT __id FROM objects WHERE __kind = 'Feature'
```

- author: admin

