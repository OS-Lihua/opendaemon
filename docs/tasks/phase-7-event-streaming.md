# Phase 7: Event Streaming

## Goal

Expose the persisted task event stream to products through Server-Sent Events
(SSE), with ordered replay, reconnect-from-cursor behavior, idle heartbeats,
and a small permission-response API for providers that require an explicit user
decision.

Phase 7 builds on Phase 6. It does not redesign task storage or runtime
execution. It turns the existing persisted `task_events` records into a stable
product-facing stream:

- `GET /v1/tasks/:task_id/events`
- SSE replay from a cursor
- live tail of newly persisted task events
- heartbeat comments for idle connections
- normalized `provider.permission_requested` events
- optional `POST /v1/tasks/:task_id/events` permission response API

This phase must not add websocket delivery, product authentication, keyring
secret storage, ACP protocol execution, remote control-plane dispatch, or a
desktop UI.

## Scope

Phase 7 delivers local product-facing event observation behavior:

- expose task events through SSE
- replay persisted events in ascending sequence order
- resume from a previously seen event sequence
- support both query-param and SSE-standard resume inputs
- keep a live SSE connection open for non-terminal tasks
- close the SSE connection after replay when the task is already terminal
- emit heartbeat comments while the connection is idle
- normalize provider permission request events into the task event stream
- accept explicit approve or deny responses for pending permission requests
- persist permission decisions as task events
- provide a small adapter-facing permission response boundary for future ACP and
  other interactive protocols
- preserve provider, runtime detection, directory, agent, task, and task result
  API behavior
- quality gates passing

Phase 7 uses the existing persisted task event log from Phase 5 and Phase 6.
It may extend task-event-related storage for permission request state, but it
must not replace the existing `task_events(task_id, sequence)` ordering
contract.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 0 spec: `docs/tasks/phase-0-project-foundation.md`
- Phase 1 spec: `docs/tasks/phase-1-provider-registry.md`
- Phase 2 spec: `docs/tasks/phase-2-runtime-detection.md`
- Phase 3 spec: `docs/tasks/phase-3-directory-grants.md`
- Phase 4 spec: `docs/tasks/phase-4-agent-profiles.md`
- Phase 5 spec: `docs/tasks/phase-5-task-scheduler.md`
- Phase 6 spec: `docs/tasks/phase-6-runtime-adapters.md`
- Phase 6 implementation:
  - `src/api/tasks.rs`
  - `src/runtime/`
  - `src/scheduler/`
  - `src/store/tasks.rs`
  - `src/task/event.rs`
  - `src/task/model.rs`
  - `src/task/result.rs`
  - `src/task/state.rs`

## Deliverables

- `GET /v1/tasks/:task_id/events` exists.
- SSE responses use `text/event-stream`.
- Event replay reads persisted task events in ascending `sequence` order.
- Resume from a cursor is supported.
- `Last-Event-ID` header is supported for reconnecting SSE clients.
- Query parameter `cursor` is supported and takes precedence over
  `Last-Event-ID` when both are present.
- The replay cursor contract is the per-task numeric event `sequence`.
- Products can connect after task creation but before task execution starts.
- Products can connect while a task is running and receive live events.
- Products can connect after a task is terminal and receive replayed history.
- No event is skipped across replay-to-live handoff within a single daemon
  process.
- SSE connections emit heartbeat comments when idle.
- Heartbeats stop once the connection closes.
- The daemon normalizes `provider.permission_requested` task events.
- Pending permission requests are tracked durably.
- `POST /v1/tasks/:task_id/events` accepts permission responses only.
- Permission responses support `approve` and `deny`.
- Permission responses are validated against a pending request for the same
  task.
- Permission responses are idempotent for repeated identical decisions.
- Permission decisions persist as task events.
- Runtime adapters that do not support interactive permission responses fail
  with a stable error instead of hanging.
- Existing task create/list/get/cancel behavior remains stable.
- Existing task result behavior remains stable.
- Existing runtime execution still persists stdout and stderr as task events.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 7:

- websocket task event delivery
- generic event injection by products
- product authentication or API scopes
- local API tokens
- keyring-backed secret storage
- provider credential UI
- ACP protocol session execution
- remote HTTP task execution
- control-plane task event push
- persistent SSE subscription records
- global cross-task event cursors
- browser UI or desktop UI
- audit log
- distributed or cross-daemon event replay

