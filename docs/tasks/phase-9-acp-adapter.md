# Phase 9: ACP Adapter

## Goal

Make Agent Client Protocol (ACP) a first-class provider integration path by
adding `integration_type = "acp"` execution, ACP session lifecycle handling,
normalized ACP event mapping, and ACP permission bridging on top of the Phase 6
runtime adapter boundary, the Phase 7 event stream, and the Phase 8 product
authentication model.

Phase 9 builds on Phase 8. It does not redesign the local API auth model, task
storage, or scheduler ownership rules. It adds the first non-CLI runtime path
that can support richer interactive agent sessions:

- `integration_type = "acp"`
- ACP session startup and shutdown
- ACP task execution through the runtime adapter boundary
- ACP event normalization into the existing task event stream
- ACP permission request and permission response bridging
- ACP session resume where the provider and stored state allow it

This phase must not add control-plane task dispatch, daemon registration,
daemon/task tokens, remote HTTP provider execution, browser or desktop UI, or a
generic new event protocol.

## Scope

Phase 9 delivers local ACP-based execution for authenticated products:

- add manifest and runtime support for `integration_type = "acp"`
- allow ACP providers to participate in provider registration and runtime
  selection
- implement an ACP adapter under the existing runtime adapter boundary
- launch or connect to ACP servers using provider manifest metadata
- translate ACP session output into existing OpenDaemon task events
- persist normalized ACP task events through the existing event store
- bridge ACP permission requests into Phase 7 `provider.permission_requested`
  events
- bridge authenticated product permission responses back into the live ACP
  session when the protocol requires a decision
- support resumable ACP sessions when the provider exposes a stable session
  identifier and resume capability
- preserve Phase 8 ownership, scope, and token boundaries
- keep local directory grants, workspace mode rules, and remote-execution
  restrictions enforced by OpenDaemon
- quality gates passing

Phase 9 is the first production ACP execution path. It is not a remote control
plane, not an HTTP upload adapter, and not a redesign of the product-facing API.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 6 spec: `docs/tasks/phase-6-runtime-adapters.md`
- Phase 7 spec: `docs/tasks/phase-7-event-streaming.md`
- Phase 8 spec: `docs/tasks/phase-8-product-authentication.md`
- Current implementation:
  - `src/runtime/adapter.rs`
  - `src/runtime/model.rs`
  - `src/runtime/store.rs`
  - `src/runtime/detect.rs`
  - `src/task/event.rs`
  - `src/task/permission.rs`
  - `src/task/service.rs`
  - `src/store/tasks.rs`
  - `src/api/tasks.rs`
  - `src/api/providers.rs`
  - `src/api/runtimes.rs`
  - `src/tests/runtime_adapter.rs`
  - `src/tests/tasks.rs`
  - `src/tests/api.rs`

## Deliverables

- `integration_type = "acp"` is accepted in provider manifests.
- ACP providers can appear in provider read APIs with stable manifest metadata.
- ACP runtimes can be represented in runtime selection and task execution.
- The runtime adapter selector no longer returns `adapter_not_implemented` for
  ACP providers that declare valid ACP configuration.
- ACP task execution uses the same scheduler, workspace, timeout, cancellation,
  and terminal-state boundaries as CLI execution where applicable.
- ACP adapter startup returns stable adapter errors for invalid ACP provider
  configuration.
- ACP adapter can either spawn a local ACP server process or connect to a local
  ACP endpoint, depending on provider manifest metadata.
- ACP session events normalize into persisted OpenDaemon task events.
- ACP text output maps into existing event types such as `agent.text` or
  `process.stderr` rather than raw protocol frames.
- ACP lifecycle events do not break the existing SSE replay contract based on
  `task_events.sequence`.
- ACP permission requests persist as `provider.permission_requested` task
  events.
- Authenticated product permission responses on `POST /v1/tasks/:task_id/events`
  can resolve a live ACP permission request.
- ACP providers that require a permission decision do not hang indefinitely when
  a deny or approve decision is provided.
- ACP providers that do not support permission responses fail with a stable
  `permission_response_not_supported`-class error path.
- ACP execution can persist a provider session identifier into the existing
  task-result/session fields when one is available.
- ACP session resume is attempted only when the provider advertises resume
  support and a stored session identifier exists.
