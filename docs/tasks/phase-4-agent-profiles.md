# Phase 4: Agent Profiles

## Goal

Add durable Agent Profiles so products can reference a stable `agent_id` instead
of sending provider-specific execution configuration with every task.

Phase 4 builds on Phase 3. It adds profile metadata, provider/model policy,
permission policy, persistence, API routes, and reusable validation helpers:

- Agent Profile model
- provider and model binding
- profile execution policy
- provider configuration policy
- SQLite-backed Agent Profile persistence
- product-scoped Agent Profile API routes
- validation that directory grants can reference existing Agent Profiles
- reusable profile authorization helpers for future task validation
- `GET /v1/agents`
- `POST /v1/agents`
- `GET /v1/agents/:agent_id`
- `PATCH /v1/agents/:agent_id`
- `DELETE /v1/agents/:agent_id`

This phase must not start tasks, spawn providers, create worktrees, stream
events, manage secrets, authenticate remote products, or connect to the remote
control plane.

## Scope

Phase 4 delivers only local Agent Profile behavior:

- create typed Agent Profile models
- persist profiles in SQLite
- provide injectable profile store configuration for tests
- list, fetch, create, update, and delete profiles through API routes
- validate provider IDs against the local provider registry at create and patch
  time
- validate profile model values against the provider manifest's supported models
- validate provider permission mode against the provider manifest's
  `provider_permission_modes`
- store profile instructions, provider args, custom environment key names, and
  optional MCP config metadata
- reject provider configuration that would bypass the profile policy at task
  validation time
- define reusable profile authorization helpers for future task validation
- preserve provider, runtime, and directory API behavior
- quality gates passing

Agent Profiles are local authorization and configuration records. They do not
prove that a provider runtime is currently installed, that a directory grant is
valid for a future task, or that any task can execute yet.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 3 spec: `docs/tasks/phase-3-directory-grants.md`
- Phase 3 implementation:
  - `src/api/mod.rs`
  - `src/api/directories.rs`
  - `src/config/mod.rs`
  - `src/registry/manifest.rs`
  - `src/security/directory.rs`
  - `src/store/sqlite.rs`
  - `src/store/directory_grants.rs`
  - `src/tests/directories.rs`

## Deliverables

- Agent Profile model types exist.
- Agent Profile IDs use stable API value `agent_id`.
- Profile creation rejects empty IDs, names, provider IDs, and model values.
- Profile creation rejects unknown provider IDs.
- Profile creation rejects models not supported by the selected provider.
- Profile creation rejects provider permission modes not declared by the
  selected provider manifest.
- Profile creation defaults execution policy to `worktree` and direct directory
  disabled.
- Profile creation can allow direct directory only when explicitly requested.
- Profile creation normalizes or rejects duplicate custom args and environment
  keys consistently.
- Profile creation stores optional instructions.
- Profile creation stores provider-specific custom args.
- Profile creation stores custom environment key names, not secret values.
- Profile creation stores optional MCP config metadata as JSON.
- SQLite schema is initialized automatically by the store.
- Store path remains injectable for tests.
- `GET /v1/agents` returns profiles sorted by creation order or `agent_id`.
- `POST /v1/agents` creates a profile from a trusted local request.
- `GET /v1/agents/:agent_id` returns one profile.
- `PATCH /v1/agents/:agent_id` updates mutable profile fields.
- `DELETE /v1/agents/:agent_id` deletes or revokes a profile.
- Missing profiles return stable `404` JSON.
- Invalid profiles return stable `400` JSON.
- Directory grant creation can optionally validate `agent_id` against the
  profile store when a profile store is present in `AppState`.
- Directory grant APIs remain compatible with existing Phase 3 clients.
- Runtime detection APIs do not depend on Agent Profiles.
- Agent Profile API responses do not include runtime status, directory paths,
  task state, raw task prompts, secret values, or control-plane fields.
- Existing provider, runtime, and directory tests still pass.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 4:

- task API
- task scheduler
- task state model
- task events
- task result model
- worktree creation
- directory locks for running tasks
- provider task execution
- runtime capacity
- runtime selection beyond profile provider binding
- keyring or secret storage
- reading secret values from environment variables
- product authentication or API scopes
- remote product authorization
- audit log
- file watching
- remote HTTP runtime execution
- ACP sessions
- control plane
- desktop UI

Phase 4 may store custom environment variable names, but it must not store
custom environment variable values or any credential material.

## Dependencies

Keep Phase 0 through Phase 3 dependencies. Phase 4 should not require new
runtime, websocket, keyring, task execution, PTY, worktree, file watching, or
control-plane dependencies.

The current Phase 3 SQLite stack is sufficient:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
directories = "6"
```

If implementation needs stable generated profile IDs beyond user-supplied
`agent_id`, prefer a small dependency only after checking whether deterministic
validation of caller-supplied IDs is enough. Do not introduce UUIDs unless the
API contract changes to daemon-generated profile IDs.

## Agent Profile Contract

### Agent Profile Identity

Use caller-supplied stable API IDs:

```text
agent_id = "frontend-fixer"
```

Requirements:

- IDs must be unique in the local SQLite database.
- IDs must be stable after daemon restart.
- IDs must be product-scoped by `owner_product_id`.
- IDs must be safe for URLs and future task references.
- IDs must not contain local paths, provider credentials, or secret material.
- IDs must be validated before persistence.

Recommended validation:

```text
^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$
```

This allows readable profile IDs while preventing empty strings, whitespace,
path separators, control characters, and very long values.

### Agent Profile Shape

Use this API shape:

```json
{
  "id": "frontend-fixer",
  "name": "Frontend Fixer",
  "owner_product_id": "product_example",
  "provider_id": "codex",
  "model": "gpt-5-codex",
  "instructions": "Fix frontend issues with minimal, well-tested changes.",
  "execution_policy": {
    "default_workspace_mode": "worktree",
    "allow_direct_directory": false
  },
  "provider_config": {
    "custom_args": [],
    "custom_env_keys": [],
    "mcp_config": null,
    "permission_mode": "provider_default"
  },
  "created_at": "2026-05-31T00:00:00Z",
  "updated_at": "2026-05-31T00:00:00Z"
}
```

Field requirements:

- `id`: product-facing Agent Profile ID
- `name`: display name for local UI and product selection
- `owner_product_id`: product scope that owns the profile
- `provider_id`: provider manifest ID
- `model`: selected provider model
- `instructions`: optional profile-level instructions
- `execution_policy`: workspace policy for future task validation
- `provider_config`: provider-specific configuration allowed by the profile
- `created_at`: UTC RFC3339 timestamp
- `updated_at`: UTC RFC3339 timestamp

### Execution Policy

Use this shape:

```json
{
  "default_workspace_mode": "worktree",
  "allow_direct_directory": false
}
```

Validation rules:

- `default_workspace_mode` must be `worktree` or `direct`.
- default mode should be `worktree` when omitted.
- `allow_direct_directory` defaults to `false`.
- `default_workspace_mode = direct` is valid only when
  `allow_direct_directory = true`.
- direct mode in a future task must be allowed by Agent Profile and Directory
  Grant.
- Phase 4 must not create worktrees.

### Provider Config

Use this shape:

```json
{
  "custom_args": [],
  "custom_env_keys": [],
  "mcp_config": null,
  "permission_mode": "provider_default"
}
```

Validation rules:

- `custom_args` defaults to an empty array.
- `custom_args` must not include empty strings.
- `custom_args` must reject protocol-critical or dangerous flags that would
  bypass OpenDaemon policy.
- `custom_env_keys` defaults to an empty array.
- `custom_env_keys` stores names only, never values.
- `custom_env_keys` must be valid environment variable names.
- duplicate environment key names must be rejected or normalized consistently.
- `mcp_config` may be stored as JSON metadata, but Phase 4 must not start MCP
  servers.
- `permission_mode` defaults to `provider_default`.
- `permission_mode = provider_default` means OpenDaemon does not add a
  provider-specific override in this phase.
- non-default permission modes must be present in the provider manifest's
  `permissions.provider_permission_modes`.

Recommended custom arg guard:

Reject args that are empty, contain NUL, or are in a denylist of
OpenDaemon-reserved provider control flags. Start with a conservative generic
denylist:

```text
--provider
--model
--cwd
--directory
--workdir
--permission-mode
--dangerously-bypass-approvals-and-sandbox
```

Provider-specific reserved args can be expanded in later provider adapter
phases. Phase 4 only needs the generic profile-level guard.

### Provider And Model Binding

At create and patch time:

- load the local provider registry
- reject unknown `provider_id`
- reject providers whose `integration_type` is not supported by current local
  profile policy when needed
- reject model values not listed in `manifest.models.supported`
- allow the manifest default model
- keep provider manifests unchanged
- do not run runtime detection
- do not check whether the provider CLI is installed

## SQLite Store Contract

Add persistent profile storage under `src/store/`.

Recommended files:

```text
src/store/
  mod.rs
  sqlite.rs
  directory_grants.rs
  agent_profiles.rs
