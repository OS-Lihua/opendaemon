# Phase 5: Task Scheduler

## Goal

Add a durable asynchronous task lifecycle so products can submit work by
referencing an existing Agent Profile and Directory Grant.

Phase 5 builds on Phase 4. It adds task records, validation, state transitions,
directory lock coordination, workspace selection, cancellation, and task API
routes:

- task model
- task state machine
- task event and result records
- SQLite-backed task persistence
- scheduler store and lock primitives
- task-time validation across Agent Profile and Directory Grant
- workspace mode selection
- worktree preparation boundary
- direct-mode lock enforcement
- `POST /v1/tasks`
- `GET /v1/tasks`
- `GET /v1/tasks/:task_id`
- `POST /v1/tasks/:task_id/cancel`

Phase 5 must not execute provider commands. Runtime adapters, process execution,
stdout/stderr streaming, provider protocol events, ACP sessions, and remote HTTP
execution remain Phase 6 and later work.

## Scope

Phase 5 delivers local task scheduling infrastructure:

- create typed task, task state, event, and result models
- persist tasks, task events, and task results in SQLite
- provide injectable task store configuration for tests
- validate task creation against existing Agent Profiles
- validate task creation against existing Directory Grants
- reject task-time provider, model, and permission-mode overrides
- reject missing agents and missing directory grants
- reject product, agent, directory, capability, or workspace policy mismatch
- choose task workspace mode from task request, directory grant, and Agent
  Profile policy
- create a scheduler boundary that can be driven by tests without provider
  execution
- implement directory lock acquisition and release state for scheduled tasks
- prevent concurrent write-capable tasks for the same directory grant
- allow compatible read-only shared tasks when lock policy permits it
- support cancellation before a task starts
- support cancellation while a task is waiting for a directory lock
- keep task terminal transitions idempotent
- add task API routes for create/list/get/cancel
- preserve provider, runtime, directory, and agent API behavior
- quality gates passing

Task creation is a durable request, not proof that a provider runtime can
execute it. Phase 5 may mark tasks as queued, waiting for lock, preparing,
cancelled, failed, or completed through scheduler test hooks, but production
provider execution must wait for Phase 6 runtime adapters.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 3 spec: `docs/tasks/phase-3-directory-grants.md`
- Phase 4 spec: `docs/tasks/phase-4-agent-profiles.md`
- Phase 4 implementation:
  - `src/agent/profile.rs`
  - `src/api/agents.rs`
  - `src/api/directories.rs`
  - `src/api/mod.rs`
  - `src/config/mod.rs`
  - `src/security/directory.rs`
  - `src/store/agent_profiles.rs`
  - `src/store/directory_grants.rs`
  - `src/store/sqlite.rs`
  - `src/task/mod.rs`
  - `src/scheduler/mod.rs`

## Deliverables

- Task model types exist.
- Task IDs use stable API value `task_id`.
- Task creation accepts `owner_product_id`, `agent_id`, `directory_id`,
  `prompt`, optional requested capabilities, optional workspace mode, optional
  metadata, and optional timeout fields.
- Task creation rejects empty prompt values.
- Task creation rejects unknown Agent Profiles.
- Task creation rejects unknown Directory Grants.
- Task creation rejects owner product mismatch.
- Task creation rejects agent mismatch.
- Task creation rejects missing required directory capabilities.
- Task creation rejects direct workspace mode unless Agent Profile and Directory
  Grant both allow it.
- Task creation rejects direct workspace mode when the Directory Grant requires
  explicit task opt-in and the task does not provide it.
- Task creation rejects provider/model/permission overrides that differ from the
  Agent Profile.
- Task creation records a queued task and an initial `task.queued` event.
- Task list can filter by owner product ID, agent ID, directory ID, status, and
  creation order.
- Task get returns one task with latest status and result summary.
- Task cancellation before execution moves the task to `cancelled`.
- Task cancellation is idempotent for already cancelled tasks.
- Terminal task states are immutable except for idempotent repeat terminal
  callbacks.
- SQLite schema initializes automatically.
- Store path remains injectable for tests.
- Task records survive store re-open from the same SQLite database.
- Scheduler lock store prevents conflicting write tasks for the same directory
  grant.
- Scheduler lock store releases locks on terminal task states.
- Scheduler lock behavior is tested without spawning providers.
- Worktree mode preparation boundary exists and is tested with a fake preparer.
- Direct mode preparation uses the canonical grant path and never deletes user
  files.
