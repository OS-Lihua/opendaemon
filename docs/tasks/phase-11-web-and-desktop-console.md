# Phase 11: Web and Desktop Console UX

## Goal

Make OpenDaemon understandable and operable for end users by adding a shared
Rust Console experience that can run as a local web page and as an optional
desktop shell, with both surfaces using the same information architecture, state
language, API client, and visual system.

Phase 11 builds on Phase 10. It does not redesign local product authentication,
directory grants, Agent Profiles, task execution, ACP permissions, control-plane
dispatch, or remote-execution policy. It gives those completed daemon boundaries
a user-facing operating surface:

- daemon and control-plane status
- product and token setup
- provider registry and runtime detection status
- Agent Profile creation and editing
- directory grant creation and editing
- task creation, history, event transcript, and result inspection
- provider permission request review and response
- remote-execution visibility where Phase 10 recorded remote upload metadata

The web Console and desktop Console must feel like the same product. The desktop
shell may add native affordances such as a directory picker, window chrome
integration, and local daemon connection checks, but it must not fork the page
structure, copy, permission rules, or API behavior.

## Scope

Phase 11 delivers the first production Console surface:

- add a shared Rust/WebAssembly web application for OpenDaemon Console
- optionally wrap the same Rust Console application in a Tauri desktop shell
- serve the Console from the daemon or run it as a local development web app
- add small UI-enabling daemon APIs only where existing Phase 0 through Phase 10
  APIs cannot support a usable screen
- keep Console authentication explicit and token-based
- provide a startup/login screen for bootstrap tokens and product API tokens
- expose the authenticated principal and scopes to the Console without exposing
  raw token material
- show daemon status, version, scheduler summary, control-plane status, and
  runtime status
- let a bootstrap operator register products and mint product tokens
- show the one-time plaintext token only on creation
- let a product-scoped operator manage its own Agent Profiles
- let a product-scoped operator grant and edit directories for its own product
  and agents
- let users inspect provider manifests, installation guidance, runtime status,
  and detection errors
- let users run runtime detection from the Console
- let users create tasks against existing Agent Profiles and Directory Grants
- let users inspect task history, state, event transcript, changed files, diffs,
  workspace mode, session ID, and remote-upload audit metadata
- let users approve or deny pending `provider.permission_requested` events
  through the existing permission-response path
- preserve all Phase 8 product ownership and scope checks
- preserve all Phase 10 remote-execution gates
- keep browser and desktop state consistent under refresh, reconnect, SSE cursor
  resume, and daemon restart
- add focused Rust Console, API, and E2E tests
- quality gates passing

Phase 11 is an operational UX phase. It is not a new daemon core, not a cloud
product backend, and not a provider-secret management phase.

## Inputs

- Roadmap: `docs/tasks/opendaemon-roadmap.md`
- Phase 8 spec: `docs/tasks/phase-8-product-authentication.md`
- Phase 9 spec: `docs/tasks/phase-9-acp-adapter.md`
- Phase 10 spec: `docs/tasks/phase-10-control-plane.md`
- Current implementation:
  - `src/api/auth.rs`
  - `src/api/mod.rs`
  - `src/api/products.rs`
  - `src/api/providers.rs`
  - `src/api/runtimes.rs`
  - `src/api/agents.rs`
  - `src/api/directories.rs`
  - `src/api/tasks.rs`
  - `src/product/mod.rs`
  - `src/agent/profile.rs`
  - `src/security/directory.rs`
  - `src/runtime/model.rs`
  - `src/control_plane/model.rs`
  - `src/store/daemon_state.rs`
  - `src/store/products.rs`
  - `src/store/tasks.rs`
  - `src/task/event.rs`
  - `src/task/model.rs`
  - `src/task/result.rs`
  - `src/task/service.rs`
  - `src/tests/api.rs`
  - `src/tests/control_plane.rs`
  - `src/tests/runtime.rs`
  - `src/tests/tasks.rs`

## Deliverables

- A shared Rust/WebAssembly Console web app exists.
- The desktop shell, if implemented in Phase 11, uses the same Rust Console app
  and shared Rust crates rather than a separate desktop-only UI.
- The Console can connect to a loopback OpenDaemon instance.
- The Console can be served as static assets without making static asset routes
  authenticated.
