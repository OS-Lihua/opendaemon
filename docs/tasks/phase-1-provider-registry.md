# Phase 1: Provider Registry

## Goal

Load provider manifests from the local repository registry, validate them, and
expose normalized provider metadata through the local HTTP API.

Phase 1 builds on Phase 0. It adds the provider registry contract and read-only
provider API surface:

- `registry/providers/<provider-id>/manifest.json`
- provider manifest validation
- `GET /v1/providers`
- `GET /v1/providers/:provider_id`
- `just registry-check`

This phase must not discover installed provider CLIs, execute providers, create
Agent Profiles, grant directories, schedule tasks, manage secrets, or connect to
the remote control plane.

## Scope

Phase 1 delivers only local registry behavior:

- repository registry directory layout
- `ProviderManifest` data model
- JSON schema generation
- manifest validation
- duplicate provider ID detection
- provider fixtures for `codex`, `claude`, and one generic test provider
- read-only provider API routes
- registry PR requirements documentation
- `just registry-check`
- quality gates passing

Provider runtime status must remain out of scope. Provider API responses can
include registry metadata, but they must not claim whether provider CLIs are
installed or online.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 0 implementation:
  - `Cargo.toml`
  - `src/api/mod.rs`
  - `src/registry/mod.rs`
  - `src/tests/mod.rs`
  - `justfile`

## Deliverables

- `registry/providers/` exists.
- Provider fixtures exist for:
  - `codex`
  - `claude`
  - `generic-test-provider`
- Each fixture includes:
  - `manifest.json`
  - `README.md`
  - `examples/basic.task.json`
- `schemas/provider-manifest.schema.json` is generated and committed.
- `ProviderManifest` and supporting types are defined.
- Registry loading reads all manifests under `registry/providers/<id>/`.
- Validation rejects malformed manifests.
- Validation rejects duplicate manifest IDs.
- Validation rejects directory IDs that do not match the provider directory name.
- `GET /v1/providers` returns normalized provider manifests.
- `GET /v1/providers/:provider_id` returns one normalized provider manifest.
- Missing providers return HTTP `404`.
- `just registry-check` validates registry fixtures and schema freshness.
- Registry PR requirements are documented.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 1:

- runtime detection
- `GET /v1/runtimes`
- `POST /v1/runtimes/detect`
- provider command execution
- command template rendering
- installed CLI version parsing
- Agent Profiles
- directory grants
- workspace policy
- task API
- task scheduler
- task-time provider override validation
- database persistence
- keyring or secret storage
- provider permission events
- ACP adapter
- remote HTTP provider adapter
- control plane
- desktop UI

## Dependencies

Keep Phase 0 dependencies and add only what is needed for manifest data, schema
generation, and validation:

```toml
serde_json = "1"
schemars = { version = "1", features = ["derive"] }
jsonschema = "0.33"
```

If the latest `schemars` or `jsonschema` API differs at implementation time,
use the current stable API and keep the dependency purpose unchanged.

Do not add SQLite, websocket, keyring, command execution, runtime detection,
template rendering, file watching, or provider process dependencies in Phase 1.

## Registry Layout

Add this repository layout:

```text
registry/
  providers/
    codex/
      manifest.json
      README.md
      examples/
        basic.task.json
    claude/
      manifest.json
      README.md
      examples/
        basic.task.json
    generic-test-provider/
      manifest.json
      README.md
      examples/
        basic.task.json
schemas/
  provider-manifest.schema.json
docs/
  registry.md
```

Only these files are required for a provider registry PR:

- `registry/providers/<provider-id>/manifest.json`
- `registry/providers/<provider-id>/README.md`
- `registry/providers/<provider-id>/examples/basic.task.json`

Optional future registry assets, detection tests, execution tests, and logos are
not required in Phase 1.

## Manifest Contract

Define `ProviderManifest` with these required fields:

- `schema_version: String`
- `id: String`
- `display_name: String`
- `status: ProviderStatus`
- `vendor: VendorInfo`
- `integration_type: IntegrationType`
- `description: String`
- `install: InstallInstructions`
- `detect: DetectConfig`
- `execution: ExecutionConfig`
- `models: ModelConfig`
- `capabilities: ProviderCapabilities`
- `permissions: ProviderPermissions`
- `environment: EnvironmentConfig`
- `security: SecurityConfig`

### Enum Values

`ProviderStatus`:

- `community`
- `verified`
- `first_party`
- `deprecated`

`IntegrationType`:

- `cli`
- `acp`
- `http`
- `native`

`ExecutionInputMode`:

- `arg`
- `stdin`
- `temp_file`

`WorkingDirectoryMode`:

- `required`
- `optional`
- `unsupported`

`CancelSignal`:

- `SIGTERM`
- `SIGINT`
- `kill`
- `none`

`DirectoryLockMode`:

- `exclusive`
- `shared`
- `none`

`SecurityReviewLevel`:

- `standard`
- `strict`
- `experimental`

### Object Requirements

`VendorInfo`:

- `name: String`
- `homepage: String`
- `support_url: Option<String>`

`InstallInstructions`:

- `macos: Vec<String>`
- `linux: Vec<String>`
- `windows: Vec<String>`

`DetectConfig`:

- `commands: Vec<String>`
- `version_args: Vec<String>`
- `version_regex: Option<String>`

`ExecutionConfig`:

- `command: String`
- `args: Vec<String>`
- `input_mode: ExecutionInputMode`
- `working_directory: WorkingDirectoryMode`
- `supports_streaming: bool`
- `cancel_signal: CancelSignal`

`ModelConfig`:

- `default: String`
- `supported: Vec<String>`

`ProviderCapabilities`:

- `filesystem_read: bool`
- `filesystem_write: bool`
- `shell: bool`
- `git: bool`
- `browser: bool`
- `mcp: bool`
- `remote_execution: bool`
- `worktree: bool`
- `direct_directory: bool`

`ProviderPermissions`:

- `requires_directory_grant: bool`
- `recommended_directory_lock: DirectoryLockMode`
- `provider_permission_modes: Vec<String>`
- `supports_permission_events: bool`

`EnvironmentConfig`:

- `required: Vec<String>`
- `optional: Vec<String>`

`SecurityConfig`:

- `runs_locally: bool`
- `sends_code_to_vendor: bool`
- `data_policy_url: Option<String>`
- `review_level: SecurityReviewLevel`

### Validation Rules

Validation must reject:

- invalid JSON
- missing required fields
- unknown enum values
- empty `id`
- empty `display_name`
- empty `vendor.name`
- empty `description`
- empty `detect.commands`
- empty `execution.command`
- empty `models.default`
- `models.default` not present in `models.supported`
- duplicate `models.supported` values
- duplicate `environment.required` values
- duplicate `environment.optional` values
- environment variable appearing in both `required` and `optional`
- duplicate `provider_permission_modes` values
- duplicate manifest IDs across registry entries
- manifest `id` not matching its provider directory name
- absolute `execution.command` paths
- absolute `detect.commands` paths
- `remote_execution = true` when `security.sends_code_to_vendor = false`

Validation should preserve a useful error path or provider ID in error messages.

## Schema Contract

Generate JSON schema from the Rust manifest types and commit it to:

```text
schemas/provider-manifest.schema.json
```

The schema must:

- describe the required manifest fields
- include enum values
- reject unknown fields if the Rust type uses deny-unknown-fields
- be stable enough for registry contributors to validate manifests locally

`just registry-check` must fail if the committed schema is stale compared with
the schema generated from current Rust types.

## Provider API Contract

Extend the Phase 0 router with these routes:

```http
GET /v1/providers
GET /v1/providers/:provider_id
```

### `GET /v1/providers`

Response requirements:

- HTTP status: `200 OK`
- content type: JSON
- response shape:

```json
{
  "providers": [
    {
      "id": "codex",
      "display_name": "Codex",
      "status": "first_party",
      "integration_type": "cli",
      "description": "...",
      "manifest": {}
    }
  ]
}
```

The `manifest` field should contain the normalized manifest data for that
provider. Provider ordering must be stable and sorted by provider ID.

### `GET /v1/providers/:provider_id`

Success response requirements:

- HTTP status: `200 OK`
- content type: JSON
- response shape:

```json
{
  "provider": {
    "id": "codex",
    "display_name": "Codex",
    "status": "first_party",
    "integration_type": "cli",
    "description": "...",
    "manifest": {}
  }
}
```

Missing provider response:

- HTTP status: `404 Not Found`
- content type: JSON
- response shape:

```json
{
  "error": {
    "code": "provider_not_found",
    "message": "provider not found"
  }
}
```

The provider API must not include runtime status, installed executable paths,
detected versions, online/offline state, directory grants, task counts, or
provider secrets in Phase 1.

## Source Layout

Expected source layout after Phase 1:

```text
src/
  api/
    mod.rs
    health.rs
    providers.rs
  registry/
    mod.rs
    manifest.rs
    schema.rs
    validate.rs
```

### File Responsibilities

- `src/api/mod.rs`
  - keep `GET /health`
  - register provider API routes
  - avoid runtime, task, agent, and directory routes

- `src/api/providers.rs`
  - define provider API response DTOs
  - implement list and get handlers
  - map missing providers to `404`
  - avoid runtime detection

- `src/registry/mod.rs`
  - expose registry loading and validation entry points
  - keep filesystem traversal details out of API handlers

- `src/registry/manifest.rs`
  - define `ProviderManifest` and supporting manifest types
  - derive `Serialize`, `Deserialize`, and `JsonSchema`
  - use snake_case JSON naming

- `src/registry/schema.rs`
  - generate provider manifest JSON schema
  - support schema freshness checks for `just registry-check`

- `src/registry/validate.rs`
  - validate loaded manifests
  - validate cross-file registry invariants such as duplicate IDs
  - return useful validation errors

Do not split into workspace crates in Phase 1. Keep the single-crate shape from
Phase 0.

## CLI and Justfile Contract

Add a just recipe:

```just
registry-check:
    cargo run -- registry-check
```

Add a hidden or documented CLI command for the recipe:

```text
opendaemon registry-check
```

CLI requirements:

- validate `registry/providers`
- validate `schemas/provider-manifest.schema.json` freshness
- exit `0` when registry is valid
- exit non-zero when any manifest, duplicate ID, required file, or schema
  freshness check fails

Do not add provider install, provider detect, or provider execution CLI commands
in Phase 1.

## Implementation Steps

### Step 1.1: Add Manifest Types

Add `src/registry/manifest.rs`.

Acceptance:

- manifest JSON deserializes into typed Rust structs
- manifest structs serialize back to stable snake_case JSON
- unknown enum values are rejected
- unit tests cover valid and invalid enum values

### Step 1.2: Add Schema Generation

Add `src/registry/schema.rs`.

Acceptance:

- schema can be generated from `ProviderManifest`
- `schemas/provider-manifest.schema.json` exists
- schema includes required fields and enum values
- stale schema detection is testable

### Step 1.3: Add Registry Loader and Validation

Add filesystem loading and validation under `src/registry/`.

Acceptance:

- loader reads `registry/providers/*/manifest.json`
- loader requires `README.md`
- loader requires `examples/basic.task.json`
- invalid JSON fails validation
- missing required files fail validation
- duplicate IDs fail validation
- manifest ID must match directory name
- absolute command paths fail validation

### Step 1.4: Add Provider Fixtures

Add fixtures:

- `registry/providers/codex/`
- `registry/providers/claude/`
- `registry/providers/generic-test-provider/`

