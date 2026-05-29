# Phase 0: Project Foundation

## Goal

Turn the copied Rust template into an OpenDaemon foundation without building the
full runtime yet.

Phase 0 should establish the project identity, development commands, module
layout, and the smallest runnable daemon surface:

- `opendaemon --version`
- `opendaemon daemon`
- `GET /health`

This phase should not implement provider registry loading, runtime detection,
directory grants, Agent Profiles, task scheduling, worktrees, ACP, or remote
control-plane dispatch. Those start in later phases.

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

- Package metadata is renamed from `template` to `opendaemon`.
- The template's formatting, linting, and pre-commit constraints remain active.
- The test directory casing is standardized.
- Initial module skeleton exists.
- The CLI can print version information.
- The daemon can start a local HTTP server.
- `GET /health` returns a stable JSON response.
- Basic tests cover the version and health contract.

## Non-Goals

- No provider manifest schema.
- No provider registry loading.
- No agent CLI discovery.
- No Agent Profile persistence.
- No directory grant persistence.
- No task scheduler.
- No worktree creation.
- No ACP adapter.
- No remote control plane.
- No desktop UI.

## Recommended Dependencies

Add only the dependencies needed for this phase:

- `anyhow`: error handling, already present.
- `clap`: CLI parsing.
- `tokio`: async runtime.
- `axum`: local HTTP server.
- `serde`: response serialization.
- `tracing`: structured logs.
- `tracing-subscriber`: log initialization.

Do not add SQLite, registry validation, keychain, websocket, or provider process
dependencies in Phase 0.

## File Plan

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

Most modules can be empty placeholders with a short module-level comment. The
important part is to reserve the boundaries described in the roadmap without
implementing later-phase behavior.

## Implementation Steps

### Step 0.1: Rename Package Identity

Update `Cargo.toml`:

- `package.name = "opendaemon"`
- update authors if the project owner wants project-specific metadata
- update cargo-release README replacement snippets from `template` to
  `opendaemon`

Update user-facing template text:

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

Add a minimal CLI with these commands:

```text
opendaemon --version
opendaemon daemon --host 127.0.0.1 --port 0
```

The `daemon` command should:

- bind to the configured host and port
- print the bound address to logs
- run until interrupted

Use `--port 0` in tests to let the OS choose a free port.

Acceptance:

- `cargo run -- --version` prints the package version
- invalid CLI arguments return a non-zero exit code

### Step 0.4: Add Health API

Add:

```http
GET /health
```

Response:

```json
{
  "status": "ok",
  "service": "opendaemon",
  "version": "0.1.0"
}
```

The response should be intentionally small and stable. Do not include provider
or task details yet.

Acceptance:

- health handler can be tested without binding a real socket
- `GET /health` returns HTTP 200
- JSON contains `status`, `service`, and `version`

### Step 0.5: Add Module Skeleton

Create placeholder modules for later phases:

- `registry`
- `runtime`
- `scheduler`
- `security`
- `store`
- `task`
- `config`

Each module should have a short comment explaining its future responsibility.
Avoid adding unused public APIs just to fill the module.

Acceptance:

- module tree compiles
- no clippy warnings are introduced

### Step 0.6: Keep Template Constraints Working

Preserve and use the existing project constraints:

- `rustfmt.toml`
- `typos.toml`
- `.pre-commit-config.yaml`
- `justfile`
- GitHub Actions workflow

Update command names only where the package rename requires it.

Acceptance:

- `cargo fmt --all` passes
- `cargo clippy --tests --all-targets --all-features -- -D warnings` passes
- `cargo test --all-features --all-targets` passes

## Test Plan

Unit tests:

- health response shape
- CLI parser accepts `daemon`
- CLI parser accepts `--version`

Integration-style tests:

- router serves `GET /health`
- daemon can bind to `127.0.0.1:0` in a spawned task and shut down cleanly

Manual verification:

```bash
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
- [ ] Release replacement snippets use `opendaemon`.
- [ ] `E2E/` is standardized to `e2e/`.
- [ ] CLI supports `--version`.
- [ ] CLI supports `daemon --host <host> --port <port>`.
- [ ] `GET /health` returns the stable JSON contract.
- [ ] Module skeleton exists for roadmap phases.
- [ ] Tests cover CLI and health API.
- [ ] `cargo fmt --all` passes.
- [ ] `cargo clippy --tests --all-targets --all-features -- -D warnings`
  passes.
- [ ] `cargo test --all-features --all-targets` passes.

## Handoff to Phase 1

Phase 1 can start when the daemon has a stable CLI, a running local HTTP server,
and clean quality gates.

The next phase should add:

- provider manifest types
- registry directory layout
- schema validation
- provider API routes
- `just registry-check`
