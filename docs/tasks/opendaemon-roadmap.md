# OpenDaemon Roadmap

## Purpose

OpenDaemon is a local agent runtime daemon. It gives products a stable API for
asking a user-selected agent to work inside a user-authorized directory on the
user's computer.

The product should not need to know how Codex, Claude, OpenCode, Hermes,
OpenClaw, Cursor, Gemini, or another future agent is launched. The product
submits a task with an agent profile, a directory grant, and a prompt.
OpenDaemon validates the request, starts the matching provider runtime, streams
events, and records the final result.

The final target must support both local and remote use:

- Local products can call the daemon over a loopback API.
- Remote products can dispatch tasks through a control plane.
- The daemon can discover local agent CLIs and use them directly.
- Remote agent services can be used through provider adapters when explicitly
  configured.
- The security boundary for local files always stays in the daemon.

## Non-Goals

- OpenDaemon is not a new coding agent model.
- OpenDaemon should not require products to pass raw local file paths.
- OpenDaemon should not grant filesystem access from the provider registry.
- OpenDaemon should not assume every provider has the same protocol.
- OpenDaemon should not expose product or daemon credentials to child agents.
- OpenDaemon should not replace each provider's own approval UI. It should
  configure provider permission mode, expose provider permission events, and
  enforce OpenDaemon's own task, directory, and credential boundaries.

## Design Principles

- Local authority: directory access is decided by the local daemon, not by a
  remote product.
- Capability declaration is not authorization: a provider may declare that it
  can write files, but the user still has to grant a directory to an agent.
- Provider-agnostic product API: products talk to tasks, agents, directories,
  and events, not provider-specific CLI flags.
- Safe defaults: exclusive directory locks, bounded timeouts, explicit grants,
  no raw path API, and task-scoped credentials.
- Protocol-inclusive runtime: CLI, ACP, HTTP, and native adapters are all part
  of the final design. Declarative CLI support is still useful as the simplest
  adapter class.
- Worktree by default, direct directory on request: the default execution mode
  creates an isolated git worktree when possible. Users can explicitly allow an
  agent to work in the original directory.
- Observable execution: every task must have state, logs, events, timestamps,
  and a final result.

## Product Decisions

- Users create Agent Profiles in OpenDaemon.
- Products can use OpenDaemon APIs to create Agent Profiles when they have the
  right scope, but task execution must reference an existing `agent_id`.
- Products cannot submit arbitrary provider configuration inside a task to
  bypass the Agent Profile.
- Directory grants are scoped by product, agent, directory, and capability.
- Default execution uses a worktree. Direct operation on the original directory
  is supported only when the grant and task both allow it.
- macOS, Linux, and Windows are all target platforms. Implementation can land
  macOS/Linux first, but the data model and process abstractions must not make
  Windows a redesign.

## Reference Model

The Multica daemon model is the closest internal reference:

- load daemon configuration
- detect installed provider CLIs
- register runtimes
- heartbeat or keep a websocket online
- claim tasks per runtime
- start tasks only after capacity is available
- execute the provider CLI in a prepared environment
- stream messages and progress
- complete, fail, cancel, or time out tasks
- lock local directories to avoid concurrent writes

OpenDaemon should keep this lifecycle, but expose it as a general-purpose local
runtime that any product can call.

## Core Concepts

### Provider

A third-party agent product or protocol implementation. Examples:

- `codex`
- `claude`
- `opencode`
- `openclaw`
- `hermes`
- `gemini`
- `cursor`
- `copilot`
- `kimi`
- `kiro`
- `antigravity`

Providers are registered through manifests under `registry/providers/<id>/`.

### Runtime

A provider installation discovered on the user's machine, or a configured
remote provider endpoint that can execute tasks through an adapter.

Example:

```json
{
  "id": "rt_codex_local",
  "provider_id": "codex",
  "kind": "local_cli",
  "executable": "/opt/homebrew/bin/codex",
  "version": "1.2.3",
  "status": "online"
}
```

Remote runtime example:

```json
{
  "id": "rt_example_remote",
  "provider_id": "example-agent",
  "kind": "remote_http",
  "endpoint": "https://agent.example.com",
  "status": "online"
}
```

### Agent Profile

A product-facing or user-facing role that uses a provider runtime.