`POST /v1/tasks/:task_id/events` in Phase 7 is not a generic append-events API.
It is only for explicit permission responses to a pending
`provider.permission_requested` event.

## Dependencies

Keep Phase 0 through Phase 6 dependencies.

Phase 7 should prefer existing `axum`, `tokio`, `serde`, `serde_json`, and
`time` support:

```toml
axum = "0.8"
tokio = { version = "1", features = ["sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }
```

Do not add websocket, keyring, ACP, notify, or control-plane dependencies in
Phase 7.

If implementation needs a small stream adapter for ergonomic SSE wiring,
`tokio-stream = "0.1"` is acceptable, but do not add it unless the existing
async primitives are clearly insufficient.

## Event Streaming Contract

### Event Identity and Ordering

Phase 5 already introduced persisted task events:

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
```

Phase 7 must use `task_events.sequence` as the only replay and resume cursor.

Requirements:

- `sequence` is monotonic within a task.
- replay ordering is ascending by `sequence`.
- SSE `id:` must be the decimal string form of `sequence`.
- resume requests ask for events with `sequence > cursor`.
- the daemon must not invent a second offset, token, or timestamp cursor in
  Phase 7.
- `created_at` is informative metadata only; it is not the replay cursor.

This keeps the Phase 7 contract aligned with existing storage and avoids a new
cursor persistence layer.

### Event View

Expose this event shape over SSE `data:`:

```json
{
  "task_id": "task_1",
  "sequence": 7,
  "type": "process.stdout",
  "payload": {
    "text": "running tests",
    "stream": "stdout"
  },
  "created_at": "2026-06-01T00:00:00Z"
}
```

Field requirements:

- `task_id`: stable task ID
- `sequence`: per-task monotonic event sequence
- `type`: normalized task event type
- `payload`: event-specific JSON payload
- `created_at`: UTC RFC3339 timestamp from persisted storage

SSE framing requirements:

- `id:` = event `sequence`
- `event:` = event `type`
- `data:` = one JSON object matching the shape above

The daemon must not send raw provider protocol frames to products.

### Supported Event Types

Phase 7 reuses existing task lifecycle and process output events from earlier
phases, including:

- `task.queued`
- `task.waiting_directory_lock`
- `task.preparing`
- `task.started`
- `process.stdout`
- `process.stderr`
- `task.completed`
- `task.failed`
- `task.cancelled`
- `task.timed_out`

Phase 7 adds these permission-related event types:

- `provider.permission_requested`
- `provider.permission_decided`

Phase 7 may forward additional normalized event types already persisted by the
runtime or scheduler, but it must not require products to understand
provider-specific protocol names.

### Replay Cursor Inputs

Support both resume mechanisms:

1. Query parameter:

```http
GET /v1/tasks/:task_id/events?cursor=7
```

2. SSE reconnect header:

```http
Last-Event-ID: 7
```

Rules:

- `cursor` is optional.
- `Last-Event-ID` is optional.
- if both are present, `cursor` wins.
- an omitted cursor means replay from the first event for that task.
- a cursor must parse as a non-negative integer.
- a cursor larger than the current max sequence is valid and yields no replay
  events before live tail starts or before the connection closes for a terminal
  task.
- invalid cursor values return `400 invalid_event_cursor`.

### Replay and Live Tail Behavior

`GET /v1/tasks/:task_id/events` must combine persisted replay with live tail.

Behavior:

1. validate that `task_id` exists
2. resolve the effective cursor
3. establish a live subscription with the in-process event notifier
4. query persisted events with `sequence > cursor`
5. stream replay events in ascending order
6. hand off to live delivery without skipping committed events
7. if the task is terminal and replay is exhausted, close the stream
8. otherwise, keep the connection open and stream new events as they are
   committed

The replay-to-live handoff must avoid a gap where an event commits after replay
query but before live subscription becomes active.

Acceptable implementation pattern:

- subscribe to a process-local event notification channel first
- replay persisted events from the store
- track the highest replayed sequence
- forward only live notifications with `sequence > highest_replayed_sequence`

Duplicate delivery within one connection is not allowed.

### Terminal Task Behavior

If the task is already terminal when the request arrives:

- replay matching persisted events
- do not wait indefinitely for future events
- close the SSE connection cleanly after replay completes

If the task becomes terminal while the connection is open:

- stream the terminal event
- flush the response
- close the SSE connection after any already-queued events for that task are
  sent

This keeps post-completion observers simple and avoids idle long-lived
connections for finished tasks.

### Heartbeats

Idle SSE connections must emit heartbeat comments.

Requirements:

- default heartbeat interval: `15 seconds`
- heartbeat format: SSE comment line such as `: keep-alive`
- heartbeats must not advance the event cursor
- heartbeats must stop once the client disconnects
- heartbeats are required only while the connection is open for a non-terminal
  task

The heartbeat interval should be configurable through a Rust config type for
tests.

## Permission Request Contract

### Goal

Some provider protocols can pause task execution and ask the product to approve
or deny an operation. Phase 7 must normalize that interaction into the same
task event stream without exposing provider-specific protocol details.

Local CLI providers do not need to emit permission requests in Phase 7. The
contract must exist now so Phase 8 ACP and later adapters can use it without
changing the product-facing API.

### Permission Requested Event

Use this payload for `provider.permission_requested`:

```json
{
  "request_id": "perm_1",
  "provider_id": "acp-example",
  "permission_kind": "shell_command",
  "summary": "Provider requests permission to run a shell command.",
  "details": {
    "command": ["git", "push"],
    "reason": "publish generated branch"
  },
  "options": ["approve", "deny"],
  "expires_at": null
}
```

Field requirements:

- `request_id`: daemon-stable identifier for this permission request
- `provider_id`: provider manifest ID
- `permission_kind`: normalized string such as `shell_command`,
  `filesystem_write`, `network_access`, or `unknown`
- `summary`: short product-facing explanation
- `details`: optional structured JSON safe to expose to the trusted local API
- `options`: exactly `["approve", "deny"]` in Phase 7
- `expires_at`: optional UTC RFC3339 timestamp when the provider enforces a
  decision deadline

If a provider exposes richer permission metadata, keep it under `details`
instead of expanding the top-level contract in Phase 7.

### Permission Decision Event

Persist the product's decision as:

```json
{
  "request_id": "perm_1",
  "decision": "approve",
  "reason": "approved by local product UI"
}
```

under task event type:

```text
provider.permission_decided
```

Rules:

- only one terminal decision is allowed per `request_id`
- repeated identical responses are idempotent
- conflicting repeated responses return `409 permission_request_already_resolved`
- the decision event is persisted before the adapter-facing response future is
  resolved

## Permission Response API

Add this route:

```http
POST /v1/tasks/:task_id/events
```

This route accepts only permission responses. It must not allow arbitrary event
insertion.

### Request Shape

Use this JSON body:

```json
{
  "event_type": "provider.permission_response",
  "request_id": "perm_1",
  "decision": "approve",
  "reason": "approved by product_example"
}
```

Field requirements:

- `event_type` must equal `provider.permission_response`
- `request_id` must identify a pending permission request for this task
- `decision` must be `approve` or `deny`
- `reason` is optional short free text

### Behavior

- validate `task_id`
- validate request shape
- load pending permission request state
- reject requests for a different task
- reject unknown permission requests
- reject already resolved permission requests unless the repeated decision is
  identical
- persist `provider.permission_decided`
- notify the adapter-facing permission responder when one exists
- return success even if no live SSE client is connected

### Response Shape

Return:

```json
{
  "task_id": "task_1",
  "request_id": "perm_1",
  "status": "resolved",
  "decision": "approve"
}
```

### Stable Error Codes

Add stable error codes:

- `invalid_event_cursor`
- `invalid_event_request`
- `permission_request_not_found`
- `permission_request_not_pending`
- `permission_request_already_resolved`
- `permission_response_not_supported`
- `invalid_permission_decision`

Status guidance:

- `400` for invalid cursor or invalid request shape
- `404` for unknown task or unknown permission request
- `409` for already resolved, not pending, or unsupported interactive response
- `500` only for store or internal delivery failures

## Store and Service Changes

### Event Store

Reuse the existing task event store for replay and SSE output. Do not replace or
reshape `task_events`.

Add a focused permission-request store if needed for durable pending state:

```sql
CREATE TABLE task_permission_requests (
  request_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  provider_id TEXT NOT NULL,
  permission_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  request_payload_json TEXT NOT NULL,
  response_payload_json TEXT,
  requested_at TEXT NOT NULL,
  responded_at TEXT
);

