# Services

## Auth Service [[auth: Service]]

- port: 8080

## User Service [[user_svc: Service]]

- port: 8081
- depends: [[#auth]]

## Order Service [[order_svc: Service]]

- port: 8082
- depends: [[[#auth]], [[#user_svc]]]
- database: [[#orders_db]]

## Payment Service [[payment_svc: Service]]

- port: 8083
- depends: [[[#auth]], [[#order_svc]]]
- notifies: [[#user_svc]]
