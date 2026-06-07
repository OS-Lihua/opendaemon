# Phase 6: Runtime Adapters

## Goal

Execute queued tasks through a provider runtime adapter, starting with local CLI
providers, while keeping task validation, directory grants, workspace policy,
and task state controlled by OpenDaemon.

Phase 6 builds on Phase 5. It adds the execution boundary that turns a validated
task into a running provider process and records normalized output:

- runtime adapter trait
- local CLI adapter
- provider command template rendering
- controlled child process environment
- prompt passing by argument, stdin, or temporary file
- stdout and stderr capture as task events
- provider process timeout and cancellation
- failed process to failed task mapping
- normalized provider execution result
- adapter registry and selection
- ACP, HTTP, and native adapter extension points

Phase 6 must not add Server-Sent Events, websocket event streaming, product
authentication, keyring secret storage, remote control-plane dispatch, or a
desktop UI. Phase 6 may persist process output as task events; Phase 7 exposes
those events to products through streaming APIs.

## Scope

Phase 6 delivers runtime execution infrastructure:

- define one runtime adapter interface for all provider integration types
- implement local CLI execution for `integration_type = cli`
- render provider manifest `execution.args` with task/profile values
- execute provider commands without invoking a shell
- set working directory to the prepared task workspace
- pass prompts according to `execution.input_mode`
- inject only controlled non-secret environment variables
- strip provider secret variables unless explicitly supplied by a future secret
  provider
- capture stdout and stderr without blocking indefinitely
- persist process output as normalized task events
- enforce task timeout metadata and adapter default timeout
- support cancellation by terminating an in-flight local child process
- map process exit status into task completion or failure
- store a normalized task result for completed and failed tasks
- keep scheduler locks released on terminal task states
- provide fake adapters for deterministic tests
- add adapter extension points for ACP, HTTP, and native providers
- reject remote HTTP execution unless every remote-execution policy is present
  and explicit
- preserve provider, runtime detection, directory, agent, and task API behavior
- quality gates passing

The first production execution path is local CLI. ACP, remote HTTP, and native
adapters should have clear traits, error types, and policy gates, but do not
need complete protocol implementations in Phase 6.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 3 spec: `docs/tasks/phase-3-directory-grants.md`
- Phase 4 spec: `docs/tasks/phase-4-agent-profiles.md`
- Phase 5 spec: `docs/tasks/phase-5-task-scheduler.md`
- Phase 5 implementation:
  - `src/agent/profile.rs`
  - `src/api/tasks.rs`
  - `src/registry/manifest.rs`
  - `src/runtime/detect.rs`
  - `src/runtime/model.rs`
  - `src/runtime/store.rs`
  - `src/scheduler/service.rs`
  - `src/scheduler/workspace.rs`
  - `src/security/directory.rs`
  - `src/store/tasks.rs`
  - `src/task/event.rs`
  - `src/task/model.rs`
  - `src/task/result.rs`
  - `src/task/state.rs`

## Deliverables

- Runtime adapter trait exists.
- Local CLI adapter exists.
- Adapter selection uses provider manifest integration type.
- Unsupported integration types fail with stable adapter errors.
- CLI adapter renders provider manifest execution arguments.
- CLI adapter rejects unknown template variables.
- CLI adapter passes prompt by argument for `input_mode = arg`.
- CLI adapter passes prompt by stdin for `input_mode = stdin`.
- CLI adapter passes prompt by temporary file for `input_mode = temp_file`.
- CLI adapter launches child processes without a shell.
- CLI adapter runs in the prepared task workspace.
- CLI adapter removes provider secret environment variables by default.
- CLI adapter supports a small explicit env allowlist from Agent Profile
  `custom_env_keys` by name only.
- CLI adapter captures stdout as `process.stdout` task events.
- CLI adapter captures stderr as `process.stderr` task events.
- CLI adapter maps successful exit to completed task.
- CLI adapter maps non-zero exit to failed task.
- CLI adapter enforces timeout.
- CLI adapter supports cancellation.
- Process cancellation releases directory locks.
- Task completion stores a normalized result.
- Task failure stores a normalized result with error.
- A generic fake CLI provider can echo a prompt.
- Existing runtime detection remains a bounded version probe.
- Existing `GET /v1/runtimes` still does not spawn commands.
- Existing task create/list/get/cancel behavior remains stable.
- No SSE event streaming route is added.
- No keyring or secret storage is added.
- No remote control-plane dispatch is added.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 6:

- `GET /v1/tasks/:task_id/events` streaming API
- Server-Sent Events
- websocket event delivery
- remote control-plane task claim/start/complete protocol
- product authentication or API scopes
- keyring-backed secret storage
- provider credential UI
- ACP protocol session implementation
- remote HTTP task upload implementation
- native provider plugins
- desktop UI
- audit log
- file watching
- multi-daemon distributed locks

Phase 6 may persist normalized process events in SQLite because Phase 5 already
added the event store. It must not expose the streaming product-facing event
endpoint; that belongs to Phase 7.

## Dependencies

Keep Phase 0 through Phase 5 dependencies.

Phase 6 can use existing dependencies:

```toml
tokio = { version = "1", features = ["process", "io-util", "time"] }
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }
```

Avoid adding PTY, websocket, keyring, notify, control-plane, or template engine
dependencies in Phase 6. Provider command rendering can be a small, explicit
renderer for known variables:

- `{{prompt}}`
- `{{model}}`
- `{{workspace}}`
- `{{task_id}}`
- `{{agent_id}}`
- `{{directory_id}}`

If implementation needs temporary files for `input_mode = temp_file`, prefer
using the existing platform temp directory APIs and deterministic cleanup rather
than adding `tempfile` unless cross-platform correctness requires it.

## Runtime Adapter Contract

Add a runtime adapter boundary under `src/runtime/`.

Expected trait shape:

```rust
pub trait RuntimeAdapter {
    async fn execute(&self, request: RuntimeExecutionRequest) -> RuntimeExecutionOutcome;
    async fn cancel(&self, task_id: &str) -> RuntimeCancelOutcome;
}
```

If `async fn` in traits would force an extra dependency, use a boxed future or
keep the service-level API async while concrete adapter methods are async
functions. Do not add a dependency only for async traits unless the ergonomics
are clearly worth it.

### Runtime Execution Request

Use this internal shape:

```json
{
  "task_id": "task_1",
  "provider_id": "generic-test-provider",
  "runtime_id": "rt_generic_test_provider_local_cli",
  "executable": "/tmp/bin/test-provider",
  "manifest": {},
  "agent_profile": {},
  "directory_grant": {},
  "task": {},
  "workspace": {
    "working_directory": "/tmp/workspace"
  },
  "timeout_seconds": 300
}
```

Requirements:

- `task_id` identifies the durable task.
- `provider_id` comes from the Agent Profile.
- `runtime_id` comes from a detected available runtime.
- `executable` must be an already-resolved executable path.
- `manifest` is the provider manifest.
- `agent_profile` is the stored Agent Profile.
- `directory_grant` is the stored Directory Grant.
- `task` is the stored task.
- `workspace` is the prepared workspace from Phase 5.
- `timeout_seconds` is bounded.

The request must not contain provider secret values in Phase 6.

### Runtime Execution Outcome

Use this internal shape:

```json
{
  "status": "completed",
  "exit_code": 0,
  "final_message": "done",
  "changed_files": [],
  "diff": null,
  "session_id": null,
  "provider_result": null,
  "usage": null,
  "error": null
}
```

Status values:

- `completed`
- `failed`
- `cancelled`
- `timed_out`

Requirements:

- successful process exit maps to `completed`
- non-zero process exit maps to `failed`
- timeout maps to `timed_out`
- cancellation maps to `cancelled`
- stderr alone does not imply failure when exit code is zero
- provider-specific data is stored under `provider_result` only when structured
  and non-secret

## Local CLI Adapter

The local CLI adapter executes providers whose manifests declare:

```json
{
  "integration_type": "cli"
}
```

### Command Resolution

Phase 6 must not redo PATH detection at task time. Use Phase 2 runtime state:

- task execution requires an `available` runtime for the task provider
- the executable path comes from `RuntimeView.executable`
- missing or unavailable runtimes leave the task queued or fail with stable
  `runtime_unavailable` depending on the scheduler entry point
- `GET /v1/runtimes` remains read-only and does not spawn commands
- `POST /v1/runtimes/detect` remains the only route that performs detection

### Command Rendering

Render `manifest.execution.args` with a small explicit template renderer.

Supported variables:

- `{{prompt}}`
- `{{model}}`
- `{{workspace}}`
- `{{task_id}}`
- `{{agent_id}}`
- `{{directory_id}}`

Rules:

- unknown variables are rejected before process spawn
- rendered args are passed directly to `Command::args`
- no shell is invoked
- empty rendered args are preserved only when the manifest explicitly produced
  them; do not drop user prompt text
- Agent Profile `provider_config.custom_args` are appended after manifest args
  only after Phase 4 validation has rejected reserved flags
- task-time provider, model, and permission overrides remain rejected

### Prompt Input Modes

Support the manifest `execution.input_mode` values:

- `arg`: prompt is rendered through `{{prompt}}` in args
- `stdin`: prompt is written to child stdin
- `temp_file`: prompt is written to an OpenDaemon-managed temporary file and the
  file path is rendered through `{{prompt}}` or a dedicated template value

Rules:

- `stdin` mode must close stdin after writing the prompt
- `temp_file` mode must clean up the prompt file after process exit or
  cancellation
- prompt contents must not be logged
- prompt contents may be persisted only as the trusted local task field already
  introduced in Phase 5

### Environment Policy

Start with a controlled child environment:

- remove provider manifest `environment.required` and `environment.optional`
  keys unless a future secret provider supplies values
- remove OpenDaemon internal variables
- do not pass product tokens
- do not pass daemon tokens
- do not pass arbitrary process environment wholesale
- allow Agent Profile `custom_env_keys` names to be copied from the daemon
  environment only if explicitly enabled by a local execution config flag

Preferred Phase 6 behavior:

- default to a minimal inherited environment sufficient for process launch
- explicitly remove provider secret keys from the environment
- test that secret-like provider env vars are not visible to fake commands

Do not store secret values in SQLite.

### Working Directory

The adapter must run in the Phase 5 prepared workspace:

- direct mode uses the canonical Directory Grant path
- worktree mode uses the prepared worktree path
- process launch must fail before spawn if working directory is missing
- adapter must not delete the original directory
- adapter must not mutate the original directory for worktree-mode tests

### Process Output Events

Persist stdout and stderr as task events:

- `process.stdout`
- `process.stderr`

Event payload shape:

```json
{
  "text": "line or chunk",
  "stream": "stdout"
}
```

Rules:

- preserve event ordering per task as much as possible
- do not require line buffering if chunked reading is simpler and reliable
- cap individual event payload size to avoid unbounded memory usage
- do not expose events through SSE in Phase 6
- tests can read events directly from the store

## Scheduler Integration

Add an execution service that composes existing Phase 5 pieces:

1. load task
2. reject terminal tasks
3. load Agent Profile
4. load Directory Grant
5. load provider manifest
6. load available runtime from `RuntimeStore`
7. acquire directory lock
8. prepare workspace
9. transition task to `running`
10. call runtime adapter
11. persist output events
12. persist normalized task result
13. transition to terminal state
14. release directory lock

This service may be called directly from tests. A background worker loop may be
added only if it is deterministic, disabled by default in tests, and has a
shutdown path.

### Worker Loop

If implemented in Phase 6, keep the worker small:

- poll queued tasks
- respect `SchedulerConfig.max_concurrent_tasks`
- start one task at a time by default
- never run on `router()` construction
- expose an explicit start function for `main.rs` or future daemon orchestration

It is acceptable for Phase 6 to implement the execution service without a
long-running worker loop if task execution can be driven by an explicit test or
internal API. Do not add public task-start API unless the implementation needs a
manual trigger for local testing.

## Task State Mapping

Adapter execution must use the Phase 5 task state machine.

Expected mappings:

- before adapter spawn: `preparing`
- after successful spawn: `running`
- exit code `0`: `completed`
- exit code non-zero: `failed`
- spawn failure: `failed`
- timeout: `timed_out`
- cancellation before spawn: `cancelled`
- cancellation during process: `cancelled`

Rules:

- terminal transitions release directory locks
- terminal transitions are idempotent where Phase 5 allows
- failed spawn records an error result
- non-zero exit records exit status and stderr summary as error metadata
- timeout kills and reaps the child process
- cancellation kills and reaps the child process when graceful signal is not
  supported

## Cancellation

Phase 6 extends cancellation from durable state to process control.

Requirements:

- cancelling a queued or waiting task still works as Phase 5
- cancelling a running local CLI task terminates the child process
- child process is reaped after cancellation
- cancellation writes `task.cancelled` event
- cancellation stores a result with status `cancelled`
- cancellation releases directory locks
- repeated cancellation remains idempotent for cancelled tasks

Use provider manifest `execution.cancel_signal`:

- `SIGTERM`: graceful terminate on Unix, best-effort fallback on Windows
- `SIGINT`: interrupt on Unix, best-effort fallback on Windows
- `kill`: force kill
- `none`: force kill if OpenDaemon must reclaim the task

Do not rely on shell process groups in Phase 6. If process-tree termination is
needed later, defer it to a dedicated hardening phase.

## Remote And Non-CLI Adapter Gates

Phase 6 should define extension points but keep non-CLI execution safe.

### ACP

For `integration_type = acp`:

- define adapter trait mapping placeholders for future ACP execution
- return stable `adapter_not_implemented` for actual execution
- do not launch ACP servers
- do not add ACP permission response APIs

### HTTP

For `integration_type = http`:

- return `remote_execution_not_allowed` unless all future remote execution
  policies are explicit
- do not upload source code
- do not call remote provider endpoints
- do not add control-plane tokens

### Native

For `integration_type = native`:

- define registry/trait extension point
- return stable `adapter_not_implemented` for actual execution
- do not load plugins or dynamic libraries

## API Contract

Phase 6 does not need new public routes. It may extend existing task behavior
through internal scheduler execution.

Existing routes remain:

```http
POST /v1/tasks
GET /v1/tasks
GET /v1/tasks/:task_id
POST /v1/tasks/:task_id/cancel
```

If an execution trigger is needed for manual verification, prefer an internal
test helper over a public route. If a public route is unavoidable, use:

```http
POST /v1/tasks/:task_id/start
```

and mark it local-development-only in this phase. Do not add it unless tests and
manual verification cannot otherwise drive execution.

Task responses may include completed/failed/timed-out results using the Phase 5
result shape. They must not include raw child process handles, environment
values, provider secrets, daemon tokens, product tokens, or remote control-plane
fields.

## Error Codes

Add stable execution-layer error codes:

- `runtime_unavailable`
- `adapter_not_found`
- `adapter_not_implemented`
- `adapter_execution_failed`
- `command_render_failed`
- `command_spawn_failed`
- `command_timeout`
- `command_cancelled`
- `working_directory_missing`
- `input_mode_not_supported`
- `remote_execution_not_allowed`
- `task_already_terminal`
- `store_error`
- `registry_error`

HTTP mapping is only needed if a public start route is added. Internal service
errors should still map to stable domain codes for future API use.

## SQLite Store Updates

Phase 5 already added task events and task results. Phase 6 may extend the
stored result payload but should avoid schema churn unless required.

Allowed store additions:

- helper to append task events outside state transitions
- helper to save execution result with exit status and error data
- helper to atomically transition task and save result
- helper to mark running task as cancelled from process cancellation

Avoid adding migrations tooling in Phase 6.

## Source Layout

Expected source layout after Phase 6:

```text
src/
  runtime/
    mod.rs
    adapter.rs
    cli.rs
    detect.rs
    model.rs
    store.rs
    template.rs
  scheduler/
    execution.rs
    locks.rs
    mod.rs
    service.rs
    workspace.rs
  task/
    event.rs
    model.rs
    mod.rs
    result.rs
    state.rs
  store/
    tasks.rs
  tests/
    runtime_adapter.rs
    tasks.rs
```

Do not split into workspace crates in Phase 6. Keep the single-crate shape from
Phase 0 through Phase 5.

### File Responsibilities

- `src/runtime/adapter.rs`
  - runtime adapter trait, execution request/outcome, adapter errors, adapter
    selection entry point

- `src/runtime/template.rs`
  - small explicit renderer for known provider command variables

- `src/runtime/cli.rs`
  - local CLI process adapter, prompt input modes, env policy, output capture,
    timeout, cancellation

- `src/scheduler/execution.rs`
  - orchestration from task to adapter execution and terminal task result

- `src/store/tasks.rs`
  - append task events, save execution results, transition terminal states

- `src/tests/runtime_adapter.rs`
  - local CLI adapter tests with fake commands and temporary workspaces

- `src/tests/tasks.rs`
  - preserve task scheduler tests from Phase 5

## Implementation Steps