- `POST /v1/tasks` exists.
- `GET /v1/tasks` exists.
- `GET /v1/tasks/:task_id` exists.
- `POST /v1/tasks/:task_id/cancel` exists.
- Task API returns stable error JSON.
- Task API responses do not include provider secrets, raw local paths outside
  the authorized directory grant contract, child process output, control-plane
  tokens, or runtime adapter internals.
- Existing provider, runtime, directory, and agent tests still pass.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 5:

- provider command execution
- local CLI runtime adapter
- process spawning
- process stdout/stderr streaming
- process timeout enforcement
- provider-specific command template rendering
- provider-native permission flags
- provider-native cancellation signals
- ACP adapter
- remote HTTP adapter
- remote execution policy
- SSE event streaming endpoint
- provider permission request response API
- keyring or secret storage
- product authentication or API scopes
- audit log
- control plane
- desktop UI

Phase 5 may persist normalized task events, but it must not implement the
streaming `GET /v1/tasks/:task_id/events` endpoint. That belongs to Phase 7.

## Dependencies

Keep Phase 0 through Phase 4 dependencies.

Phase 5 should not need new runtime adapter, websocket, keyring, PTY,
file-watching, control-plane, or command-template dependencies.

Use the current SQLite stack:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
directories = "6"
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }
```

Use daemon-generated `task_<rowid>` IDs unless a stronger ID requirement appears
during implementation. Do not introduce UUIDs for Phase 5 unless the task API
contract changes to require caller-independent globally unique IDs.

## Task Contract

### Task Identity

Use daemon-generated stable API IDs:

```text
task_<opaque_suffix>
```

Requirements:

- IDs must be unique in the local SQLite database.
- IDs must not expose prompts, paths, provider credentials, or secret material.
- IDs must remain stable after daemon restart.
- IDs must be generated by the daemon during task creation.
- IDs must be URL safe.

Acceptable implementation:

- insert a SQLite row with an empty or temporary ID
- read `last_insert_rowid`
- update the row to `task_<rowid>`

Do not use `std::collections::hash_map::DefaultHasher` for persisted task IDs.

### Task Creation Request

Use this API shape:

```json
{
  "owner_product_id": "product_example",
  "agent_id": "frontend-fixer",
  "directory_id": "dir_1",
  "prompt": "Fix the mobile login button alignment.",
  "required_capabilities": ["read", "write"],
  "workspace_mode": "worktree",
  "direct_mode_task_opt_in": false,
  "metadata": {
    "issue_id": "BUG-123"
  },
  "provider_id": null,
  "model": null,
  "permission_mode": null,
  "timeout_seconds": null
}
```

Field requirements:

- `owner_product_id`: product scope creating the task
- `agent_id`: existing Agent Profile ID
- `directory_id`: existing Directory Grant ID
- `prompt`: non-empty task prompt
- `required_capabilities`: optional capability list; defaults to `["read"]`
- `workspace_mode`: optional requested workspace mode; defaults through the
  directory grant and Agent Profile policy
- `direct_mode_task_opt_in`: explicit direct-mode opt-in; defaults to `false`
- `metadata`: optional JSON object for product references
- `provider_id`: optional override attempt; only accepted when equal to the
  stored Agent Profile provider ID
- `model`: optional override attempt; only accepted when equal to the stored
  Agent Profile model
- `permission_mode`: optional override attempt; only accepted when equal to the
  stored Agent Profile provider config permission mode
- `timeout_seconds`: optional execution timeout metadata for future runtime
  adapters; Phase 5 stores and validates bounds but does not enforce process
  timeouts

Phase 5 must not accept raw local paths in task creation.

### Task View

Use this response shape:

```json
{
  "id": "task_1",
  "owner_product_id": "product_example",
  "agent_id": "frontend-fixer",
  "directory_id": "dir_1",
  "status": "queued",
  "required_capabilities": ["read", "write"],
  "workspace_mode": "worktree",
  "direct_mode_task_opt_in": false,
  "prompt": "Fix the mobile login button alignment.",
  "metadata": {
    "issue_id": "BUG-123"
  },
  "result": null,
  "created_at": "2026-05-31T00:00:00Z",
  "updated_at": "2026-05-31T00:00:00Z",
  "started_at": null,
  "completed_at": null,
  "cancelled_at": null,
  "failed_at": null
}
```

Phase 5 may include `prompt` in trusted local API responses because no
authentication model exists yet. It must not copy prompt values into logs,
runtime records, provider metadata, or error messages.

### Task List Response

Use this shape:

```json
{
  "tasks": []
}
```

Query parameters:

- `owner_product_id`: optional product filter
- `agent_id`: optional agent filter
- `directory_id`: optional directory filter
- `status`: optional task status filter

Sort consistently by creation order or `task_id`.

### Task Cancel Response

Successful cancellation may return `200 OK` with the updated task:

```json
{
  "task": {
    "id": "task_1",
    "status": "cancelled"
  }
}
```

Do not use `204 No Content` for cancellation because products need the latest
task state.

## Task State Machine

Define these API status values:

- `queued`
- `waiting_directory_lock`
- `preparing`
- `running`
- `completed`
- `failed`
- `cancelled`
- `timed_out`

Allowed transitions:

```text
queued -> waiting_directory_lock
queued -> cancelled

