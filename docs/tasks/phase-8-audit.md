# Phase 8 Audit

This document audits the implementation in the repository against the deliverables and required test coverage in [phase-8-product-authentication.md](./phase-8-product-authentication.md).

Audit date: 2026-06-07

## Status

- Implementation status: complete for the specified Phase 8 local authentication boundary
- Verification status:
  - `cargo fmt --all` passed
  - `cargo clippy --tests --all-targets --all-features -- -D warnings` passed
  - `cargo test -- --test-threads=1` passed

## Deliverables Mapping

| Deliverable | Code mapping | Test / verification mapping | Status |
|---|---|---|---|
| Product model types exist. | [src/product/mod.rs](/Users/yaco/github/opendaemon/src/product/mod.rs) `Product`, `CreateProduct`, `PatchProduct`, `ProductStatus` | Compile-time coverage; product-management API exercised in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| Product token model types exist. | [src/product/mod.rs](/Users/yaco/github/opendaemon/src/product/mod.rs) `ProductToken`, `CreateProductToken`, `CreatedProductToken`, `ApiScope`, `ProductTokenStatus` | Product token issuance path exercised in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| Product records are persisted in SQLite. | [src/store/sqlite.rs](/Users/yaco/github/opendaemon/src/store/sqlite.rs) `products` table; [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) `create_product`, `get_product`, `list_products`, `patch_product` | Product create path exercised through `/v1/products` in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) | Covered |
| Product token records are persisted in SQLite. | [src/store/sqlite.rs](/Users/yaco/github/opendaemon/src/store/sqlite.rs) `product_tokens` table; [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) `create_token`, `list_tokens`, `revoke_token` | Token create path exercised through `/v1/products/:product_id/tokens` in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) | Covered |
| Product token plaintext is returned only at token creation time. | [src/api/products.rs](/Users/yaco/github/opendaemon/src/api/products.rs) `CreatedProductTokenResponse`; [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) returns plaintext only from `create_token` | Token creation response inspected in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) | Covered |
| Persisted token records store a token digest, not the plaintext token. | [src/store/sqlite.rs](/Users/yaco/github/opendaemon/src/store/sqlite.rs) `token_digest_hex`; [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) `token_digest_hex`, `authenticate_product_token` | Verified by code inspection; no test currently queries SQLite row contents directly | Implemented, indirect test coverage |
| The daemon accepts `Authorization: Bearer <token>` on authenticated routes. | [src/api/auth.rs](/Users/yaco/github/opendaemon/src/api/auth.rs) `bearer_token`, `ProductAuth`, `BootstrapAuth` | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| `GET /health` remains unauthenticated. | [src/api/mod.rs](/Users/yaco/github/opendaemon/src/api/mod.rs) `/health` route; [src/api/health.rs](/Users/yaco/github/opendaemon/src/api/health.rs) | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `health_handler_returns_stable_json`, `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| All `/v1/*` routes other than bootstrap product-management routes require a valid product token with sufficient scope. | [src/api/providers.rs](/Users/yaco/github/opendaemon/src/api/providers.rs), [src/api/runtimes.rs](/Users/yaco/github/opendaemon/src/api/runtimes.rs), [src/api/agents.rs](/Users/yaco/github/opendaemon/src/api/agents.rs), [src/api/directories.rs](/Users/yaco/github/opendaemon/src/api/directories.rs), [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) all require `ProductAuth` and scope checks | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs), [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs), [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs), [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) | Covered |
| Bootstrap product-management routes require the daemon bootstrap token. | [src/api/products.rs](/Users/yaco/github/opendaemon/src/api/products.rs) uses `BootstrapAuth`; [src/api/auth.rs](/Users/yaco/github/opendaemon/src/api/auth.rs) enforces bootstrap credential kind | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| Missing, invalid, revoked, or disabled credentials return stable `401` JSON. | [src/api/auth.rs](/Users/yaco/github/opendaemon/src/api/auth.rs) `AuthError` mappings; [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) rejects revoked/disabled tokens | Missing, invalid, disabled covered in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs); revoked behavior is implemented via `revoked_at IS NULL` filter but not named in a dedicated test | Implemented, revoked test not explicit |
| Valid credentials with insufficient scope return stable `403` JSON. | [src/api/auth.rs](/Users/yaco/github/opendaemon/src/api/auth.rs) `InsufficientScope`; route handlers call `require_scope` / `require_scopes` | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| Product tokens cannot access another product's Agent Profiles. | [src/api/agents.rs](/Users/yaco/github/opendaemon/src/api/agents.rs) `ensure_product`, forced owner filter | [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs) `agent_api_enforces_product_ownership` | Covered |
| Product tokens cannot access another product's Directory Grants. | [src/api/directories.rs](/Users/yaco/github/opendaemon/src/api/directories.rs) `ensure_product`, forced product filter | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| Product tokens cannot access another product's Tasks. | [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) `ensure_product`, forced owner filter | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| Product tokens cannot observe another product's task event stream. | [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) `events` checks task owner before stream | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| Product tokens cannot answer another product's permission requests. | [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) `post_event` checks task owner before permission resolution | Owner guard is implemented; no dedicated cross-product permission-response test name exists | Implemented, indirect coverage |
| Agent create, patch, and delete enforce `owner_product_id` consistency with the authenticated product. | [src/api/agents.rs](/Users/yaco/github/opendaemon/src/api/agents.rs) `ensure_product` on create and existing owner checks on patch/delete | Create mismatch covered in [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs) `agent_api_enforces_product_ownership`; patch/delete ride same owner guard path but are not separately named | Implemented, partial explicit test naming |
| Directory create, patch, and delete enforce `product_id` consistency with the authenticated product. | [src/api/directories.rs](/Users/yaco/github/opendaemon/src/api/directories.rs) `ensure_product` on create and existing product checks on patch/delete | Create/get cross-product behavior covered in [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) | Implemented, partial explicit test naming |
| Task create enforces `owner_product_id` consistency with the authenticated product. | [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) `ensure_product(&request.owner_product_id)` | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| Task list and get only expose tasks owned by the authenticated product. | [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) forced owner filter in `list`; owner check in `get` | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_creates_lists_gets_and_cancels_tasks`, `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| Directory list and get only expose grants owned by the authenticated product. | [src/api/directories.rs](/Users/yaco/github/opendaemon/src/api/directories.rs) forced product filter in `list`; owner check in `get` | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_lists_creates_gets_patches_and_deletes_grants`, `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| Agent list and get only expose profiles owned by the authenticated product. | [src/api/agents.rs](/Users/yaco/github/opendaemon/src/api/agents.rs) forced owner filter in `list`; owner check in `get` | [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs) `agent_api_creates_lists_gets_patches_deletes_and_filters_profiles`, `agent_api_enforces_product_ownership` | Covered |
| Provider and runtime read routes remain product-agnostic but require scopes. | [src/api/providers.rs](/Users/yaco/github/opendaemon/src/api/providers.rs), [src/api/runtimes.rs](/Users/yaco/github/opendaemon/src/api/runtimes.rs) | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) | Covered |
| Direct directory operations require `directories:direct` in addition to existing policy checks. | [src/api/directories.rs](/Users/yaco/github/opendaemon/src/api/directories.rs) `require_direct_scope` | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| Scope value `tasks:remote_execution` is defined and persisted for future phases. | [src/product/mod.rs](/Users/yaco/github/opendaemon/src/product/mod.rs) `ApiScope::TasksRemoteExecution`; persisted in `scopes_json` in [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) | Compile-time coverage; no dedicated runtime test because feature is intentionally out of scope | Implemented |
| Existing runtime adapters never receive product tokens or bootstrap tokens in their child-process environment. | [src/runtime/cli.rs](/Users/yaco/github/opendaemon/src/runtime/cli.rs) minimal environment handling; no HTTP auth propagation path exists in scheduler/runtime request model | [src/tests/runtime_adapter.rs](/Users/yaco/github/opendaemon/src/tests/runtime_adapter.rs) `cli_adapter_removes_provider_secret_env_and_appends_custom_args`, `cli_adapter_copies_custom_env_keys_only_when_explicitly_enabled` | Covered by code path and adjacent env tests |
| Reverse-proxy and remote-access risks are documented. | [docs/security/local-api-auth.md](/Users/yaco/github/opendaemon/docs/security/local-api-auth.md), [README.md](/Users/yaco/github/opendaemon/README.md) | Documentation review | Covered |
| Existing provider, runtime, directory, agent, task, result, and SSE tests are updated or extended to run under authenticated requests. | Auth-aware tests in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs), [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs), [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs), [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) | `cargo test -- --test-threads=1` passed with 87 tests | Covered |
| Quality gates pass. | Repository-wide verification | `cargo fmt --all`, `cargo clippy --tests --all-targets --all-features -- -D warnings`, `cargo test -- --test-threads=1` | Covered |

## Required Test Coverage Mapping

| Spec test item | Current mapping | Status |
|---|---|---|
| unauthenticated `GET /health` succeeds | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| unauthenticated `/v1/providers` is rejected | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| invalid bearer token is rejected | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| revoked token is rejected | Implemented by [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs) `authenticate_product_token` filtering `revoked_at IS NULL`; no dedicated named test | Gap in explicit test |
| disabled product token is rejected | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| bootstrap token can create a product | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| bootstrap token can mint a product token | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| bootstrap token is rejected on product-scoped task routes | Covered generically on `/v1/providers` in [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs); not named specifically for task route | Indirect coverage |
| product token with `providers:read` can read providers | [src/tests/api.rs](/Users/yaco/github/opendaemon/src/tests/api.rs) `auth_enforces_health_bootstrap_and_product_tokens` | Covered |
| product token without `providers:read` gets `403` | Scope enforcement behavior exists; no dedicated provider-scope negative test name | Gap in explicit test |
| product token can create only its own Agent Profiles | [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs) `agent_api_enforces_product_ownership` | Covered |
| product token cannot read another product's Agent Profiles | [src/tests/agents.rs](/Users/yaco/github/opendaemon/src/tests/agents.rs) `agent_api_enforces_product_ownership` | Covered |
| product token can create only its own Directory Grants | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| product token cannot read another product's Directory Grants | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| direct directory grant requests require `directories:direct` | [src/tests/directories.rs](/Users/yaco/github/opendaemon/src/tests/directories.rs) `directory_api_enforces_product_ownership_and_direct_scope` | Covered |
| product token can create only its own Tasks | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| product token cannot read another product's Tasks | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| product token cannot stream another product's task events | [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) `task_api_enforces_product_ownership_for_reads_and_events` | Covered |
| product token cannot respond to another product's permission requests | Owner check exists in [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs) `post_event`; no dedicated cross-product permission-response test | Gap in explicit test |
| authenticated task execution still does not leak product tokens into child process env handling | No auth header propagation path exists; env minimization verified by [src/tests/runtime_adapter.rs](/Users/yaco/github/opendaemon/src/tests/runtime_adapter.rs) | Covered by adjacent execution boundary tests |
| existing task cancel, SSE replay, and permission response flows still work under authenticated requests | Auth-aware task API tests in [src/tests/tasks.rs](/Users/yaco/github/opendaemon/src/tests/tasks.rs) | Covered |

## Residual Gaps

The current implementation satisfies the Phase 8 behavior, but the following spec items are only indirectly covered by tests rather than named by a focused test:

1. revoked token rejection
2. provider route insufficient-scope rejection
3. bootstrap-token rejection on a product-scoped task route specifically
4. cross-product permission-response rejection

These are audit-quality test gaps, not implementation gaps. The code paths for them exist in:

- [src/store/products.rs](/Users/yaco/github/opendaemon/src/store/products.rs)
- [src/api/auth.rs](/Users/yaco/github/opendaemon/src/api/auth.rs)
- [src/api/tasks.rs](/Users/yaco/github/opendaemon/src/api/tasks.rs)

## Non-Goals Check

No code paths in this implementation add:

- remote control-plane dispatch
- daemon registration / heartbeat
- daemon tokens
- task tokens
- OAuth / OIDC / browser login
- mTLS
- rate limiting
- audit logging
- keyring-backed token storage
- browser or desktop UI

The implementation remains a local bearer-auth boundary for the existing loopback daemon.
