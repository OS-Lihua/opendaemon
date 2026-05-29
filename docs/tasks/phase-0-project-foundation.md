# Phase 0: Project Foundation

## Goal

Turn the copied Rust template into an OpenDaemon foundation without building the
full runtime yet.

Phase 0 establishes the project identity, development commands, module
boundaries, and the smallest runnable daemon surface:

- `opendaemon --version`
- `opendaemon daemon --host 127.0.0.1 --port 19514`
- `GET /health`

The document is an execution specification for implementation. It should be
detailed enough to build from, but it should not contain full source listings.

## Scope

Phase 0 delivers only project foundation behavior:

- package metadata renamed from `template` to `opendaemon`
- E2E directory casing standardized from `E2E/` to `e2e/`
- minimal CLI contract
- daemon bind configuration
- local HTTP server startup
- stable health endpoint
- initial module skeleton
- quality gates passing

Phase 0 must not implement provider registry loading, runtime detection,
directory grants, Agent Profiles, task scheduling, worktrees, ACP, remote
control-plane dispatch, daemon service installation, product authentication, or
desktop UI.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Current template files:
  - `Cargo.toml`
  - `src/main.rs`
  - `src/lib.rs`
  - `justfile`
  - `.pre-commit-config.yaml`
  - `E2E/`

## Deliverables

- `Cargo.toml` package identity is `opendaemon`.
- Package authors use neutral project metadata, for example
  `OpenDaemon contributors`.
- Cargo release replacement snippets refer to `opendaemon`.
- User-facing template identity is replaced where necessary in `README.md`,
  `CHANGELOG.md`, and `docs/README.md`.
- `E2E/` is renamed to `e2e/`.
- `opendaemon --version` prints the package version.
- `opendaemon daemon --host 127.0.0.1 --port 19514` starts a local HTTP server.
- `GET /health` returns the exact stable JSON contract defined below.
- Initial source modules exist for later roadmap phases.
- Basic tests cover CLI parsing, health response shape, router behavior, and
  daemon bind/shutdown behavior.
- The quality gate commands listed in this document pass.

## Non-Goals

Do not add or design these in Phase 0:

- registry directory layout
- provider manifest schema or `ProviderManifest`
- provider registry loading
- agent CLI discovery
- runtime detection
- Agent Profile persistence
- directory grant persistence
- task API
- task scheduler
- graceful task shutdown
- worktree creation
- ACP adapter
- remote control plane
- product authentication
- daemon install/service management
- desktop UI

The only graceful shutdown required in Phase 0 is HTTP server shutdown.

## Dependencies

Add only the dependencies needed for this phase:

```toml
anyhow = "1"
axum = "0.8"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "signal"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

Keep dependency scope intentionally small. Do not add SQLite, websocket,
registry schema validation, keyring/keychain, process runtime, task execution,
or provider process dependencies in Phase 0.

## CLI Contract

The binary name is `opendaemon`.

Supported root behavior:

```text
opendaemon --version
```

Supported subcommands:

```text
opendaemon daemon [--host <host>] [--port <port>]
```

Daemon defaults:

- `host = 127.0.0.1`
- `port = 19514`

CLI requirements:

- `--version` prints the package version from Cargo metadata.
- `daemon` binds the configured host and port.
- `--port 0` is valid and lets the OS assign a free port.
- tests should use `--port 0` instead of a fixed port.
- invalid arguments must exit non-zero.
- startup should log the bound address.
- daemon process runs until interrupted or until the supplied shutdown signal is
  triggered by tests.

## Health API Contract

Add this route:

```http
GET /health
```

Response requirements:

- HTTP status: `200 OK`
- content type: JSON
- exact fields:
  - `status: "ok"`
  - `service: "opendaemon"`
  - `version: env!("CARGO_PKG_VERSION")`

Expected JSON shape:

```json
{"status":"ok","service":"opendaemon","version":"0.1.0"}
```

The response must not include provider, runtime, task, directory grant, registry,
or control-plane status in Phase 0.

## Source Layout

Expected source layout after Phase 0:

```text
src/
  main.rs
  lib.rs
  api/
    mod.rs
    health.rs
  cli/
    mod.rs
  config/
    mod.rs
  runtime/
    mod.rs
  registry/
    mod.rs
  scheduler/
    mod.rs
  security/
    mod.rs
  store/
    mod.rs
  task/
    mod.rs
  tests/
    mod.rs
