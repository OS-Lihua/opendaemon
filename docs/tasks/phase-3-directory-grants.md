# Phase 3: Directory Grants

## Goal

Safely authorize local directories for specific product and agent scopes, store
those grants durably, and expose directory grant metadata through the local HTTP
API.

Phase 3 builds on Phase 2. It adds the local directory authorization contract
without executing provider tasks:

- directory grant model
- path canonicalization and path guard helpers
- SQLite-backed grant persistence
- product, agent, directory, and capability scope checks
- `worktree` and `direct` workspace mode policy
- `GET /v1/directories`
- `POST /v1/directories/grant`
- `GET /v1/directories/:directory_id`
- `PATCH /v1/directories/:directory_id`
- `DELETE /v1/directories/:directory_id`

This phase must not start tasks, create Agent Profiles, spawn providers, create
worktrees, stream events, manage secrets, authenticate remote products, or
connect to the remote control plane.

## Scope

Phase 3 delivers only local directory grant behavior:

- create typed directory grant models
- canonicalize requested local paths before storing grants
- reject missing paths, non-directory paths, and invalid workspace policies
- persist grants in SQLite
- provide an injectable database path for tests
- list, fetch, update, and delete directory grants through API routes
- define reusable authorization helpers for future task validation
- enforce product ID, agent ID, directory ID, capability, and workspace mode
  combinations in those helpers
- reject direct mode unless the grant allows it and the caller explicitly opts
  in
- add path guard tests for traversal and symlink behavior
- keep provider and runtime API behavior unchanged
- quality gates passing

Directory grants are local authorization records only. They do not prove that a
provider runtime is installed, that an Agent Profile exists, or that any task
can execute yet.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 2 implementation:
  - `src/api/mod.rs`
  - `src/api/providers.rs`
  - `src/api/runtimes.rs`
  - `src/config/mod.rs`
  - `src/security/mod.rs`
  - `src/store/mod.rs`
  - `src/tests/mod.rs`

## Deliverables

- Directory grant model types exist.
- Directory capability values exist for `read`, `write`, `shell`, and `git`.
- Workspace mode values exist for `worktree` and `direct`.
- Directory lock policy values exist for `exclusive`, `shared`, and `none`.
- Grant IDs are deterministic enough for stable API responses and unique in the
  SQLite database.
- Grant creation canonicalizes paths before persistence.
- Grant creation rejects missing paths.
- Grant creation rejects non-directory paths.
- Grant creation rejects empty capabilities.
- Grant creation rejects unsupported workspace mode combinations.
- Grant creation defaults to `worktree` mode for git repositories when possible.
- Grant creation can allow `direct` only when explicitly requested.
- Direct mode defaults to requiring explicit task opt-in.
- SQLite schema is initialized automatically by the grant store.
- Grant store path is injectable for tests.
- `GET /v1/directories` returns grants sorted by creation order or ID.
- `POST /v1/directories/grant` creates a grant from a trusted local request.
- `GET /v1/directories/:directory_id` returns one grant.
- `PATCH /v1/directories/:directory_id` updates mutable grant policy fields.
- `DELETE /v1/directories/:directory_id` deletes or revokes a grant.
- Missing grants return stable `404` JSON.
- Invalid grants return stable `400` JSON.
- Authorization helpers reject product, agent, capability, workspace, and direct
  opt-in mismatches.
- Directory API responses do not include provider secrets, runtime status, task
  state, raw task prompts, or control-plane fields.
- Existing provider and runtime API tests still pass.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 3:

- Agent Profile API
- Agent Profile persistence
- validation that `agent_id` references an existing Agent Profile
- product authentication or API scopes
- remote product authorization
- task API
- task scheduler
- task state model
- task events
- task result model
- worktree creation
- directory locks for running tasks
- provider task execution
- runtime capacity
- task-time provider configuration validation
- keyring or secret storage
- audit log
- file watching
- remote HTTP runtime execution
- ACP sessions
- control plane
- desktop UI

Phase 3 may accept a raw local path only through `POST /v1/directories/grant`,
which represents a trusted local UI or local CLI action. It must not add any
raw-path task API.

## Dependencies

Keep Phase 0, Phase 1, and Phase 2 dependencies. Add only what is needed for
SQLite storage and platform data directory selection.

Add SQLite storage:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

