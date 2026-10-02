## System [[system: Doc]]

Nesting builds hierarchical ids: `system` -> `system.data` -> `system.data.postgres`. The
deepest object's `__local_id` is `postgres`, which is what makes the leaf form in the
consumer meaningful.

### Data [[data: Layer]]

- purpose: storage layer

#### Postgres [[postgres: Store]]

- engine: postgres
- port: 5432