waiting_directory_lock -> preparing
waiting_directory_lock -> cancelled

preparing -> running
preparing -> failed
preparing -> cancelled

running -> completed
running -> failed
running -> cancelled
running -> timed_out
```

Phase 5 should implement the full transition validator even though production
runtime adapters are deferred. Tests may drive transitions directly through the
store or scheduler service.

Rules:

- A task starts in `queued`.
- `created_at` is set at creation.
- `updated_at` changes on every successful transition.
- `started_at` is set when entering `running`.
- `completed_at` is set when entering `completed`.
- `cancelled_at` is set when entering `cancelled`.
- `failed_at` is set when entering `failed` or `timed_out`.
- Terminal states are `completed`, `failed`, `cancelled`, and `timed_out`.
- A terminal task cannot transition to a non-terminal state.
- Repeating the same terminal transition for the same task is idempotent.
- Invalid transitions return a stable domain error.

## Task Event Contract

Persist normalized events for task lifecycle changes. Phase 5 stores and lists
events internally for tests and future Phase 7 streaming, but does not expose
SSE yet.

Minimum event shape:

```json
{
  "id": "evt_1",
  "task_id": "task_1",
  "sequence": 1,
  "type": "task.queued",
  "payload": {},
  "created_at": "2026-05-31T00:00:00Z"
}
```

Phase 5 event types:

- `task.queued`
- `task.waiting_directory_lock`
- `task.preparing`
- `task.running`
- `task.completed`
- `task.failed`
- `task.cancelled`
- `task.timed_out`

Requirements:

- Event sequence is monotonic per task.
- Event rows survive store re-open.
- Transition and event insert happen in one transaction.
- Event payloads are JSON objects.
- Event payloads must not store provider secrets or child process output in
  Phase 5.

## Task Result Contract

Persist an optional normalized result only for terminal tasks.

Minimum shape:

```json
{
  "task_id": "task_1",
  "status": "completed",
  "final_message": "Task completed.",
  "changed_files": [],
  "diff": null,
  "workspace_mode": "worktree",
  "worktree_path": null,
  "source_directory_id": "dir_1",
  "branch_name": null,
  "commit_hash": null,
  "session_id": null,
  "provider_result": null,
  "usage": null,
  "artifacts": [],
  "error": null,
  "created_at": "2026-05-31T00:00:00Z",
  "updated_at": "2026-05-31T00:00:00Z"
}
```

Phase 5 should support storing this result shape but should not populate
provider-specific fields from real execution. Tests may insert synthetic results
through store APIs.

## Task-Time Validation

Task creation must validate these records in order:

1. Agent Profile exists.
2. Directory Grant exists.
3. Agent Profile owner matches `owner_product_id`.
4. Directory Grant product matches `owner_product_id`.
5. Directory Grant agent matches `agent_id`.
6. Provider/model/permission override attempts match the Agent Profile.
7. Required capabilities are present in the Directory Grant.
8. Workspace mode is allowed by Agent Profile and Directory Grant.
9. Direct mode task opt-in is present when the Directory Grant requires it.

Use the Phase 4 Agent Profile authorization helper and Phase 3 Directory Grant
authorization helper where possible. Do not duplicate policy logic in API
handlers.

### Capability Defaults

If `required_capabilities` is omitted, default to:

```json
["read"]
```

If the task is expected to modify files, the product must request `write`. Phase
5 does not infer write intent from prompt contents.

Reject empty `required_capabilities`.

### Workspace Mode Selection

If the request includes `workspace_mode`, validate that exact mode.

If the request omits `workspace_mode`, select:

1. the Directory Grant default workspace mode when allowed by the Agent Profile
2. otherwise `worktree` when both records allow it
3. otherwise `direct` only when both records allow direct and direct opt-in is
   present when required
4. otherwise reject with `workspace_mode_not_allowed`

Persist the selected workspace mode on the task.

## Directory Locks

Phase 5 introduces lock records used by the scheduler.

Minimum lock shape:

```json
{
  "directory_id": "dir_1",
  "task_id": "task_1",
  "mode": "exclusive",
  "status": "held",
  "created_at": "2026-05-31T00:00:00Z",
  "released_at": null
}
```

Lock modes:

- `exclusive`: only one task may hold a lock for the directory
- `shared`: multiple read-only tasks may hold compatible locks
- `none`: no durable lock is acquired

Rules:

- Write-capable tasks must use `exclusive` unless the Directory Grant has a
  stricter policy already requiring exclusive.
- Tasks requiring `write`, `shell`, or `git` should be treated as write-capable
  for lock conflict purposes.
- Read-only tasks may use `shared` when the Directory Grant lock policy is
  `shared` or `none`.
- A task waiting for a conflicting lock moves to `waiting_directory_lock`.
- Lock acquisition and task transition happen in one transaction when possible.
- Locks are released when a task reaches a terminal state.
- Releasing an already released lock is idempotent.

This phase does not implement cross-process distributed locks beyond SQLite
state. That is sufficient for the current single-daemon local process model.

## Workspace Preparation Boundary

Add a scheduler-facing workspace preparation abstraction but keep it local and
testable.

Expected output:

```json
{
  "workspace_mode": "worktree",
  "working_directory": "/path/to/prepared/workspace",
  "source_directory_id": "dir_1",
  "worktree_path": "/path/to/prepared/worktree",
  "branch_name": "opendaemon/task_1"
}
```

Phase 5 requirements:

- For `direct`, the working directory is the canonical Directory Grant path.
- For `direct`, no cleanup operation may delete the original directory.
- For `worktree`, define the data model and abstraction for a prepared
  workspace.
- For `worktree`, implementation may use a fake preparer in tests and defer real
  `git worktree add` until the scheduler boundary is ready.
- If real worktree creation is implemented in Phase 5, it must be behind a small
  interface, must use direct `git` process invocation without shell, and must
  have tests that do not depend on a developer's global git config.

Preferred Phase 5 implementation is the abstraction plus deterministic fake
preparer tests. Real git worktree creation can land later if it stays small and
does not require provider execution.

## SQLite Store Contract

Extend `src/store/sqlite.rs` to initialize these tables idempotently:

```sql
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  owner_product_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  directory_id TEXT NOT NULL,
  prompt TEXT NOT NULL,
  required_capabilities_json TEXT NOT NULL,
  workspace_mode TEXT NOT NULL,
  direct_mode_task_opt_in INTEGER NOT NULL,
  metadata_json TEXT,
  provider_id TEXT NOT NULL,
  model TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  timeout_seconds INTEGER,
  status TEXT NOT NULL,
  result_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  cancelled_at TEXT,
  failed_at TEXT
);