Example:

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
    "custom_env": {},
    "mcp_config": null,
    "permission_mode": "provider_default"
  }
}
```

Agent Profiles are the stable product-facing selection unit. Products may create
or update profiles through API scopes such as `agents:write`, but a task can
only reference an existing Agent Profile.

### Directory Grant

A local directory that the user has authorized.

Products must use `directory_id`. They must not submit arbitrary local paths.

Example:

```json
{
  "id": "repo_web_app",
  "path": "/Users/alice/github/web-app",
  "product_id": "product_example",
  "agent_id": "frontend-fixer",
  "capabilities": ["read", "write", "shell", "git"],
  "workspace_modes": ["worktree", "direct"],
  "default_workspace_mode": "worktree",
  "lock_policy": "exclusive",
  "direct_mode_requires_explicit_task_opt_in": true
}
```

Directory grants are scoped by product, agent, directory, and capability. This
allows a user to give one product's reviewer agent read-only access while giving
another product's fixer agent write access to the same repository.

### Workspace Mode

Workspace mode controls where the provider process runs.

- `worktree`: create or reuse an isolated git worktree and run the agent there.
  This is the default for repositories because it gives better rollback,
  review, diff extraction, and concurrent task isolation.
- `direct`: run the agent in the original authorized directory. This is useful
  when a user explicitly wants the agent to operate on the real working tree,
  when the directory is not a git repository, or when a provider cannot work in
  a generated worktree.

The daemon should choose `worktree` by default when the directory is a git
repository and the grant permits it. A task can request `direct`, but direct mode
is allowed only when the grant, Agent Profile, and product scope all permit it.

### Task

A unit of work submitted by a product or user.

Example:

```json
{
  "agent_id": "frontend-fixer",
  "directory_id": "repo_web_app",
  "workspace_mode": "worktree",
  "prompt": "Fix the mobile login button alignment.",
  "metadata": {
    "product": "example-product",
    "issue_id": "BUG-123"
  }
}
```

### Event

A normalized stream item emitted during task execution. Events should hide
provider-specific protocol differences from products.

Example event types:

- `task.started`
- `agent.text`
- `agent.thinking`
- `agent.tool_use`
- `agent.tool_result`
- `process.stderr`
- `task.completed`
- `task.failed`
- `task.cancelled`

### Task Result

The task result is the stable summary products can rely on after execution.
Different products need different levels of detail, so the result should have a
small required core and optional provider/workspace artifacts.

Required fields:

```json
{
  "task_id": "task_123",
  "status": "completed",
  "final_message": "Fixed the mobile login button alignment.",
  "started_at": "2026-05-29T00:00:00Z",
  "completed_at": "2026-05-29T00:03:12Z"
}
```

Optional fields:

- `summary`: short structured summary written by the daemon or agent.
- `changed_files`: files changed inside the execution workspace.
- `diff`: unified diff when the workspace is git-backed and diff extraction is
  enabled.
- `workspace_mode`: `worktree` or `direct`.
- `worktree_path`: local worktree path when using worktree mode.
- `source_directory_id`: original directory grant.
- `branch_name`: generated branch name when the daemon creates one.
- `commit_hash`: optional commit produced by the daemon or provider.
- `session_id`: provider resume pointer when available.
- `provider_result`: provider-specific normalized result.
- `usage`: token or credit usage when the provider reports it.
- `artifacts`: links or local references to logs, patches, screenshots, or
  generated files.
- `error`: terminal error message for failed, cancelled, or timed-out tasks.

Products should treat events as the execution transcript and task result as the
final contract. For code-modification tasks, `changed_files` and `diff` are more
useful than only `final_message`.

## Architecture

```text
Product UI / Product Backend
        |
        | HTTP, SSE, WebSocket
        v
OpenDaemon API
        |
        | task validation, auth, directory grants
        v
Task Scheduler
        |
        | provider selection, locks, concurrency
        v
Runtime Adapter
        |
        | local CLI, ACP, remote HTTP, native protocol
        v
Third-Party Agent
        |
        | read/write within authorized workspace
        v