- Static Console assets never embed bootstrap tokens, product tokens, daemon
  tokens, task tokens, provider credentials, local paths, or control-plane
  credentials.
- A login/startup view supports bootstrap-token mode and product-token mode.
- `GET /v1/session` or an equivalent credential-introspection route returns the
  current credential type, product ID when present, and scopes without returning
  raw token material.
- `GET /v1/daemon/status` or an equivalent status route returns daemon version,
  configured bind status, scheduler summary, control-plane connection status,
  and runtime summary without exposing raw local directory paths or credentials.
- A pending permission inbox API exists if existing task event APIs cannot list
  unresolved permission requests across product-owned tasks.
- Existing `POST /v1/tasks/:task_id/events` remains the permission response
  write path.
- Bootstrap users can create, disable, list, and inspect products through the
  Console.
- Bootstrap users can create and revoke product tokens through the Console.
- Product-token users can only see actions allowed by their scopes.
- Product-token users can only see their own Agent Profiles, Directory Grants,
  Tasks, task events, and permission requests.
- Provider and runtime screens require the same read scopes as the underlying
  APIs.
- Runtime detection can be triggered from the Console and displays bounded
  progress, success, unavailable, and error states.
- Agent Profile forms reflect the existing `AgentProfile` shape, including
  provider, model, instructions, execution policy, provider config, permission
  mode, custom args, custom env keys, and MCP config.
- Directory Grant forms reflect the existing `DirectoryGrant` shape, including
  product, agent, path, capabilities, workspace modes, default workspace mode,
  lock policy, direct-mode opt-in, and remote-execution allowance.
- Web Console directory grants support explicit local path entry.
- Desktop Console directory grants use a native directory picker when available
  and write the selected path into the same shared grant form.
- Task creation uses existing Agent Profiles and Directory Grants instead of
  allowing task-time provider bypasses.
- Task history supports filtering by status, agent, directory, and product
  ownership according to existing API rules.
- Task details show status, prompt, metadata, provider, model, permission mode,
  workspace mode, timestamps, transcript, final result, changed files, diff,
  artifacts, usage, session ID, and error.
- Task event transcript supports initial load, live SSE updates, idle heartbeat
  tolerance, and cursor-based resume.
- Permission request rows show request summary, provider, permission kind,
  details, expiration when present, task context, and approve or deny controls.
- Permission responses require an explicit user action and optional reason.
- Remote execution is clearly marked when task metadata or result artifacts show
  that code or workspace content was sent to a remote provider.
- Remote-execution controls are hidden or disabled unless the authenticated
  product token has `tasks:remote_execution` and the selected profile, grant,
  and provider path can satisfy Phase 10 policy gates.
- The UI never suggests that provider capability declarations alone grant
  authorization.
- The UI never exposes raw local paths to a remote product context beyond the
  local Console session.
- Browser refresh does not lose authenticated mode, selected route, active task
  detail, or SSE cursor beyond the documented token-storage behavior.
- The desktop shell does not add OS service installation or automatic daemon
  autostart in Phase 11.
- Quality gates pass.

## Non-Goals

Do not add or design these in Phase 11:

- new local authentication model
- user accounts, OAuth, OIDC, SSO, passkeys, or browser login flows
- cloud product backend
- changes to daemon registration, daemon tokens, task tokens, or control-plane
  protocol semantics
- new remote task dispatch pipeline
- new remote-execution policy gates
- provider API-key management
- provider secret storage
- keyring-backed persistent Console credential storage
- daemon service installation
- OS autostart, launch agents, systemd units, or Windows service installation
- tray app lifecycle management
- notification center integration
- generic audit log system
- cross-daemon fleet management
- collaborative multi-user Console sessions
- bidirectional websocket event APIs for local clients
- replacing provider-native approval UIs
- provider-specific custom task APIs outside the normalized task/event/result
  model

Phase 11 can display existing daemon state and add narrow UI helper APIs. It
must not reopen completed daemon policy decisions.

## Dependencies

Keep Phase 0 through Phase 10 Rust dependencies. Add only what is required for
serving static Console assets, compiling the Rust/WebAssembly Console, testing
the Console, and narrow UI helper routes.

Required Rust Console frontend stack:

```text
Rust
Leptos CSR
leptos_router
gloo-net
wasm-bindgen
wasm-bindgen-test
web-sys
serde
trunk
fantoccini
```

