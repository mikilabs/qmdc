# Search Test [[search_test]]

## Auth Service [[auth_service: Service]]

- port: 8080
- depends: [[#user_db]]

## User Database [[user_db: Database]]

- host: localhost
- port: 5432

## Payment Gateway [[payment_gw: Gateway]]

- provider: stripe
