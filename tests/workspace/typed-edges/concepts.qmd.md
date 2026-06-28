# Concepts

## Authentication [[authentication: Concept]]

- status: stable

### Description [[auth_desc: text]]

- about: [[#authentication]]

Authentication is the process of verifying identity.
All services must authenticate before processing requests.

## Authorization [[authorization: Concept]]

- status: stable

### Description [[authz_desc: text]]

- about: [[#authorization]]
- depends: [[#authentication]]

Authorization determines what an authenticated user can do.
It builds on top of [[#authentication]] to enforce access control.

## Checkout Flow [[checkout_flow: Concept]]

- status: draft

### Rationale [[checkout_rationale: text]]

- about: [[#checkout_flow]]
- depends: [[#order_svc]], [[#payment_svc]]

The checkout flow orchestrates order creation and payment processing.
It relies on the order service and payment service working together.
