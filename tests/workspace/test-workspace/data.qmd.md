# Database Schema [[db7: __Namespace]]

## Users [[tbl_users: Table]]

- description: Users table
- schema: public

### id [[col_users_id: Column]]

- type: serial
- primary_key: true
- table: [[#tbl_users]]

### name [[col_users_name: Column]]

- type: varchar(100)
- nullable: false
- table: [[#tbl_users]]

### email [[col_users_email: Column]]

- type: varchar(255)
- nullable: false
- table: [[#tbl_users]]

---

## Orders [[tbl_orders: Table]]

- description: Orders table
- schema: public

### id [[col_orders_id: Column]]

- type: serial
- primary_key: true
- table: [[#tbl_orders]]

### user_id [[col_orders_user_id: Column]]

- type: integer
- references: [[#tbl_users]]
- table: [[#tbl_orders]]

### total [[col_orders_total: Column]]

- type: decimal(10,2)
- table: [[#tbl_orders]]

---

## Products [[tbl_products: Table]]

- description: Product catalog
- schema: public

### id [[col_products_id: Column]]

- type: serial
- primary_key: true
- table: [[#tbl_products]]

### name [[col_products_name: Column]]

- type: varchar(200)
- table: [[#tbl_products]]

### price [[col_products_price: Column]]

- type: decimal(10,2)
- table: [[#tbl_products]]

