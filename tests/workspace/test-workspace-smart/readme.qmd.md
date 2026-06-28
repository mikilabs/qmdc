# Test Workspace [[test_ws: __Workspace]]

## System Purpose [[system_purpose: SystemPurpose]]

- description: Overall system purpose
- __parent: [[#test_ws]]

## Storage [[storage: __Namespace]]

- description: Storage namespace

### Users Table [[users: Table]]

- name: users
- __parent: [[#storage]]

#### ID Column [[users_id: Column]]

- name: id
- type: bigint
- __parent: [[#users]]

#### Name Column [[users_name: Column]]

- name: name
- type: varchar
- __parent: [[#users]]