Add platform data directory discovery if the implementation chooses a default
on-disk database path outside tests:

```toml
directories = "6"
```

If a current stable crate API differs at implementation time, use the current
stable API and keep the dependency purpose unchanged.

Do not add websocket, keyring, task execution, PTY, worktree, file watching, or
control-plane dependencies in Phase 3.

## Directory Grant Contract

### Directory Grant Identity

Use stable API IDs with this prefix:

```text
dir_<opaque_suffix>
```

Requirements:

- IDs must be unique in the local SQLite database.
- IDs must not expose the raw directory path.
- IDs must remain stable after daemon restart.
- IDs must be generated by the daemon during grant creation.
- Clients must use `directory_id` in future task APIs, not raw paths.

Acceptable implementation choices:

- insert a SQLite row, then derive `dir_<rowid>` and update the row
- generate a random or time-sortable suffix if a small dependency is already
  justified

Do not use `std::collections::hash_map::DefaultHasher` for persisted IDs because
its output is not a stable persistence contract.

### Directory Grant Shape

Use this API shape:

```json
{
  "id": "dir_1",
  "product_id": "product_example",
  "agent_id": "frontend-fixer",
  "path": "/Users/alice/github/web-app",
  "capabilities": ["read", "write", "shell", "git"],
  "workspace_modes": ["worktree"],
  "default_workspace_mode": "worktree",
  "lock_policy": "exclusive",
  "direct_mode_requires_explicit_task_opt_in": true,
  "created_at": "2026-05-30T00:00:00Z",
  "updated_at": "2026-05-30T00:00:00Z"
}
```

Field requirements:

- `id`: daemon-generated directory grant ID
- `product_id`: product scope that owns the grant
- `agent_id`: agent scope string reserved for Phase 4 Agent Profiles
- `path`: canonical local directory path
- `capabilities`: non-empty set of allowed directory capabilities
- `workspace_modes`: non-empty set of allowed workspace modes
- `default_workspace_mode`: must be in `workspace_modes`
- `lock_policy`: lock behavior requested for future task execution
- `direct_mode_requires_explicit_task_opt_in`: protects direct mode by default
- `created_at`: UTC RFC3339 timestamp
- `updated_at`: UTC RFC3339 timestamp

### Capabilities

Define directory capabilities with these API values:

- `read`: provider may read files under the grant
- `write`: provider may create, modify, or delete files under the grant
- `shell`: provider may run shell or process tools scoped to the grant
- `git`: provider may run git commands scoped to the grant

Validation rules:

- capabilities must not be empty
- duplicate capabilities must be rejected or normalized away consistently
- `write` grants should default to `exclusive` lock policy
- `shell` does not imply `write`
- `git` does not imply `write`
- future task validation must request every capability it needs explicitly

### Workspace Modes

Define workspace modes with these API values:

- `worktree`: future tasks should run in an OpenDaemon-managed git worktree
- `direct`: future tasks may run in the original granted directory

Validation rules:

- workspace modes must not be empty
- `default_workspace_mode` must be one of the grant's `workspace_modes`
- `direct` is allowed only when the create or patch request explicitly includes
  it
- direct mode should set `direct_mode_requires_explicit_task_opt_in = true` by
  default
- if a directory is a git repository and `worktree` is allowed,
  `default_workspace_mode` should default to `worktree`
- if a directory is not a git repository and `worktree` is the only requested
  mode, grant creation should fail with a stable validation error
- Phase 3 must not create actual worktrees

### Lock Policy

Define lock policy values with these API values:

- `exclusive`
- `shared`
- `none`

Default rules:

- grants with `write` capability default to `exclusive`
- read-only grants may default to `shared`
- `none` is allowed only for read-only grants
- lock policy is stored for Phase 5 task scheduling but no runtime lock is
  acquired in Phase 3

## Path Guard Contract

Path handling must be centralized under `src/security/path_guard.rs` or an
equivalent focused module.

Requirements:

- canonicalize grant paths before persistence
- reject paths that do not exist
- reject paths that are not directories
- reject empty paths
- reject path traversal attempts when authorizing child paths
- reject symlink escapes by default when authorizing child paths
- compare canonical paths, not raw strings
- never grant access based on provider registry capabilities alone
- keep raw input path out of error messages when it may contain sensitive local
  details; prefer stable error codes plus concise messages