CREATE INDEX tasks_owner_product_idx ON tasks(owner_product_id);
CREATE INDEX tasks_agent_idx ON tasks(agent_id);
CREATE INDEX tasks_directory_idx ON tasks(directory_id);
CREATE INDEX tasks_status_idx ON tasks(status);
```

```sql
CREATE TABLE task_events (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(task_id, sequence)
);

CREATE INDEX task_events_task_idx ON task_events(task_id, sequence);
```

```sql
CREATE TABLE directory_locks (
  directory_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  released_at TEXT,
  PRIMARY KEY(directory_id, task_id)
);

CREATE INDEX directory_locks_active_idx
ON directory_locks(directory_id, status);
```

Store requirements:

- Use transactions for task create, transition, cancellation, result update,
  event insert, lock acquire, and lock release.
- Store timestamps as UTC RFC3339 strings.
- Store arrays and nested objects as JSON columns.
- Expose stable domain errors instead of raw SQLite errors from API handlers.
- Keep database path injectable for tests.
- Do not add migrations tooling in Phase 5.

## API Contract

Add these routes:

```http
POST /v1/tasks
GET /v1/tasks
GET /v1/tasks/:task_id
POST /v1/tasks/:task_id/cancel
```

Do not add `GET /v1/tasks/:task_id/events` in Phase 5.

### Error Response Shape

Reuse the stable API error envelope:

```json
{
  "error": {
    "code": "task_not_found",
    "message": "task not found"
  }
}
```

Stable error codes:

- `task_not_found`
- `invalid_task`
- `invalid_task_id`
- `invalid_task_state`
- `invalid_task_prompt`
- `agent_not_found`
- `directory_not_found`
- `agent_authorization_failed`
- `directory_authorization_failed`
- `capability_not_allowed`
- `workspace_mode_not_allowed`
- `direct_mode_not_allowed`
- `provider_override_not_allowed`
- `model_override_not_allowed`
- `permission_mode_override_not_allowed`
- `directory_lock_conflict`
- `task_already_terminal`
- `store_error`
- `registry_error`

Route-level `500` errors should be reserved for registry or store failures that
prevent route execution. User input and authorization failures should be `400`,
`403`, `404`, or `409` with stable codes.

### `POST /v1/tasks`

Behavior:

- validate request shape
- validate task-time Agent Profile policy
- validate task-time Directory Grant policy
- select workspace mode
- persist queued task
- insert `task.queued` event
- return `201 Created`
- do not spawn a provider
- do not run runtime detection

Response:

```json
{
  "task": {
    "id": "task_1",
    "status": "queued"
  }
}
```

### `GET /v1/tasks`

Behavior:

- list stored tasks
- apply query filters
- sort consistently
- do not include events inline
- do not include provider runtime state inline

Response:

```json
{
  "tasks": []
}
```

### `GET /v1/tasks/:task_id`

Behavior:

- return one stored task
- include result when present
- return stable `404` JSON when missing

Response:

```json
{
  "task": {}
}
```

### `POST /v1/tasks/:task_id/cancel`

Behavior:

- cancel a queued or waiting task
- insert `task.cancelled` event
- release any held lock
- return updated task
- return the already-cancelled task when cancellation is repeated
- return `409 task_already_terminal` for completed, failed, or timed-out tasks

Phase 5 does not need to send provider cancellation signals because it does not
start provider processes.

## Scheduler Boundary

Add a scheduler service that can be used by tests and future runtime adapters.

Expected operations:

- `enqueue_task(create_request) -> Task`
- `try_acquire_directory_lock(task_id) -> LockDecision`
- `mark_preparing(task_id) -> Task`
- `mark_running(task_id) -> Task`
- `complete_task(task_id, result) -> Task`
- `fail_task(task_id, error) -> Task`
- `cancel_task(task_id) -> Task`
- `release_locks(task_id)`

Phase 5 should keep the scheduler deterministic and mostly synchronous at the
store boundary. If an async worker loop is added, it must be injectable and
disabled by default in tests unless a test starts it explicitly.

Global concurrency:

- add a config value such as `SchedulerConfig { max_concurrent_tasks }`
- default to `1` until runtime capacity exists
- enforce the limit in scheduler lock/claim logic
- do not infer provider runtime capacity in Phase 5

## Source Layout

Expected source layout after Phase 5:

```text
src/
  api/
    mod.rs
    agents.rs
    directories.rs
    health.rs
    providers.rs
    runtimes.rs
    tasks.rs
  scheduler/
    mod.rs
    locks.rs
    service.rs
    workspace.rs
  store/
    mod.rs
    sqlite.rs
    agent_profiles.rs
    directory_grants.rs
    tasks.rs
  task/
    mod.rs
    event.rs
    model.rs
    result.rs
    state.rs
  tests/
    mod.rs
    agents.rs
    api.rs
    cli.rs
    directories.rs
    registry.rs
    runtime.rs
    tasks.rs