Required shared Rust Console structure:

```text
crates/opendaemon-console-api      # shared request/response DTOs and API client types
crates/opendaemon-console-ui       # Leptos CSR app, routes, screens, and state
console/index.html                 # Trunk entry point for web builds
console/Trunk.toml                 # static asset build configuration
```

Suggested desktop stack, only if the optional desktop shell is implemented:

```text
Tauri v2
```

Dependency constraints:

- do not add a large component framework unless it demonstrably reduces
  implementation risk
- do not add a JavaScript or TypeScript application framework for the Console UI
  in Phase 11
- do not add OAuth or identity SDKs
- do not add provider secret storage libraries in Phase 11
- do not add Electron
- do not make default CI depend on a real provider account, real control plane,
  or platform-specific desktop bundle signing
- if current stable package versions differ at implementation time, use current
  stable versions and keep the dependency purposes unchanged

## Console Product Contract

### Product Shape

The Console is an operational tool. It should be quiet, dense enough for
repeated use, and optimized for scanning state and taking explicit action. It
must not be a marketing landing page.

Required layout:

- persistent app shell
- left navigation on desktop widths
- compact top status bar
- route content region
- detail drawers or route-level detail pages for entities with long state
- responsive single-column navigation on narrow widths

Required navigation:

- Overview
- Products
- Providers
- Runtimes
- Agents
- Directories
- Tasks
- Permissions
- Settings

The desktop shell and web page must use the same route names, labels, empty
states, error states, confirmation copy, and table/detail layouts. Native
desktop affordances may fill inputs or open OS dialogs, but they must not create
different workflows.

### State Language

Use existing daemon state names where possible:

- product status: `active`, `disabled`
- runtime status: `not_detected`, `available`, `unavailable`, `error`
- runtime kind: `local_cli`, `local_acp`, `remote_http`
- daemon status: `online`, `offline`, `connecting`, `error`
- task status: `queued`, `waiting_directory_lock`, `preparing`, `running`,
  `completed`, `failed`, `cancelled`, `timed_out`
- workspace mode: `worktree`, `direct`
- permission decision: `approve`, `deny`

UI copy may translate these into human-readable labels, but payloads, filters,
and tests must remain tied to the canonical values.

### Visual System

Use a restrained product UI system:

- no hero page before the usable Console
- no decorative card-heavy landing composition
- no nested cards
- no gradient text
- no oversized rounded cards
- use tables, split panes, forms, segmented controls, tabs, badges, and drawers
  where they fit operational workflows
- use icon buttons with tooltips for repeated actions
- use text buttons only for clear commands such as "Create token", "Grant
  directory", "Detect runtimes", "Approve request", and "Deny request"
- keep cards to individual repeated items or modal surfaces only
- keep body text contrast at accessible levels
- support keyboard navigation and visible focus states
- support reduced motion

The first viewport after login should show useful daemon state immediately:
runtime health, active tasks, pending permission requests, and control-plane
connection status.

## Authentication UX Contract

### Credential Modes

The Console supports two credential modes:

1. bootstrap token
2. product API token

Bootstrap mode is for local product management. Product-token mode is for normal
product-scoped operation.

Requirements:

- the Console accepts tokens only through an explicit credential form
- token inputs are password-style fields by default
- raw tokens are never written to logs, URLs, route params, task metadata, task
  events, screenshots, or static assets
- web mode stores credentials in memory by default
- if web mode offers session persistence, it may use `sessionStorage` only and
  must default to off
- desktop mode must not persist credentials in Phase 11
- the Console must provide a clear "Forget token" action
- the Console must verify the token through `GET /v1/session` or equivalent
  before showing privileged screens
- route actions must be hidden or disabled based on scopes returned by session
  introspection
- disabled products and revoked tokens must show stable `401` copy
- insufficient scopes must show stable `403` copy and the missing scope when the
  API exposes it

### Session Introspection Route

Add a route equivalent to:

```http
GET /v1/session
```

Accepted credentials:

- bootstrap token
- product API token

Response for bootstrap credential:

```json
{
  "credential_type": "bootstrap",
  "product_id": null,
  "scopes": [],
  "product_status": null
}
```

Response for product credential:

```json
{
  "credential_type": "product",
  "product_id": "product_example",
  "scopes": ["providers:read", "runtimes:read", "tasks:read"],
  "product_status": "active"
}
```

