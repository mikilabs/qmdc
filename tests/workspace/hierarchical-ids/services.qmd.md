## Auth Service [[auth_svc]]

- port: 8080

### Config [[config]]

- timeout: 30
- retries: 3

### Endpoints [[endpoints: [Endpoint]]]

#### Login [[login]]

- method: POST
- path: /auth/login

#### Logout [[logout]]

- method: POST
- path: /auth/logout

## Payment Service [[payment_svc]]

- port: 8081
- auth: [[#auth_svc.endpoints.login]]
- auth_timeout: [[#auth_svc.config.timeout]]

### Config [[config]]

- timeout: 60

### Endpoints [[endpoints: [Endpoint]]]

#### Charge [[charge]]

- method: POST
- path: /payments/charge