```

SQLite requirements:

- initialize schema on store creation
- use the same local database file as directory grants
- use transactions for create, patch, and delete operations
- store timestamps as UTC RFC3339 strings
- store nested policy objects and arrays as JSON columns
- enforce unique profile IDs
- support listing profiles by `owner_product_id` and `provider_id` filters
- support fetching a single profile by ID
- support deleting or revoking a profile by ID
- expose stable domain errors instead of raw SQLite errors from API handlers

Minimum table shape:

```sql
CREATE TABLE agent_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  owner_product_id TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model TEXT NOT NULL,
  instructions TEXT,
  execution_policy_json TEXT NOT NULL,
  provider_config_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Optional but recommended:

```sql
CREATE INDEX agent_profiles_owner_product_idx
ON agent_profiles(owner_product_id);

CREATE INDEX agent_profiles_provider_idx
ON agent_profiles(provider_id);
```

Do not add migrations tooling in Phase 4 unless the implementation needs it.
Schema initialization can remain an idempotent function for this phase.

## Agent API Contract

Add these routes:

```http
GET /v1/agents
POST /v1/agents
GET /v1/agents/:agent_id
PATCH /v1/agents/:agent_id
DELETE /v1/agents/:agent_id
```

All routes return JSON except successful delete, which may return JSON or
`204 No Content` if the project prefers that convention.

### Error Response Shape

Reuse the stable API error envelope:

```json
{
  "error": {
    "code": "agent_not_found",
    "message": "agent profile not found"
  }
}
```

Stable error codes:

- `agent_not_found`
- `invalid_agent_id`
- `invalid_agent_profile`
- `invalid_execution_policy`
- `invalid_provider_config`
- `provider_not_found`
- `model_not_supported`
- `permission_mode_not_supported`
- `agent_authorization_failed`
- `store_error`
- `registry_error`

Route-level `500` errors should be reserved for registry or store failures that
prevent route execution. User input and authorization failures should be `400`,
`403`, or `404` with stable codes.

### `GET /v1/agents`

Query parameters:

- `owner_product_id`: optional product filter
- `provider_id`: optional provider filter

Response requirements:

- HTTP status: `200 OK`
- response shape:

```json
{
  "agents": []
}
```

Behavior:

- list stored profiles
- sort by creation order or profile ID consistently
- apply filters when present
- do not include runtime status
- do not include directory paths
- do not include task state
- do not expose provider secrets

### `POST /v1/agents`

Request shape:

```json
{
  "id": "frontend-fixer",
  "name": "Frontend Fixer",
  "owner_product_id": "product_example",
  "provider_id": "codex",
  "model": "gpt-5-codex",
  "instructions": "Fix frontend issues with minimal, well-tested changes.",
  "execution_policy": {
    "default_workspace_mode": "worktree",
    "allow_direct_directory": false
  },
  "provider_config": {
    "custom_args": [],
    "custom_env_keys": [],
    "mcp_config": null,
    "permission_mode": "provider_default"
  }
}
```