### Step 6.1: Add Runtime Adapter Model

Add `src/runtime/adapter.rs`.

Acceptance:

- execution request type exists
- execution outcome type exists
- adapter error type has stable codes
- CLI, ACP, HTTP, and native integration choices are represented
- unsupported integrations return stable errors
- no provider command is spawned by adapter selection alone

### Step 6.2: Add Command Template Renderer

Add `src/runtime/template.rs`.

Acceptance:

- known variables render correctly
- unknown variables are rejected
- rendering never invokes a shell
- prompt text remains a single argument when rendered into args
- missing required variable values return `command_render_failed`

### Step 6.3: Add Local CLI Adapter

Add `src/runtime/cli.rs`.

Acceptance:

- launches executable path directly with `tokio::process::Command`
- sets current directory to prepared workspace
- supports `input_mode = arg`
- supports `input_mode = stdin`
- supports `input_mode = temp_file`
- appends validated Agent Profile custom args
- removes provider secret environment keys by default
- captures stdout and stderr
- returns completed outcome on exit code `0`
- returns failed outcome on non-zero exit
- does not use a shell

### Step 6.4: Add Timeout And Cancellation

Extend `src/runtime/cli.rs`.

Acceptance:

- timeout kills and reaps the child process
- timeout returns `command_timeout`
- cancellation kills and reaps the child process
- cancellation returns `command_cancelled`
- cancellation does not leave a running child process in tests
- Windows and Unix tests use platform-appropriate fake commands

### Step 6.5: Add Task Event Persistence Helpers

Modify `src/store/tasks.rs`.

Acceptance:

- append arbitrary task event with monotonic sequence
- append `process.stdout`
- append `process.stderr`
- event payloads are JSON objects
- output event persistence survives store re-open
- output event persistence does not require SSE

### Step 6.6: Add Scheduler Execution Service

Add `src/scheduler/execution.rs`.

Acceptance:

- loads task, Agent Profile, Directory Grant, provider manifest, and runtime
- rejects missing or unavailable runtime
- acquires directory lock
- prepares workspace
- transitions task to running before adapter execution
- persists stdout/stderr events
- saves completed result on success
- saves failed result on adapter failure
- saves timed-out result on timeout
- releases locks on terminal state
- does not start automatically during router construction

### Step 6.7: Add Fake CLI Provider Tests

Add `src/tests/runtime_adapter.rs`.

Acceptance:

- fake command echoes prompt in arg mode
- fake command echoes prompt in stdin mode
- fake command reads prompt temp file in temp-file mode
- non-zero fake command fails task
- slow fake command times out
- cancellable fake command is killed and reaped
- fake command sees prepared working directory
- fake command does not receive provider secret environment variables
- process output is available through task events

### Step 6.8: Preserve Existing Behavior

Keep Phase 1 through Phase 5 behavior stable.

Acceptance:

- `GET /v1/providers` remains manifest-only
- `GET /v1/runtimes` still does not spawn commands
- `POST /v1/runtimes/detect` still only runs bounded version probes
- task creation still does not execute providers
- task cancellation before execution still works
- directory grant behavior remains stable
- Agent Profile behavior remains stable
- no SSE endpoint is added
- no remote HTTP execution is added
- no ACP session execution is added

## Test Plan

Add tests for:

- adapter request serializes/describes expected fields without secrets
- adapter selection returns CLI adapter for CLI provider
- adapter selection rejects ACP with `adapter_not_implemented`
- adapter selection rejects HTTP with `remote_execution_not_allowed`
- adapter selection rejects native with `adapter_not_implemented`
- template renderer replaces `{{prompt}}`
- template renderer replaces `{{model}}`
- template renderer replaces `{{workspace}}`
- template renderer rejects unknown variable
- template renderer preserves prompt as one argument
- CLI adapter runs fake command without shell
- CLI adapter uses prepared working directory
- CLI adapter supports arg input mode
- CLI adapter supports stdin input mode
- CLI adapter supports temp-file input mode
- CLI adapter appends validated custom args
- CLI adapter removes provider required env vars
- CLI adapter captures stdout as task event data
- CLI adapter captures stderr as task event data
- CLI adapter succeeds on exit code `0`
- CLI adapter fails on non-zero exit code
- CLI adapter timeout kills and reaps child process
- CLI adapter cancellation kills and reaps child process
- execution service rejects unavailable runtime
- execution service moves task to running before adapter execution
- execution service stores completed result
- execution service stores failed result
- execution service stores timed-out result
- execution service releases locks on completion
- execution service releases locks on failure
- execution service releases locks on timeout
- task creation API still returns queued without executing
- task cancellation API still cancels queued task
- `GET /v1/runtimes` still does not spawn commands
- provider, runtime, directory, agent, and task tests still pass

