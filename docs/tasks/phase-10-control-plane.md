# Phase 10: Control Plane

## Goal

Support remote products and multiple machines by adding an optional control
plane connection for OpenDaemon, plus the first constrained remote HTTP
execution slice, while keeping local directory grants, workspace policy,
product scopes, and runtime execution ownership inside the daemon.

Phase 10 builds on Phase 9. It does not redesign local product auth, task
storage, SSE event replay, scheduler ownership, CLI execution, or ACP session
execution. It extends the daemon so a remote control plane can dispatch work to
an enrolled daemon without bypassing the local safety model:

- daemon registration
- heartbeat and liveness
- websocket control-plane task dispatch
- claim/start/progress/complete/fail/cancel lifecycle bridging
- daemon token
- task token
- runtime status publication
- first production `integration_type = "http"` remote execution path
- explicit remote-execution policy enforcement

This phase must not add desktop UX, browser login flows, keyring-backed secret
storage, multi-daemon coordination, or a general cloud orchestration platform.

## Scope

Phase 10 delivers optional control-plane connectivity and the first remote
execution path:

- add a daemon identity and registration flow for a remote control plane
- authenticate control-plane connections with a daemon-scoped credential
- maintain a long-lived websocket session for task dispatch and liveness
- publish daemon and runtime status to the control plane
- accept remotely dispatched tasks without exposing the local HTTP API directly
- bridge remote task dispatch into the existing local scheduler and task service
- add task-scoped credentials or opaque task tokens for control-plane protocol
  integrity where the daemon needs to acknowledge or complete remote work
- keep remote and local tasks on the same durable task state machine
- support reconnect without losing local task state
- mark stale daemons or runtimes offline when heartbeat/liveness expires
- add the first `integration_type = "http"` runtime adapter path
- allow remote execution only when every existing remote-execution policy gate
  is explicitly satisfied
- package only the approved workspace subset or diff/context needed by the HTTP
  adapter
- persist explicit metadata that code or workspace content was sent to a remote
  provider
- preserve existing Phase 8 product auth and Phase 9 permission/event
  boundaries for locally created tasks
- quality gates passing

Phase 10 is the remote-control and remote-execution bridge. It is not a full
cloud product backend, not a general-purpose queueing system, and not a secret
management phase.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 6 spec: `docs/tasks/phase-6-runtime-adapters.md`
- Phase 7 spec: `docs/tasks/phase-7-event-streaming.md`
- Phase 8 spec: `docs/tasks/phase-8-product-authentication.md`
- Phase 9 spec: `docs/tasks/phase-9-acp-adapter.md`
- Current implementation:
  - `src/api/auth.rs`
  - `src/api/mod.rs`
  - `src/api/products.rs`
  - `src/api/providers.rs`
  - `src/api/runtimes.rs`
  - `src/api/tasks.rs`
  - `src/config/mod.rs`
  - `src/product/mod.rs`
  - `src/registry/manifest.rs`
  - `src/registry/validate.rs`
  - `src/runtime/adapter.rs`
  - `src/runtime/acp.rs`
  - `src/runtime/cli.rs`
  - `src/runtime/detect.rs`
  - `src/runtime/model.rs`
  - `src/runtime/store.rs`
  - `src/scheduler/execution.rs`
  - `src/scheduler/service.rs`
  - `src/scheduler/workspace.rs`
  - `src/store/products.rs`
  - `src/store/sqlite.rs`
  - `src/store/tasks.rs`
  - `src/task/event.rs`
  - `src/task/model.rs`
  - `src/task/result.rs`
  - `src/task/service.rs`
  - `src/tests/api.rs`
  - `src/tests/registry.rs`
  - `src/tests/runtime.rs`
  - `src/tests/runtime_adapter.rs`
  - `src/tests/tasks.rs`

## Deliverables