Response requirements:

- HTTP status: `201 Created`
- response shape:

```json
{
  "agent": {
    "id": "frontend-fixer",
    "name": "Frontend Fixer",
    "owner_product_id": "product_example",
    "provider_id": "codex",
    "model": "gpt-5-codex",
    "instructions": "Fix frontend issues with minimal, well-tested changes.",
    "execution_policy": {
      "default_workspace_mode": "worktree",
      "allow_direct_directory": false
    },
    "provider_config": {
      "custom_args": [],
      "custom_env_keys": [],
      "mcp_config": null,
      "permission_mode": "provider_default"
    },
    "created_at": "2026-05-31T00:00:00Z",
    "updated_at": "2026-05-31T00:00:00Z"
  }
}
```

Behavior:

- validate ID and required strings
- load provider registry
- validate `provider_id`
- validate `model`
- validate `permission_mode`
- fill omitted optional policy fields with defaults
- persist the profile
- return the stored profile

### `GET /v1/agents/:agent_id`

Response requirements:

- HTTP status: `200 OK`
- response shape:

```json
{
  "agent": {}
}
```

Behavior:

- return the stored profile
- return stable `404` JSON when missing

### `PATCH /v1/agents/:agent_id`

Mutable fields:

- `name`
- `provider_id`
- `model`
- `instructions`
- `execution_policy`
- `provider_config`

Immutable fields:

- `id`
- `owner_product_id`
- `created_at`

Behavior:

- reject empty patch bodies
- reject immutable field changes
- validate the resulting profile as a whole
- validate provider/model/permission against the registry
- update `updated_at`
- return the updated profile
- return stable `404` JSON when missing

Changing `owner_product_id` should require deleting and creating a new profile.

### `DELETE /v1/agents/:agent_id`

Behavior:

- delete or revoke the stored profile
- return success when the profile existed
- return stable `404` JSON when missing
- do not delete directory grants in Phase 4
- do not delete provider registry entries
- do not cancel tasks, because Phase 4 does not create tasks

## Directory Grant Integration

Phase 3 reserved `agent_id` on directory grants. Phase 4 should add optional
profile validation without breaking existing persisted grants.

Required behavior:

- `POST /v1/directories/grant` should reject unknown `agent_id` when the
  `owner_product_id`/`product_id` and profile store are available.
- existing directory grants created before Agent Profiles must still list, get,
  patch, and delete.
- directory grant creation must verify that the profile `owner_product_id`
  matches the grant `product_id`.
- directory grant APIs must not embed the full Agent Profile object in the
  directory response.
- directory authorization helper should keep validating product, agent,
  directory, capability, and workspace policy as in Phase 3.

This integration prepares Phase 5 task validation while keeping directory grants
as independent local authorization records.

## Profile Authorization Helper Contract

Add a reusable helper for future task validation. It should not be wired to a
task API in Phase 4.

Expected input:

```text
owner_product_id
agent_id
provider_id override attempt
model override attempt
permission mode override attempt
requested_workspace_mode
```

Expected behavior:

- load the profile by `agent_id`
- reject missing profiles
- reject owner product mismatch
- reject provider override attempts
- reject model override attempts
- reject permission mode override attempts
- reject direct workspace mode when the profile does not allow direct directory
- return the stored profile when authorized

This helper is the bridge to Phase 5 task validation. It must not start a task,
select a runtime, acquire a directory lock, or read provider secrets in Phase 4.

## Source Layout

Expected source layout after Phase 4:

```text
src/
  agent/
    mod.rs
    profile.rs
  api/
    mod.rs
    agents.rs
    directories.rs
    health.rs
    providers.rs
    runtimes.rs
  config/
    mod.rs
  registry/
    manifest.rs
    mod.rs
    validate.rs
  security/
    mod.rs
    directory.rs
    path_guard.rs
  store/
    mod.rs
    sqlite.rs
    directory_grants.rs
    agent_profiles.rs
  tests/
    mod.rs
    agents.rs
    api.rs
    cli.rs
    directories.rs
    registry.rs
    runtime.rs
```