- Resume failure falls back to a stable task failure or fresh-session behavior
  defined by provider capability metadata; it must not silently attach to the
  wrong session.
- Existing product auth, product ownership, and scope checks remain unchanged.
- Product or bootstrap tokens are never forwarded into ACP session payloads,
  child process environments, or ACP transport metadata.
- Existing CLI execution behavior remains stable.
- Existing task create/get/list/cancel/events APIs remain stable apart from ACP
  now using them.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 9:

- control-plane websocket dispatch
- daemon registration or heartbeat
- daemon token
- task token
- remote product connectivity
- remote HTTP provider execution
- workspace upload packaging for cloud providers
- generic websocket event delivery to products
- browser UI or desktop UI for permission prompts
- keyring-backed provider credential storage
- ACP support for unauthenticated products
- distributed session coordination across multiple daemons
- full protocol parity for every theoretical ACP feature
- provider-specific custom product APIs outside the normalized task/event model

Phase 9 is about local ACP runtime integration inside the existing daemon
boundary. Phase 10 remains the control-plane phase.

## Dependencies

Keep Phase 0 through Phase 8 dependencies. Add only what is required for ACP
transport, framing, and cancellation.

Use the smallest stable ACP-compatible dependency set that matches the chosen
transport model at implementation time. The exact crate list can change if
current stable ACP ecosystem libraries differ, but the dependency purposes must
stay narrow:

- async process and IO support for spawned ACP servers
- stream framing / serialization for ACP messages
- optional local socket or stdio transport support
- cancellation-safe async task coordination

Do not add control-plane, browser, desktop, OAuth, keyring, or remote-upload
dependencies in Phase 9.

## ACP Integration Contract

### Provider Manifest Contract

Phase 9 extends provider manifests so ACP providers can be registered without
special-casing them in product APIs.

Required shape:

```json
{
  "id": "acp-example",
  "integration_type": "acp",
  "display_name": "ACP Example",
  "capabilities": {
    "supports_resume": true,
    "supports_permission_requests": true
  },
  "acp": {
    "transport": "stdio",
    "command": ["acp-example", "serve"],
    "endpoint": null,
    "working_directory_mode": "workspace",
    "env_allowlist": ["HOME"]
  }
}
```

Requirements:

- `integration_type` must equal `acp`.
- ACP manifest data must live under a dedicated `acp` section instead of
  overloading CLI execution fields.
- exactly one startup mode must be valid:
  - spawn local ACP server process through `command`, or
  - connect to an already-running local endpoint through `endpoint`
- `transport` must be explicit.
- ACP metadata must declare whether resume and interactive permissions are
  supported.
- manifest validation must reject ambiguous ACP configuration.

Phase 9 should support only local transports such as stdio or a local socket.
Any transport that implies remote network task dispatch belongs to a later
phase.

### Runtime Selection

Phase 9 keeps the existing provider/runtime separation:

- provider manifest declares ACP integration capability
- runtime detection or runtime configuration resolves a usable local ACP target
- task execution still selects a concrete runtime

Requirements:

- ACP runtimes must be visible in runtime read models with a stable kind.
- runtime detection must not accidentally spawn full task execution during read
  APIs.
- if a provider uses endpoint-based ACP, runtime validation may perform a
  bounded liveness check but must not mutate remote state.
- if no valid ACP runtime is available, task execution fails with
  `runtime_unavailable`.

### Runtime Adapter Boundary

Phase 9 fills the ACP extension point that Phase 6 intentionally left
unimplemented.

Expected behavior:

- `AdapterSelector` chooses an ACP adapter for ACP manifests.
- ACP execution still accepts the existing `RuntimeExecutionRequest`.
- ACP execution still produces the existing `RuntimeExecutionOutcome`.
- ACP cancellation still returns the existing `RuntimeCancelOutcome`.

Phase 9 should not redesign the runtime adapter trait just for ACP. If ACP needs
extra internal context, add it behind the adapter boundary rather than changing
the product-facing task contract.

## ACP Session Model

### Session Start

When a queued task reaches execution:

1. scheduler validates the task, grant, product scope, and workspace as it
   already does