Requirements:

- the route must never return the raw bearer token
- the route must reject missing, invalid, revoked, or disabled credentials with
  existing stable auth errors
- the route must not let bootstrap credentials impersonate a product
- the route must not authorize any action by itself; route-level checks still
  happen on each API call

## Daemon Status Contract

The Overview screen needs a compact status payload. Add a route equivalent to:

```http
GET /v1/daemon/status
```

Required scopes:

- bootstrap credential, or
- product token with `runtimes:read` and `tasks:read`

Representative response:

```json
{
  "service": "opendaemon",
  "version": "0.1.0",
  "status": "online",
  "control_plane": {
    "status": "online",
    "daemon_id": "daemon_123",
    "last_heartbeat_at": "2026-06-08T00:00:00Z",
    "last_error_code": null
  },
  "scheduler": {
    "queued": 3,
    "running": 1,
    "max_concurrent_tasks": 4
  },
  "runtimes": {
    "available": 2,
    "unavailable": 1,
    "error": 0,
    "not_detected": 4
  },
  "permissions": {
    "pending": 1
  }
}
```

Requirements:

- do not include bootstrap tokens, product tokens, daemon tokens, task tokens,
  or provider credentials
- do not include raw directory grant paths in the summary
- product-token callers only see task and permission counts for their own tasks
- control-plane details must be safe local operational metadata, not cloud
  secrets
- if control-plane connectivity is disabled, return a stable disabled state
  rather than an error

## Console Screens

### Overview

Show the current operational state:

- daemon online/offline
- version
- control-plane status
- runtime availability summary
- queued/running task summary
- pending permission count
- recent task results
- recent runtime detection errors

Primary actions:

- Detect runtimes
- Create task
- Review permissions
- Grant directory
- Create agent

Actions must appear only when the authenticated credential can perform them.

### Products

Bootstrap mode only.

Requirements:

- list products with status, description, created time, and updated time
- create product
- edit display name and description
- disable or reactivate product
- list product token metadata
- create token with explicit scopes
- show plaintext token only in the creation result
- revoke token
- never show token digests or plaintext tokens after creation

Product-token users must not see this screen as an available route.

### Providers

Product-token users need `providers:read`. Bootstrap users may read providers
for setup.

Requirements:

- list provider manifests
- show registry status, vendor, integration type, models, capabilities,
  permission modes, environment requirements, install guidance, and security
  disclosures
- make remote code-upload disclosures visible for HTTP providers
- avoid presenting provider capability declarations as granted permissions
- link each provider to matching runtime status
- show unsupported or invalid provider metadata as stable error states

### Runtimes

Product-token users need `runtimes:read`. Bootstrap users may read runtimes for
setup.

Requirements:

- list runtime ID, provider ID, kind, status, executable when safe for local
  display, version, detected time, and error
- run runtime detection
- show unavailable and error states without blocking the whole screen
- never run provider task execution from runtime detection
- distinguish local CLI, local ACP, and remote HTTP runtime kinds
- show control-plane runtime publication status only if Phase 10 exposes it
  locally

### Agents

Product-token users need `agents:read` and `agents:write` for mutations.

Requirements:

- list product-owned Agent Profiles
- create Agent Profile
- edit Agent Profile
- delete Agent Profile where the existing API allows it
- select provider and model from provider registry data
- validate permission mode against provider manifest data
- edit execution policy:
  - default workspace mode
  - direct directory allowance
- edit provider config:
  - custom args
  - custom env keys
  - MCP config JSON
  - provider permission mode
- prevent task-time provider bypasses by making the profile the stable selection
  unit
- warn when selected provider runtime is not available without blocking profile
  creation if the API permits profile creation

### Directories

Product-token users need `directories:read` and `directories:grant` for grant
creation. Direct-mode actions also require `directories:direct`.

Requirements:

- list product-owned Directory Grants
- create Directory Grant for product and agent
- edit grant policy
- delete grant where the existing API allows it
- show path only in the local Console context
- support manual path entry in web mode
- support native directory picker in desktop mode
- edit capabilities:
  - `read`
  - `write`
  - `shell`
  - `git`
- edit workspace modes:
  - `worktree`
  - `direct`
- edit default workspace mode
- edit lock policy
- edit direct-mode explicit task opt-in
- edit remote-execution allowance
- prevent direct-mode controls unless credential scope and current grant policy
  allow them
