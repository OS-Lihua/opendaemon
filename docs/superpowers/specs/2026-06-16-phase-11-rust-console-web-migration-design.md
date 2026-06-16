# Phase 11 Rust Console Web Migration Design

## Goal

Replace the current React/Vite Console with a Rust-only web Console that matches
the Phase 11 product contract, uses shared Rust crates for DTOs and API access,
and is served by the daemon as static web assets.

This migration is a full replacement, not a dual-stack transition. React,
TypeScript, Vite, and pnpm Console code will be removed from the repository.

## Scope

This design covers:

- a Rust/WebAssembly web Console for Phase 11
- a shared Rust API crate for Console DTOs, client logic, and SSE parsing
- a shared Rust UI crate for Leptos CSR routes, screens, forms, and state
- daemon static asset serving for the Rust Console build output
- Rust test coverage for Console API logic, UI state behavior, and daemon
  Console integration

This design does not cover:

- a Tauri desktop shell
- preserving React page structure or implementation details
- new daemon authentication models
- broad API redesign beyond narrow UI-enabling additions if later proven
  necessary

## Decisions

- Deliver a Phase 11-spec-first Console rather than a React-parity migration.
- Implement only the web Console in this phase; desktop remains a later step.
- Keep daemon API route shapes stable wherever possible.
- Remove the React/Vite Console completely rather than running both stacks in
  parallel.
- Persist only minimal browser session state; refresh should reload live server
  state rather than cached resource snapshots.

## Target Architecture

The repository will move to this structure:

```text
Cargo.toml
src/**
crates/
  opendaemon-console-api/
    src/
      client.rs
      dto.rs
      error.rs
      lib.rs
  opendaemon-console-ui/
    src/
      app.rs
      shell.rs
      components/
      routes/
        login.rs
        overview.rs
        products.rs
        providers.rs
        agents.rs
        directories.rs
        tasks.rs
        permissions.rs
        settings.rs
      state/
        session.rs
        resources.rs
        tasks.rs
      lib.rs
console/
  index.html
  Trunk.toml
  static/
  dist/
```

### Responsibilities

- `opendaemon`: daemon runtime, HTTP APIs, static asset serving
- `opendaemon-console-api`: shared request/response types, auth header logic,
  API helpers, SSE parsing, scope and gate helpers
- `opendaemon-console-ui`: Leptos CSR UI, routes, forms, app shell, client-side
  state
- `console/`: Trunk entrypoint and generated build output

## UI Information Architecture

The Console will use these top-level routes:

- `Login`
- `Overview`
- `Products`
- `Providers`
- `Agents`
- `Directories`
- `Tasks`
- `Permissions`
- `Settings`

### Route intent

- `Login`: bootstrap-token or product-token connection flow
- `Overview`: daemon status, scheduler summary, control-plane summary, runtime
  summary, permission count, fast links into operational work
- `Products`: product lifecycle and token management for bootstrap users
- `Providers`: provider registry visibility plus runtime availability and
  detection controls in one combined operational view
- `Agents`: Agent Profile CRUD
- `Directories`: Directory Grant CRUD
- `Tasks`: list, filters, creation, detail, transcript, result, remote metadata
- `Permissions`: pending permission request inbox and explicit responses
- `Settings`: current session, principal, scopes, base URL, sign-out

### Layout

- persistent left navigation on desktop
- compact top status bar
- route-level main content region
- task experience optimized around list plus detail rather than multiple
  separate pages

## Client State Model

The UI will keep state in three focused areas.

### SessionState

Responsibilities:

- credential mode
- bearer token
- base URL
- `/v1/session` response
- route guard behavior
- persisted login/session restoration

Persisted keys:

- `base_url`
- `credential_mode`
- `bearer_token`
- `last_route`
- `active_task_id`
- per-task latest SSE cursor

### ResourceState

Responsibilities:

- request/refresh data for status, products, providers, agents, directories,
  permissions
- track loading and error states for ordinary CRUD or read-heavy resources

### TaskState

Responsibilities:

- task list filters
- task detail loading
- transcript event accumulation
- SSE subscription lifecycle
- cursor-based resume after refresh or reconnect
- targeted refresh after cancel or permission response

The task state is separate because it is the only area with long-lived event
stream behavior and incremental detail updates.

## API Boundary

The daemon will continue serving these routes:

- `/console`
- `/v1/session`
- `/v1/daemon/status`
- `/v1/products`
- `/v1/providers`
- `/v1/runtimes`
- `/v1/agents`
- `/v1/directories`
- `/v1/tasks`
- `/v1/permissions`

