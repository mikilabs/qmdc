# Queries [[queries: __Namespace]]

- description: Example queries for QMD dynamic blocks

## Get All Objects [[get_all_objects: Query]]

- description: Returns all objects in the workspace
- sql: SELECT __id, __kind, __label FROM objects

## Get Tables [[get_tables: Query]]

- description: Returns all Table objects
- sql: SELECT __id, __label, data ->> '$.description' as description FROM objects WHERE __kind = 'Table'

---

# Report

## All Objects

```table
query: [[#get_all_objects]]
```

## Tables Only

```table
query: [[#get_tables]]
columns: [__id, __label, description]
```

## Inline SQL Example

```table
sql: |
 SELECT __kind as k, COUNT(*) as count 
 FROM objects GROUP BY __kind
```