- make remote execution opt-in visibly separate from local write permissions

### Tasks

Product-token users need `tasks:read` for history, `tasks:create` for creation,
and `tasks:cancel` for cancellation.

Requirements:

- list product-owned tasks
- filter by status, agent, directory, and runtime/provider where data exists
- create task from existing Agent Profile and Directory Grant
- collect prompt, required capabilities, workspace mode, direct-mode opt-in,
  timeout, and metadata
- block direct-mode task submission unless required scope and grant/profile
  policy allow it
- block remote HTTP submission unless Phase 10 remote-execution gates can be
  satisfied and the credential includes `tasks:remote_execution`
- show task detail with event transcript and result
- support task cancellation
- show terminal states clearly
- show timeout and failure reason when available
- show changed files and diff from `TaskResult`
- show artifacts and usage when available
- show session ID only as a provider artifact, never as authorization material
- mark worktree and direct-mode tasks distinctly
- mark remote-upload audit metadata distinctly

### Permissions

Product-token users need `tasks:read`. The existing permission response route is
used for decisions.

Requirements:

- list pending permission requests for product-owned tasks
- show task, provider, permission kind, summary, details, options, and
  expiration
- approve or deny with an explicit click
- allow optional reason
- disable decision controls after a request is resolved, expired, or the task is
  terminal in a way that cannot accept responses
- write responses through `POST /v1/tasks/:task_id/events`
- show `provider.permission_decided` events in the task transcript
- never invent a permission decision locally if the API rejects the response

If existing task event APIs cannot efficiently list pending permission requests,
add a route equivalent to:

```http
GET /v1/permissions?status=pending
```

Representative response:

```json
{
  "permissions": [
    {
      "task_id": "task_123",
      "request_id": "perm_123",
      "provider_id": "codex",
      "permission_kind": "shell",
      "summary": "Run cargo test",
      "details": {},
      "options": ["approve", "deny"],
      "expires_at": null,
      "created_at": "2026-06-08T00:00:00Z"
    }
  ]
}
```

Requirements:

- product-token callers only see permission requests for their own tasks
- bootstrap callers may see all local pending permissions for setup and support
- responses still go through the existing task-scoped permission response API

### Settings

Requirements:

- show current credential mode
- show current product ID and scopes for product-token mode
- provide "Forget token"
- show daemon base URL
- show Console version
- show control-plane connectivity status when available
- show documentation links for local API auth and remote execution risk

Settings must not expose raw token values after login.

## Web Console Contract

The web Console may be served by the daemon or run through a local development
server.

Static serving requirements:

- `GET /console` returns the Console shell
- `GET /console/*` returns static assets
- static routes are unauthenticated
- all data routes still require their normal API credentials
- serving Console assets must not change `/v1/*` API auth behavior
- direct navigation to a Console route must load the shell and let the Leptos
  router handle the route

Development requirements:

- local dev server can target a configured OpenDaemon base URL
- CORS support, if required for development, must be limited to explicit local
  origins and must not become a broad production reverse-proxy policy

Browser constraints:

- web mode cannot rely on native OS directory picker behavior
- web mode supports manual path entry
- web mode may use browser file-system APIs only as a progressive enhancement
  when they can produce an API-compatible local path safely

## Desktop Shell Contract

The desktop shell is optional in Phase 11. If implemented, it wraps the same web
Console.

Requirements:

- use the shared Trunk-built Console bundle and shared Rust API client crate
- connect to a configured loopback daemon URL
- show a not-running state if the daemon is unavailable
- do not install, autostart, or supervise an OS service in Phase 11
- use a native directory picker for Directory Grant creation when available
- pass selected directory paths into the shared Directory Grant form
- never bypass the daemon's directory grant API
- never bypass Phase 8 product auth
- never bypass Phase 10 remote-execution policy
- never persist tokens in Phase 11
- desktop-only code is limited to shell setup, native picker bridge, and daemon
  connection checks

Desktop shell tests should not require signed bundles in default CI.

## Data Flow

### Login

1. User opens the Console.
2. User enters bootstrap token or product API token.
3. Console calls `GET /v1/session`.
4. Console stores the session principal in Rust/WASM client state.
5. Console shows routes and actions allowed by credential type and scopes.
6. Each API call still includes the bearer token and relies on daemon-side
   authorization.

