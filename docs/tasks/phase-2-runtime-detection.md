# Phase 2: Runtime Detection

## Goal

Discover installed local provider CLIs from provider manifests, run bounded
version probes, store the latest in-memory runtime status, and expose runtime
metadata through the local HTTP API.

Phase 2 builds on Phase 1. It adds the runtime detection contract and read-only
runtime API surface:

- command detection from `ProviderManifest.detect`
- provider-specific environment variable path overrides
- bounded version command execution
- in-memory runtime status storage
- `GET /v1/runtimes`
- `POST /v1/runtimes/detect`
- fake command fixtures for detection tests

This phase must not execute provider tasks, create Agent Profiles, grant
directories, manage provider secrets, render task command templates, schedule
work, or connect to the remote control plane.

## Scope

Phase 2 delivers only local CLI runtime detection behavior:

- detect local CLI providers from committed registry manifests
- resolve executable paths from explicit environment overrides or `PATH`
- run provider version commands without a shell
- enforce per-command timeouts
- parse versions from manifest `version_regex` when present
- report unavailable providers when commands are missing
- report detection errors without crashing the daemon
- keep detection state in memory for the current daemon process
- expose runtime status through dedicated runtime API routes
- add focused tests using fake commands
- quality gates passing

Runtime status is discovery state only. It must not imply task capacity,
directory authorization, product authorization, provider authentication, or
remote control-plane availability.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 1 implementation:
  - `src/api/mod.rs`
  - `src/api/providers.rs`
  - `src/registry/mod.rs`
  - `src/registry/manifest.rs`
  - `registry/providers/*/manifest.json`
  - `schemas/provider-manifest.schema.json`
  - `justfile`

## Deliverables

- Runtime model types exist for local CLI detection.
- Runtime detection reads provider manifests from the registry.
- Detection only attempts providers whose `integration_type` is `cli`.
- Environment override path support exists.
- Detection searches manifest `detect.commands` through `PATH`.
- Version probes use manifest `detect.version_args`.
- Version probes are killed and reaped after a timeout.
- Version parsing uses manifest `detect.version_regex` when present.
- Missing commands are reported as unavailable, not as daemon errors.
- Failed or timed-out version probes are reported as provider-level detection
  errors.
- Latest runtime statuses are stored in memory for the daemon process.
- `GET /v1/runtimes` returns the latest known runtime statuses.
- `POST /v1/runtimes/detect` runs detection and updates runtime statuses.
- Runtime API responses include executable path and parsed version when
  available.
- Runtime API responses do not include provider secrets, environment variable
  values, directory grants, task counts, or task capacity claims.
- Fake command fixtures or helpers cover successful detection, missing command,
  version parsing, and timeout behavior.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 2:

- provider task execution
- `POST /v1/tasks`
- task scheduler or worker loop
- task event model
- command template rendering for task prompts
- stdin/temp-file prompt delivery
- stdout/stderr task streaming
- cancellation of long-running provider tasks
- Agent Profiles
- directory grants
- path guards
- workspace or git worktree creation
- SQLite persistence
- keyring or secret storage
- product authentication
- provider installation management
- remote HTTP runtime execution
- ACP sessions
- native provider adapters
- file watching or automatic registry reload
- control plane
- desktop UI

Phase 2 may spawn short-lived version commands only. It must not spawn providers
to perform agent work.

## Dependencies

Keep Phase 0 and Phase 1 dependencies. Add only what is needed for command
discovery, version parsing, timestamps, and async process timeouts.

Extend the existing `tokio` dependency with the needed features:

```toml
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "signal", "process", "sync", "time"] }
```

Add runtime detection dependencies:

```toml
regex = "1"
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }
which = "7"
```

Add test-only support if needed:

```toml
[dev-dependencies]
tempfile = "3"
```

If a current stable crate API differs at implementation time, use the current
stable API and keep the dependency purpose unchanged.

Do not add SQLite, websocket, keyring, provider task execution, template
rendering, PTY, file watching, or control-plane dependencies in Phase 2.

## Runtime Detection Contract