```

Do not split into workspace crates in Phase 5. Keep the single-crate shape from
Phase 0 through Phase 4.

### File Responsibilities

- `src/task/model.rs`
  - task request, task view, creation input, filters, validation

- `src/task/state.rs`
  - task status enum, terminal state rules, transition validation

- `src/task/event.rs`
  - task event model, event type enum, sequence rules

- `src/task/result.rs`
  - normalized task result model

- `src/store/tasks.rs`
  - SQLite task persistence, event persistence, result persistence, transition
    transactions, filter queries, lock persistence

- `src/scheduler/locks.rs`
  - lock mode decisions and conflict rules

- `src/scheduler/workspace.rs`
  - workspace selection and preparation trait/fake implementation

- `src/scheduler/service.rs`
  - task enqueue, validation orchestration, lock acquisition, cancellation, and
    terminal transition helpers

- `src/api/tasks.rs`
  - task API request/response DTOs and stable HTTP error mapping

- `src/api/mod.rs`
  - register task routes and extend `AppState` with task store and scheduler
    config

- `src/config/mod.rs`
  - add scheduler config defaults without starting workers or writing database
    files during construction

- `src/store/sqlite.rs`
  - initialize task, task event, and directory lock schemas idempotently

- `src/tests/tasks.rs`
  - cover model validation, state transitions, store persistence, task API, lock
    behavior, cancellation, and Phase 4 integration

## Application State

Extend the Phase 4 `AppState`.

Required state:

```text
AppState
  providers_dir
  runtime_store
  runtime_detection_config
  directory_grant_store
  agent_profile_store
  task_store
  scheduler_config
