## Flat Store [[flat_store: Store]]

A top-level object, so its `__id` has no dots and it carries no `__local_id`.

- engine: sqlite

## System [[system: Doc]]

Nesting builds hierarchical ids. The deepest object's `__id` is `system.data.postgres`
and its `__local_id` is `postgres` — the two id shapes that a reference can name it by.

### Data [[data: Layer]]

- purpose: storage layer

#### Postgres [[postgres: Store]]

- engine: postgres
- port: 5432
