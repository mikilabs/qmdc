# Storage Tables

## Users Table [[users: Table]]

- description: Main users table
- name: users
- columns: [id, email, name, created_at]

## Orders Table [[orders: Table]]

- description: Customer orders
- name: orders
- columns: [id, user_id, total, status, created_at]
- user_ref: [[#users]]

## Products Table [[products: Table]]

- description: Product catalog
- name: products
- columns: [id, name, price, stock]

## Order Items [[order_items: Table]]

- description: Items in orders (many-to-many)
- name: order_items
- columns: [id, order_id, product_id, quantity, price]
- order_ref: [[#orders]]
- product_ref: [[#products]]

