# Queries [[queries: __Namespace]]

- description: SQL queries for architecture reports

## All Tables [[get_tables: Query]]

- description: List of all database tables
- sql: SELECT __id as id, __label as name, json_extract(data, '$.description') as description FROM objects WHERE __kind = 'Table' ORDER BY __label

## Table Columns [[get_columns: Query]]

- description: Columns with data types
- sql: SELECT __id as column_id, json_extract(data, '$.type') as type, json_extract(data, '$.table') as table_ref FROM objects WHERE __kind = 'Column' ORDER BY __id

## Foreign Keys [[get_fk: Query]]

- description: Foreign keys between tables
- sql: SELECT __id as column_id, json_extract(data, '$.references') as references_table FROM objects WHERE __kind = 'Column' AND json_extract(data, '$.references') IS NOT NULL

## Schema Stats [[schema_stats: Query]]

- description: Database schema statistics
- sql: SELECT __kind as type, COUNT(*) as count FROM objects GROUP BY __kind ORDER BY count DESC

---

# 📊 E-Commerce Database Report

Documentation of the e-commerce database structure.

## Tables in the system

Core entities stored in PostgreSQL:

```table
query: [[#get_tables]]
```

## Table structure

Columns and data types:

```table
query: [[#get_columns]]
```

## Relationships between tables

Foreign keys:

```table
query: [[#get_fk]]
```

## Schema statistics

```table
query: [[#schema_stats]]
```

---

## Example: users with orders

Direct SQL for ad-hoc queries:

```table
sql: SELECT o.__id as id, o.__label as name, e.target_id as depends_on FROM objects o LEFT JOIN edges e ON o.__id = e.source_id WHERE o.__kind = 'Table' ORDER BY o.__id
```