```

Requirements:

- task store is shared by API handlers and scheduler service
- database path is injectable for tests
- task creation can access Agent Profile and Directory Grant stores
- default router construction remains simple for `main.rs`
- provider, runtime, directory, and agent tests remain straightforward
- no global static SQLite connection is introduced
- no worker loop starts automatically during router construction

## Implementation Steps

### Step 5.1: Add Task Model Types

Add `src/task/model.rs`, `src/task/state.rs`, `src/task/event.rs`, and
`src/task/result.rs`.

Acceptance:

- task status serializes as stable snake_case JSON
- workspace mode reuses the Phase 3/4 API value names
- task creation rejects empty prompt
- task creation rejects empty required capabilities
- task metadata must be a JSON object when present
- task timeout, when present, must be positive and within a bounded maximum
- state transition validator accepts only documented transitions
- terminal transition idempotency is represented explicitly
- task result model stores all roadmap result fields

### Step 5.2: Add Task-Time Policy Validation

Add validation orchestration under `src/scheduler/service.rs` or
`src/task/model.rs`.

Acceptance:

- unknown Agent Profiles are rejected
- unknown Directory Grants are rejected
- owner product mismatch is rejected
- agent mismatch is rejected
- provider override attempts are rejected
- model override attempts are rejected
- permission mode override attempts are rejected
- missing capabilities are rejected
- direct workspace mode is rejected unless Agent Profile and Directory Grant
  allow it
- direct mode task opt-in is required when the Directory Grant requires it
- validation does not run runtime detection
- validation does not spawn provider commands

### Step 5.3: Add SQLite Task Store

Add `src/store/tasks.rs` and extend `src/store/sqlite.rs`.

Acceptance:

- store initializes schema idempotently
- store can create a queued task
- store inserts `task.queued` with sequence `1`
- store can list tasks with filters
- store can fetch by task ID
- store can transition task states
- store can persist task events in transition transactions
- store can persist task results for terminal states
- store can cancel queued and waiting tasks
- store can reject invalid transitions
- tasks and events survive store re-open
- store returns stable domain errors for missing tasks and persistence failures

### Step 5.4: Add Directory Lock Store And Rules

Add lock behavior under `src/scheduler/locks.rs` and `src/store/tasks.rs`.

Acceptance:

- write-capable task lock requests conflict with active locks for the same
  directory
- compatible read-only shared lock requests can coexist when policy allows
- lock acquisition records task ID and directory ID
- lock conflict leaves task in `waiting_directory_lock`
- lock release is idempotent
- terminal task transitions release active locks
- lock tests do not spawn providers

### Step 5.5: Add Workspace Preparation Boundary

Add `src/scheduler/workspace.rs`.

Acceptance:

- direct mode returns the canonical Directory Grant path as working directory
- direct mode never schedules cleanup for the original directory
- worktree mode returns a structured prepared workspace record
- fake worktree preparer can be injected in tests
- workspace preparation failures move a task to `failed`
- no provider command is spawned

### Step 5.6: Add Scheduler Service

Add `src/scheduler/service.rs` and update `src/scheduler/mod.rs`.

Acceptance:

- service can enqueue validated tasks
- service can attempt lock acquisition
- service can move tasks through documented states
- service enforces `max_concurrent_tasks`
- service can cancel queued or waiting tasks
- service returns stable domain errors
- service uses injected stores and config
- no background worker starts during `AppState::default`

### Step 5.7: Add Task API Routes

Add `src/api/tasks.rs` and wire routes in `src/api/mod.rs`.

Acceptance:

- `POST /v1/tasks` returns `201` and a queued task
- `GET /v1/tasks` returns `{ "tasks": [] }`
- list filters work for owner product, agent, directory, and status
- `GET /v1/tasks/:task_id` returns one task
- missing tasks return stable `404` JSON
- invalid task requests return stable `400` JSON
- lock conflicts return stable `409` JSON when surfaced by the API
- `POST /v1/tasks/:task_id/cancel` returns the updated task
- task API responses exclude provider secrets, runtime adapter internals,
  control-plane tokens, and unapproved paths

### Step 5.8: Preserve Existing Behavior

Keep Phase 1 through Phase 4 behavior stable.

Acceptance:

- `GET /v1/providers` remains manifest-only
- `GET /v1/runtimes` still does not spawn commands
- `POST /v1/runtimes/detect` still only runs bounded version probes
- directory grant list/get/patch/delete behavior remains unchanged
- directory grant creation still validates Agent Profiles
- Agent Profile API behavior remains unchanged
- no task event streaming route is added
- no provider process execution is added

## Test Plan

Add tests for:

- task status values serialize to stable snake_case strings
- task creation input rejects empty prompt
- task creation input rejects empty required capabilities
- task creation input defaults required capabilities to `read`
- task creation input requires metadata to be an object when present
- task timeout validation rejects zero and excessive values
- state transition validator accepts documented transitions
- state transition validator rejects invalid transitions
- terminal transition idempotency works
- task result serializes roadmap result fields
- task-time validation accepts matching product, agent, directory, capabilities,
  and workspace mode
- task-time validation rejects missing Agent Profile
- task-time validation rejects missing Directory Grant
- task-time validation rejects owner product mismatch
- task-time validation rejects directory agent mismatch
- task-time validation rejects provider override attempts
- task-time validation rejects model override attempts
- task-time validation rejects permission mode override attempts
- task-time validation rejects missing capabilities
- task-time validation rejects direct mode when Agent Profile disallows direct
- task-time validation rejects direct mode when Directory Grant disallows direct
- task-time validation rejects missing direct task opt-in when required
- SQLite schema initializes on first open
- task creation persists queued task and initial event
- tasks survive store re-open
- events survive store re-open
- list returns tasks sorted consistently
- owner product filter returns only matching tasks
- agent filter returns only matching tasks
- directory filter returns only matching tasks
- status filter returns only matching tasks
- get returns one task by ID
- get missing task returns `task_not_found`
- transition updates timestamps correctly
- invalid transition returns `invalid_task_state`
- result persistence works for completed task
- cancellation works for queued task
- cancellation works for waiting lock task
- repeated cancellation is idempotent
- cancellation of completed task returns `task_already_terminal`
- exclusive lock rejects concurrent write task for same directory
- shared read locks can coexist when policy allows
- terminal transition releases active lock
- direct workspace preparation returns canonical grant path
- fake worktree preparation returns a structured prepared workspace
- workspace preparation failure marks task failed
- `POST /v1/tasks` returns `201` and `queued`
- `POST /v1/tasks` rejects invalid agent/directory/policy with stable JSON
- `GET /v1/tasks` returns JSON shape `{ "tasks": [] }`
- `GET /v1/tasks/:task_id` returns one task
- `POST /v1/tasks/:task_id/cancel` returns cancelled task
- task API does not expose provider secrets or runtime adapter internals
- provider, runtime, directory, and agent API tests still pass

Tests must use temporary directories and temporary SQLite database files. Tests
must not depend on the developer machine having real `codex`, `claude`, `git`,
or any provider CLI installed.

## Manual Verification

Run these commands before completing Phase 5:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/tasks
curl -X POST http://127.0.0.1:19514/v1/agents \
  -H 'content-type: application/json' \
  -d '{"id":"frontend-fixer","name":"Frontend Fixer","owner_product_id":"product_example","provider_id":"codex","model":"gpt-5-codex","execution_policy":{"default_workspace_mode":"direct","allow_direct_directory":true},"provider_config":{"custom_args":[],"custom_env_keys":[],"mcp_config":null,"permission_mode":"provider_default"}}'
curl -X POST http://127.0.0.1:19514/v1/directories/grant \
  -H 'content-type: application/json' \
  -d '{"product_id":"product_example","agent_id":"frontend-fixer","path":"/tmp","capabilities":["read"],"workspace_modes":["direct"],"default_workspace_mode":"direct","lock_policy":"shared","direct_mode_requires_explicit_task_opt_in":true}'
curl -X POST http://127.0.0.1:19514/v1/tasks \
  -H 'content-type: application/json' \
  -d '{"owner_product_id":"product_example","agent_id":"frontend-fixer","directory_id":"dir_1","prompt":"Inspect this project and report status.","required_capabilities":["read"],"workspace_mode":"direct","direct_mode_task_opt_in":true,"metadata":{"manual":true}}'
curl http://127.0.0.1:19514/v1/tasks/task_1
curl -X POST http://127.0.0.1:19514/v1/tasks/task_1/cancel
```

