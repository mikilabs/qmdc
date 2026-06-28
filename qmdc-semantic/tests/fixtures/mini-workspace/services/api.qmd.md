# API Services

## User Service [[user_service: Service]]

- description: Handles user authentication and profiles
- port: 8081
- database: [[#users]]

## Order Service [[order_service: Service]]

- description: Manages orders and checkout
- port: 8082
- tables: [[[#orders]], [[#order_items]]]

## Product Service [[product_service: Service]]

- description: Product catalog and inventory
- port: 8083
- database: [[#products]]

## API Gateway [[api_gateway: Service]]

- description: Main entry point for all API requests
- port: 8080
- dependencies: [[[#user_service]], [[#order_service]], [[#product_service]]]