### File Responsibilities

- `src/agent/mod.rs`
  - expose Agent Profile domain model entry points

- `src/agent/profile.rs`
  - define profile model types, execution policy, provider config, validation,
    and authorization helper types
  - keep API-specific DTOs out of agent internals when practical

- `src/api/mod.rs`
  - keep health, provider, runtime, and directory routes
  - register agent routes
  - keep `AppState` explicit and injectable

- `src/api/agents.rs`
  - define agent request and response DTOs
  - implement Agent Profile routes
  - map domain, registry, and store errors to stable HTTP JSON errors
  - avoid task execution behavior

- `src/api/directories.rs`
  - validate new directory grants against the Agent Profile store when creating
    grants
  - keep Phase 3 response shape unchanged

- `src/config/mod.rs`
  - keep daemon bind, runtime detection, and store configuration
  - no database writes during config construction

- `src/store/sqlite.rs`
  - initialize both directory grant and agent profile schemas idempotently

- `src/store/agent_profiles.rs`
  - persist Agent Profile records
  - serialize and deserialize policy/config JSON
  - expose CRUD and authorization lookup operations

- `src/tests/agents.rs`
  - cover model validation, provider/model/permission checks, SQLite
    persistence, API routes, directory grant integration, and authorization
    helper behavior

Do not split into workspace crates in Phase 4. Keep the single-crate shape from
Phase 0 through Phase 3.

## Application State

Extend the Phase 3 `AppState` rather than adding globals.

Required state:

```text
AppState
  providers_dir
  runtime_store
  runtime_detection_config
  directory_grant_store
  agent_profile_store
```

Requirements:

- Agent Profile store is shared by API handlers
- database path is injectable for tests
- directory grant creation can access the profile store for validation
- default router construction remains simple for `main.rs`
- provider, runtime, and directory tests remain straightforward
- no global static SQLite connection is introduced

## Implementation Steps

### Step 4.1: Add Agent Profile Model Types

Add `src/agent/profile.rs` and `src/agent/mod.rs`.

Acceptance:

- profile model serializes with stable snake_case JSON values
- execution policy supports `default_workspace_mode` and
  `allow_direct_directory`
- provider config supports `custom_args`, `custom_env_keys`, `mcp_config`, and
  `permission_mode`
- validation rejects invalid profile IDs
- validation rejects empty required strings
- validation rejects direct default workspace mode unless direct directory is
  allowed
- validation rejects empty custom args
- validation rejects invalid custom environment variable names
- validation rejects generic reserved provider args
- timestamps use UTC RFC3339 strings

### Step 4.2: Add Provider Registry Profile Validation

Add focused validation functions under `src/agent/profile.rs` or
`src/store/agent_profiles.rs`.

Acceptance:

- unknown provider IDs are rejected
- unsupported model values are rejected
- supported model values are accepted
- `provider_default` permission mode is accepted
- provider-declared permission modes are accepted
- unknown permission modes are rejected
- validation does not run runtime detection
- validation does not spawn provider commands

### Step 4.3: Add SQLite Agent Profile Store

Add `src/store/agent_profiles.rs` and extend `src/store/sqlite.rs`.

Acceptance:

- store initializes schema idempotently
- store can create a profile
- store can list all profiles
- store can filter profiles by owner product ID and provider ID
- store can fetch by agent ID
- store can patch mutable profile fields
- store can delete or revoke by agent ID
- store returns stable domain errors for missing profiles and persistence
  failures
- profiles survive store re-open from the same database path

### Step 4.4: Add Profile Authorization Helper

Add authorization functions under `src/agent/profile.rs` or
`src/store/agent_profiles.rs`, depending on where store access naturally fits.

Acceptance:

- authorized product-agent combinations pass
- owner product ID mismatch is rejected
- provider override attempt is rejected
- model override attempt is rejected
- permission mode override attempt is rejected
- direct workspace mode is rejected when profile does not allow direct
- helper returns the stored profile when authorized