2. runtime selection resolves an ACP runtime
3. ACP adapter starts or connects to the ACP session
4. adapter performs any required ACP initialization handshake
5. adapter submits the task prompt and relevant execution context
6. adapter streams normalized events into the existing event store

Requirements:

- the ACP session must run within the prepared workspace or an explicitly
  declared safe equivalent
- OpenDaemon remains the owner of timeout and cancellation
- startup failures map to stable adapter errors
- task state must not move to `running` until ACP session start succeeds

### Session Identity

ACP may expose a protocol session identifier. OpenDaemon should preserve it
without making products ACP-aware.

Requirements:

- when ACP returns a stable session ID, persist it through the existing
  `session_id` result field and any internal task-session linkage required for
  resume
- session IDs are provider artifacts, not authorization tokens
- session IDs must never bypass product ownership or task identity checks

### Session Resume

Phase 9 supports resume only where the ACP provider makes it safe and explicit.

Requirements:

- resume is optional per provider
- resume requires all of:
  - provider capability `supports_resume = true`
  - a stored `session_id`
  - a task state or restart flow that explicitly requests resume
- if resume is not supported, OpenDaemon starts a fresh session
- if resume is attempted and rejected by the provider, the daemon must record a
  stable failure or explicit fallback path
- OpenDaemon must not guess a session to resume

Phase 9 does not need a new public resume API. It only needs internal session
plumbing so later phases can build on it safely.

## Event Normalization Contract

Phase 9 must reuse the existing task event stream rather than introducing an
ACP-specific stream.

### Event Mapping

Representative ACP-to-OpenDaemon mapping:

- ACP assistant/user-visible text -> `agent.text`
- ACP progress/status updates -> existing normalized task or agent event types
- ACP stderr/process faults for spawned local servers -> `process.stderr`
- ACP permission request -> `provider.permission_requested`
- ACP permission decision acknowledgement -> `provider.permission_decided`

Rules:

- do not persist raw ACP frames as product-facing events
- preserve useful structured payloads under normalized event payload JSON
- continue using per-task increasing `sequence` values from the existing event
  store
- SSE replay and live-tail behavior from Phase 7 must continue to work without
  ACP-specific client logic

### Event Payload Discipline

ACP payload normalization should keep the product-facing contract small:

- stable high-level event type
- provider-safe structured details
- no raw bearer tokens
- no transport secrets
- no provider-specific opaque blobs unless they are safely nested under a
  documented payload field

If the ACP protocol exposes richer metadata than current event types can hold,
keep it under payload `details` or `provider_result` rather than expanding the
top-level SSE contract in this phase.

## Permission Bridge Contract

Phase 7 already defined the product-facing permission event and response API.
Phase 9 makes ACP the first runtime path that uses it in production.

### ACP Permission Requests

Requirements:

- ACP permission prompts must normalize into `provider.permission_requested`
- the request record must persist through the existing pending-permission store
- a live ACP session waiting for a decision must be linked to the persisted
  request ID
- if an ACP provider emits unsupported permission option shapes, OpenDaemon must
  normalize to approve/deny where safe or fail with a stable adapter error

### ACP Permission Responses

Requirements:

- authenticated product responses on `POST /v1/tasks/:task_id/events` continue
  to enforce Phase 8 ownership checks
- once a response is accepted and persisted, the adapter-facing resolver must
  signal the live ACP session
- repeated identical responses remain idempotent
- conflicting repeated responses remain `409`
- if the ACP session has already terminated, the permission response resolves in
  storage and returns a stable result without reviving a dead session

### Permission Timeout Behavior

ACP providers may impose a decision deadline.

Requirements:

- if ACP exposes a deadline, include it in `expires_at`
- if OpenDaemon receives no response before the ACP deadline and the provider
  reports timeout/failure, record a stable terminal task outcome
- Phase 9 does not need daemon-owned reminder or UI flows

## Store And Service Changes

### Task Store

Phase 9 may extend existing task persistence narrowly:

- store ACP session metadata needed for safe resume
- associate pending permission requests with a live ACP resolver handle
- persist normalized ACP task events through the existing event append path

Avoid unrelated schema churn. Reuse existing task result, event, and permission
tables where practical.

### Task Service

Phase 9 should extend the existing task service rather than introducing a
parallel ACP executor:

- scheduler still owns task lifecycle
- task service still owns durable state transitions
- runtime adapter still owns protocol execution details
- permission response service still owns durable permission resolution

The ACP adapter should plug into the current boundaries, not bypass them.

## API Contract

Phase 9 should avoid new product-facing routes if the current API is sufficient.

Public route expectations:

- `POST /v1/tasks` remains the task submission entrypoint
- `GET /v1/tasks/:task_id/events` remains the task event stream entrypoint
- `POST /v1/tasks/:task_id/events` remains the permission response entrypoint
- provider and runtime read APIs can expose ACP capability metadata

Allowed API additions in this phase:

- small read-model additions for ACP runtime/provider metadata
- stable error codes for ACP startup, handshake, and resume failures

Do not add an ACP-specific task route, session route, or permission route unless
implementation proves the existing API is insufficient.

## Stable Error Codes

Add ACP-specific stable domain codes as needed:

- `acp_invalid_configuration`
- `acp_runtime_unavailable`
- `acp_handshake_failed`
- `acp_session_start_failed`
- `acp_session_resume_failed`
- `acp_permission_not_supported`
- `acp_transport_closed`

Reuse existing codes where they already fit:

- `runtime_unavailable`
- `adapter_not_found`
- `adapter_execution_failed`
- `command_cancelled`
- `command_timeout`
- `permission_request_not_found`
- `permission_request_already_resolved`
- `permission_response_not_supported`

Do not create ACP-only HTTP semantics when the failure is really a general task
execution failure.

## Testing Requirements

Add focused coverage at the adapter, service, and API layers.

Unit tests:

- ACP manifest validation accepts valid stdio and endpoint configurations
- ACP manifest validation rejects ambiguous or incomplete ACP config
- ACP event normalization maps representative ACP frames to normalized
  `TaskEventType` values
- ACP permission request normalization produces the Phase 7 payload shape
- ACP resume metadata validation rejects missing or unsafe resume inputs

Integration tests:

- authenticated task execution through an ACP fake provider reaches terminal
  success
- ACP text and status output replay over SSE through the existing event API
- ACP permission request pauses execution until a product decision is posted
- approve response unblocks the ACP session and completes the task
- deny response unblocks the ACP session and records the correct terminal result
- cross-product permission responses are rejected for ACP tasks
- cancellation terminates an in-flight ACP session
- timeout terminates an unresponsive ACP session
- resume path uses stored `session_id` only when provider capability allows it

Regression tests:

- CLI providers still execute unchanged
- unauthenticated or insufficient-scope product requests remain rejected
- bootstrap token still cannot operate product task routes
- SSE replay ordering remains based on persisted `sequence`

## Quality Gates

Phase 9 is complete only when these pass:

- `cargo fmt --all`
- `cargo clippy --tests --all-targets --all-features -- -D warnings`
- `cargo test -- --test-threads=1`

If ACP fake-provider integration needs extra deterministic fixtures, keep them
local to repository tests and do not require an external ACP service for default
CI.

## Acceptance Checklist

- [ ] ACP providers can be registered with `integration_type = "acp"`.
- [ ] ACP runtimes can be selected for task execution.
- [ ] ACP tasks execute through the existing scheduler and task service.
- [ ] ACP session events persist as normalized OpenDaemon task events.
- [ ] SSE clients can observe ACP task events through the existing event API.
- [ ] ACP permission requests surface as `provider.permission_requested`.
- [ ] Authenticated product responses can resolve live ACP permission requests.
- [ ] Product ownership and scope boundaries still apply to ACP tasks.
- [ ] ACP session IDs persist only as task/session metadata, not auth
- [ ] ACP resume is attempted only when explicitly supported and safe.
- [ ] CLI behavior remains stable.
- [ ] Quality gates pass.

## Handoff To Phase 10

Phase 9 should leave OpenDaemon ready for control-plane work without mixing the
concerns:

- task execution now supports both CLI and ACP locally
- permission bridging is proven against a real interactive protocol
- session identity plumbing exists for richer remote lifecycle work later
- product-facing APIs remain provider-agnostic

Phase 10 can build daemon registration, heartbeat, remote dispatch, and daemon
or task tokens on top of these boundaries instead of reopening local runtime
integration again.
