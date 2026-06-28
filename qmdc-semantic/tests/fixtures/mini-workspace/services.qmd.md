# Services [[services: __Namespace]]

Platform microservices.

## Auth Service [[auth: Service]]

- description: Authentication and authorization
- port: 8001
- team: Platform
- status: production

## User Service [[user-svc: Service]]

- description: User management
- port: 8002
- team: Platform
- status: production
- depends: [[#auth]]

## Order Service [[order-svc: Service]]

- description: Order processing
- port: 8003
- team: Commerce
- status: production
- depends: [[#auth]], [[#user-svc]], [[#inventory]]

## Inventory Service [[inventory: Service]]

- description: Inventory management
- port: 8004
- team: Commerce
- status: production
- depends: [[#auth]]

## Notification Service [[notify: Service]]

- description: Email and push notifications
- port: 8005
- team: Platform
- status: beta
- depends: [[#auth]], [[#user-svc]]

## Payment Service [[payment: Service]]

- description: Payment processing
- port: 8006
- team: Commerce
- status: production
- depends: [[#auth]], [[#order-svc]]