Tests must use temporary directories and fake commands. Tests must not depend on
the developer machine having real `codex`, `claude`, `git`, ACP servers, or
remote provider credentials installed.

## Manual Verification

Run these commands before completing Phase 6:

```bash
cargo fmt --all -- --check
cargo clippy --tests --all-targets --all-features -- -D warnings
cargo test --all-features --all-targets
just registry-check
```

If an internal or development-only start hook exists, also verify a fake CLI
provider end to end:

```bash
cargo run -- daemon --host 127.0.0.1 --port 19514
curl http://127.0.0.1:19514/v1/tasks
curl -X POST http://127.0.0.1:19514/v1/runtimes/detect
curl -X POST http://127.0.0.1:19514/v1/tasks \
  -H 'content-type: application/json' \
  -d '{"owner_product_id":"product_example","agent_id":"fake-cli","directory_id":"dir_1","prompt":"echo this","required_capabilities":["read"],"workspace_mode":"direct","direct_mode_task_opt_in":true}'
```

Expected behavior:

- quality gates pass
- registry check exits `0`
- fake CLI task can complete through the runtime adapter in tests
- stdout/stderr are persisted as task events
- successful fake command stores completed result
- failed fake command stores failed result
- timeout stores timed-out result
- task creation still only creates queued task unless execution service is
  explicitly driven
- no SSE endpoint is exposed
- daemon startup does not run provider detection or execute tasks

## Completion Checklist

- [x] Runtime adapter trait exists.
- [x] Runtime execution request model exists.
- [x] Runtime execution outcome model exists.
- [x] Adapter error codes are stable.
- [x] CLI adapter selection works.
- [x] ACP adapter gate returns `adapter_not_implemented`.
- [x] HTTP adapter gate returns `remote_execution_not_allowed`.
- [x] Native adapter gate returns `adapter_not_implemented`.
- [x] Command template renderer exists and is tested.
- [x] Unknown template variables are rejected.
- [x] CLI adapter launches without shell.
- [x] CLI adapter uses prepared workspace directory.
- [x] CLI adapter supports arg input mode.
- [x] CLI adapter supports stdin input mode.
- [x] CLI adapter supports temp-file input mode.
- [x] CLI adapter appends validated custom args.
- [x] CLI adapter removes provider secret environment variables.
- [x] CLI adapter captures stdout.
- [x] CLI adapter captures stderr.
- [x] stdout events persist as task events.
- [x] stderr events persist as task events.
- [x] exit code `0` completes task execution.
- [x] non-zero exit fails task execution.
- [x] timeout kills and reaps child process.
- [x] cancellation kills and reaps child process.
- [x] completed task result is stored.
- [x] failed task result is stored.
- [x] timed-out task result is stored.
- [x] cancelled task result is stored.
- [x] execution releases directory locks.
- [x] unavailable runtime is rejected.
- [x] task creation API still does not execute providers.
- [x] `GET /v1/runtimes` still does not spawn commands.
- [x] no SSE endpoint is added.
- [x] no remote HTTP execution is added.
- [x] no ACP session execution is added.
- [x] provider API behavior remains stable.
- [x] runtime API behavior remains stable.
- [x] directory API behavior remains stable.
- [x] agent API behavior remains stable.
- [x] task API behavior remains stable.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --tests --all-targets --all-features -- -D warnings` passes.
- [x] `cargo test --all-features --all-targets` passes.
- [x] `just registry-check` passes.

## Handoff to Phase 7

Phase 7 can start when local CLI adapter execution is available, process output
is persisted as task events, task terminal results are normalized, timeouts and
cancellation are enforced, and task creation still remains separated from
execution.

The next phase should add:

- event replay API
- `GET /v1/tasks/:task_id/events`
- Server-Sent Events
- event cursors
- reconnect and resume behavior
- heartbeat comments for idle SSE connections
- provider permission request events
- optional permission response API for protocols that support responses