### Directory Grant

1. User opens Directories.
2. Web mode user enters a local path manually, or desktop mode user selects a
   path through the native picker.
3. User selects product, agent, capabilities, workspace modes, lock policy, and
   remote-execution allowance.
4. Console calls `POST /v1/directories/grant`.
5. Daemon canonicalizes path and enforces ownership, scope, path guard, and
   policy.
6. Console displays the returned `DirectoryGrant`.

### Task Execution

1. User opens Tasks and creates a task.
2. Console submits existing agent ID and directory ID with prompt and policy
   choices.
3. Daemon validates product, agent, directory, scope, workspace, runtime, and
   remote-execution gates.
4. Console opens task detail and subscribes to `GET /v1/tasks/:task_id/events`.
5. Console renders events in sequence.
6. On reconnect, Console resumes from cursor or `Last-Event-ID`.
7. When terminal state arrives, Console fetches the final task result.

### Permission Response

1. ACP or another adapter emits `provider.permission_requested`.
2. Daemon persists the event.
3. Console lists the request in Permissions and task detail.
4. User approves or denies.
5. Console sends `provider.permission_response` through
   `POST /v1/tasks/:task_id/events`.
6. Daemon resolves the live permission request and persists the decision event.
7. Console disables the decision controls and updates the transcript.

## Error Handling

Requirements:

- `401` means the credential is missing, invalid, revoked, or disabled
- `403` means the credential is valid but lacks scope or ownership
- route-level errors must preserve the daemon's stable error code
- Rust Console forms must not guess authorization outcomes when the daemon
  rejects a request
- failed runtime detection must not make provider and task screens unusable
- failed SSE connection must show reconnect state and retry with cursor support
- stale task detail must be refreshed after reconnect
- permission response rejection must leave the request visible with the API
  error
- desktop daemon-unavailable state must show the configured daemon URL and a
  copyable command to start the daemon

Recommended daemon-unavailable copy:

```text
OpenDaemon is not reachable at http://127.0.0.1:19514.
Start the daemon, then reconnect.
```

## Accessibility And Responsive Requirements

Requirements:

- all controls are keyboard reachable
- focus indicators are visible
- dialogs trap focus while open
- tables have accessible headers
- icon-only buttons have accessible labels and tooltips
- form errors are associated with fields
- destructive actions require confirmation
- color is not the only indicator of status
- body text contrast is at least 4.5:1
- large text contrast is at least 3:1
- reduced-motion preference is respected
- narrow viewports support navigation, forms, task detail, and permission
  decisions without horizontal overflow

Minimum viewport coverage:

- 390px wide mobile browser
- 768px tablet
- 1280px desktop
- 1440px desktop

## Store And API Changes

Keep schema changes narrow.

Allowed daemon changes:

- session introspection response types
- daemon status summary response types
- pending permission list query if needed
- static Console asset serving
- optional UI preferences stored locally without credentials

Avoid:

- changing existing product, token, agent, directory, task, event, or result API
  response shapes unless tests prove the old shape was unusable
- adding raw local path APIs for remote products
- adding provider secret tables
- adding persistent Console credential tables
- adding a new event broker
- adding a new task state machine

## Testing Requirements

Add focused coverage at API, Rust Console, desktop-shell, and E2E layers.

Rust API tests:

- session route accepts bootstrap token and returns
  `credential_type =
  "bootstrap"`
- session route accepts product token and returns product ID plus scopes
- session route rejects missing, invalid, revoked, and disabled credentials
- daemon status route hides raw tokens and raw directory paths
- daemon status route scopes task and permission counts to the authenticated
  product
- permission inbox lists only product-owned pending permission requests
- permission inbox does not replace the task-scoped permission response API
- static Console routes serve assets without weakening `/v1/*` auth

Rust Console unit tests:

- API client attaches bearer token headers
- API client never places tokens in URLs
- scope gating hides or disables unavailable actions
- product token creation displays plaintext token only in the creation result
- Agent Profile form serializes `ExecutionPolicy` and `ProviderConfig` correctly
- Directory Grant form serializes capabilities, workspace modes, lock policy,
  direct-mode opt-in, and remote-execution allowance correctly
- Task creation blocks direct-mode and remote-execution controls when scopes are
  missing