### Runtime Kind

Phase 2 supports one runtime kind:

- `local_cli`: a provider command installed on the user's machine

Do not add `remote_http`, `acp`, or `native` runtime detection in Phase 2.

### Runtime Status

Define a runtime status enum with these API values:

- `not_detected`: detection has not run in this daemon process
- `available`: an executable was found and the version probe succeeded or was
  skipped because no version args were configured
- `unavailable`: no configured command was found
- `error`: an executable was found, but detection failed

`available` means the CLI exists. It does not mean the provider has credentials,
task capacity, directory access, or permission to execute a task.

### Runtime Identity

Use deterministic local CLI runtime IDs:

```text
rt_<normalized_provider_id>_local_cli
```

Normalization rules:

- lowercase the provider ID
- replace non-ASCII-alphanumeric characters with `_`
- collapse repeated `_`

Examples:

- `codex` -> `rt_codex_local_cli`
- `generic-test-provider` -> `rt_generic_test_provider_local_cli`

Phase 2 should expose at most one local CLI runtime per CLI provider. If a
future phase needs multiple installations per provider, it can add a suffix
without changing the Phase 2 detection algorithm.

### Environment Override

Detection must support a provider-specific executable override:

```text
OPENDAEMON_PROVIDER_<NORMALIZED_PROVIDER_ID>_PATH
```

Normalization rules for the environment variable provider segment:

- uppercase the provider ID
- replace non-ASCII-alphanumeric characters with `_`
- collapse repeated `_`

Examples:

- `codex` -> `OPENDAEMON_PROVIDER_CODEX_PATH`
- `generic-test-provider` -> `OPENDAEMON_PROVIDER_GENERIC_TEST_PROVIDER_PATH`

Discovery order for each CLI provider:

1. If the provider-specific override is set, use that path.
2. Otherwise, search manifest `detect.commands` in order through `PATH`.
3. If none are found, report `unavailable`.

Override requirements:

- the override is a path to an executable, not a shell command string
- no shell splitting is performed
- if the override is set but invalid, report `error`
- do not silently fall back to `PATH` when an explicit override is invalid
- do not expose the override environment variable value in error messages

### Command Resolution

Manifest `detect.commands` are command names. Detection must resolve them with
`PATH` semantics and execute the resolved path directly.

Requirements:

- do not invoke a shell
- preserve manifest command ordering
- prefer the first command that resolves successfully
- include the resolved executable path in successful runtime responses
- report missing commands as `unavailable`
- include a useful provider ID and error code in detection errors

### Version Probe

After resolving an executable, run:

```text
<executable> <detect.version_args...>
```

Requirements:

- run the process without a shell
- pass arguments exactly as manifest strings
- capture stdout and stderr
- enforce a per-command timeout
- kill and reap the process after timeout
- reject non-zero exits as detection `error`
- parse stdout and stderr together for version output
- do not inherit provider secrets or task-scoped credentials

Default timeout:

- `2 seconds` per version command

The timeout should be configurable through a Rust config type for tests. Do not
add a public CLI flag for it in Phase 2.

Version parsing rules:

- if `detect.version_regex` is present, apply it to combined stdout and stderr
- if the regex has a named capture group `version`, use it
- otherwise, if the regex has a first capture group, use that group
- otherwise, use the full regex match
- if no regex is present, use the first non-empty output line
- trim whitespace from the parsed version
- if no version can be parsed, report detection `error`

If `detect.version_args` is empty:

- skip the version command
- set `version` to `null`
- report the runtime as `available`

## Runtime API Contract

Add these routes:

```http
GET /v1/runtimes
POST /v1/runtimes/detect
```

Both routes return JSON.

### Runtime Response Shape

Use this runtime object shape:

```json
{
  "id": "rt_codex_local_cli",
  "provider_id": "codex",
  "kind": "local_cli",
  "status": "available",
  "executable": "/opt/homebrew/bin/codex",
  "version": "1.2.3",
  "detected_at": "2026-05-29T00:00:00Z",
  "error": null
}
```

For unavailable providers:

```json
{
  "id": "rt_claude_local_cli",
  "provider_id": "claude",
  "kind": "local_cli",
  "status": "unavailable",
  "executable": null,
  "version": null,
  "detected_at": "2026-05-29T00:00:00Z",
  "error": {
    "code": "command_not_found",
    "message": "no configured detect command was found"
  }
}
```

For providers that have not been detected in this daemon process:

```json
{
  "id": "rt_codex_local_cli",
  "provider_id": "codex",
  "kind": "local_cli",
  "status": "not_detected",
  "executable": null,
  "version": null,
  "detected_at": null,
  "error": null
}
```

Error codes should be stable enough for tests:

- `command_not_found`
- `override_not_executable`
- `version_timeout`
- `version_command_failed`
- `version_parse_failed`
- `unsupported_provider_integration`
- `registry_error`

Only provider-level detection problems should appear inside runtime objects.
Route-level `500` errors should be reserved for failures that prevent the route
from loading registry metadata or building a response at all.

### `GET /v1/runtimes`

Response requirements:

- HTTP status: `200 OK`
- content type: JSON
- response shape:

```json
{
  "runtimes": []
}
```

Behavior:

- load registry provider metadata
- include one local CLI runtime entry for each CLI provider
- merge the latest in-memory status if detection has run
- return `not_detected` for CLI providers with no stored status yet
- sort runtimes by provider ID
- do not spawn commands
- do not block on detection

### `POST /v1/runtimes/detect`

Response requirements:

- HTTP status: `200 OK`
- content type: JSON
- response shape:

```json
{
  "runtimes": []
}
```

Behavior:

- load registry provider metadata
- detect every CLI provider
- update the in-memory runtime store
- return the updated runtime list sorted by provider ID
- report missing commands as `unavailable`
- report provider-level version errors in the runtime object
- complete even when some providers are unavailable or fail version probing

`POST /v1/runtimes/detect` should not accept provider task prompts, directory
IDs, Agent Profile IDs, custom args, custom environment, or raw executable
commands in Phase 2.

## Source Layout

Expected source layout after Phase 2:

```text
src/
  api/
    mod.rs
    health.rs
    providers.rs
    runtimes.rs
  config/
    mod.rs
  runtime/
    mod.rs
    detect.rs
    model.rs
    store.rs
  tests/
    mod.rs
    api.rs
    cli.rs
    registry.rs
    runtime.rs
```

### File Responsibilities

- `src/api/mod.rs`
  - keep `GET /health`
  - keep provider API routes
  - register runtime API routes
  - provide a testable router construction path with injected state

- `src/api/runtimes.rs`
  - define runtime API response DTOs
  - implement `GET /v1/runtimes`
  - implement `POST /v1/runtimes/detect`
  - map route-level failures to stable JSON errors
  - avoid task execution behavior

- `src/config/mod.rs`
  - keep daemon bind configuration
  - define runtime detection timeout defaults
  - allow tests to inject shorter detection timeouts

- `src/runtime/mod.rs`
  - expose runtime model, detection, and store entry points
  - keep API-specific DTOs out of runtime internals when practical

- `src/runtime/model.rs`
  - define runtime IDs, kind, status, error, and detection result types
  - define serialization values used by API responses
  - avoid provider task execution state

- `src/runtime/detect.rs`
  - resolve environment overrides and `PATH` commands
  - run version probes with timeout
  - parse version output
  - return provider-level detection results
  - avoid shell invocation

- `src/runtime/store.rs`
  - store latest runtime statuses in memory
  - support safe concurrent access from API handlers
  - avoid SQLite or filesystem persistence

- `src/tests/runtime.rs`
  - cover command resolution, version parsing, timeout, missing command, and
    runtime store behavior
  - use temporary fake commands
  - serialize environment-variable mutation where needed

Do not split into workspace crates in Phase 2. Keep the single-crate shape from
Phase 0 and Phase 1.

## Application State

Runtime detection needs shared daemon state. Add a small application state type
rather than using globals.

Requirements:

- runtime store is shared by API handlers
- detection config is injectable for tests
- registry path is injectable for tests
- default router construction remains simple for `main.rs`
- existing health and provider tests stay straightforward

Acceptable shape:

```text
AppState
  registry_root or providers_dir
  runtime_store
  runtime_detection_config
```

The exact type names can follow the surrounding code, but state must be explicit
and testable.

## Implementation Steps

### Step 2.1: Add Runtime Model Types

Add `src/runtime/model.rs`.

Acceptance:

- local CLI runtime IDs are deterministic
- provider ID normalization is tested
- runtime status serializes to the API values above
- runtime errors serialize with stable `code` and `message`
- runtime objects do not include provider secrets or environment values

### Step 2.2: Add In-Memory Runtime Store

Add `src/runtime/store.rs`.

Acceptance:

- store can save and return the latest status by provider ID
- store returns `not_detected` for CLI providers with no status
- store output is sorted by provider ID when exposed through API helpers
- concurrent API access is safe
- no filesystem or SQLite persistence is introduced

### Step 2.3: Add Command Resolution

Add command resolution in `src/runtime/detect.rs`.

Acceptance:

- provider-specific environment override is checked before `PATH`
- valid override path resolves successfully
- invalid override path returns `override_not_executable`
- invalid override does not fall back to `PATH`
- manifest commands are searched in order through `PATH`
- missing commands return `command_not_found`
- resolved commands are executed by path, not through a shell

### Step 2.4: Add Version Probe and Parsing

Extend `src/runtime/detect.rs`.

Acceptance:

- version command uses resolved executable plus manifest `version_args`
- stdout and stderr are captured
- non-zero exit returns `version_command_failed`
- timeout returns `version_timeout`
- timed-out child process is killed and reaped
- named `version` regex capture is parsed
- first regex capture fallback is parsed
- no-regex output fallback is parsed
- unparseable version output returns `version_parse_failed`
- empty `version_args` skips the command and returns `available` with
  `version = null`

### Step 2.5: Add Runtime Detection Orchestration

Add a registry-to-runtime detection entry point.

Acceptance:

- detection loads provider manifests through existing registry code
- only `integration_type = "cli"` providers are detected
- non-CLI providers are skipped or reported as unsupported only if they appear
  in an explicit detection path
- each provider result is isolated so one failure does not abort all detection
- detection has bounded per-command runtime
- daemon startup does not run detection

### Step 2.6: Add Runtime API Routes

Add `src/api/runtimes.rs` and wire routes in `src/api/mod.rs`.

Acceptance:

- `GET /v1/runtimes` returns `not_detected` entries before detection runs
- `GET /v1/runtimes` does not spawn commands
- `POST /v1/runtimes/detect` runs detection
- `POST /v1/runtimes/detect` updates in-memory statuses
- missing commands are returned as provider-level `unavailable` statuses
- detected commands include executable path and version
- runtimes are sorted by provider ID
- response JSON does not contain directory, task, secret, or capacity fields

### Step 2.7: Add Fake Command Tests

Add fake command helpers or fixtures for detection tests.

Acceptance:

- tests can create a fake executable in a temporary directory
- tests can make the fake command print a version
- tests can make the fake command print version output to stderr
- tests can make the fake command exit non-zero
- tests can make the fake command sleep past the timeout
- tests do not require real `codex` or `claude` installations
- tests that mutate `PATH` or provider override environment variables are
  serialized with a shared test lock

### Step 2.8: Keep Existing Provider Registry Behavior Stable

Preserve Phase 1 behavior while adding runtime detection.

Acceptance:

- `GET /v1/providers` still returns normalized manifests
- `GET /v1/providers/:provider_id` still returns one normalized manifest
- provider API responses do not gain runtime status fields in Phase 2
- `just registry-check` still validates committed fixtures and schema freshness

## Test Plan

Add tests for:

- runtime ID normalization
- environment override variable name normalization
- override path resolves before `PATH`
- invalid override path returns `override_not_executable`
- invalid override path does not fall back to `PATH`
- manifest commands are searched in order
- missing commands return `command_not_found`
- version command receives manifest `version_args`
- version parsing with named `version` capture
- version parsing with first capture fallback
- version parsing with no-regex first-line fallback
- version output can come from stderr
- non-zero version command returns `version_command_failed`
- timed-out version command returns `version_timeout`
- timed-out process is killed and reaped
- empty `version_args` returns available runtime with null version
- runtime store returns `not_detected` before detection
- runtime store updates after detection
- `GET /v1/runtimes` returns sorted `not_detected` runtimes before detection
- `GET /v1/runtimes` does not execute fake commands
- `POST /v1/runtimes/detect` detects fake commands
- `POST /v1/runtimes/detect` reports missing commands as unavailable
- runtime API responses include executable path and parsed version when
  available
- runtime API responses do not include provider secrets, environment variable
  values, directory grants, task counts, or task capacity fields
- provider API responses remain manifest-only
- `just registry-check` still passes

Tests must use temporary registry or fake command directories for invalid and
environment-specific cases. Tests must not depend on real provider CLIs being
installed on the developer machine.

## Manual Verification

Run these commands before completing Phase 2:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/runtimes
curl -X POST http://127.0.0.1:19514/v1/runtimes/detect
curl http://127.0.0.1:19514/v1/runtimes
```

Expected behavior:

- quality gates pass
- registry check exits `0`
- daemon startup does not run provider detection
- first runtime list returns `not_detected` entries for CLI providers
- detection request completes within bounded time
- missing provider CLIs appear as `unavailable`
- installed or fake provider CLIs appear as `available`
- detected runtimes include executable path and parsed version

For manual fake command testing, prefer an environment override such as:

```bash
OPENDAEMON_PROVIDER_GENERIC_TEST_PROVIDER_PATH=/tmp/opendaemon-fake-provider \
  cargo run -- daemon --host 127.0.0.1 --port 19514
```

The fake command should implement the provider manifest's `version_args` and
print a version string that matches `detect.version_regex`.

## Completion Checklist

- [ ] Runtime model types exist.
- [ ] Runtime status values are stable and tested.
- [ ] Runtime IDs are deterministic.
- [ ] Provider-specific environment override names are deterministic.
- [ ] Environment overrides are checked before `PATH`.
- [ ] Invalid environment overrides do not fall back to `PATH`.
- [ ] Manifest detect commands are searched in order.
- [ ] Missing commands return provider-level `unavailable` status.
- [ ] Version probes run without a shell.
- [ ] Version probes enforce a timeout.
- [ ] Timed-out version probes are killed and reaped.
- [ ] Version regex parsing is tested.
- [ ] No-regex version output parsing is tested.
- [ ] Empty `version_args` is supported.
- [ ] Runtime detection only handles CLI providers.
- [ ] Runtime status is stored in memory.
- [ ] Daemon startup does not run detection.
- [ ] `GET /v1/runtimes` exists.
- [ ] `GET /v1/runtimes` returns `not_detected` before detection.
- [ ] `POST /v1/runtimes/detect` exists.
- [ ] `POST /v1/runtimes/detect` updates runtime statuses.
- [ ] Runtime API responses are sorted by provider ID.
- [ ] Runtime API responses include executable path and version when available.
- [ ] Runtime API responses do not include secrets, grants, task counts, or
  capacity claims.
- [ ] Provider API responses remain manifest-only.
- [ ] Fake command tests cover success, missing command, parse failure,
  non-zero exit, and timeout.
- [ ] Tests do not require real provider CLIs.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --tests --all-targets --all-features -- -D warnings`
  passes.
- [ ] `cargo test --all-features --all-targets` passes.
- [ ] `just registry-check` passes.

## Handoff to Phase 3

Phase 3 can start when provider manifests load, local CLI runtimes can be
detected with bounded version probes, runtime status is available through
dedicated API routes, and quality gates are clean.

The next phase should add:

- directory grant model
- path canonicalization
- SQLite persistence for grants
- product, agent, directory, and capability enforcement
- `worktree` and `direct` workspace mode policy
- directory API routes
- path guard tests for traversal and symlink behavior
