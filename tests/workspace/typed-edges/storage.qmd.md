# Storage

## Orders Database [[orders_db: Database]]

- engine: postgresql
- version: 15

### Overview [[orders_db_overview: text]]

- about: [[#orders_db]]

The orders database stores all order-related data.

## Users Table [[users_tbl: Table]]

- database: [[#orders_db]]

## Orders Table [[orders_tbl: Table]]

- database: [[#orders_db]]
- user_ref: [[#users_tbl]]