Expected helper behavior:

```text
canonicalize_grant_path(path) -> canonical directory path or path_guard error
ensure_child_path_within_grant(grant_root, candidate_path) -> canonical child path or error
```

`ensure_child_path_within_grant` is primarily for future task and file APIs. It
should still be implemented and tested in Phase 3 so the boundary is ready
before tasks exist.

## SQLite Store Contract

Add persistent grant storage under `src/store/`.

Recommended files:

```text
src/store/
  mod.rs
  sqlite.rs
  directory_grants.rs
```

SQLite requirements:

- initialize schema on store creation
- use a single local database file
- use transactions for create, patch, and delete operations
- store timestamps as UTC RFC3339 strings
- store enum sets as JSON arrays or normalized join rows
- enforce unique grant IDs
- support listing grants by product ID and agent ID filters
- support fetching a single grant by ID
- support deleting or revoking a grant by ID
- expose stable domain errors instead of raw SQLite errors from API handlers

Minimum table shape:

```sql
CREATE TABLE directory_grants (
  id TEXT PRIMARY KEY,
  product_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  path TEXT NOT NULL,
  capabilities_json TEXT NOT NULL,
  workspace_modes_json TEXT NOT NULL,
  default_workspace_mode TEXT NOT NULL,
  lock_policy TEXT NOT NULL,
  direct_mode_requires_explicit_task_opt_in INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Optional but recommended:

```sql
CREATE INDEX directory_grants_product_agent_idx
ON directory_grants(product_id, agent_id);
```

Do not add migrations tooling in Phase 3 unless the implementation needs it.
Schema initialization can be an idempotent function for this phase.

## Directory API Contract

Add these routes:

```http
GET /v1/directories
POST /v1/directories/grant
GET /v1/directories/:directory_id
PATCH /v1/directories/:directory_id
DELETE /v1/directories/:directory_id
```

All routes return JSON except successful delete, which may return JSON or
`204 No Content` if the project prefers that convention.

### Error Response Shape

Reuse the stable API error envelope:

```json
{
  "error": {
    "code": "directory_not_found",
    "message": "directory grant not found"
  }
}
```

Stable error codes:

- `directory_not_found`
- `invalid_directory_path`
- `path_not_directory`
- `path_outside_grant`
- `symlink_escape`
- `invalid_capability`
- `invalid_workspace_mode`
- `invalid_lock_policy`
- `direct_mode_not_allowed`
- `directory_authorization_failed`
- `store_error`

Route-level `500` errors should be reserved for failures that prevent the route
from loading or writing store metadata. User input and authorization failures
should be `400`, `403`, or `404` with stable codes.

### `GET /v1/directories`

Query parameters:

- `product_id`: optional product filter
- `agent_id`: optional agent filter

Response requirements:

- HTTP status: `200 OK`
- response shape:

```json
{
  "directories": []
}
```

Behavior:

- list stored grants
- sort by creation order or ID consistently
- apply filters when present
- do not include runtime status
- do not include task state
- do not expose provider secrets

### `POST /v1/directories/grant`

Request shape:

```json
{
  "product_id": "product_example",
  "agent_id": "frontend-fixer",
  "path": "/Users/alice/github/web-app",
  "capabilities": ["read", "write", "shell", "git"],
  "workspace_modes": ["worktree"],
  "default_workspace_mode": "worktree",
  "lock_policy": "exclusive",
  "direct_mode_requires_explicit_task_opt_in": true
}
```

Response requirements:

- HTTP status: `201 Created`
- response shape:

```json
{
  "directory": {
    "id": "dir_1",
    "product_id": "product_example",
    "agent_id": "frontend-fixer",
    "path": "/Users/alice/github/web-app",
    "capabilities": ["read", "write", "shell", "git"],
    "workspace_modes": ["worktree"],
    "default_workspace_mode": "worktree",
    "lock_policy": "exclusive",
    "direct_mode_requires_explicit_task_opt_in": true,
    "created_at": "2026-05-30T00:00:00Z",
    "updated_at": "2026-05-30T00:00:00Z"
  }
}
```

Behavior:

- canonicalize `path`
- reject invalid paths before writing to SQLite
- normalize or reject duplicate enum values
- fill defaults for omitted optional policy fields
- persist the grant
- return the stored canonical grant

### `GET /v1/directories/:directory_id`

Response requirements:

- HTTP status: `200 OK`
- response shape:

```json
{
  "directory": {}
}
```

Behavior:

- return the stored grant
- return stable `404` JSON when missing

### `PATCH /v1/directories/:directory_id`

Mutable fields:

- `capabilities`
- `workspace_modes`
- `default_workspace_mode`
- `lock_policy`
- `direct_mode_requires_explicit_task_opt_in`

Immutable fields:

- `id`
- `product_id`
- `agent_id`
- `path`
- `created_at`

Behavior:

- reject empty patch bodies
- validate the resulting policy as a whole
- update `updated_at`
- return the updated grant
- return stable `404` JSON when missing

Changing `path`, `product_id`, or `agent_id` should require deleting and
creating a new grant.

### `DELETE /v1/directories/:directory_id`

Behavior:

- delete or revoke the stored grant
- return success when the grant existed
- return stable `404` JSON when missing
- do not delete files from the granted directory
- do not delete worktrees, because Phase 3 does not create worktrees

## Authorization Helper Contract

Add a reusable helper for future task validation. It should not be wired to a
task API in Phase 3.

Expected input:

```text
product_id
agent_id
directory_id
required_capabilities
requested_workspace_mode
direct_mode_task_opt_in
```

Expected behavior:

- load the grant by `directory_id`
- reject missing grants
- reject product ID mismatch
- reject agent ID mismatch
- reject missing required capabilities
- reject unsupported workspace mode
- reject direct mode when the grant does not include `direct`
- reject direct mode when explicit task opt-in is required and absent
- return the grant when authorized

This helper is the bridge to Phase 5 task validation. It must not start a task
or acquire a directory lock in Phase 3.

## Source Layout

Expected source layout after Phase 3:

```text
src/
  api/
    mod.rs
    health.rs
    providers.rs
    runtimes.rs
    directories.rs
  config/
    mod.rs
  security/
    mod.rs
    directory.rs
    path_guard.rs
  store/
    mod.rs
    sqlite.rs
    directory_grants.rs
  tests/
    mod.rs
    api.rs
    cli.rs
    registry.rs
    runtime.rs
    directories.rs
