# 🏗 Platform Architecture

## What breaks if you disable Auth?

All services that depend on `auth`:

```table
sql: SELECT e.source_id as service, json_extract(o.data, '$.description') as description, json_extract(o.data, '$.status') as status FROM edges e JOIN objects o ON e.source_id = o.__id WHERE e.target_id = 'auth' AND o.__kind = 'Service'
```

---

## All services by team

```table
sql: SELECT json_extract(data, '$.team') as team, __id as service, json_extract(data, '$.status') as status, json_extract(data, '$.port') as port FROM objects WHERE __kind = 'Service' ORDER BY team, __id
```

---

## Dependency graph

Who depends on whom:

```table
sql: SELECT e.source_id as service, '→' as '', e.target_id as depends_on FROM edges e JOIN objects o ON e.source_id = o.__id WHERE o.__kind = 'Service' ORDER BY e.source_id
```

---

## Production services

```table
sql: SELECT __id as service, json_extract(data, '$.description') as description, json_extract(data, '$.port') as port FROM objects WHERE __kind = 'Service' AND json_extract(data, '$.status') = 'production'
```