CREATE INDEX task_permission_requests_task_idx
ON task_permission_requests(task_id, status);
```

Status values:

- `pending`
- `approved`
- `denied`

Requirements:

- creation of `provider.permission_requested` and insertion of the pending
  request row must be atomic
- resolution of a pending request and insertion of
  `provider.permission_decided` must be atomic
- repeated identical decisions must return the already-resolved state without
  duplicating task events
- no store operation may expose raw SQLite errors directly through the API

### Event Fanout Service

Add a small in-process fanout service under `src/task/` or `src/api/` that:

- publishes committed task events to local subscribers
- supports filtering by `task_id`
- supports replay-plus-live SSE connections
- supports adapter-facing permission decision waiters

This service is process-local only. It is not a durable broker and it is not a
cross-daemon message bus.

If the daemon restarts:

- persisted replay still works
- live connections are lost and clients must reconnect
- pending permission requests remain in durable storage

## API Contract

Add these routes:

```http
GET /v1/tasks/:task_id/events
POST /v1/tasks/:task_id/events
```

### `GET /v1/tasks/:task_id/events`

Behavior:

- validate task exists
- resolve replay cursor
- return SSE response
- replay stored events in ascending order
- tail new committed events
- emit heartbeat comments while idle
- close after replay for terminal tasks

Response headers should include at least:

- `content-type: text/event-stream`
- `cache-control: no-cache`

Do not buffer the whole event history in memory before sending it.

### `POST /v1/tasks/:task_id/events`

Behavior:

- accept only `provider.permission_response`
- validate pending permission request state
- persist `provider.permission_decided`
- notify any waiting runtime adapter
- return stable JSON

Do not use this route for stdout, stderr, lifecycle, or arbitrary product
events. It is not a generic event-ingest endpoint for local APIs, future
control-plane delivery, or provider-to-daemon event append.

## Adapter Boundary

Phase 7 needs one small adapter-facing interactive permission boundary for
future protocols.

Expected operations:

- `record_permission_request(task_id, request) -> persisted_event`
- `await_permission_decision(task_id, request_id) -> decision`
- `resolve_permission_request(task_id, request_id, decision) -> resolution`

Rules:

- local CLI adapter may implement none of these operations in production Phase 7
- adapters that cannot consume a product decision must fail with stable
  `permission_response_not_supported`
- fake adapters must cover the permission request and response loop in tests
- Phase 7 must not require ACP transport code

## Testing Requirements

Add focused tests for:

- replay all events from the beginning
- replay from a cursor
- replay using `Last-Event-ID`
- query `cursor` taking precedence over `Last-Event-ID`
- invalid cursor returns stable `400`
- terminal task replay closes without idle waiting
- running task replay transitions into live tail
- no skipped event across replay-to-live handoff
- heartbeat comment is emitted when idle
- heartbeat is not treated as an event cursor
- concurrent subscribers to the same task each receive ordered events
- `provider.permission_requested` persists as a task event
- pending permission request state survives store re-open
- permission response resolves a pending request
- repeated identical permission response is idempotent
- conflicting repeated permission response returns `409`
- unsupported interactive adapter path returns stable error
- existing task create/get/list/cancel tests still pass
- existing Phase 6 stdout and stderr event persistence tests still pass

## Acceptance Checklist

- [ ] Products can connect before task execution starts and observe later events.
- [ ] Products can connect after task completion and replay the full history.
- [ ] Events are emitted in ascending per-task sequence order.
- [ ] Reconnecting clients can resume from a cursor.
- [ ] Idle SSE connections emit heartbeat comments.
- [ ] Heartbeats do not affect replay cursors.
- [ ] Permission request events appear in the same task event stream.
- [ ] Products can approve or deny a pending permission request through the API.
- [ ] Permission decisions persist durably.
- [ ] Existing task and runtime behavior remains stable.

## Handoff to Phase 8

Phase 8 can start when SSE replay and live tail are stable, permission request
and response contracts are product-facing and durable, and runtime adapters have
a small interactive permission boundary ready for ACP integration.

The next phase should add:

- `integration_type = "acp"`
- ACP session startup and shutdown
- ACP event normalization into the same task event stream
- ACP permission requests wired to the Phase 7 permission response API
- session resume where the upstream protocol supports it