Worktree or User Directory
```

## Local and Remote Execution

OpenDaemon must support both execution locations.

### Local Execution

Local execution uses a provider command installed on the user's machine.

The daemon is responsible for:

- discovering the command
- detecting version and capabilities
- preparing the workspace
- launching the process
- streaming provider output
- cancelling and reaping the process
- extracting changed files and diff when possible

### Remote Execution

Remote execution uses a provider service through HTTP, WebSocket, ACP over a
transport, or a provider-specific native adapter.

Remote execution is allowed only when:

- the Agent Profile chooses a remote-capable provider runtime
- the provider manifest declares that code may leave the machine
- the Directory Grant permits remote execution for the requested capabilities
- the product scope allows remote execution

For remote providers, OpenDaemon still owns the local file boundary. It should
package only the approved workspace content or diff context required by the
adapter and should record that code was sent to a remote service.

## Multica Compatibility Notes

The Multica daemon exposes a task lifecycle rather than a generic approval UI:

- server registers runtimes and assigns tasks to them
- daemon claims a task, starts it, runs the provider, then completes or fails it
- Agent data includes `custom_args`, `custom_env`, `mcp_config`, `model`, and
  `thinking_level`
- provider backends filter protocol-critical flags from custom args
- several providers are launched in autonomous daemon mode with provider-native
  bypass or trust flags
- local directory resources are locked before execution
- worktrees are used for regular repo checkout flows, while local directory
  resources can run against a user path

OpenDaemon should follow the same separation:

- OpenDaemon enforces product, profile, directory, workspace, and credential
  boundaries.
- Provider-specific approval or trust behavior is configured on the Agent
  Profile and implemented in the provider adapter.
- Provider permission requests should be surfaced as events when the protocol
  supports them, but OpenDaemon does not need to own a full approval UI in the
  daemon core.

## Rust Project Shape

Start with the current template as a single crate, but keep module boundaries
clear. Split into workspace crates once the core boundaries are proven by a
working daemon path.

Initial module layout:

```text
src/
  main.rs
  lib.rs
  api/
    mod.rs
    agents.rs
    directories.rs
    providers.rs
    tasks.rs
    events.rs
  agent/
    mod.rs
    profile.rs
  config/
    mod.rs
  registry/
    mod.rs
    manifest.rs
    validate.rs
  runtime/
    mod.rs
    detect.rs
    process.rs
    adapter.rs
    cli.rs
    acp.rs
  scheduler/
    mod.rs
    locks.rs
    worker.rs
  security/
    mod.rs
    directory.rs
    path_guard.rs
    secrets.rs
  store/
    mod.rs
    sqlite.rs
  task/
    mod.rs
    state.rs
    event.rs
```

Future workspace layout:

```text
crates/
  opendaemon-core/
  opendaemon-registry/
  opendaemon-runtime/
  opendaemon-api/
  opendaemon-store/
  opendaemon-security/
  opendaemon-cli/
apps/
  daemon/
registry/
  providers/
schemas/
  provider-manifest.schema.json
docs/
  tasks/