### Step 4.5: Add Agent API Routes

Add `src/api/agents.rs` and wire routes in `src/api/mod.rs`.

Acceptance:

- `GET /v1/agents` returns sorted profiles
- `GET /v1/agents?owner_product_id=...` filters by product
- `GET /v1/agents?provider_id=...` filters by provider
- `POST /v1/agents` creates a profile and returns `201`
- invalid profile requests return stable `400` JSON
- registry load failures return stable `500` JSON with `registry_error`
- missing profiles return stable `404` JSON
- `GET /v1/agents/:agent_id` returns one profile
- `PATCH /v1/agents/:agent_id` updates mutable fields
- `PATCH /v1/agents/:agent_id` rejects immutable field changes
- `DELETE /v1/agents/:agent_id` removes or revokes the profile
- response JSON does not contain provider secrets, runtime status, directory
  paths, task state, prompts, or control-plane fields

### Step 4.6: Integrate Directory Grant Creation With Profiles

Update `src/api/directories.rs` and any necessary store helper.

Acceptance:

- new directory grants can validate `agent_id` against an existing profile
- grant product ID must match profile owner product ID
- missing profile returns stable `400` JSON or `404` JSON with an agent-related
  error code
- existing directory list/get/patch/delete behavior remains unchanged
- directory response shape remains Phase 3 compatible
- no Agent Profile object is embedded in directory responses

### Step 4.7: Preserve Provider, Runtime, And Directory Behavior

Keep Phase 1, Phase 2, and Phase 3 behavior stable.

Acceptance:

- `GET /v1/providers` remains manifest-only
- `GET /v1/providers/:provider_id` remains manifest-only
- `GET /v1/runtimes` still does not spawn commands
- `POST /v1/runtimes/detect` still only runs bounded version probes
- directory grant store initialization does not run provider detection
- Agent Profile store initialization does not run provider detection
- no task API is added

## Test Plan

Add tests for:

- profile ID validation accepts `frontend-fixer`
- profile ID validation rejects empty, whitespace, slash, and very long values
- execution policy serializes to stable snake_case workspace mode values
- execution policy defaults to `worktree` and direct disabled
- execution policy rejects direct default when direct directory is disabled
- provider config defaults to empty args, empty env keys, null MCP config, and
  `provider_default`
- provider config rejects empty custom args
- provider config rejects reserved provider args
- provider config rejects invalid environment key names
- provider config normalizes or rejects duplicate environment key names
- unknown provider ID is rejected
- unsupported model is rejected
- supported model is accepted
- provider default model is accepted
- unsupported permission mode is rejected
- provider-declared permission mode is accepted
- SQLite schema initializes on first open
- profile creation persists provider config and execution policy
- profiles survive store re-open
- list returns profiles sorted consistently
- owner product filter returns only matching profiles
- provider filter returns only matching profiles
- get returns one profile by ID
- get missing profile returns `agent_not_found`
- patch updates mutable profile fields
- patch rejects immutable field changes
- patch rejects invalid resulting profile
- delete removes or revokes a profile
- authorization helper accepts valid profile scope
- authorization helper rejects wrong owner product ID
- authorization helper rejects provider override attempts
- authorization helper rejects model override attempts
- authorization helper rejects permission mode override attempts
- authorization helper rejects direct workspace mode when direct is not allowed
- `GET /v1/agents` returns JSON shape `{ "agents": [] }`
- `POST /v1/agents` returns `201` and stored profile
- `POST /v1/agents` rejects invalid provider/model/permission with stable JSON
- `GET /v1/agents/:agent_id` returns one profile
- `PATCH /v1/agents/:agent_id` returns updated profile
- `DELETE /v1/agents/:agent_id` succeeds without deleting directory grants
- creating a directory grant for a missing profile is rejected
- creating a directory grant for a profile owned by a different product is
  rejected
- directory API responses remain Phase 3 compatible
- provider and runtime API tests still pass