The daemon should keep current route semantics unless a concrete UI gap forces a
small additive change later.

### Console API crate surface

`opendaemon-console-api` will expose typed methods such as:

- `session()`
- `daemon_status()`
- `list_products()`
- `create_product()`
- `list_providers()`
- `list_runtimes()`
- `detect_runtimes()`
- `list_agents()`
- `create_agent()`
- `list_directories()`
- `create_directory()`
- `list_tasks()`
- `create_task()`
- `task()`
- `cancel_task()`
- `stream_task_events()`
- `list_permissions()`
- `respond_to_permission()`

The UI crate must not construct URLs, auth headers, or SSE parsing inline.

### Shared logic in the API crate

The following client-side policy helpers belong in the shared Rust API crate,
not in screen components:

- scope checks such as `has_scope`
- remote-execution visibility and enablement checks
- direct-mode gating derived from agent, directory, and token constraints
- SSE event application and cursor tracking

## Server Integration

The daemon will continue to expose public Console assets without weakening API
authentication.

### Static assets

- `/console` and `/console/{*path}` remain public
- static assets will come from Trunk output under `console/dist`
- deep links under `/console/...` return the built `index.html`
- API routes remain token-authenticated

### Server code changes

Expected server-side work:

- update Console asset serving to support Trunk output types such as `.wasm`
- keep the existing shell route behavior
- only add narrow helper APIs if the Rust UI cannot be made usable with current
  endpoints

## Deletion Plan

The following React/Vite Console files will be removed:

- `console/src/**`
- `console/package.json`
- `console/pnpm-lock.yaml`
- `console/pnpm-workspace.yaml`
- `console/vite.config.ts`
- `console/tsconfig.json`

The `console/` directory remains, but only as a Trunk web entrypoint and build
output location.

## Migration Sequence

1. Convert the repository root into a Cargo workspace that includes the new
   Console crates while preserving the daemon crate.
2. Add `opendaemon-console-api` with DTOs, client logic, shared helpers, and SSE
   parsing.
3. Add `opendaemon-console-ui` with Leptos CSR app shell, routes, components,
   and focused state modules.
4. Replace the current `console/` Vite entry with Trunk `index.html` and
   `Trunk.toml`.
5. Update daemon static asset serving for Trunk output and any required content
   types.
6. Remove the React/Vite/pnpm Console implementation.
7. Expand Rust tests until the new Console behavior is covered by repository
   quality gates.

## Testing Strategy

Three layers of testing will be used.

### Console API crate tests

- DTO serialization and deserialization
- error mapping
- scope and gate helper behavior
- SSE parsing
- event application and cursor resume logic

### Console UI wasm tests

- login/session restoration
- route guards
- resource loading transitions
- task transcript append behavior
- reconnect or refresh cursor resume behavior

### Daemon integration tests

Expand existing
[src/tests/console.rs](/Users/yaco/github/opendaemon/src/tests/console.rs:1) to
verify:

- `/console` and deep links stay public
- API routes still require auth
- session introspection remains token-safe
- daemon status and permission inbox contracts do not regress

## Acceptance Criteria

- The repository no longer contains a React/TypeScript Console application.
- The web Console is implemented only in Rust using Leptos CSR and shared Rust
  crates.
- The daemon serves the Rust Console build successfully at `/console`.
- Login, Overview, Products, Providers, Agents, Directories, Tasks, Permissions,
  and Settings are all functional.
- The Providers view includes runtime status and runtime detection controls.
- Task detail supports initial load, live SSE updates, and cursor-based resume.
- Existing Phase 8 through Phase 10 scope and remote-execution gates do not
  regress.
- `cargo fmt`, `cargo check`, and `cargo test` pass.
- Trunk build succeeds for the Console web app.

## Risks And Controls

- Risk: UI migration bloats into API redesign. Control: keep existing route
  shapes unless a concrete UI blocker appears.

- Risk: task transcript behavior regresses during SSE migration. Control:
  centralize SSE parsing and event application in the shared API crate and cover
  it with Rust tests.

- Risk: workspace restructuring destabilizes the existing daemon crate. Control:
  keep daemon responsibilities unchanged and verify full cargo gates.

- Risk: Web UI grows over-abstracted during rewrite. Control: keep state limited
  to session, resources, and tasks; avoid generic framework layers beyond
  current needs.

## Out Of Scope Follow-up

If a desktop shell is pursued later, it should wrap the shared Rust UI crate
rather than introducing a desktop-only UI stack.