e2e/
```

## Recommended Rust Stack

- `tokio`: async runtime
- `axum`: local HTTP API
- `tower-http`: tracing, CORS, compression, request IDs
- `serde`, `serde_json`: API and manifest data
- `schemars`, `jsonschema`: manifest schema generation and validation
- `clap`: command-line interface
- `tracing`, `tracing-subscriber`: structured logs
- `sqlx` with SQLite: local persistent state
- `uuid`: stable IDs
- `time`: timestamps
- `directories`: config, data, and cache directories
- `camino`: UTF-8 path handling where useful
- `keyring`, `secrecy`: local token and secret storage
- `handlebars` or `minijinja`: provider command templates
- `tokio-tungstenite`: remote control plane websocket
- `notify`: registry and config reload
- `portable-pty`: optional support for providers that require a PTY

## Local API

### Health

```http
GET /health
```

Returns daemon status.

### Providers

```http
GET /v1/providers
GET /v1/providers/:provider_id
```

Returns provider manifests and detected runtime status.

### Runtimes

```http
GET /v1/runtimes
POST /v1/runtimes/detect
```

Returns installed provider CLIs.

### Agent Profiles

```http
GET /v1/agents
POST /v1/agents
GET /v1/agents/:agent_id
PATCH /v1/agents/:agent_id
DELETE /v1/agents/:agent_id
```

Stores product-facing agent profiles.

### Directory Grants

```http
GET /v1/directories
POST /v1/directories/grant
GET /v1/directories/:directory_id
PATCH /v1/directories/:directory_id
DELETE /v1/directories/:directory_id
```

`POST /v1/directories/grant` accepts a local path only from a trusted local UI or
CLI. Remote products should use existing `directory_id` values.

### Tasks

```http
POST /v1/tasks
GET /v1/tasks
GET /v1/tasks/:task_id
POST /v1/tasks/:task_id/cancel
GET /v1/tasks/:task_id/events
POST /v1/invoke
```

`POST /v1/tasks` creates an async task.

`GET /v1/tasks/:task_id/events` streams Server-Sent Events.

`POST /v1/invoke` is a bounded synchronous helper for short tasks.

## Control Plane API

The control plane is optional for local-only deployments but part of the final
target. It lets remote products dispatch tasks to a user's daemon without
directly exposing the local HTTP API.

Control-plane-facing daemon endpoints or websocket messages should cover:

```http
POST /v1/daemons/register
POST /v1/daemons/:daemon_id/heartbeat
POST /v1/runtimes/register
POST /v1/runtimes/:runtime_id/tasks/claim
POST /v1/tasks/:task_id/start
POST /v1/tasks/:task_id/events
POST /v1/tasks/:task_id/complete
POST /v1/tasks/:task_id/fail
POST /v1/tasks/:task_id/cancelled
```

The local daemon should keep the same internal lifecycle for local and remote
tasks. Remote dispatch changes how a task arrives, not how directory grants,
workspace mode, provider adapters, events, and results are enforced.

## Task State Machine

```text
queued
  -> waiting_directory_lock
  -> preparing
  -> running
  -> completed

queued
  -> cancelled

waiting_directory_lock
  -> cancelled

preparing
  -> failed

running
  -> completed
  -> failed
  -> cancelled
  -> timed_out
```

Rules:

- A task must not move to `running` before a process has started.
- A task must not be claimed if global capacity is unavailable.
- A task must acquire its directory lock before spawning the provider.
- Terminal callbacks must be idempotent.
- Cancellation should send a graceful signal first and force-kill after a
  bounded grace period.

## Provider Registry

The registry is the public index of supported third-party agents.

Directory layout:

```text
registry/providers/<provider-id>/
  manifest.json
  README.md
  examples/
    basic.task.json
  assets/
    logo.svg
  tests/
    detect.json
    execution.json