```

### File Responsibilities

- `src/api/mod.rs`
  - keep health, provider, and runtime routes
  - register directory routes
  - keep `AppState` explicit and injectable

- `src/api/directories.rs`
  - define directory request and response DTOs
  - implement directory grant routes
  - map domain and store errors to stable HTTP JSON errors
  - avoid task execution behavior

- `src/config/mod.rs`
  - keep daemon bind and runtime detection configuration
  - add store configuration, including injectable SQLite database path
  - avoid reading or writing the database at config construction time

- `src/security/mod.rs`
  - expose directory security helpers

- `src/security/directory.rs`
  - define grant model types, capability enums, workspace mode enums, and
    authorization helper types
  - keep API-specific DTOs out of security internals when practical

- `src/security/path_guard.rs`
  - canonicalize grant paths
  - reject traversal and symlink escapes
  - provide focused path guard errors

- `src/store/mod.rs`
  - expose SQLite and directory grant store entry points
  - avoid API-specific response types

- `src/store/sqlite.rs`
  - open SQLite connections
  - initialize schema
  - keep low-level SQLite setup isolated

- `src/store/directory_grants.rs`
  - persist directory grant records
  - serialize and deserialize enum sets
  - expose CRUD and authorization lookup operations

- `src/tests/directories.rs`
  - cover model validation, path guard behavior, SQLite persistence, API routes,
    and authorization helper behavior
  - use temporary directories and temporary SQLite database files

Do not split into workspace crates in Phase 3. Keep the single-crate shape from
Phase 0 through Phase 2.

## Application State

Extend the Phase 2 `AppState` rather than adding globals.

Required state:

```text
AppState
  providers_dir
  runtime_store
  runtime_detection_config
  directory_grant_store