- task transcript applies SSE events in sequence
- permission response form sends `provider.permission_response` payloads

Rust Console integration tests with fake API:

- bootstrap login can create a product and mint a token
- product login can view providers, runtimes, agents, directories, tasks, and
  permissions according to scopes
- runtime detection screen displays available, unavailable, and error states
- task detail resumes from cursor after simulated SSE reconnect
- permission inbox updates after approve and deny
- remote-upload metadata renders as a visible audit marker

Desktop shell tests, if desktop shell is implemented:

- shell loads the shared Trunk-built Console bundle
- shell reports daemon unavailable when loopback API is down
- native directory picker writes the selected path into the shared grant form
- shell does not persist tokens
- shell does not bypass daemon APIs for grant creation

E2E tests:

- start daemon with fake provider registry
- register a product with bootstrap token
- mint product token
- log into Console with product token
- detect fake runtime
- create Agent Profile
- grant temporary directory
- create task
- stream task events
- inspect final result
- resolve a fake pending permission request
- verify unauthorized product cannot see another product's tasks or permissions

Accessibility and visual verification:

- Rust WebDriver screenshots for web Console at required viewport widths
- Rust WebDriver accessibility checks for primary routes, including landmark
  roles, accessible names, focus order, keyboard operation, and contrast smoke
  checks
- keyboard navigation smoke test for login, navigation, forms, task detail, and
  permission decisions

## Quality Gates

Phase 11 is complete only when these pass:

- `cargo fmt --all`
- `cargo clippy --tests --all-targets --all-features -- -D warnings`
- `cargo test -- --test-threads=1`
- Rust/WASM dependency resolution is reproducible from the committed
  `Cargo.lock`
- Console crates pass `cargo check`, including the `wasm32-unknown-unknown`
  target
- Console unit tests pass
- Console production build through `trunk build --release` passes
- Rust WebDriver web Console smoke tests pass
- desktop shell check passes if the desktop shell is implemented

Default CI must not require:

- real third-party provider credentials
- a real control plane
- signed desktop bundles
- OS keychain access
- external network access beyond dependency installation performed before the
  locked CI run

## Acceptance Checklist

- [ ] Shared Rust/WebAssembly web Console exists.
- [ ] Desktop shell, if implemented, reuses the same Console screens and API
      client crate.
- [ ] Console startup supports bootstrap-token and product-token modes.
- [ ] Session introspection exists and never returns raw tokens.
- [ ] Overview shows daemon, scheduler, control-plane, runtime, task, and
      pending-permission status.
- [ ] Bootstrap users can manage products and product tokens.
- [ ] Product users only see routes and actions allowed by scopes.
- [ ] Provider screen shows registry metadata, install guidance, capabilities,
      permission modes, and security disclosures.
- [ ] Runtime screen can run detection and display stable status/error states.
- [ ] Agent Profile screen can create and edit profiles using existing model
      fields.
- [ ] Directory screen can create and edit grants using existing grant fields.
- [ ] Web mode supports manual path entry.
- [ ] Desktop mode, if implemented, supports native directory picking.
- [ ] Task screen can create tasks only through existing agents and directory
      grants.
- [ ] Task detail shows transcript, result, changed files, diff, workspace mode,
      session ID, artifacts, usage, and errors.
- [ ] Permission screen lists pending provider permission requests.
- [ ] Permission responses use the existing task event response API.
- [ ] Remote-execution controls and audit markers respect Phase 10 policy.
- [ ] Console never forwards or displays OpenDaemon credentials beyond the
      required credential entry and one-time token creation result.
- [ ] Static Console asset serving does not weaken `/v1/*` auth.
- [ ] Accessibility, responsive, Rust Console, API, and E2E tests pass.
- [ ] Quality gates pass.

## Handoff To Phase 12

Phase 11 should leave OpenDaemon with a usable local operating surface for
ordinary users and product integrators:

- users can set up products, agents, directories, and tasks without editing JSON
- users can inspect runtime and task state without reading daemon logs
- users can respond to provider permission requests when protocols support them
- desktop and web surfaces stay consistent
- daemon core policy boundaries remain unchanged

Phase 12 can focus on deeper operational hardening that Phase 11 intentionally
does not own, such as daemon service installation, OS autostart, persistent
secure credential storage, provider secret management, signed desktop builds,
and a durable audit log.
