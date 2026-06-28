# [[users:Table]] Users Table

- columns: id, email, name, created_at
- primary_key: id

# [[orders:Table]] Orders Table

- columns: id, user_id, total, status
- primary_key: id
- foreign_key: user_id -> [[#users]]

# [[products:Table]] Products Table

- columns: id, name, price, stock
- primary_key: id

# [[order_items:Table]] Order Items Table

- columns: id, order_id, product_id, quantity
- primary_key: id
- foreign_keys: order_id -> [[#orders]], product_id -> [[#products]]