Acceptance:

- all three fixtures pass `just registry-check`
- each fixture has a useful README
- each fixture has a minimal valid `examples/basic.task.json`
- fixtures do not claim local access is granted by the registry

### Step 1.5: Add Provider API

Add `src/api/providers.rs` and wire routes in `src/api/mod.rs`.

Acceptance:

- `GET /v1/providers` returns all fixture providers sorted by ID
- `GET /v1/providers/:provider_id` returns the requested provider
- missing provider returns `404` with stable error JSON
- responses do not include runtime detection status

### Step 1.6: Add `registry-check`

Add CLI dispatch and just recipe.

Acceptance:

- `just registry-check` passes for committed fixtures
- broken fixture tests can assert non-zero validation behavior without modifying
  committed registry files

### Step 1.7: Document Registry PR Requirements

Add `docs/registry.md`.

Acceptance:

- documents required provider registry PR files
- documents `just registry-check`
- states that registry capabilities are declarations, not authorization grants
- states that providers must disclose remote code upload behavior

## Test Plan

Add tests for:

- valid manifest deserializes
- missing required field fails
- unknown enum value fails
- `models.default` must be in `models.supported`
- duplicate model names fail
- duplicate environment variables fail
- environment variable cannot be both required and optional
- duplicate provider permission modes fail
- absolute `execution.command` fails
- absolute `detect.commands` fails
- manifest ID must match directory name
- duplicate provider IDs fail registry validation
- missing `README.md` fails registry validation
- missing `examples/basic.task.json` fails registry validation
- committed fixtures load successfully
- generated schema matches committed schema
- provider list API returns sorted providers
- provider get API returns one provider
- provider get API returns `404` for an unknown provider
- provider API responses do not contain runtime status fields
- `registry-check` command exits successfully for committed fixtures

Tests must use temporary registry directories for invalid fixture cases.

## Manual Verification

Run these commands before completing Phase 1:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
cargo run -- registry-check
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/providers
curl http://127.0.0.1:19514/v1/providers/codex
```

Expected behavior:

- registry check exits `0`
- provider list includes `claude`, `codex`, and `generic-test-provider`
- provider list is sorted by provider ID
- provider get returns the requested manifest
- unknown provider returns `404`

## Completion Checklist

- [ ] `ProviderManifest` and supporting types exist.
- [ ] Manifest schema generation exists.
- [ ] `schemas/provider-manifest.schema.json` is committed.
- [ ] Registry loader reads `registry/providers/*/manifest.json`.
- [ ] Registry validation rejects malformed manifests.
- [ ] Registry validation rejects duplicate provider IDs.
- [ ] Registry validation rejects manifest ID and directory mismatches.
- [ ] Registry validation requires `README.md`.
- [ ] Registry validation requires `examples/basic.task.json`.
- [ ] Fixtures exist for `codex`, `claude`, and `generic-test-provider`.
- [ ] `GET /v1/providers` returns normalized manifests sorted by ID.
- [ ] `GET /v1/providers/:provider_id` returns one normalized manifest.
- [ ] Missing provider returns stable `404` JSON.
- [ ] Provider API has no runtime detection status fields.
- [ ] `opendaemon registry-check` exists.
- [ ] `just registry-check` exists.
- [ ] `docs/registry.md` documents PR requirements.
- [ ] Tests cover manifest, validation, schema, API, and registry-check behavior.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --tests --all-targets --all-features -- -D warnings`
  passes.
- [ ] `cargo test --all-features --all-targets` passes.
- [ ] `just registry-check` passes.

## Handoff to Phase 2

Phase 2 can start when provider manifests load from the local registry, registry
validation is enforced, provider metadata is available through read-only API
routes, and quality gates are clean.

The next phase should add:

- runtime detection from provider manifests
- installed provider CLI discovery
- `GET /v1/runtimes`
- `POST /v1/runtimes/detect`
- fake command fixtures for detection tests