Tests must use temporary directories and temporary SQLite database files. Tests
must not depend on the developer machine having real `codex`, `claude`, or any
provider CLI installed.

## Manual Verification

Run these commands before completing Phase 4:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/agents
curl -X POST http://127.0.0.1:19514/v1/agents \
  -H 'content-type: application/json' \
  -d '{"id":"frontend-fixer","name":"Frontend Fixer","owner_product_id":"product_example","provider_id":"codex","model":"gpt-5-codex","instructions":"Fix frontend issues with minimal, well-tested changes.","execution_policy":{"default_workspace_mode":"worktree","allow_direct_directory":false},"provider_config":{"custom_args":[],"custom_env_keys":[],"mcp_config":null,"permission_mode":"provider_default"}}'
curl http://127.0.0.1:19514/v1/agents/frontend-fixer
curl -X POST http://127.0.0.1:19514/v1/directories/grant \
  -H 'content-type: application/json' \
  -d '{"product_id":"product_example","agent_id":"frontend-fixer","path":"/tmp","capabilities":["read"],"workspace_modes":["direct"],"default_workspace_mode":"direct","lock_policy":"shared","direct_mode_requires_explicit_task_opt_in":true}'
curl http://127.0.0.1:19514/v1/directories
```

Expected behavior:

- quality gates pass
- registry check exits `0`
- first agent list returns an empty list or existing local profiles
- profile creation returns `201`
- returned profile includes defaults and no secret values
- fetching the profile returns the same stored profile
- directory grant creation can reference the created profile
- provider and runtime routes still respond as they did in Phase 2
- directory routes still respond as they did in Phase 3
- daemon startup does not run provider detection or task execution

## Completion Checklist

- [x] Agent Profile model types exist.
- [x] Agent Profile ID validation exists and is tested.
- [x] Execution policy model exists and is tested.
- [x] Provider config model exists and is tested.
- [x] Profile validation rejects invalid policy combinations.
- [x] Unknown providers are rejected.
- [x] Unsupported models are rejected.
- [x] Unsupported permission modes are rejected.
- [x] Reserved provider args are rejected.
- [x] Secret values are not stored in profile config.
- [x] SQLite profile schema initializes automatically.
- [x] Profile store path is injectable for tests.
- [x] Profiles persist across store re-open.
- [x] Profile create/list/get/patch/delete behavior is tested.
- [x] Profile authorization helper validates owner product ID.
- [x] Profile authorization helper rejects provider overrides.
- [x] Profile authorization helper rejects model overrides.
- [x] Profile authorization helper rejects permission mode overrides.
- [x] Profile authorization helper rejects direct mode when disallowed.
- [x] `GET /v1/agents` exists.
- [x] `POST /v1/agents` exists.
- [x] `GET /v1/agents/:agent_id` exists.
- [x] `PATCH /v1/agents/:agent_id` exists.
- [x] `DELETE /v1/agents/:agent_id` exists.
- [x] Agent API returns stable error JSON.
- [x] Agent API responses exclude secrets, runtime status, directory paths, task
      state, prompts, and capacity claims.
- [x] Directory grant creation validates known Agent Profiles.
- [x] Directory API behavior remains stable.
- [x] Provider API behavior remains stable.
- [x] Runtime API behavior remains stable.
- [x] No task API is added.
- [x] No provider task execution is added.
- [x] No worktrees are created.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --tests --all-targets --all-features -- -D warnings` passes.
- [x] `cargo test --all-features --all-targets` passes.
- [x] `just registry-check` passes.

## Handoff to Phase 5

Phase 5 can start when Agent Profiles are durably stored, provider and model
bindings are validated against the provider registry, directory grants can
reference existing profiles, and reusable authorization helpers can reject task
requests that try to bypass profile policy.

The next phase should add:

- task model
- task persistence
- task creation API
- task state machine
- task-time validation across Agent Profile and Directory Grant
- initial scheduler boundary
- directory lock preparation
- workspace mode selection
- no provider process execution until the runtime adapter phase explicitly adds
  it