```

### File Responsibilities

- `src/main.rs`
  - initialize logging
  - start CLI execution
  - convert errors into process exit behavior
  - avoid daemon business logic

- `src/cli/mod.rs`
  - define `clap` parser types
  - define `daemon` subcommand arguments
  - provide command dispatch
  - keep default host and port centralized with config types

- `src/api/mod.rs`
  - build and return the `axum` router
  - register `GET /health`
  - avoid provider, task, registry, or control-plane routes

- `src/api/health.rs`
  - define the health response type
  - implement the health handler
  - keep the JSON response stable and testable without binding a socket

- `src/config/mod.rs`
  - define daemon bind configuration
  - expose defaults for `127.0.0.1:19514`
  - allow port `0` for tests

- `src/registry/mod.rs`
  - placeholder module comment only
  - no registry directory creation
  - no provider manifest types

- `src/runtime/mod.rs`
  - placeholder module comment only
  - no runtime detection
  - no provider process management

- `src/scheduler/mod.rs`
  - placeholder module comment only
  - no task scheduler or task execution

- `src/security/mod.rs`
  - placeholder module comment only
  - no directory grants or product authentication

- `src/store/mod.rs`
  - placeholder module comment only
  - no database setup or migrations

- `src/task/mod.rs`
  - placeholder module comment only
  - no task API or lifecycle model

Placeholder modules should avoid unused public APIs. A short module-level comment
is enough when the module has no Phase 0 behavior.

## Implementation Steps

### Step 0.1: Rename Package Identity

Update `Cargo.toml`:

- `package.name = "opendaemon"`
- authors use neutral project metadata, for example
  `OpenDaemon contributors`
- cargo-release README replacement snippets use `opendaemon`

Update only necessary user-facing template text:

- `README.md`
- `CHANGELOG.md`
- `docs/README.md` if needed

Acceptance:

- `cargo metadata` reports package name `opendaemon`
- no user-facing template package name remains except in historical notes

### Step 0.2: Standardize E2E Directory Casing

The template currently uses `E2E/`, while the `justfile` references `e2e`.
Choose lowercase `e2e/` for cross-platform consistency with the `justfile`.

Tasks:

- rename `E2E/` to `e2e/`
- update references if any still point to `E2E`
- keep `e2e/README.md`, `e2e/pyproject.toml`, and `e2e/uv.lock`

Acceptance:

- `just init-e2e` points to an existing directory
- `just e2e` points to an existing directory

### Step 0.3: Add CLI Contract

Add a minimal CLI with `clap`.

Required supported commands:

```text
opendaemon --version
opendaemon daemon
opendaemon daemon --host 127.0.0.1 --port 19514
opendaemon daemon --host 127.0.0.1 --port 0
```

Acceptance:

- `cargo run -- --version` prints the package version
- `daemon` uses `127.0.0.1:19514` by default
- `daemon --host <host> --port <port>` overrides both defaults
- `daemon --port 0` is accepted for tests
- invalid CLI arguments return a non-zero exit code

### Step 0.4: Add Health API

Implement `GET /health` with the contract above.

Acceptance:

- health handler can be tested without binding a real socket
- `GET /health` returns HTTP 200
- JSON is exactly:

```json
{"status":"ok","service":"opendaemon","version":"<package version>"}
```

### Step 0.5: Add Daemon Server Runtime

Implement the daemon runner around the `axum` router.

Requirements:

- bind using the daemon config
- allow `127.0.0.1:0` for tests
- log the final bound address
- shut down the HTTP server cleanly when interrupted
- expose a testable path that accepts an injected shutdown signal

Acceptance:

- daemon can bind `127.0.0.1:0` in a test
- test can trigger shutdown without killing the test process

### Step 0.6: Add Module Skeleton

Create placeholder modules for later phases:

- `registry`
- `runtime`
- `scheduler`
- `security`
- `store`
- `task`

Acceptance:

- module tree compiles
- no clippy warnings are introduced
- placeholders do not expose unused public APIs

### Step 0.7: Keep Template Constraints Working

Preserve and use the existing project constraints:

- `rustfmt.toml`
- `typos.toml`
- `.pre-commit-config.yaml`
- `justfile`
- GitHub Actions workflow

Update command names only where the package rename requires it.

Acceptance:

- `cargo fmt --all -- --check` passes
- `cargo clippy --tests --all-targets --all-features -- -D warnings` passes
- `cargo test --all-features --all-targets` passes

## Test Plan

Add tests for:

- CLI parser accepts `daemon`
- CLI parser defaults to `127.0.0.1:19514`
- CLI parser accepts `--host` and `--port`
- CLI parser accepts `--port 0`
- invalid CLI arguments fail parsing
- health handler returns stable JSON
- axum router serves `GET /health`
- daemon can bind `127.0.0.1:0` and stop through a shutdown signal

Tests must not depend on port `19514` being available.

## Manual Verification

Run these commands before completing Phase 0:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
cargo run -- --version
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/health
```

Expected curl response:

```json
{"status":"ok","service":"opendaemon","version":"0.1.0"}
```

## Completion Checklist

- [ ] `Cargo.toml` package is renamed to `opendaemon`.
- [ ] Package authors use neutral project metadata.
- [ ] Release replacement snippets use `opendaemon`.
- [ ] User-facing template identity is replaced where necessary.
- [ ] `E2E/` is standardized to `e2e/`.
- [ ] CLI supports `--version`.
- [ ] CLI supports `daemon --host <host> --port <port>`.
- [ ] CLI defaults are `127.0.0.1:19514`.
- [ ] CLI accepts `--port 0` for tests.
- [ ] `GET /health` returns the stable JSON contract.
- [ ] Health response contains no later-phase status fields.
- [ ] Module skeleton exists for roadmap phases.
- [ ] Placeholder modules do not expose unused public APIs.
- [ ] Tests cover CLI, health API, router, and daemon shutdown.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --tests --all-targets --all-features -- -D warnings`
  passes.
- [ ] `cargo test --all-features --all-targets` passes.

## Handoff to Phase 1

Phase 1 can start when the daemon has a stable CLI, a running local HTTP server,
stable `GET /health`, and clean quality gates.

The next phase should add:

- provider manifest types
- registry directory layout
- schema validation
- provider API routes
- `just registry-check`