- Daemon registration model types exist.
- Daemon session or daemon-token model types exist.
- Control-plane connectivity can be enabled through daemon configuration.
- A daemon can register with a control plane and receive a stable daemon ID.
- A daemon can reconnect with existing identity without losing durable local
  task state.
- A daemon can maintain heartbeat or websocket liveness to the control plane.
- Daemon offline detection uses bounded staleness rules.
- Runtime status can be reported to the control plane with stable online/offline
  semantics.
- Remote task dispatch maps onto the existing task lifecycle instead of creating
  a parallel execution pipeline.
- Control-plane task callbacks support stable claim/start/progress/complete/fail
  or cancelled semantics.
- Callback delivery is idempotent for repeated terminal updates.
- Control-plane protocol authentication uses daemon credentials and task-scoped
  credentials without exposing local product tokens.
- `integration_type = "http"` can be selected by the runtime adapter when
  policy allows it.
- HTTP providers remain rejected with a stable error when remote-execution
  policy gates are not all satisfied.
- Remote execution is allowed only when:
  - provider manifest declares remote execution capability
  - provider security metadata declares code may leave the machine
  - Agent Profile selects a remote-capable runtime
  - Directory Grant allows remote execution
  - authenticated product scope includes `tasks:remote_execution`
- HTTP adapter uploads only the bounded workspace package, diff, or context
  explicitly defined by adapter policy.
- Task events and/or task result metadata persist that remote code upload
  occurred.
- Remote execution never sends bootstrap tokens, product tokens, daemon tokens,
  or unrelated local secrets to provider endpoints.
- Existing local CLI and ACP execution behavior remains stable.
- Existing local task create/get/list/cancel/events APIs remain stable.
- Existing ownership, directory, workspace, and permission rules remain
  enforced by OpenDaemon.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 10:

- desktop UI or Tauri app
- browser login or human account flows
- OAuth, OIDC, SSO, or user identity platform work
- keyring-backed provider secret storage
- provider API-key management UX
- general-purpose cloud orchestration features such as retries, dead-letter
  queues, priority scheduling, or fleet balancing
- distributed locks or shared task state across multiple daemons
- arbitrary cross-daemon task migration
- provider-specific custom cloud APIs outside the normalized task lifecycle
- remote execution for every possible transport class
- full bidirectional product event websocket APIs for local clients
- exposing raw local paths to the control plane
- letting the control plane bypass local directory grants or workspace policy
- redesigning the Phase 8 scope model
- redesigning ACP session semantics

Phase 10 is about remote task delivery to a local policy-enforcing daemon plus
one constrained HTTP remote-execution slice.

## Dependencies

Keep Phase 0 through Phase 9 dependencies. Add only what is required for
control-plane websocket transport, bounded reconnect logic, and HTTP provider
calls.

Likely additions:

```toml
tokio-tungstenite = "0.24"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

If current stable crate APIs differ at implementation time, use the current
stable APIs and keep dependency purposes narrow:

- websocket session transport with bounded reconnect
- HTTP request/response transport for remote providers
- cancellation-safe async coordination
- request signing or bearer header helpers if needed

Do not add browser, desktop, OAuth, keyring, database-replication, or generic
workflow-engine dependencies in Phase 10.

## Control Plane Contract

Phase 10 adds an optional remote control-plane connection. The daemon remains
the source of truth for local authorization and execution policy.

### Daemon Registration

Requirements:

- a daemon can be configured with control-plane endpoint metadata and a local
  registration credential or enrollment secret
- first registration returns or confirms a stable `daemon_id`
- repeated registration for the same local daemon must be idempotent or
  explicitly resumable
- registration must include enough metadata for the control plane to route
  tasks safely:
  - daemon version
  - platform information
  - runtime inventory summary
  - declared capability summary
- registration must not expose raw local directory paths
- registration must not expose local product API tokens

Recommended persisted daemon identity shape:

```json
{
  "daemon_id": "daemon_123",
  "control_plane_url": "wss://control.example.com",
  "status": "online",
  "registered_at": "2026-06-07T00:00:00Z",
  "last_heartbeat_at": "2026-06-07T00:00:30Z"
}
```

### Daemon Authentication

Phase 10 introduces cloud-side daemon credentials without replacing Phase 8
local product credentials.

Requirements:

- daemon authentication is separate from local product auth
- daemon credentials authenticate the daemon to the control plane only
- daemon credentials must never authorize local `/v1/*` product APIs
- product tokens must never be accepted as daemon credentials
- bootstrap token must never be forwarded to the control plane
- daemon auth failures produce stable reconnectable errors

Recommended stable error codes:

- `control_plane_auth_failed`
- `control_plane_registration_failed`
- `control_plane_unavailable`
- `control_plane_protocol_error`

### Heartbeat And Liveness

Requirements:

- the daemon must keep a bounded liveness signal through heartbeat, websocket
  keepalive, or both
- missed liveness beyond a configured threshold marks the daemon offline at the
  control plane
- runtime status derived from local detection must eventually converge at the
  control plane
- reconnect must not wipe durable local tasks or task-event history
- reconnect may require re-registration or session resume, but the daemon must
  not duplicate task execution for already-running tasks

### Remote Task Dispatch

The control plane changes how a task arrives, not how it executes locally.

Requirements:

- remotely dispatched tasks must still become durable local tasks
- the existing scheduler continues to own capacity checks, directory locks,
  workspace preparation, timeout handling, and cancellation
- remotely dispatched tasks must use the same terminal states as local tasks
- claim/start/complete/fail/cancelled callbacks must be idempotent
- remote cancellation must bridge into the existing cancellation path
- remote tasks must still persist normalized task events and final task results

Phase 10 may introduce a separate internal task-ingest path for control-plane
messages, but it must feed the same durable task store and scheduler used by
local API task creation.

### Task Token

Phase 10 introduces a task-scoped cloud credential boundary.

Requirements:

- task tokens are scoped to one control-plane task
- task tokens authenticate daemon callbacks or event pushes for that task only
- task tokens are not product tokens and are not provider credentials
- task tokens must not authorize access to unrelated local tasks
- task tokens must not be written into provider child-process environments or
  remote provider payloads
- task token replay or mismatch must fail with a stable control-plane error

Phase 10 does not need a public local task-token API. It only needs enough
internal model support to authenticate daemon-to-control-plane task updates.

## Remote Execution Policy Contract

Phase 10 is the first production remote-execution phase. Remote execution must
stay explicit and auditable.

### Policy Gates

A task may use a remote HTTP provider only when all of these are true:

1. provider manifest `integration_type = "http"` or equivalent runtime path is
   selected
2. provider manifest capability `remote_execution = true`
3. provider security metadata declares that code may leave the machine
4. Agent Profile selects that remote-capable provider/runtime intentionally
5. Directory Grant allows remote execution for the requested capabilities
6. authenticated product token holds `tasks:remote_execution`

Requirements:

- any missing policy gate must fail closed
- failure should use a stable policy or adapter error rather than a transport
  error
- remote-execution policy checks happen before any workspace packaging or
  upload
- policy enforcement belongs to OpenDaemon, not provider manifests alone

### Data-Boundary Requirements

OpenDaemon remains the owner of the local file boundary even when the provider
is remote.

Requirements:

- remote adapters may package only the minimum approved workspace content, diff,
  or contextual files needed for the task
- adapter packaging must respect the prepared workspace and path guard rules
- packaging must not escape the granted directory or workspace
- remote providers must not receive raw local directory paths when a relative or
  logical identifier is sufficient
- remote providers must not receive unrelated repositories, caches, or daemon
  state
- the daemon must persist explicit metadata that code or file content was sent
  to a remote provider

Recommended recorded metadata:

- provider ID
- runtime ID
- remote endpoint origin
- upload mode such as `workspace_subset`, `diff`, or `context_only`
- file count and byte-count summaries where practical

### Ownership And Scope

Phase 8 ownership rules continue to apply.

Requirements:

- a product can trigger remote execution only for its own tasks
- remote-execution scope must be additive to the existing task-creation scope,
  not a replacement
- control-plane dispatch must not silently bypass local product, agent,
  directory, or capability relationships
- daemon or task credentials must not be mistaken for product identity when
  recording ownership locally

## HTTP Adapter Slice

Phase 10 adds the first production `integration_type = "http"` adapter, but it
should stay narrow.

### Provider Manifest Contract

Phase 10 should extend manifest support so HTTP providers can be described
without overloading CLI or ACP fields.

Representative shape:

```json
{
  "id": "remote-example",
  "integration_type": "http",
  "display_name": "Remote Example",
  "capabilities": {
    "remote_execution": true,
    "supports_resume": false
  },
  "security": {
    "runs_locally": false,
    "sends_code_to_vendor": true,
    "data_policy_url": "https://example.com/privacy",
    "review_level": "standard"
  },
  "http": {
    "endpoint": "https://agent.example.com/v1/tasks",
    "auth_scheme": "bearer",
    "upload_mode": "workspace_subset",
    "supports_streaming": true,
    "supports_cancel": true
  }
}
```

Requirements:

- HTTP-specific provider data must live under a dedicated `http` section
- manifest validation must reject HTTP providers that omit upload/security
  disclosures
- manifest validation must reject ambiguous transport or auth configuration
- provider metadata must make remote code upload behavior visible to users and
  reviewers

### Runtime Adapter Boundary

Phase 10 should fill the existing `IntegrationType::Http` adapter gap without
redesigning the runtime adapter contract.

Requirements:

- `AdapterSelector` chooses an HTTP adapter when manifest and policy permit it
- HTTP execution still accepts the existing `RuntimeExecutionRequest`
- HTTP execution still returns the existing `RuntimeExecutionOutcome`
- HTTP cancellation still returns the existing `RuntimeCancelOutcome`
- HTTP adapter errors map to stable adapter or policy codes

Representative stable error codes:

- `remote_execution_not_allowed`
- `http_invalid_configuration`
- `http_runtime_unavailable`
- `http_request_failed`
- `http_protocol_error`
- `http_cancel_not_supported`

### Request/Response Discipline

Requirements:

- HTTP provider requests must be constructed from normalized OpenDaemon task,
  agent, runtime, and workspace context
- HTTP adapter must not forward local bootstrap tokens, product tokens, daemon
  tokens, or task tokens
- provider auth material, if required, must come from provider-specific config
  paths rather than borrowed OpenDaemon auth credentials
- streaming HTTP provider output should normalize into the existing task event
  stream where supported
- non-streaming HTTP provider output should still produce a stable final result
- cancellation should use provider-native cancel APIs only when safely
  supported; otherwise fail with a stable error

## Store And Service Changes

### Product And Daemon State

Phase 10 may extend persistence narrowly:

- daemon registration metadata
- control-plane session metadata
- remote-dispatched task linkage
- remote-upload audit metadata
- task-token or callback-token metadata needed for daemon-to-control-plane task
  updates

Avoid unrelated schema churn. Reuse existing task, event, result, and product
ownership tables wherever practical.

### Scheduler And Task Service

Phase 10 should extend existing execution paths rather than adding a separate
remote-task executor:

- scheduler still owns queue admission and capacity
- task service still owns durable task state transitions
- runtime adapters still own protocol execution details
- event store still owns ordering and replay
- permission-response flow remains the same for local products observing local
  tasks

The control plane should attach above the current lifecycle, not tunnel around
it.

## API And Protocol Contract

Phase 10 adds control-plane protocol behavior, but local product APIs should
remain stable where possible.

Local API expectations:

- `POST /v1/tasks` remains the local async task entrypoint
- `GET /v1/tasks/:task_id/events` remains the local SSE task-event entrypoint
- `POST /v1/tasks/:task_id/events` remains the local permission response
  entrypoint
- local provider and runtime read APIs may expose additional control-plane or
  remote-execution metadata where useful

Control-plane protocol expectations:

- daemon registration and heartbeat use stable request/response shapes
- websocket dispatch messages use stable type-tagged envelopes
- task lifecycle callbacks are idempotent and include task identity plus auth
  proof
- cancellation messages map cleanly to existing task cancellation semantics

Phase 10 does not need to expose the control-plane protocol through the local
HTTP API unless implementation simplicity clearly requires a thin local
transport abstraction.

## Testing Requirements

Add focused coverage at the auth, protocol, adapter, scheduler, and API layers.

Unit tests:

- daemon registration state validates required identity fields
- control-plane auth rejects daemon-token and task-token misuse
- heartbeat staleness calculation marks daemons or runtimes offline correctly
- remote-execution policy evaluation fails closed when any gate is missing
- HTTP manifest validation rejects missing security or upload metadata
- HTTP adapter request building excludes OpenDaemon auth credentials
- remote-upload audit metadata serialization is stable

Integration tests:

- a daemon can register to a fake control plane and reconnect with stable local
  identity
- remote task dispatch becomes a durable local task and reaches terminal
  completion
- remote cancellation triggers the existing task cancellation path
- repeated control-plane terminal callbacks remain idempotent
- runtime offline state is reported after heartbeat expiry
- HTTP adapter executes through a fake remote provider when every policy gate is
  satisfied
- HTTP adapter is rejected before upload when directory grants disallow remote
  execution
- HTTP adapter is rejected before upload when product scope lacks
  `tasks:remote_execution`
- HTTP adapter persists upload audit metadata when remote execution occurs
- SSE clients can still observe remotely dispatched task events through the
  existing local event API

Regression tests:

- CLI providers still execute unchanged
- ACP providers still execute unchanged
- local product auth and ownership rules remain unchanged
- local `GET /v1/tasks/:task_id/events` replay ordering remains based on
  persisted `sequence`
- control-plane connectivity loss does not duplicate already-running local task
  execution

## Quality Gates

Phase 10 is complete only when these pass:

- `cargo fmt --all`
- `cargo clippy --tests --all-targets --all-features -- -D warnings`
- `cargo test -- --test-threads=1`

If control-plane integration tests need a fake websocket server or fake remote
HTTP provider, keep them repository-local and deterministic. Default CI must
not require an external control plane or third-party provider account.

## Acceptance Checklist

- [ ] Daemon registration exists with stable identity semantics.
- [ ] Control-plane liveness is maintained through heartbeat or websocket
  keepalive.
- [ ] Reconnect does not lose durable local task state.
- [ ] Runtime online/offline state can be published to the control plane.
- [ ] Remotely dispatched tasks use the existing local scheduler and task
  lifecycle.
- [ ] Claim/start/complete/fail/cancelled callbacks are idempotent.
- [ ] Daemon credentials are distinct from local product credentials.
- [ ] Task tokens are scoped to one remote task lifecycle.
- [ ] `integration_type = "http"` can execute only when every remote-execution
  policy gate is satisfied.
- [ ] Remote execution persists explicit metadata that code or workspace content
  was sent to a remote provider.
- [ ] Remote execution never forwards OpenDaemon auth credentials to providers.
- [ ] Existing CLI and ACP behavior remains stable.
- [ ] Existing local task APIs remain stable.
- [ ] Quality gates pass.

## Handoff To Phase 11

Phase 10 should leave OpenDaemon ready for user-facing operational UX without
mixing desktop concerns into the daemon core:

- remote products can dispatch work through a control plane
- daemon identity, liveness, and runtime status are visible
- remote execution is explicit, opt-in, and audited
- local directory and workspace boundaries remain enforced by the daemon

Phase 11 can build desktop surfaces for grants, profiles, runtime status, task
history, and permission-response workflows on top of these boundaries instead
of reopening control-plane or remote-execution policy design again.