Expected behavior:

- quality gates pass
- registry check exits `0`
- first task list returns an empty list or existing local tasks
- profile and directory grant creation still work
- task creation returns `201`
- returned task is `queued`
- task creation does not run provider detection
- task creation does not spawn a provider command
- task get returns the same stored task
- cancellation returns the task in `cancelled`
- provider, runtime, directory, and agent routes still respond as in previous
  phases

When using a default local SQLite database, manual verification may leave local
Agent Profile, Directory Grant, and Task records. This is runtime data outside
the repository and should not be committed.

## Completion Checklist

- [x] Task model types exist.
- [x] Task ID generation exists and is tested.
- [x] Task state model exists and is tested.
- [x] Task event model exists and is tested.
- [x] Task result model exists and is tested.
- [x] Task creation validates prompt and required capabilities.
- [x] Task creation defaults required capabilities to `read`.
- [x] Task creation validates metadata shape.
- [x] Task creation validates timeout bounds.
- [x] Task-time validation rejects missing Agent Profiles.
- [x] Task-time validation rejects missing Directory Grants.
- [x] Task-time validation rejects owner product mismatches.
- [x] Task-time validation rejects directory agent mismatches.
- [x] Task-time validation rejects provider overrides.
- [x] Task-time validation rejects model overrides.
- [x] Task-time validation rejects permission mode overrides.
- [x] Task-time validation rejects missing capabilities.
- [x] Task-time validation rejects disallowed direct mode.
- [x] SQLite task schema initializes automatically.
- [x] Task store path is injectable for tests.
- [x] Tasks persist across store re-open.
- [x] Task events persist across store re-open.
- [x] Task create/list/get/transition/cancel behavior is tested.
- [x] Task result persistence is tested.
- [x] Directory lock conflict behavior is tested.
- [x] Directory lock release behavior is tested.
- [x] Workspace preparation boundary exists and is tested.
- [x] Scheduler service exists and is tested.
- [x] Scheduler config exists and is tested.
- [x] `POST /v1/tasks` exists.
- [x] `GET /v1/tasks` exists.
- [x] `GET /v1/tasks/:task_id` exists.
- [x] `POST /v1/tasks/:task_id/cancel` exists.
- [x] Task API returns stable error JSON.
- [x] Task API responses exclude provider secrets, runtime adapter internals,
      control-plane tokens, and unapproved paths.
- [x] Provider API behavior remains stable.
- [x] Runtime API behavior remains stable.
- [x] Directory API behavior remains stable.
- [x] Agent API behavior remains stable.
- [x] No task events streaming route is added.
- [x] No provider process execution is added.
- [x] No remote runtime execution is added.
- [x] No ACP adapter is added.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --tests --all-targets --all-features -- -D warnings` passes.
- [x] `cargo test --all-features --all-targets` passes.
- [x] `just registry-check` passes.

## Handoff to Phase 6

Phase 6 can start when tasks are durably stored, task creation validates Agent
Profile and Directory Grant policy, the scheduler can move tasks through the
state machine, directory locks prevent conflicting write tasks, and cancellation
works without provider execution.

The next phase should add:

- runtime adapter trait
- local CLI adapter
- provider command template rendering
- controlled process environment
- prompt passing by arg, stdin, or temp file
- stdout/stderr capture as task events
- provider process timeout and cancellation
- failed process to failed task mapping
- normalized provider execution result
- no SSE endpoint until Phase 7 unless needed for adapter tests