```

Requirements:

- directory grant store is shared by API handlers
- database path is injectable for tests
- default router construction remains simple for `main.rs`
- provider and runtime tests remain straightforward
- no global static SQLite connection is introduced

## Implementation Steps

### Step 3.1: Add Directory Grant Model Types

Add `src/security/directory.rs`.

Acceptance:

- grant model serializes with stable snake_case JSON values
- capability enum supports `read`, `write`, `shell`, and `git`
- workspace mode enum supports `worktree` and `direct`
- lock policy enum supports `exclusive`, `shared`, and `none`
- validation rejects empty capability lists
- validation rejects empty workspace mode lists
- validation rejects default workspace mode not present in workspace modes
- validation rejects `none` lock policy for write-capable grants
- timestamps use UTC RFC3339 strings

### Step 3.2: Add Path Guard Helpers

Add `src/security/path_guard.rs`.

Acceptance:

- existing directories canonicalize successfully
- missing paths return `invalid_directory_path`
- file paths return `path_not_directory`
- child paths inside the grant are accepted
- `..` traversal outside the grant is rejected
- symlink escapes outside the grant are rejected
- returned paths are canonical paths

### Step 3.3: Add SQLite Store Configuration

Extend `src/config/mod.rs` and `src/api/mod.rs`.

Acceptance:

- production router can build a default store configuration
- tests can inject a temporary SQLite database path
- store configuration does not create files until the store opens
- existing `router()` remains the default production entry point
- `router_with_state(state)` remains available for tests

### Step 3.4: Add SQLite Directory Grant Store

Add `src/store/sqlite.rs` and `src/store/directory_grants.rs`.

Acceptance:

- store initializes schema idempotently
- store can create a grant
- store can list all grants
- store can filter grants by product ID and agent ID
- store can fetch by directory ID
- store can patch mutable policy fields
- store can delete or revoke by directory ID
- store returns stable domain errors for missing grants and persistence failures
- grants survive store re-open from the same database path

### Step 3.5: Add Authorization Helper

Add authorization functions under `src/security/directory.rs` or
`src/store/directory_grants.rs`, depending on where store access naturally fits.

Acceptance:

- authorized product-agent-directory-capability combinations pass
- product ID mismatch is rejected
- agent ID mismatch is rejected
- missing capability is rejected
- unsupported workspace mode is rejected
- direct mode without grant support is rejected
- direct mode without required task opt-in is rejected
- helper returns the stored grant when authorized

### Step 3.6: Add Directory API Routes

Add `src/api/directories.rs` and wire routes in `src/api/mod.rs`.

Acceptance:

- `GET /v1/directories` returns sorted grants
- `GET /v1/directories?product_id=...` filters by product
- `GET /v1/directories?agent_id=...` filters by agent
- `POST /v1/directories/grant` creates a grant and returns `201`
- `POST /v1/directories/grant` stores a canonical path
- invalid path requests return stable `400` JSON
- missing grants return stable `404` JSON
- `GET /v1/directories/:directory_id` returns one grant
- `PATCH /v1/directories/:directory_id` updates mutable policy fields
- `PATCH /v1/directories/:directory_id` rejects immutable field changes
- `DELETE /v1/directories/:directory_id` removes or revokes the grant
- response JSON does not contain provider secrets, runtime status, task state,
  or control-plane fields

### Step 3.7: Preserve Provider and Runtime Behavior

Keep Phase 1 and Phase 2 behavior stable.

Acceptance:

- `GET /v1/providers` remains manifest-only
- `GET /v1/providers/:provider_id` remains manifest-only
- `GET /v1/runtimes` still does not spawn commands
- `POST /v1/runtimes/detect` still only runs bounded version probes
- directory grant store initialization does not run provider detection
- directory grant store initialization does not spawn provider commands

## Test Plan

Add tests for:

- capability enum serializes to stable snake_case values
- workspace mode enum serializes to stable snake_case values
- lock policy enum serializes to stable snake_case values
- grant validation rejects empty capabilities
- grant validation rejects empty workspace modes
- grant validation rejects default workspace mode outside workspace modes
- grant validation rejects `none` lock policy with `write`
- missing paths are rejected
- file paths are rejected
- existing directories are canonicalized
- child path inside grant is accepted
- traversal outside grant is rejected
- symlink escape outside grant is rejected
- SQLite schema initializes on first open
- grant creation persists canonical path
- grants survive store re-open
- list returns grants sorted consistently
- product filter returns only matching grants
- agent filter returns only matching grants
- get returns one grant by ID
- get missing grant returns `directory_not_found`
- patch updates mutable policy fields
- patch rejects invalid resulting policy
- delete removes or revokes a grant
- authorization helper accepts valid scope and capabilities
- authorization helper rejects wrong product ID
- authorization helper rejects wrong agent ID
- authorization helper rejects missing capability
- authorization helper rejects unsupported workspace mode
- authorization helper rejects direct mode without grant support
- authorization helper rejects direct mode without required task opt-in
- `GET /v1/directories` returns JSON shape `{ "directories": [] }`
- `POST /v1/directories/grant` returns `201` and stored grant
- `POST /v1/directories/grant` rejects invalid paths with stable error JSON
- `GET /v1/directories/:directory_id` returns one grant
- `PATCH /v1/directories/:directory_id` returns updated grant
- `DELETE /v1/directories/:directory_id` succeeds without deleting user files
- directory API responses do not include provider secrets, runtime status, task
  state, prompts, or capacity fields
- provider and runtime API tests still pass

Tests must use temporary directories and temporary SQLite database files. Tests
must not depend on the developer machine having real `codex`, `claude`, or git
repositories outside the test temp directory.

For git repository default behavior tests, create a temporary directory and run
`git init` only when `git` is available. If the project prefers no external git
dependency in unit tests, isolate git detection behind an injectable helper and
test it with fakes.

## Manual Verification

Run these commands before completing Phase 3:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/directories
curl -X POST http://127.0.0.1:19514/v1/directories/grant \
  -H 'content-type: application/json' \
  -d '{"product_id":"product_example","agent_id":"frontend-fixer","path":"/tmp","capabilities":["read"],"workspace_modes":["direct"],"default_workspace_mode":"direct","lock_policy":"shared","direct_mode_requires_explicit_task_opt_in":true}'
curl http://127.0.0.1:19514/v1/directories
```

