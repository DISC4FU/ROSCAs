# ROSCAs

Rails 8 administration and a Rust/Axum service share a PostgreSQL database.
Docker Compose provides PostgreSQL 16 and Redis 7; Redis is reserved for the
USSD session work in a later phase.

## Phase 1: foundation

The Rails schema covers chamas, members, payout cycles, contributions,
disbursements, and audit logs. Rails enforces domain validations, while
PostgreSQL constraints protect financial amounts, relationships, and external
payment identifiers. The Rust `/health` endpoint uses a SQLx compile-time
checked query against the migrated `chamas` table.

## Local setup

Requirements: Ruby 3.3+, Bundler, Docker with Compose, and (for local Rust
development outside Compose) Rust 1.88+.

1. Install Rails dependencies: `bundle install`
2. Start the shared database services: `docker compose up -d db redis`
3. Apply the Rails migrations:
   `docker compose run --build --rm rails ./bin/rails db:prepare`
4. Start Rails and Rust:
   `docker compose --profile apps up --build`
5. Check service health at `http://localhost:3000/up` and
   `http://localhost:8000/health`.

The Compose password and Rails secret defaults are for local development only.
Set `DB_PASSWORD` and `SECRET_KEY_BASE` in the environment before deploying.
The Rails service automatically runs `db:prepare` on startup.
The Rust container is a multi-stage build that uses checked-in SQLx offline
metadata, so its release build does not need database credentials or network
access to PostgreSQL at image-build time.

To run model specs from the host, prepare the test database and run RSpec:

```sh
RAILS_ENV=test bundle exec rails db:prepare
bundle exec rspec
```

RSpec uses transactional fixtures, FactoryBot factories, Shoulda Matchers, and
WebMock with outbound network calls disabled by default.

## Phase 2: Daraja STK Push

The Rust API accepts a member ID, payout-cycle ID, and positive whole-number
amount at `POST /api/v1/payments/stk-push`. Successful Daraja callbacks are
recorded as `paid` contributions (the existing Rails status), and failed
callbacks are recorded as `failed`. Both outcomes store the callback JSON and
create an audit log entry. Repeated callbacks do not create duplicate audits
or change a terminal contribution.

Configure these variables for sandbox requests:

```sh
export DARAJA_BASE_URL=https://sandbox.safaricom.co.ke
export DARAJA_CONSUMER_KEY=...
export DARAJA_CONSUMER_SECRET=...
export DARAJA_SHORTCODE=...
export DARAJA_PASSKEY=...
export DARAJA_CALLBACK_URL=https://<public-tunnel>/api/v1/mpesa/callback
export DARAJA_TRANSACTION_TYPE=CustomerPayBillOnline
```

Use `CustomerPayBillOnline` for a PayBill shortcode or
`CustomerBuyGoodsOnline` for a Till shortcode. Confirm the type in the Daraja
application configuration; the Rust service does not guess. Run
`docker compose --profile apps up --build`, then point the tunnel at port 8080
(for example, `ngrok http 8080`). The service also remains available on port
8000 for existing local health checks.

The STK callback endpoint is `POST /api/v1/mpesa/callback`. To run the callback
integration test, point `DATABASE_URL` at a migrated test PostgreSQL database
when running the Rust tests in the Rust 1.88 container.
