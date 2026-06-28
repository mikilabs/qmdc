# Database [[db:__Namespace]]

Project database documentation.

## Users Table [[users:Table]]

User management table.

### Columns Section [[cols:Section]]

User table columns.

#### id Column [[id_col:Column]]

- type: integer
- primary_key: true

#### email Column [[email_col:Column]]

- type: string
- unique: true

### Indexes Section [[indexes:Section]]

Table indexes.

#### Email Index [[email_idx:Index]]

- column: email
- unique: true

## Products Table [[products:Table]]

Product catalog.

### name Column [[name_col:Column]]

- type: string
