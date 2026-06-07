# OpenDaemon

Local daemon foundation for coordinating OpenDaemon providers and tasks.

## Phase 8

Phase 8 adds local product authentication for the HTTP API:

- bootstrap product-management token
- durable product registry
- durable product API tokens
- scope-based authorization
- product ownership enforcement across agents, directories, tasks, and task
  events

The daemon remains a local service. Phase 8 does not add remote control-plane
auth, user login, OAuth, or reverse-proxy hardening.

## Running The Daemon

Set a bootstrap token before starting the daemon:

```bash
export OPENDAEMON_BOOTSTRAP_TOKEN="replace-with-a-random-local-token"
cargo run -- daemon
```

The daemon still binds to loopback by default on `127.0.0.1:19514`.

## Authentication

Phase 8 uses two bearer credential types:

1. bootstrap token for `/v1/products*`
2. product API tokens for all other `/v1/*` routes

`GET /health` remains unauthenticated.

Example bootstrap request:

```bash
curl \
  -H "Authorization: Bearer $OPENDAEMON_BOOTSTRAP_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"id":"product_example","display_name":"Example Product"}' \
  http://127.0.0.1:19514/v1/products
```

Example token issuance:

```bash
curl \
  -H "Authorization: Bearer $OPENDAEMON_BOOTSTRAP_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"label":"local-dev","scopes":["providers:read","tasks:create","tasks:read","tasks:cancel"]}' \
  http://127.0.0.1:19514/v1/products/product_example/tokens
```

The plaintext product token is returned only once at creation time.

Example product-scoped API call:

```bash
curl \
  -H "Authorization: Bearer odpk_..." \
  http://127.0.0.1:19514/v1/providers
```

## Product Management Routes

Bootstrap token only:

- `GET /v1/products`
- `POST /v1/products`
- `GET /v1/products/:product_id`
- `PATCH /v1/products/:product_id`
- `GET /v1/products/:product_id/tokens`
- `POST /v1/products/:product_id/tokens`
- `DELETE /v1/products/:product_id/tokens/:token_id`

## Security Notes

- Treat bootstrap tokens and product tokens as local machine credentials.
- Do not expose the daemon on a public interface unless you are deliberately
  accepting the added risk.
- Do not pass daemon credentials through task payloads, provider config, or
  child-process environment variables.

See [docs/security/local-api-auth.md](docs/security/local-api-auth.md) for the
local boundary notes.

### DEV

Phase 0 provides the project identity, CLI entrypoint, daemon HTTP server, and
health endpoint.

#### Tests

Unit and integration-style tests live in the Rust crate.

E2E tests should live in the `e2e` directory and use `uv` plus Python.