```

Only `manifest.json`, `README.md`, and `examples/basic.task.json` should be
required for a provider registry PR.

### Manifest Example

```json
{
  "schema_version": "1.0",
  "id": "example-agent",
  "display_name": "Example Agent",
  "status": "community",
  "vendor": {
    "name": "Example Inc.",
    "homepage": "https://example.com",
    "support_url": "https://example.com/support"
  },
  "integration_type": "cli",
  "description": "A local CLI coding agent.",
  "install": {
    "macos": ["brew install example-agent"],
    "linux": ["curl -fsSL https://example.com/install.sh | sh"],
    "windows": ["winget install Example.Agent"]
  },
  "detect": {
    "commands": ["example-agent"],
    "version_args": ["--version"],
    "version_regex": "example-agent\\s+(?<version>\\d+\\.\\d+\\.\\d+)"
  },
  "execution": {
    "command": "example-agent",
    "args": ["run", "--model", "{{model}}", "{{prompt}}"],
    "input_mode": "arg",
    "working_directory": "required",
    "supports_streaming": true,
    "cancel_signal": "SIGTERM"
  },
  "models": {
    "default": "example-agent-pro",
    "supported": ["example-agent-pro", "example-agent-lite"]
  },
  "capabilities": {
    "filesystem_read": true,
    "filesystem_write": true,
    "shell": true,
    "git": true,
    "browser": false,
    "mcp": false,
    "remote_execution": false,
    "worktree": true,
    "direct_directory": true
  },
  "permissions": {
    "requires_directory_grant": true,
    "recommended_directory_lock": "exclusive",
    "provider_permission_modes": ["provider_default", "trusted", "restricted"],
    "supports_permission_events": false
  },
  "environment": {
    "required": ["EXAMPLE_AGENT_API_KEY"],
    "optional": ["EXAMPLE_AGENT_BASE_URL"]
  },
  "security": {
    "runs_locally": true,
    "sends_code_to_vendor": true,
    "data_policy_url": "https://example.com/privacy",
    "review_level": "standard"
  }
}
```

### Integration Types

- `cli`: a command launched by OpenDaemon.
- `acp`: an Agent Client Protocol server.
- `http`: a remote API adapter. This should be opt-in because it may upload
  code.
- `native`: a Rust adapter for providers that need custom logic.

Providers may support more than one integration type. For example, a provider
could have a simple declarative CLI mode and a richer ACP mode. Agent Profiles
select the preferred integration mode when multiple modes are available.

### Provider PR Flow

Third-party agent products can join by submitting a registry PR.

Required steps:

1. Fork the repository.
2. Add `registry/providers/<provider-id>/manifest.json`.
3. Add `registry/providers/<provider-id>/README.md`.
4. Add `registry/providers/<provider-id>/examples/basic.task.json`.
5. Run `just registry-check`.
6. Open a PR named `registry: add <provider-id>`.

CI should validate:

- provider ID is unique
- manifest matches the JSON schema
- required files exist
- template variables are known
- command names are not absolute paths unless explicitly allowed
- install, detect, execution, capabilities, permissions, and security fields
  are present
- examples parse as valid tasks
- capabilities do not imply default authorization
- README documents installation, environment variables, data policy, and known
  limitations

Maintainer review should check:

- the provider is a real product or project
- install instructions are official and safe
- data handling is clearly described
- capabilities are honest
- the provider does not try to grant itself local directory access
- the provider does not hide remote code upload behavior

Registry status values:

- `community`: community submitted and schema-valid
- `verified`: vendor identity verified
- `first_party`: maintained by OpenDaemon maintainers
- `deprecated`: no longer recommended

## Security Model

### Directory Access

Products must use `directory_id`, not raw paths.

The daemon must:

- canonicalize configured paths
- reject non-existent grants
- reject path traversal
- reject symlink escapes by default
- keep directory grants local
- enforce product, agent, directory, and capability grants
- use exclusive locks by default for write-capable tasks
- default to worktree mode for git repositories
- require explicit grant and task opt-in for direct mode

### Process Execution

The runtime must:

- pass only task-scoped credentials to child processes
- set a controlled environment
- set the working directory to the authorized directory
- capture stdout and stderr
- enforce timeout and idle timeout
- support cancellation
- record exit code and signal
- avoid shell invocation for normal command execution

### Workspace Isolation

Worktree mode should be the default execution mode for git repositories.

The daemon should:

- create a dedicated worktree per task or per resumable session
- create a deterministic branch name when needed
- keep the worktree under an OpenDaemon-managed data directory unless configured
  otherwise
- extract changed files and diff from the worktree
- keep or clean the worktree according to retention policy
- fall back to direct mode only when explicitly allowed

Direct mode should:

- run in the original granted directory
- acquire an exclusive directory lock for write-capable tasks
- clearly mark task results as direct-mode results
- avoid cleanup that could delete user files

### Secrets

The daemon should store product tokens and provider secrets with the OS keychain
when possible.

Child agents should receive only the minimum environment required for the task.

### Product Authentication

Local API authentication can evolve in phases:

1. Development: bind to `127.0.0.1`, require a local token.
2. Local production: per-product API keys with scopes.
3. Cloud control plane: daemon token, product token, and task token.

Scopes:

- `providers:read`
- `runtimes:read`
- `agents:read`
- `agents:write`
- `directories:read`
- `directories:grant`
- `directories:direct`
- `tasks:create`
- `tasks:read`
- `tasks:cancel`
- `tasks:remote_execution`

### Provider Permissions

OpenDaemon should not become the universal approval UI for all providers. The
daemon should model provider permission behavior as configuration and events:

- Agent Profiles store provider permission configuration, such as trusted mode,
  restricted mode, sandbox mode, MCP config, and custom args.
- Provider adapters translate that profile configuration into provider-native
  flags or config files.
- Provider adapters filter flags that would break OpenDaemon's protocol,
  credential, workspace, or event guarantees.
- If a protocol such as ACP emits permission requests, OpenDaemon should emit
  `provider.permission_requested` events and optionally expose a response API.
- Products can build an approval UI on top of those events, but the daemon core
  remains a policy enforcement and execution pipeline.

## Phase Plan

### Phase 0: Project Foundation

Detailed plan: `docs/tasks/phase-0-project-foundation.md`.

Goal: make the template represent OpenDaemon.

Tasks:

- rename package metadata from `template` to `opendaemon`
- keep the existing formatting, linting, and pre-commit constraints
- standardize `E2E` or `e2e` casing
- add initial module skeleton
- add a minimal CLI with `opendaemon --version`
- add `GET /health`

Acceptance criteria:

- `cargo fmt --all` passes
- `cargo clippy --tests --all-targets --all-features -- -D warnings` passes
- `cargo test --all-features --all-targets` passes
- `opendaemon --version` works
- `GET /health` returns `ok`

### Phase 1: Provider Registry

Goal: load provider manifests from the local registry.

Tasks:

- define `ProviderManifest`
- define the JSON schema
- add validation
- add provider fixtures for `codex`, `claude`, and one generic test provider
- add `GET /v1/providers`
- add `GET /v1/providers/:provider_id`
- add `just registry-check`

Acceptance criteria:

- invalid manifests fail validation
- duplicate provider IDs fail validation
- provider API returns normalized manifests
- registry PR requirements are documented

### Phase 2: Runtime Detection

Goal: discover installed provider CLIs.

Tasks:

- implement command detection from manifests
- support environment variable path overrides
- run version commands with timeout
- store runtime status
- add `GET /v1/runtimes`
- add `POST /v1/runtimes/detect`

Acceptance criteria:

- missing commands are reported as unavailable
- detected commands include executable path and version
- detection never blocks daemon startup indefinitely

### Phase 3: Directory Grants

Goal: safely authorize local directories for specific products, agents, and
capabilities.

Tasks:

- implement directory grant model
- canonicalize paths
- persist grants in SQLite
- enforce product, agent, directory, and capability scopes
- support `worktree` and `direct` workspace modes
- default grants to worktree mode when possible
- add directory API routes
- add path guard tests for traversal and symlink behavior

Acceptance criteria:

- products can list grants but cannot create arbitrary raw-path tasks
- invalid paths are rejected
- unauthorized product-agent-directory-capability combinations are rejected
- direct mode is rejected unless the grant and task both allow it

### Phase 4: Agent Profiles

Goal: let users or products create named agents backed by providers.

Tasks:

- implement agent profile model
- bind agent profiles to provider IDs
- support model, instructions, custom args, custom env, and grant references
- support provider permission config, MCP config, and workspace policy
- allow products with `agents:write` to create profiles
- reject task-time provider overrides that are not part of the profile
- add CRUD API routes

Acceptance criteria:

- tasks can reference agents by ID
- missing providers are rejected
- agent profiles cannot bypass directory grants
- products can create profiles only within their scopes

### Phase 5: Task Scheduler

Goal: run asynchronous tasks across local or remote runtimes.

Tasks:

- implement task state model
- persist tasks and events
- add global concurrency limit
- add per-directory lock
- add worktree preparation and retention policy
- add direct-mode locking
- implement worker loop
- add `POST /v1/tasks`
- add `GET /v1/tasks/:task_id`
- add `POST /v1/tasks/:task_id/cancel`
- produce normalized task results with final message, changed files, diff,
  workspace mode, session ID, and provider-specific result fields

Acceptance criteria:

- tasks move through the expected state machine
- cancellation works before and during execution
- two write tasks for the same directory do not run concurrently
- worktree tasks do not mutate the original directory
- direct tasks mutate the original directory only when explicitly authorized

### Phase 6: Runtime Adapters

Goal: execute local CLI, ACP, HTTP, and native providers through one runtime
interface.

Tasks:

- render command templates from provider manifests
- launch processes without a shell
- inject controlled environment variables
- pass prompt by argument, stdin, or temp file depending on manifest
- stream stdout and stderr as events
- enforce timeout and cancellation
- normalize final result
- add ACP session adapter
- add remote HTTP adapter with explicit remote-execution grants
- add native adapter extension points

Acceptance criteria:

- a generic test provider can echo a prompt
- a failed process becomes a failed task
- process output is available through task events
- ACP events normalize into OpenDaemon events
- remote execution is rejected unless all remote-execution policies allow it

### Phase 7: Event Streaming

Goal: let products observe task execution.

Tasks:

- implement event store
- implement `GET /v1/tasks/:task_id/events` with SSE
- add event replay from a given cursor
- add heartbeat comments for idle SSE connections
- add provider permission request events
- add optional response API for protocols that can accept permission decisions

Acceptance criteria:

- products can connect before or after task start
- events are ordered
- reconnecting clients can resume from a cursor
- products can build their own approval UI from provider permission events

### Phase 8: ACP Adapter

Goal: make Agent Client Protocol a first-class integration path.

Tasks:

- add `integration_type = "acp"`
- launch or connect to ACP servers
- map ACP session events to OpenDaemon events
- map ACP permission requests to OpenDaemon permission events and optional
  response endpoints
- support session resume where possible

Acceptance criteria:

- ACP providers can be registered through manifest metadata
- ACP task events normalize into the same product-facing event stream
- permission behavior follows Agent Profile provider config and explicit
  product responses when the protocol requires a response

### Phase 9: Product Integration Hardening

Goal: make the local API safe for multiple products.

Tasks:

- add local API tokens
- add product registration
- add scopes
- add audit log
- add rate limits
- bind server to `127.0.0.1` by default
- document reverse proxy and remote access risks
- support product-scoped Agent Profile creation

Acceptance criteria:

- unauthorized requests are rejected
- products can only use scopes they were granted
- audit log records task creation, cancellation, and directory access

### Phase 10: Control Plane

Goal: support remote products and multiple machines.

Tasks:

- add daemon registration
- add heartbeat
- add websocket task dispatch
- add task claim/start/complete/fail protocol
- add daemon token
- add task token
- add runtime status

Acceptance criteria:

- daemon can reconnect without losing local state
- stale runtimes become offline
- remote tasks still obey local directory grants

### Phase 11: Desktop UX

Goal: make grants, profiles, runtime status, and task history understandable to
end users after the daemon core is complete.

Tasks:

- add optional Tauri desktop app
- directory picker
- agent profile editor
- provider install and detection view
- task history
- optional provider permission response surfaces
- logs viewer

Acceptance criteria:

- users can grant a directory without editing JSON
- users can see what product requested a task
- products or UI clients can respond to provider permission events when needed

## Testing Strategy

Unit tests:

- manifest validation
- command template rendering
- path guard behavior
- task state transitions
- lock behavior

Integration tests:

- local API routes
- SQLite persistence
- provider detection with fake commands
- task execution with fake providers
- SSE replay

E2E tests:

- start daemon
- register a test provider
- grant a temp directory
- create an agent profile
- create a task
- stream events
- verify final status
- verify worktree mode does not mutate the original directory
- verify direct mode mutates only when explicitly authorized
- verify a remote runtime is rejected without remote-execution grants

Security tests:

- raw path task rejection
- traversal rejection
- symlink escape rejection
- unauthorized product-agent-directory-capability rejection
- cancellation and timeout behavior
- task-time provider override rejection
- direct-mode opt-in enforcement
- remote-execution opt-in enforcement

## Open Questions

- Should first-party provider manifests live in the repo or be bundled at build
  time?
- Should registry sync be pull-based from GitHub, a signed release artifact, or
  both?
- Which task-result fields should be mandatory for every product integration
  beyond status, final message, and timestamps?
- Should the control plane protocol mirror Multica's task lifecycle exactly, or
  should OpenDaemon define a provider-neutral version with compatibility
  adapters?
- Which providers require PTY support rather than plain stdio?
- Which providers can safely support remote execution without uploading an
  entire workspace?
- How should Windows service installation, process cancellation, and path
  canonicalization differ from macOS/Linux?

## Initial Validation Milestone

The initial validation milestone should exercise the final concepts in the
smallest reliable loop:

1. Start `opendaemon`.
2. Load provider manifests.
3. Detect one fake provider command.
4. Grant a temporary directory for one product, one agent, and specific
   capabilities.
5. Create an agent profile backed by that provider.
6. Submit a task that requests worktree mode.
7. Execute the fake provider in an isolated workspace.
8. Stream task events over SSE.
9. Persist the final task result, including workspace mode and changed files.
10. Reject the same task in direct mode until the grant explicitly allows it.

This milestone proves the architecture without depending on real third-party
agent behavior. It does not reduce the final target; it validates the data
model, permission model, workspace model, task lifecycle, and event/result
contract before adding every provider.
