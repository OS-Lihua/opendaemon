# Phase 8: Product Authentication

## Goal

Make the local HTTP API safe for multiple products by adding authenticated
product identities, scoped API tokens, and ownership enforcement across the
existing provider, runtime, agent, directory, task, and event APIs.

Phase 8 builds on Phase 7. It does not redesign provider execution, task
storage, or SSE delivery. It adds the local authentication and authorization
boundary that earlier phases intentionally deferred:

- product registration
- local API bearer tokens
- product-scoped API authorization
- scope checks per route
- ownership checks across Agent Profiles, Directory Grants, and Tasks
- token issuance and revocation
- reverse-proxy and remote-access risk documentation

This phase must not add remote control-plane dispatch, daemon registration,
daemon tokens, task tokens, audit logging, rate limiting, keyring-backed secret
storage, or a desktop UI.

## Scope

Phase 8 delivers local multi-product authentication and authorization:

- add a durable product registry
- add product status and metadata
- add durable per-product API token records
- issue random bearer tokens and store only token metadata plus a token digest
- require bearer authentication for all `/v1/*` routes except where explicitly
  exempted below
- keep `GET /health` unauthenticated
- add route-level scope checks
- add ownership checks so one product cannot read or mutate another product's
  agents, directories, tasks, task events, or permission responses
- add a bootstrap administration token for local product registration and token
  issuance
- keep existing request and response shapes stable where practical by validating
  caller product identity against existing `owner_product_id` and `product_id`
  fields
- reject disabled products and revoked tokens
- ensure product or bootstrap credentials are never passed to child agents
- document reverse-proxy and remote-access risks for the local API
- preserve provider registry, runtime detection, scheduler, runtime adapter, and
  SSE behavior apart from the new auth boundary
- quality gates passing

Phase 8 is the local production authentication layer from the roadmap. It is
not the cloud control-plane token model. The daemon remains a loopback service
in this phase.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 3 spec: `docs/tasks/phase-3-directory-grants.md`
- Phase 4 spec: `docs/tasks/phase-4-agent-profiles.md`
- Phase 5 spec: `docs/tasks/phase-5-task-scheduler.md`
- Phase 6 spec: `docs/tasks/phase-6-runtime-adapters.md`
- Phase 7 spec: `docs/tasks/phase-7-event-streaming.md`
- Phase 7 implementation:
  - `src/api/mod.rs`
  - `src/api/agents.rs`
  - `src/api/directories.rs`
  - `src/api/providers.rs`
  - `src/api/runtimes.rs`
  - `src/api/tasks.rs`
  - `src/config/mod.rs`
  - `src/store/sqlite.rs`
  - `src/store/agent_profiles.rs`
  - `src/store/directory_grants.rs`
  - `src/store/tasks.rs`
  - `src/task/service.rs`
  - `src/tests/api.rs`
  - `src/tests/agents.rs`
  - `src/tests/directories.rs`
  - `src/tests/tasks.rs`

## Deliverables

- Product model types exist.
- Product token model types exist.
- Product records are persisted in SQLite.
- Product token records are persisted in SQLite.
- Product token plaintext is returned only at token creation time.
- Persisted token records store a token digest, not the plaintext token.
- The daemon accepts `Authorization: Bearer <token>` on authenticated routes.
- `GET /health` remains unauthenticated.
- All `/v1/*` routes other than the bootstrap product-management routes require
  a valid product token with sufficient scope.
- Bootstrap product-management routes require the daemon bootstrap token.
- Missing, invalid, revoked, or disabled credentials return stable `401` JSON.
- Valid credentials with insufficient scope return stable `403` JSON.
- Product tokens cannot access another product's Agent Profiles.
- Product tokens cannot access another product's Directory Grants.
- Product tokens cannot access another product's Tasks.
- Product tokens cannot observe another product's task event stream.
- Product tokens cannot answer another product's permission requests.
- Agent create, patch, and delete enforce `owner_product_id` consistency with
  the authenticated product.
- Directory create, patch, and delete enforce `product_id` consistency with the
  authenticated product.
- Task create enforces `owner_product_id` consistency with the authenticated
  product.
- Task list and get only expose tasks owned by the authenticated product.
- Directory list and get only expose grants owned by the authenticated product.
- Agent list and get only expose profiles owned by the authenticated product.
- Provider and runtime read routes remain product-agnostic but require scopes.
- Direct directory operations require `directories:direct` in addition to the
  existing Phase 3 and Phase 5 policy checks.