Expected behavior:

- quality gates pass
- registry check exits `0`
- first directory list returns an empty list or existing local grants
- grant creation returns `201`
- returned grant path is canonical
- directory list includes the created grant
- provider and runtime routes still respond as they did in Phase 2
- daemon startup does not run provider detection or task execution

## Completion Checklist

- [x] Directory grant model types exist.
- [x] Directory capability enum exists and is tested.
- [x] Workspace mode enum exists and is tested.
- [x] Lock policy enum exists and is tested.
- [x] Grant validation rejects invalid policy combinations.
- [x] Grant paths are canonicalized before persistence.
- [x] Missing paths are rejected.
- [x] Non-directory paths are rejected.
- [x] Path traversal outside a grant is rejected.
- [x] Symlink escape outside a grant is rejected.
- [x] SQLite grant schema initializes automatically.
- [x] Grant store path is injectable for tests.
- [x] Grants persist across store re-open.
- [x] Grant create/list/get/patch/delete behavior is tested.
- [x] Authorization helper validates product ID.
- [x] Authorization helper validates agent ID.
- [x] Authorization helper validates directory ID.
- [x] Authorization helper validates required capabilities.
- [x] Authorization helper validates workspace mode.
- [x] Authorization helper rejects direct mode without explicit opt-in when
      required.
- [x] `GET /v1/directories` exists.
- [x] `POST /v1/directories/grant` exists.
- [x] `GET /v1/directories/:directory_id` exists.
- [x] `PATCH /v1/directories/:directory_id` exists.
- [x] `DELETE /v1/directories/:directory_id` exists.
- [x] Directory API returns stable error JSON.
- [x] Directory API responses exclude secrets, runtime status, task state,
      prompts, and capacity claims.
- [x] Provider API behavior remains stable.
- [x] Runtime API behavior remains stable.
- [x] No task API is added.
- [x] No provider task execution is added.
- [x] No worktrees are created.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --tests --all-targets --all-features -- -D warnings` passes.
- [x] `cargo test --all-features --all-targets` passes.
- [x] `just registry-check` passes.

## Handoff to Phase 4

Phase 4 can start when directory grants are durably stored, local paths are
canonicalized and guarded, directory grant API routes exist, and reusable
authorization helpers can validate product, agent, directory, capability, and
workspace mode combinations.

The next phase should add:

- Agent Profile model
- Agent Profile persistence
- provider ID binding for profiles
- model, instructions, custom args, custom env, and permission mode policy
- product-scoped Agent Profile API routes
- task-time provider override rejection based on profile policy
- integration between Agent Profiles and existing directory grant scopes