- Scope value `tasks:remote_execution` is defined and persisted for future
  phases, even though remote control-plane execution remains out of scope here.
- Existing runtime adapters never receive product tokens or bootstrap tokens in
  their child-process environment.
- Reverse-proxy and remote-access risks are documented.
- Existing provider, runtime, directory, agent, task, result, and SSE tests are
  updated or extended to run under authenticated requests.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 8:

- ACP protocol execution
- `integration_type = "acp"` runtime execution
- remote control-plane dispatch
- daemon registration or heartbeat
- daemon token
- task token
- audit log
- rate limits
- keyring-backed token storage
- provider secret storage
- browser UI or desktop UI
- generic user accounts or human login flows
- OAuth, OIDC, or browser-based auth
- mTLS
- reverse-proxy deployment support beyond documentation of risks
- distributed auth across multiple daemons

Phase 8 is about local API trust boundaries for products. It is not a general
identity platform.

## Dependencies

Keep Phase 0 through Phase 7 dependencies. Add only what is needed for token
generation, token digesting, and constant-time comparison.

Suggested additions:

```toml
sha2 = "0.10"
hex = "0.4"
rand = "0.9"
subtle = "2"
```

If current stable crate APIs differ at implementation time, use the current
stable APIs and keep the dependency purpose unchanged.

Do not add keyring, OAuth, JWT, websocket, ACP, control-plane, or reverse-proxy
dependencies in Phase 8.

## Product Authentication Contract

### Authentication Model

Phase 8 uses two local bearer credential classes:

1. bootstrap token
2. product API token

The bootstrap token exists so a trusted local operator can register products and
mint or revoke product tokens without leaving the API unauthenticated.

The product API token exists so a product can call ordinary OpenDaemon APIs
within granted scopes.

Requirements:

- `GET /health` is unauthenticated.
- `/v1/products` bootstrap-management routes require the bootstrap token.
- all other `/v1/*` routes require a product API token.
- bearer tokens are supplied through:

```http
Authorization: Bearer <token>
```

- tokens must not be accepted from query parameters.
- tokens must not be accepted from task payloads or provider config.
- the daemon must not log raw bearer tokens.
- the daemon must not include raw bearer tokens in task events, task results, or
  child process environments.

### Bootstrap Token

Phase 8 needs one trusted local bootstrap path. Use daemon configuration for it.

Recommended config shape:

```text
OPENDAEMON_BOOTSTRAP_TOKEN=<opaque-random-token>
```

Requirements:

- the bootstrap token is loaded from daemon config or environment, not from a
  product database row
- the bootstrap token is never returned by any API
- bootstrap authentication is only for local product-management routes
- bootstrap requests do not impersonate a product
- product-scoped routes must reject a bootstrap token with a stable error
- if the bootstrap token is missing, product-management routes fail closed

Phase 8 does not need bootstrap-token rotation APIs. Rotation can happen by
changing daemon configuration and restarting the daemon.

### Product Model

Use this API shape:

```json
{
  "id": "product_example",
  "display_name": "Example Product",
  "status": "active",
  "description": "Optional local integration metadata.",
  "created_at": "2026-06-07T00:00:00Z",
  "updated_at": "2026-06-07T00:00:00Z"
}
```

Field requirements:

- `id`: stable product ID used across agents, directories, and tasks
- `display_name`: human-readable local name
- `status`: `active` or `disabled`
- `description`: optional non-secret text metadata
- `created_at`: UTC RFC3339 timestamp
- `updated_at`: UTC RFC3339 timestamp

Validation rules:

- IDs must be unique in the local SQLite database
- IDs must be URL safe
- IDs must not contain local paths, credentials, or whitespace
- disabled products cannot authenticate successfully
- disabling a product must immediately disable all of its product tokens
- disabling a product must not delete historical tasks, events, grants, or
  profiles

Recommended product ID validation:

```text
^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$
```

### Product Token Model

Use durable token metadata with a separate create response that includes the
plaintext token once.

Stored token metadata shape:

```json
{
  "id": "ptok_1",
  "product_id": "product_example",
  "label": "local-dev",
  "scopes": ["tasks:create", "tasks:read"],
  "token_prefix": "odpk_7f2a",
  "status": "active",
  "created_at": "2026-06-07T00:00:00Z",
  "last_used_at": null,
  "revoked_at": null
}
```

Create response shape:

```json
{
  "id": "ptok_1",
  "product_id": "product_example",
  "label": "local-dev",
  "scopes": ["tasks:create", "tasks:read"],
  "token_prefix": "odpk_7f2a",
  "token": "odpk_7f2ac8b0f8d4...",
  "created_at": "2026-06-07T00:00:00Z"
}
```

Requirements:

- tokens are daemon-generated, high-entropy random values
- tokens should use a recognizable prefix such as `odpk_`
- the plaintext token is returned only once at creation time
- the daemon stores only a token digest plus metadata
- token digests must be compared in constant time
- token lookup must reject revoked tokens
- token lookup must reject tokens whose product is disabled
- token metadata list responses must never include the plaintext token

Acceptable storage fields:

- `id`
- `product_id`
- `label`
- `scopes_json`
- `token_prefix`
- `token_digest_hex`
- `created_at`
- `last_used_at`
- `revoked_at`

Phase 8 does not need token expiry, token refresh, or rolling session semantics.
Explicit revoke plus reissue is sufficient.

### Scope Set

Phase 8 must implement the roadmap scope set exactly:

- `providers:read`
- `runtimes:read`
- `agents:read`
- `agents:write`
- `directories:read`
- `directories:grant`
- `directories:direct`
- `tasks:create`
- `tasks:read`
- `tasks:cancel`
- `tasks:remote_execution`

Rules:

- scopes are stored as a normalized set
- duplicate scopes are rejected or normalized away consistently
- unknown scopes are rejected
- tokens may hold any subset of the defined scopes
- bootstrap-token routes do not use this scope model
- future phases may add scopes, but Phase 8 must not rename these values

### Route Authorization Matrix

Use this route-to-scope contract:

- `GET /v1/providers` -> `providers:read`
- `GET /v1/providers/:provider_id` -> `providers:read`
- `GET /v1/runtimes` -> `runtimes:read`
- `POST /v1/runtimes/detect` -> `runtimes:read`
- `GET /v1/agents` -> `agents:read`
- `GET /v1/agents/:agent_id` -> `agents:read`
- `POST /v1/agents` -> `agents:write`
- `PATCH /v1/agents/:agent_id` -> `agents:write`
- `DELETE /v1/agents/:agent_id` -> `agents:write`
- `GET /v1/directories` -> `directories:read`
- `GET /v1/directories/:directory_id` -> `directories:read`
- `POST /v1/directories/grant` -> `directories:grant`
- `PATCH /v1/directories/:directory_id` -> `directories:grant`
- `DELETE /v1/directories/:directory_id` -> `directories:grant`
- direct-mode create or patch on directory grants -> `directories:grant` and
  `directories:direct`
- `GET /v1/tasks` -> `tasks:read`
- `GET /v1/tasks/:task_id` -> `tasks:read`
- `GET /v1/tasks/:task_id/events` -> `tasks:read`
- `POST /v1/tasks` -> `tasks:create`
- `POST /v1/tasks/:task_id/cancel` -> `tasks:cancel`
- `POST /v1/tasks/:task_id/events` for permission responses -> `tasks:read`

Rationale for `POST /v1/tasks/:task_id/events`:

- Phase 7 defines this endpoint only for permission responses
- those responses are tied to observing the caller's own task event stream
- Phase 8 should avoid inventing a new scope value that is not in the roadmap

### Ownership Enforcement

Product tokens must be product-scoped in addition to scope-scoped.

Rules:

- Agent Profile reads and writes are limited to rows where
  `owner_product_id == authenticated_product_id`
- Directory Grant reads and writes are limited to rows where
  `product_id == authenticated_product_id`
- Task reads and writes are limited to rows where
  `owner_product_id == authenticated_product_id`
- Task event replay and permission response are limited to tasks owned by the
  authenticated product
- route handlers must not trust caller-supplied product IDs without comparing
  them to the authenticated product

Compatibility rule:

- existing request bodies that already include `owner_product_id` or
  `product_id` remain valid
- Phase 8 enforces equality between those fields and the authenticated product
- a mismatch returns `403 product_scope_mismatch`

Phase 8 may internally infer the product ID from auth context to reduce
duplicate handler logic, but the externally visible API contract does not need a
breaking request-shape redesign.

### Product Management API

Add bootstrap-token-only product-management routes:

- `GET /v1/products`
- `POST /v1/products`
- `GET /v1/products/:product_id`
- `PATCH /v1/products/:product_id`
- `GET /v1/products/:product_id/tokens`
- `POST /v1/products/:product_id/tokens`
- `DELETE /v1/products/:product_id/tokens/:token_id`

Phase 8 may omit `DELETE /v1/products/:product_id` if implementation prefers
soft-disable through `PATCH status=disabled`.

Recommended create-product request:

```json
{
  "id": "product_example",
  "display_name": "Example Product",
  "description": "Optional local integration metadata."
}
```

Recommended patch-product request:

```json
{
  "display_name": "Example Product",
  "description": "Updated description.",
  "status": "disabled"
}
```

Recommended create-token request:

```json
{
  "label": "local-dev",
  "scopes": ["tasks:create", "tasks:read", "tasks:cancel"]
}
```

Requirements:

- product create rejects duplicate IDs
- product patch rejects invalid status transitions
- token create rejects unknown scopes
- token create rejects empty scope sets
- token revoke is idempotent
- token list omits token digest and plaintext token
- missing products return stable `404` JSON
- invalid product or token inputs return stable `400` JSON

### Authentication Middleware

Add a reusable auth layer for `axum` handlers.

Requirements:

- authentication runs before route handler business logic
- authenticated request context includes:
  - credential kind: bootstrap or product token
  - authenticated product ID for product tokens
  - granted scope set for product tokens
- route handlers can require:
  - bootstrap credential
  - product credential
  - one or more scopes
- product token last-used timestamps may be updated opportunistically
- auth failures use stable JSON error bodies consistent with existing API style

Recommended stable error codes:

- `missing_authentication`
- `invalid_token`
- `bootstrap_token_required`
- `product_token_required`
- `insufficient_scope`
- `product_scope_mismatch`
- `product_disabled`

### SQLite Schema Additions

Extend `src/store/sqlite.rs` with durable product-auth tables.

Recommended schema:

```sql
CREATE TABLE IF NOT EXISTS products (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  status TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS product_tokens (
  id TEXT PRIMARY KEY,
  product_id TEXT NOT NULL,
  label TEXT NOT NULL,
  scopes_json TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  token_digest_hex TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS product_tokens_product_idx
ON product_tokens(product_id, revoked_at);
```

Requirements:

- product token rows survive daemon restart
- token digests are unique
- stores remain injectable for tests through `StoreConfig`
- schema initialization remains automatic

Phase 8 does not need a full migration framework. The existing schema bootstrap
pattern is sufficient.

### Child Process Credential Boundary

Phase 8 must preserve the roadmap rule that product and daemon credentials do
not reach child agents.

Requirements:

- runtime adapter execution requests must not contain product tokens
- scheduler or runtime code must not copy inbound HTTP auth headers into child
  process environments
- task events must not echo inbound auth headers
- token values must not be written to logs or persisted task records

### Reverse Proxy and Remote Access Documentation

Add a short operator document, for example:

- `docs/security/local-api-auth.md`

Document at least:

- the daemon is a local API, not a hardened internet-facing service
- default bind remains loopback
- exposing the daemon through a reverse proxy expands the attack surface
- bootstrap tokens and product tokens should be treated like local machine
  credentials
- remote control-plane support belongs to a later phase with different tokens
  and trust boundaries

## Testing

Add focused tests for authentication and authorization without weakening earlier
coverage.

Required test coverage:

- unauthenticated `GET /health` succeeds
- unauthenticated `/v1/providers` is rejected
- invalid bearer token is rejected
- revoked token is rejected
- disabled product token is rejected
- bootstrap token can create a product
- bootstrap token can mint a product token
- bootstrap token is rejected on product-scoped task routes
- product token with `providers:read` can read providers
- product token without `providers:read` gets `403`
- product token can create only its own Agent Profiles
- product token cannot read another product's Agent Profiles
- product token can create only its own Directory Grants
- product token cannot read another product's Directory Grants
- direct directory grant requests require `directories:direct`
- product token can create only its own Tasks
- product token cannot read another product's Tasks
- product token cannot stream another product's task events
- product token cannot respond to another product's permission requests
- authenticated task execution still does not leak product tokens into child
  process env handling
- existing task cancel, SSE replay, and permission response flows still work
  under authenticated requests

## Quality Gates

Phase 8 is complete when these continue to pass:

- `cargo fmt --all`
- `cargo clippy --tests --all-targets --all-features -- -D warnings`
- `cargo test --all-features --all-targets`

And when the new acceptance checks are satisfied:

- unauthorized requests are rejected
- products can use only the scopes they were granted
- one product cannot read or mutate another product's resources
- direct-directory operations require explicit direct scope
- bootstrap credentials stay limited to product-management routes
