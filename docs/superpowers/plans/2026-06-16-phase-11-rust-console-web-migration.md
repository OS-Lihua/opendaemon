# Phase 11 Rust Console Web Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the React/Vite Console with a Rust-only Leptos web Console
served by the daemon at `/console`.

**Architecture:** Keep the daemon crate responsible for API routes and static
asset serving, add a shared Rust Console API crate for DTOs/client/SSE/policy
helpers, and add a Leptos CSR UI crate for the web Console. The `console/`
directory becomes only the Trunk entrypoint and generated asset output.

**Tech Stack:** Rust 2024, Axum, Serde, Leptos CSR, leptos_router, gloo-net,
wasm-bindgen, web-sys, Trunk, wasm-bindgen-test.

---

## File Structure

- Create `crates/opendaemon-console-api/Cargo.toml`: shared API crate manifest.
- Create `crates/opendaemon-console-api/src/lib.rs`: public module exports.
- Create `crates/opendaemon-console-api/src/dto.rs`: request and response DTOs
  matching daemon JSON contracts.
- Create `crates/opendaemon-console-api/src/error.rs`: typed API/client errors.
- Create `crates/opendaemon-console-api/src/client.rs`: browser HTTP client and
  auth header handling.
- Create `crates/opendaemon-console-api/src/events.rs`: SSE parsing, event
  application helpers, cursor tracking.
- Create `crates/opendaemon-console-api/src/policy.rs`: scope checks,
  direct-mode gate, remote-execution gate.
- Create `crates/opendaemon-console-api/tests/events.rs`: host tests for SSE
  parsing and cursor behavior.
- Create `crates/opendaemon-console-api/tests/policy.rs`: host tests for scope
  and gate helpers.
- Create `crates/opendaemon-console-ui/Cargo.toml`: Leptos UI crate manifest.
- Create `crates/opendaemon-console-ui/src/lib.rs`: wasm entrypoint.
- Create `crates/opendaemon-console-ui/src/app.rs`: root app component and route
  switch.
- Create `crates/opendaemon-console-ui/src/shell.rs`: app shell, navigation, top
  status bar.
- Create `crates/opendaemon-console-ui/src/components.rs`: shared form, table,
  badge, and empty-state components.
- Create `crates/opendaemon-console-ui/src/state/session.rs`: browser storage,
  session restore, route persistence.
- Create `crates/opendaemon-console-ui/src/state/resources.rs`: load/refresh
  ordinary resources.
- Create `crates/opendaemon-console-ui/src/state/tasks.rs`: task
  list/detail/transcript state.
- Create `crates/opendaemon-console-ui/src/routes/*.rs`: route components for
  login, overview, products, providers, agents, directories, tasks, permissions,
  settings.
- Create `crates/opendaemon-console-ui/tests/session.rs`: wasm tests for session
  persistence helpers.
- Create `crates/opendaemon-console-ui/tests/tasks.rs`: wasm tests for task
  transcript state helpers.
- Modify `Cargo.toml`: add workspace members and dependencies needed by the
  daemon for static assets.
- Modify `src/api/console.rs`: add Trunk asset content types, keep public
  console routes, keep API auth unchanged.
- Modify `src/tests/console.rs`: add coverage for Trunk asset serving and API
  auth boundaries.
- Replace `console/index.html`: Trunk web entrypoint.
- Create `console/Trunk.toml`: Trunk build configuration.
- Create `console/static/styles.css`: operational Console visual system.
- Delete `console/src/**`, `console/package.json`, `console/pnpm-lock.yaml`,
  `console/pnpm-workspace.yaml`, `console/vite.config.ts`, and
  `console/tsconfig.json`.

---

## Task 1: Workspace And Trunk Skeleton

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/opendaemon-console-api/Cargo.toml`
- Create: `crates/opendaemon-console-api/src/lib.rs`
- Create: `crates/opendaemon-console-ui/Cargo.toml`
- Create: `crates/opendaemon-console-ui/src/lib.rs`
- Replace: `console/index.html`
- Create: `console/Trunk.toml`
- Create: `console/static/styles.css`

- [ ] **Step 1: Inspect the current manifest and console entry**

Run: `sed -n '1,220p' Cargo.toml`

Expected: root package is `opendaemon`, edition is `2024`, and there is no
`[workspace]` section yet.

Run: `sed -n '1,120p' console/index.html`

Expected: current file is a Vite/React entry or placeholder that can be replaced
by Trunk markup.

- [ ] **Step 2: Convert the root manifest into a package workspace**

Modify `Cargo.toml` by adding this section below the `[package]` block:

```toml
[workspace]
members = [
    ".",
    "crates/opendaemon-console-api",
    "crates/opendaemon-console-ui",
]
resolver = "3"
```

Keep the existing `[package]`, `[dependencies]`, `[profile.release]`, and
`[package.metadata.release]` sections intact.

- [ ] **Step 3: Add the shared API crate manifest**

Create `crates/opendaemon-console-api/Cargo.toml`:

```toml
[package]
name = "opendaemon-console-api"
version = "0.1.0"
edition = "2024"

[dependencies]
gloo-net = { version = "0.6", features = ["http"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
wasm-bindgen-futures = "0.4"

[dev-dependencies]
serde_json = "1"
```

- [ ] **Step 4: Add the API crate module shell**

Create `crates/opendaemon-console-api/src/lib.rs`:

```rust
pub mod client;
pub mod dto;
pub mod error;
pub mod events;
pub mod policy;

pub use client::ConsoleApiClient;
pub use error::ConsoleApiError;
```

- [ ] **Step 5: Add the Leptos UI crate manifest**

Create `crates/opendaemon-console-ui/Cargo.toml`:

```toml
[package]
name = "opendaemon-console-ui"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = { version = "0.7", features = ["csr"] }
leptos_router = { version = "0.7", features = ["csr"] }
opendaemon-console-api = { path = "../opendaemon-console-api" }
serde_json = "1"
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "HtmlInputElement",
    "HtmlSelectElement",
    "HtmlTextAreaElement",
    "Storage",
    "Window",
] }

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

- [ ] **Step 6: Add the UI crate wasm entrypoint**

Create `crates/opendaemon-console-ui/src/lib.rs`:

```rust
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

mod app;
mod components;
mod routes;
mod shell;
mod state;

#[wasm_bindgen(start)]
pub fn mount() {
    console_error_panic_hook::set_once();
    mount_to_body(app::App);
}
```

Then add `console_error_panic_hook = "0.1"` to
`crates/opendaemon-console-ui/Cargo.toml`.

- [ ] **Step 7: Replace the Trunk entrypoint**

Replace `console/index.html` with:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>OpenDaemon Console</title>
    <link data-trunk rel="css" href="static/styles.css" />
    <link
      data-trunk
      rel="rust"
      data-wasm-opt="z"
      href="../crates/opendaemon-console-ui/Cargo.toml"
    />
  </head>
  <body></body>
</html>
```

- [ ] **Step 8: Add Trunk configuration**

Create `console/Trunk.toml`:

```toml
[build]
target = "index.html"
dist = "dist"
public_url = "/console/"

[watch]
watch = [
    "../crates/opendaemon-console-api/src",
    "../crates/opendaemon-console-ui/src",
    "index.html",
    "static",
]
```

- [ ] **Step 9: Add a minimal visual system file**

Create `console/static/styles.css`:

```css
:root {
  color: #17211f;
  background: #f6f4ef;
  font-family: "Avenir Next", "Segoe UI", sans-serif;
  line-height: 1.45;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
  background:
    linear-gradient(135deg, rgba(20, 83, 45, 0.08), transparent 34%),
    linear-gradient(315deg, rgba(180, 83, 9, 0.08), transparent 30%),
    #f6f4ef;
}

button,
input,
select,
textarea {
  font: inherit;
}
```

- [ ] **Step 10: Run initial check and record expected failures**

Run: `cargo check --workspace`

Expected: FAIL because `client`, `dto`, `error`, `events`, `policy`, `app`,
`components`, `routes`, `shell`, and `state` modules are declared but not
created yet.

- [ ] **Step 11: Commit**

Run:

```bash
git add Cargo.toml crates/opendaemon-console-api crates/opendaemon-console-ui console/index.html console/Trunk.toml console/static/styles.css
git commit -m "chore: add rust console workspace skeleton"
```

Expected: commit succeeds. If the repository owner does not want commits during
execution, skip only this commit step and continue tracking files manually.

---

## Task 2: Shared DTOs And Policy Helpers

**Files:**

- Create: `crates/opendaemon-console-api/src/dto.rs`
- Create: `crates/opendaemon-console-api/src/policy.rs`
- Create: `crates/opendaemon-console-api/src/error.rs`
- Create: `crates/opendaemon-console-api/src/client.rs`
- Create: `crates/opendaemon-console-api/src/events.rs`
- Create: `crates/opendaemon-console-api/tests/policy.rs`

- [ ] **Step 1: Write policy tests first**

Create `crates/opendaemon-console-api/tests/policy.rs`:

```rust
use opendaemon_console_api::{
    dto::{
        AgentProfile, DirectoryGrant, ExecutionPolicy, ProviderCapability, ProviderConfig,
        Session, WorkspaceMode,
    },
    policy::{can_use_direct_mode, can_use_remote_execution, has_scope},
};

fn session(scopes: &[&str]) -> Session {
    Session {
        credential_type: "product".to_owned(),
        product_id: Some("product_a".to_owned()),
        scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        product_status: Some("active".to_owned()),
    }
}

fn agent(allow_direct: bool) -> AgentProfile {
    AgentProfile {
        id: "agent_a".to_owned(),
        name: "Agent A".to_owned(),
        owner_product_id: "product_a".to_owned(),
        provider_id: "provider_a".to_owned(),
        model: "model-a".to_owned(),
        instructions: None,
        execution_policy: ExecutionPolicy {
            default_workspace_mode: WorkspaceMode::Worktree,
            allow_direct_directory: allow_direct,
        },
        provider_config: ProviderConfig::default(),
        created_at: None,
        updated_at: None,
    }
}

fn grant(allow_direct: bool, allow_remote: bool) -> DirectoryGrant {
    DirectoryGrant {
        id: "grant_a".to_owned(),
        product_id: "product_a".to_owned(),
        agent_id: "agent_a".to_owned(),
        path: "/tmp/work".to_owned(),
        capabilities: vec!["read".to_owned(), "write".to_owned()],
        workspace_modes: if allow_direct {
            vec![WorkspaceMode::Worktree, WorkspaceMode::Direct]
        } else {
            vec![WorkspaceMode::Worktree]
        },
        default_workspace_mode: WorkspaceMode::Worktree,
        lock_policy: "exclusive".to_owned(),
        direct_mode_requires_explicit_task_opt_in: true,
        allow_remote_execution: allow_remote,
        created_at: None,
        updated_at: None,
    }
}

#[test]
fn has_scope_matches_exact_scope() {
    assert!(has_scope(&session(&["tasks:read"]), "tasks:read"));
    assert!(!has_scope(&session(&["tasks:read"]), "tasks:create"));
}

#[test]
fn direct_mode_requires_scope_agent_and_grant_support() {
    assert!(can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(true),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&[]),
        &agent(true),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(false),
        &grant(true, false)
    ));
    assert!(!can_use_direct_mode(
        &session(&["directories:direct"]),
        &agent(true),
        &grant(false, false)
    ));
}

#[test]
fn remote_execution_requires_scope_grant_and_provider_capability() {
    assert!(can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, true),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&[]),
        &grant(true, true),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, false),
        &[ProviderCapability::RemoteExecution]
    ));
    assert!(!can_use_remote_execution(
        &session(&["tasks:remote_execution"]),
        &grant(true, true),
        &[]
    ));
}
```

- [ ] **Step 2: Run the failing policy tests**

Run: `cargo test -p opendaemon-console-api --test policy`

Expected: FAIL because DTO and policy modules are not implemented.

- [ ] **Step 3: Implement DTOs**

Create `crates/opendaemon-console-api/src/dto.rs` with the DTOs migrated from
`console/src/api.ts`, using these key definitions:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    Worktree,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    WaitingDirectoryLock,
    Preparing,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCapability {
    RemoteExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub credential_type: String,
    pub product_id: Option<String>,
    pub scopes: Vec<String>,
    pub product_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub default_workspace_mode: WorkspaceMode,
    pub allow_direct_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub custom_args: Vec<String>,
    #[serde(default)]
    pub custom_env_keys: Vec<String>,
    pub mcp_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    pub execution_policy: ExecutionPolicy,
    pub provider_config: ProviderConfig,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryGrant {
    pub id: String,
    pub product_id: String,
    pub agent_id: String,
    pub path: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub workspace_modes: Vec<WorkspaceMode>,
    pub default_workspace_mode: WorkspaceMode,
    pub lock_policy: String,
    pub direct_mode_requires_explicit_task_opt_in: bool,
    pub allow_remote_execution: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
```

Then add the remaining DTOs from `console/src/api.ts`: `DaemonStatus`,
`Product`, `ProductToken`, `CreatedProductToken`, `Provider`, `RuntimeView`,
`Task`, `TaskResult`, `TaskEventView`, `PermissionRequest`, plus create/update
payload structs. Use `serde_json::Value` for manifest, provider result, usage,
artifacts, details, metadata, and error payloads.

- [ ] **Step 4: Implement policy helpers**

Create `crates/opendaemon-console-api/src/policy.rs`:

```rust
use crate::dto::{AgentProfile, DirectoryGrant, ProviderCapability, Session, WorkspaceMode};

pub fn has_scope(session: &Session, scope: &str) -> bool {
    session.scopes.iter().any(|candidate| candidate == scope)
}

pub fn can_use_direct_mode(
    session: &Session,
    agent: &AgentProfile,
    grant: &DirectoryGrant,
) -> bool {
    has_scope(session, "directories:direct")
        && agent.execution_policy.allow_direct_directory
        && grant.workspace_modes.contains(&WorkspaceMode::Direct)
}

pub fn can_use_remote_execution(
    session: &Session,
    grant: &DirectoryGrant,
    provider_capabilities: &[ProviderCapability],
) -> bool {
    has_scope(session, "tasks:remote_execution")
        && grant.allow_remote_execution
        && provider_capabilities.contains(&ProviderCapability::RemoteExecution)
}
```

- [ ] **Step 5: Add error, client, and event stubs that compile**

Create `crates/opendaemon-console-api/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsoleApiError {
    #[error("http request failed: {0}")]
    Request(String),
    #[error("api returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("failed to decode response: {0}")]
    Decode(String),
}
```

Create `crates/opendaemon-console-api/src/client.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleApiClient {
    base_url: String,
    token: String,
}

impl ConsoleApiClient {
    #[must_use]
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token: token.into(),
        }
    }

    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    #[must_use]
    pub fn bearer_token(&self) -> &str {
        &self.token
    }
}
```

Create `crates/opendaemon-console-api/src/events.rs`:

```rust
use crate::dto::TaskEventView;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCursor {
    latest_sequence: Option<u64>,
}

impl EventCursor {
    #[must_use]
    pub fn latest_sequence(&self) -> Option<u64> {
        self.latest_sequence
    }

    pub fn observe(&mut self, event: &TaskEventView) {
        self.latest_sequence = Some(self.latest_sequence.map_or(event.sequence, |current| {
            current.max(event.sequence)
        }));
    }
}
```

Adjust `TaskEventView.sequence` in `dto.rs` to `u64`.

- [ ] **Step 6: Run policy tests**

Run: `cargo test -p opendaemon-console-api --test policy`

Expected: PASS.

- [ ] **Step 7: Run API crate check**

Run: `cargo check -p opendaemon-console-api`

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/opendaemon-console-api
git commit -m "feat: add console api dto and policy helpers"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 3: SSE Parsing And API Client Methods

**Files:**

- Modify: `crates/opendaemon-console-api/src/events.rs`
- Modify: `crates/opendaemon-console-api/src/client.rs`
- Modify: `crates/opendaemon-console-api/src/error.rs`
- Create: `crates/opendaemon-console-api/tests/events.rs`
- Create: `crates/opendaemon-console-api/tests/client.rs`

- [ ] **Step 1: Write SSE parser tests**

Create `crates/opendaemon-console-api/tests/events.rs`:

```rust
use opendaemon_console_api::{
    dto::TaskEventView,
    events::{EventCursor, parse_sse_block},
};

#[test]
fn parses_data_only_sse_block() {
    let event = parse_sse_block(
        r#"event: task_event
data: {"task_id":"task_1","sequence":7,"type":"task.started","payload":{},"created_at":"2026-06-16T00:00:00Z"}
"#,
    )
    .unwrap();

    assert_eq!(event.task_id, "task_1");
    assert_eq!(event.sequence, 7);
    assert_eq!(event.r#type, "task.started");
}

#[test]
fn ignores_comment_heartbeat_blocks() {
    assert!(parse_sse_block(": heartbeat\n").unwrap().is_none());
}

#[test]
fn cursor_tracks_highest_sequence() {
    let mut cursor = EventCursor::default();
    cursor.observe(&TaskEventView {
        task_id: "task_1".to_owned(),
        sequence: 3,
        r#type: "task.output".to_owned(),
        payload: serde_json::json!({}),
        created_at: "2026-06-16T00:00:00Z".to_owned(),
    });
    cursor.observe(&TaskEventView {
        task_id: "task_1".to_owned(),
        sequence: 2,
        r#type: "task.output".to_owned(),
        payload: serde_json::json!({}),
        created_at: "2026-06-16T00:00:01Z".to_owned(),
    });

    assert_eq!(cursor.latest_sequence(), Some(3));
}
```

- [ ] **Step 2: Run failing SSE tests**

Run: `cargo test -p opendaemon-console-api --test events`

Expected: FAIL because `parse_sse_block` is missing.

- [ ] **Step 3: Implement SSE parsing**

Replace `crates/opendaemon-console-api/src/events.rs` with:

```rust
use crate::{dto::TaskEventView, error::ConsoleApiError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCursor {
    latest_sequence: Option<u64>,
}

impl EventCursor {
    #[must_use]
    pub fn latest_sequence(&self) -> Option<u64> {
        self.latest_sequence
    }

    pub fn observe(&mut self, event: &TaskEventView) {
        self.latest_sequence = Some(self.latest_sequence.map_or(event.sequence, |current| {
            current.max(event.sequence)
        }));
    }
}

pub fn parse_sse_block(block: &str) -> Result<Option<TaskEventView>, ConsoleApiError> {
    let mut data = Vec::new();
    for line in block.lines() {
        if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.trim_start());
        }
    }

    if data.is_empty() {
        return Ok(None);
    }

    let joined = data.join("\n");
    serde_json::from_str(&joined)
        .map(Some)
        .map_err(|error| ConsoleApiError::Decode(error.to_string()))
}
```

- [ ] **Step 4: Write client URL tests**

Create `crates/opendaemon-console-api/tests/client.rs`:

```rust
use opendaemon_console_api::ConsoleApiClient;

#[test]
fn client_normalizes_base_url() {
    let client = ConsoleApiClient::new("http://127.0.0.1:3000/", "secret");
    assert_eq!(client.url("/v1/session"), "http://127.0.0.1:3000/v1/session");
    assert_eq!(client.bearer_token(), "secret");
}
```

- [ ] **Step 5: Implement typed client methods**

Extend `crates/opendaemon-console-api/src/client.rs` with generic JSON helpers
using `gloo_net::http::Request`:

```rust
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    dto::{
        AgentProfile, CreatedProductToken, DaemonStatus, DirectoryGrant, PermissionRequest,
        Product, ProductToken, Provider, RuntimeView, Session, Task,
    },
    error::ConsoleApiError,
};

impl ConsoleApiClient {
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ConsoleApiError> {
        let response = gloo_net::http::Request::get(&self.url(path))
            .header("authorization", &format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?;
        decode_response(response).await
    }

    async fn post_json<I: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        input: &I,
    ) -> Result<T, ConsoleApiError> {
        let response = gloo_net::http::Request::post(&self.url(path))
            .header("authorization", &format!("Bearer {}", self.token))
            .json(input)
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?
            .send()
            .await
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?;
        decode_response(response).await
    }

    pub async fn session(&self) -> Result<Session, ConsoleApiError> {
        self.get_json("/v1/session").await
    }

    pub async fn daemon_status(&self) -> Result<DaemonStatus, ConsoleApiError> {
        self.get_json("/v1/daemon/status").await
    }
}

async fn decode_response<T: DeserializeOwned>(
    response: gloo_net::http::Response,
) -> Result<T, ConsoleApiError> {
    let status = response.status();
    if !(200..300).contains(&status) {
        let message = response.text().await.unwrap_or_else(|_| "request failed".to_owned());
        return Err(ConsoleApiError::Api { status, message });
    }
    response
        .json()
        .await
        .map_err(|error| ConsoleApiError::Decode(error.to_string()))
}
```

Then add the remaining typed methods from the design using the same helper. Use
response wrapper structs where daemon endpoints return objects like
`{ "products": [...] }`, `{ "providers": [...] }`, `{ "tasks": [...] }`, and
`{ "permissions": [...] }`.

- [ ] **Step 6: Run API tests**

Run: `cargo test -p opendaemon-console-api`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/opendaemon-console-api
git commit -m "feat: add console api client and sse parsing"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 4: Leptos App Shell And Session State

**Files:**

- Create: `crates/opendaemon-console-ui/src/app.rs`
- Create: `crates/opendaemon-console-ui/src/shell.rs`
- Create: `crates/opendaemon-console-ui/src/components.rs`
- Create: `crates/opendaemon-console-ui/src/state/mod.rs`
- Create: `crates/opendaemon-console-ui/src/state/session.rs`
- Create: `crates/opendaemon-console-ui/src/routes/mod.rs`
- Create: `crates/opendaemon-console-ui/src/routes/login.rs`
- Create route module files for the remaining routes.
- Create: `crates/opendaemon-console-ui/tests/session.rs`

- [ ] **Step 1: Write session storage tests**

Create `crates/opendaemon-console-ui/tests/session.rs`:

```rust
use opendaemon_console_ui::state::session::{StoredSession, storage_key};

#[test]
fn storage_key_is_stable() {
    assert_eq!(storage_key(), "opendaemon.console.session");
}

#[test]
fn stored_session_round_trips_json() {
    let session = StoredSession {
        base_url: "http://127.0.0.1:3000".to_owned(),
        credential_mode: "product".to_owned(),
        bearer_token: "secret".to_owned(),
        last_route: "/tasks".to_owned(),
        active_task_id: Some("task_1".to_owned()),
    };

    let encoded = serde_json::to_string(&session).unwrap();
    let decoded: StoredSession = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, session);
}
```

- [ ] **Step 2: Run failing UI session tests**

Run: `cargo test -p opendaemon-console-ui --test session`

Expected: FAIL because session state is not implemented and `state` is private.

- [ ] **Step 3: Implement route module shell**

Create `crates/opendaemon-console-ui/src/routes/mod.rs`:

```rust
pub mod agents;
pub mod directories;
pub mod login;
pub mod overview;
pub mod permissions;
pub mod products;
pub mod providers;
pub mod settings;
pub mod tasks;
```

Create each route file except `login.rs` with:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! { <section class="route-panel"></section> }
}
```

- [ ] **Step 4: Implement session state module**

Create `crates/opendaemon-console-ui/src/state/mod.rs`:

```rust
pub mod resources;
pub mod session;
pub mod tasks;
```

Create `crates/opendaemon-console-ui/src/state/session.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSession {
    pub base_url: String,
    pub credential_mode: String,
    pub bearer_token: String,
    pub last_route: String,
    pub active_task_id: Option<String>,
}

#[must_use]
pub fn storage_key() -> &'static str {
    "opendaemon.console.session"
}
```

Update `crates/opendaemon-console-ui/src/lib.rs` to make `state` public:

```rust
pub mod state;
```

- [ ] **Step 5: Implement root app**

Create `crates/opendaemon-console-ui/src/app.rs`:

```rust
use leptos::prelude::*;
use leptos_router::{components::Router, path};

use crate::{routes, shell::Shell};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Shell>
                <main class="workspace">
                    <routes::login::LoginRoute />
                </main>
            </Shell>
        </Router>
    }
}
```

If the exact `leptos_router` API differs in the installed version, use the
current stable Leptos CSR route API and keep the component behavior identical.

- [ ] **Step 6: Implement shell**

Create `crates/opendaemon-console-ui/src/shell.rs`:

```rust
use leptos::prelude::*;

#[component]
pub fn Shell(children: Children) -> impl IntoView {
    view! {
        <div class="app-shell">
            <aside class="sidebar" aria-label="Console navigation">
                <div class="brand-lockup">
                    <strong>"OpenDaemon"</strong>
                    <span>"Console"</span>
                </div>
                <nav>
                    <a href="/console/">"Overview"</a>
                    <a href="/console/products">"Products"</a>
                    <a href="/console/providers">"Providers"</a>
                    <a href="/console/agents">"Agents"</a>
                    <a href="/console/directories">"Directories"</a>
                    <a href="/console/tasks">"Tasks"</a>
                    <a href="/console/permissions">"Permissions"</a>
                    <a href="/console/settings">"Settings"</a>
                </nav>
            </aside>
            <section class="shell-content">
                <header class="top-bar">
                    <span>"Local daemon"</span>
                </header>
                {children()}
            </section>
        </div>
    }
}
```

- [ ] **Step 7: Implement login route**

Create `crates/opendaemon-console-ui/src/routes/login.rs`:

```rust
use leptos::prelude::*;

#[component]
pub fn LoginRoute() -> impl IntoView {
    view! {
        <section class="login-panel" aria-labelledby="login-title">
            <h1 id="login-title">"Connect to OpenDaemon"</h1>
            <form class="form-grid">
                <label>
                    <span>"Credential mode"</span>
                    <select name="credential_mode">
                        <option value="product">"Product token"</option>
                        <option value="bootstrap">"Bootstrap token"</option>
                    </select>
                </label>
                <label>
                    <span>"Base URL"</span>
                    <input name="base_url" value="" placeholder="http://127.0.0.1:3000" />
                </label>
                <label>
                    <span>"Bearer token"</span>
                    <input name="token" type="password" />
                </label>
                <button type="submit">"Connect"</button>
            </form>
        </section>
    }
}
```

- [ ] **Step 8: Add shared components and empty state structs**

Create `crates/opendaemon-console-ui/src/components.rs`:

```rust
use leptos::prelude::*;

#[component]
pub fn EmptyState(title: &'static str) -> impl IntoView {
    view! { <p class="empty-state">{title}</p> }
}
```

Create empty compiling state files:

```rust
// crates/opendaemon-console-ui/src/state/resources.rs
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResourceState;
```

```rust
// crates/opendaemon-console-ui/src/state/tasks.rs
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TaskState;
```

- [ ] **Step 9: Expand CSS for shell**

Append to `console/static/styles.css`:

```css
.app-shell {
  display: grid;
  grid-template-columns: 248px minmax(0, 1fr);
  min-height: 100vh;
}

.sidebar {
  padding: 20px;
  border-right: 1px solid rgba(23, 33, 31, 0.12);
  background: rgba(255, 255, 255, 0.68);
}

.brand-lockup {
  display: grid;
  gap: 2px;
  margin-bottom: 24px;
}

.brand-lockup span {
  color: #5d6763;
  font-size: 0.88rem;
}

.sidebar nav {
  display: grid;
  gap: 6px;
}

.sidebar a {
  color: #17211f;
  padding: 9px 10px;
  border-radius: 8px;
  text-decoration: none;
}

.sidebar a:hover {
  background: rgba(23, 33, 31, 0.08);
}

.shell-content {
  min-width: 0;
}

.top-bar {
  display: flex;
  align-items: center;
  min-height: 56px;
  padding: 0 24px;
  border-bottom: 1px solid rgba(23, 33, 31, 0.1);
  background: rgba(246, 244, 239, 0.8);
}

.workspace {
  padding: 24px;
}

.login-panel,
.route-panel {
  max-width: 880px;
}

.form-grid {
  display: grid;
  gap: 14px;
  max-width: 460px;
}

.form-grid label {
  display: grid;
  gap: 6px;
}

.form-grid input,
.form-grid select,
.form-grid textarea {
  min-height: 38px;
  border: 1px solid rgba(23, 33, 31, 0.18);
  border-radius: 8px;
  padding: 7px 10px;
  background: #fffefa;
}

.form-grid button {
  min-height: 38px;
  border: 0;
  border-radius: 8px;
  background: #1f5f4a;
  color: white;
}

@media (max-width: 760px) {
  .app-shell {
    grid-template-columns: 1fr;
  }

  .sidebar {
    border-right: 0;
    border-bottom: 1px solid rgba(23, 33, 31, 0.12);
  }

  .sidebar nav {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
```

- [ ] **Step 10: Run UI tests and check**

Run: `cargo test -p opendaemon-console-ui --test session`

Expected: PASS.

Run: `cargo check -p opendaemon-console-ui`

Expected: PASS, after adjusting for exact Leptos API names if needed.

- [ ] **Step 11: Commit**

Run:

```bash
git add crates/opendaemon-console-ui console/static/styles.css
git commit -m "feat: add leptos console shell"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 5: Resource Routes And Forms

**Files:**

- Modify: `crates/opendaemon-console-ui/src/state/resources.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/overview.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/products.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/providers.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/agents.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/directories.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/permissions.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/settings.rs`
- Modify: `crates/opendaemon-console-ui/src/app.rs`
- Modify: `console/static/styles.css`

- [ ] **Step 1: Add resource state struct**

Replace `crates/opendaemon-console-ui/src/state/resources.rs` with:

```rust
use opendaemon_console_api::dto::{
    AgentProfile, DaemonStatus, DirectoryGrant, PermissionRequest, Product, Provider, RuntimeView,
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ResourceState {
    pub status: Option<DaemonStatus>,
    pub products: Vec<Product>,
    pub providers: Vec<Provider>,
    pub runtimes: Vec<RuntimeView>,
    pub agents: Vec<AgentProfile>,
    pub directories: Vec<DirectoryGrant>,
    pub permissions: Vec<PermissionRequest>,
    pub loading: bool,
    pub error: Option<String>,
}
```

- [ ] **Step 2: Implement Overview route**

Replace `crates/opendaemon-console-ui/src/routes/overview.rs` with:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Overview"</h1>
                <button type="button">"Refresh"</button>
            </div>
            <div class="metric-grid">
                <article><span>"Scheduler"</span><strong>"0 running"</strong></article>
                <article><span>"Runtimes"</span><strong>"Not detected"</strong></article>
                <article><span>"Permissions"</span><strong>"0 pending"</strong></article>
            </div>
        </section>
    }
}
```

- [ ] **Step 3: Implement Products route**

Replace `crates/opendaemon-console-ui/src/routes/products.rs` with:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Products"</h1>
            </div>
            <form class="inline-form">
                <input name="id" placeholder="product_id" />
                <input name="display_name" placeholder="Display name" />
                <button type="submit">"Create"</button>
            </form>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Product"</th><th>"Status"</th><th>"Tokens"</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>
        </section>
    }
}
```

- [ ] **Step 4: Implement Providers route**

Replace `crates/opendaemon-console-ui/src/routes/providers.rs` with:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Providers"</h1>
                <button type="button">"Detect runtimes"</button>
            </div>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Provider"</th><th>"Runtime"</th><th>"Status"</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>
        </section>
    }
}
```

- [ ] **Step 5: Implement Agents route**

Replace `crates/opendaemon-console-ui/src/routes/agents.rs` with a form covering
`id`, `name`, `provider_id`, `model`, `permission_mode`, `instructions`,
`allow_direct_directory`, `custom_args`, `custom_env_keys`, and `mcp_config`.

Use this structure:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Agents"</h1></div>
            <form class="form-grid wide-form">
                <input name="id" placeholder="agent_id" />
                <input name="name" placeholder="Name" />
                <input name="provider_id" placeholder="Provider" />
                <input name="model" placeholder="Model" />
                <select name="permission_mode">
                    <option value="">"Default permissions"</option>
                    <option value="ask">"Ask"</option>
                    <option value="auto">"Auto"</option>
                </select>
                <textarea name="instructions" placeholder="Instructions"></textarea>
                <label class="checkbox-row"><input type="checkbox" name="allow_direct_directory" />"Allow direct directory"</label>
                <input name="custom_args" placeholder="Custom args, comma separated" />
                <input name="custom_env_keys" placeholder="Custom env keys, comma separated" />
                <textarea name="mcp_config" placeholder="MCP config JSON"></textarea>
                <button type="submit">"Save agent"</button>
            </form>
        </section>
    }
}
```

- [ ] **Step 6: Implement Directories route**

Replace `crates/opendaemon-console-ui/src/routes/directories.rs` with a form
covering product, agent, path, capabilities, workspace modes, lock policy,
direct opt-in, and remote execution.

Use this structure:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Directories"</h1></div>
            <form class="form-grid wide-form">
                <input name="product_id" placeholder="Product" />
                <input name="agent_id" placeholder="Agent" />
                <input name="path" placeholder="/absolute/local/path" />
                <input name="capabilities" placeholder="read, write" />
                <fieldset class="choice-row">
                    <label><input type="checkbox" name="workspace_worktree" checked />"Worktree"</label>
                    <label><input type="checkbox" name="workspace_direct" />"Direct"</label>
                </fieldset>
                <select name="default_workspace_mode">
                    <option value="worktree">"Worktree"</option>
                    <option value="direct">"Direct"</option>
                </select>
                <input name="lock_policy" value="exclusive" />
                <label class="checkbox-row"><input type="checkbox" name="direct_mode_task_opt_in" checked />"Require task opt-in for direct mode"</label>
                <label class="checkbox-row"><input type="checkbox" name="allow_remote_execution" />"Allow remote execution"</label>
                <button type="submit">"Save grant"</button>
            </form>
        </section>
    }
}
```

- [ ] **Step 7: Implement Permissions route**

Replace `crates/opendaemon-console-ui/src/routes/permissions.rs` with a table
containing request summary, provider, kind, expiration, approve, deny, and
reason input.

Use this structure:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Permissions"</h1></div>
            <div class="table-shell">
                <table>
                    <thead>
                        <tr>
                            <th>"Request"</th>
                            <th>"Provider"</th>
                            <th>"Kind"</th>
                            <th>"Expires"</th>
                            <th>"Reason"</th>
                            <th>"Decision"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td colspan="6">"No pending permission requests"</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </section>
    }
}
```

- [ ] **Step 8: Implement Settings route**

Replace `crates/opendaemon-console-ui/src/routes/settings.rs` with current base
URL, credential type, product ID, scopes, and sign-out controls.

Use this structure:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Settings"</h1></div>
            <dl class="settings-list">
                <dt>"Base URL"</dt><dd>"-"</dd>
                <dt>"Credential"</dt><dd>"-"</dd>
                <dt>"Product"</dt><dd>"-"</dd>
                <dt>"Scopes"</dt><dd>"-"</dd>
            </dl>
            <button type="button">"Sign out"</button>
        </section>
    }
}
```

- [ ] **Step 9: Wire routes in App**

Update `crates/opendaemon-console-ui/src/app.rs` so it renders the appropriate
route component for the current path. Use `leptos_router` if it compiles cleanly
in this project; otherwise use a minimal browser `window.location.pathname()`
switch for the first migration pass:

```rust
fn route_from_path(path: &str) -> &'static str {
    match path.trim_end_matches('/') {
        "/console/products" => "products",
        "/console/providers" => "providers",
        "/console/agents" => "agents",
        "/console/directories" => "directories",
        "/console/tasks" => "tasks",
        "/console/permissions" => "permissions",
        "/console/settings" => "settings",
        _ => "overview",
    }
}
```

- [ ] **Step 10: Add table and route CSS**

Append route/table CSS to `console/static/styles.css`:

```css
.route-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.metric-grid article,
.table-shell {
  border: 1px solid rgba(23, 33, 31, 0.12);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.72);
}

.metric-grid article {
  display: grid;
  gap: 8px;
  padding: 16px;
}

.table-shell {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
}

th,
td {
  padding: 10px 12px;
  border-bottom: 1px solid rgba(23, 33, 31, 0.1);
  text-align: left;
}

.inline-form {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 16px;
}

.wide-form {
  max-width: 760px;
}

.checkbox-row,
.choice-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-list {
  display: grid;
  grid-template-columns: 140px minmax(0, 1fr);
  gap: 10px 16px;
}
```

- [ ] **Step 11: Run UI check**

Run: `cargo check -p opendaemon-console-ui`

Expected: PASS.

- [ ] **Step 12: Commit**

Run:

```bash
git add crates/opendaemon-console-ui console/static/styles.css
git commit -m "feat: add console resource routes"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 6: Task Route And Transcript State

**Files:**

- Modify: `crates/opendaemon-console-ui/src/state/tasks.rs`
- Modify: `crates/opendaemon-console-ui/src/routes/tasks.rs`
- Create: `crates/opendaemon-console-ui/tests/tasks.rs`
- Modify: `console/static/styles.css`

- [ ] **Step 1: Write transcript state tests**

Create `crates/opendaemon-console-ui/tests/tasks.rs`:

```rust
use opendaemon_console_api::dto::TaskEventView;
use opendaemon_console_ui::state::tasks::TaskTranscript;

fn event(sequence: u64) -> TaskEventView {
    TaskEventView {
        task_id: "task_1".to_owned(),
        sequence,
        r#type: "task.output".to_owned(),
        payload: serde_json::json!({ "text": format!("line {sequence}") }),
        created_at: "2026-06-16T00:00:00Z".to_owned(),
    }
}

#[test]
fn transcript_appends_events_and_tracks_cursor() {
    let mut transcript = TaskTranscript::default();
    transcript.apply(event(1));
    transcript.apply(event(3));
    transcript.apply(event(2));

    assert_eq!(transcript.events.len(), 3);
    assert_eq!(transcript.latest_cursor(), Some(3));
}
```

- [ ] **Step 2: Run failing transcript tests**

Run: `cargo test -p opendaemon-console-ui --test tasks`

Expected: FAIL because `TaskTranscript` is missing.

- [ ] **Step 3: Implement task transcript state**

Replace `crates/opendaemon-console-ui/src/state/tasks.rs` with:

```rust
use opendaemon_console_api::{dto::TaskEventView, events::EventCursor};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TaskTranscript {
    pub events: Vec<TaskEventView>,
    cursor: EventCursor,
}

impl TaskTranscript {
    pub fn apply(&mut self, event: TaskEventView) {
        self.cursor.observe(&event);
        self.events.push(event);
        self.events.sort_by_key(|event| event.sequence);
    }

    #[must_use]
    pub fn latest_cursor(&self) -> Option<u64> {
        self.cursor.latest_sequence()
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TaskState {
    pub active_task_id: Option<String>,
    pub transcript: TaskTranscript,
}
```

- [ ] **Step 4: Implement task route**

Replace `crates/opendaemon-console-ui/src/routes/tasks.rs` with:

```rust
use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="task-layout">
            <div class="route-heading">
                <h1>"Tasks"</h1>
                <button type="button">"Create task"</button>
            </div>
            <div class="task-columns">
                <section class="task-list">
                    <form class="inline-form">
                        <select name="status">
                            <option value="">"All statuses"</option>
                            <option value="queued">"Queued"</option>
                            <option value="running">"Running"</option>
                            <option value="completed">"Completed"</option>
                            <option value="failed">"Failed"</option>
                        </select>
                        <input name="agent_id" placeholder="Agent" />
                        <input name="directory_id" placeholder="Directory" />
                    </form>
                    <div class="table-shell">
                        <table>
                            <thead><tr><th>"Task"</th><th>"Status"</th><th>"Agent"</th></tr></thead>
                            <tbody></tbody>
                        </table>
                    </div>
                </section>
                <aside class="task-detail">
                    <h2>"Task detail"</h2>
                    <dl>
                        <dt>"Workspace"</dt><dd>"-"</dd>
                        <dt>"Provider"</dt><dd>"-"</dd>
                        <dt>"Session"</dt><dd>"-"</dd>
                    </dl>
                    <h3>"Transcript"</h3>
                    <div class="transcript"></div>
                    <h3>"Result"</h3>
                    <pre class="result-block"></pre>
                </aside>
            </div>
        </section>
    }
}
```

- [ ] **Step 5: Add task CSS**

Append to `console/static/styles.css`:

```css
.task-columns {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 420px);
  gap: 16px;
}

.task-detail {
  border: 1px solid rgba(23, 33, 31, 0.12);
  border-radius: 8px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.76);
}

.task-detail dl {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 8px;
}

.transcript,
.result-block {
  min-height: 120px;
  border-radius: 8px;
  padding: 12px;
  overflow: auto;
  background: #17211f;
  color: #ecf5ef;
}

@media (max-width: 980px) {
  .task-columns {
    grid-template-columns: 1fr;
  }
}
```

- [ ] **Step 6: Run transcript tests and UI check**

Run: `cargo test -p opendaemon-console-ui --test tasks`

Expected: PASS.

Run: `cargo check -p opendaemon-console-ui`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/opendaemon-console-ui console/static/styles.css
git commit -m "feat: add console task transcript state"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 7: Daemon Static Asset Serving

**Files:**

- Modify: `src/api/console.rs`
- Modify: `src/tests/console.rs`

- [ ] **Step 1: Add a failing content-type assertion**

Modify `src/tests/console.rs` by extending
`console_static_routes_are_public_without_weakening_api_authentication` with an
assertion that a `.wasm` asset is served as `application/wasm` when present. If
the existing test fixture cannot create `console/dist`, add a small helper that
writes a temporary file only when `console_dist_dir` is injectable; otherwise
test `content_type` directly by making it `pub(crate)`.

Expected test intent:

```rust
assert_eq!(
    crate::api::console::content_type(std::path::Path::new("app.wasm")),
    "application/wasm"
);
```

- [ ] **Step 2: Run the failing console test**

Run:
`cargo test console_static_routes_are_public_without_weakening_api_authentication --quiet`

Expected: FAIL because `.wasm` content type is not handled or `content_type` is
not visible to tests.

- [ ] **Step 3: Update content types**

Modify `src/api/console.rs`:

```rust
pub(crate) fn content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}
```

Keep `sanitize_path` and fallback behavior unchanged.

- [ ] **Step 4: Run daemon console tests**

Run: `cargo test console --quiet`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/api/console.rs src/tests/console.rs
git commit -m "fix: serve trunk console assets"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 8: Remove React/Vite Console

**Files:**

- Delete: `console/src/**`
- Delete: `console/package.json`
- Delete: `console/pnpm-lock.yaml`
- Delete: `console/pnpm-workspace.yaml`
- Delete: `console/vite.config.ts`
- Delete: `console/tsconfig.json`

- [ ] **Step 1: Confirm the Rust console builds before deletion**

Run: `cargo check --workspace`

Expected: PASS.

Run: `trunk build console/index.html --dist console/dist`

Expected: PASS and writes `console/dist`.

- [ ] **Step 2: Delete React/Vite files**

Delete these paths with non-interactive filesystem operations:

```text
console/src
console/package.json
console/pnpm-lock.yaml
console/pnpm-workspace.yaml
console/vite.config.ts
console/tsconfig.json
```

Do not delete `console/index.html`, `console/Trunk.toml`, `console/static`, or
`console/dist`.

- [ ] **Step 3: Verify no React or TypeScript Console files remain**

Run: `rg --files console`

Expected: output contains only Trunk files, static files, and `dist` outputs.

Run: `rg -n "react|vite|typescript|tsx|pnpm" console Cargo.toml crates`

Expected: no hits related to active Console implementation. Mentions in
generated assets or documentation should be evaluated and removed only if they
are part of the old Console stack.

- [ ] **Step 4: Run checks**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add console Cargo.toml crates
git add -u console
git commit -m "chore: remove react console implementation"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Task 9: Final Gates And Browser Smoke

**Files:**

- Modify only files directly implicated by failures found by gates.

- [ ] **Step 1: Format Rust code**

Run: `cargo fmt --all`

Expected: exits successfully with no manual formatting needed afterward.

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`

Expected: PASS.

- [ ] **Step 3: Run tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 4: Build the Console**

Run: `trunk build console/index.html --dist console/dist`

Expected: PASS and produces `.html`, `.js`, `.wasm`, and `.css` assets under
`console/dist`.

- [ ] **Step 5: Run daemon console test**

Run: `cargo test console --quiet`

Expected: PASS.

- [ ] **Step 6: Start a local daemon for smoke testing**

Run: `cargo run -- --help`

Expected: output shows the CLI shape. Use the existing project command for
starting the API server from `src/cli/mod.rs`; do not invent flags.

After identifying the correct command, run it on a free local port and open
`/console` in the in-app browser.

Expected browser smoke:

- `/console` loads the Rust Console shell.
- `/console/tasks` deep link loads without server 404.
- Console static files do not require API tokens.
- API calls still require bearer tokens.

- [ ] **Step 7: Scan for old stack remnants**

Run:
`rg -n "React|Vite|pnpm|typescript|tsx|lucide-react" . --glob '!target/**' --glob '!console/dist/**'`

Expected: no active implementation references remain. Documentation references
are acceptable only if they describe removed history or non-goals.

- [ ] **Step 8: Final status**

Run: `git status --short`

Expected: only intentional migration files are modified or deleted.

- [ ] **Step 9: Commit final fixes**

Run:

```bash
git add .
git commit -m "feat: migrate console to rust web ui"
```

Expected: commit succeeds, unless commit steps are skipped by repository
preference.

---

## Self-Review

- Spec coverage: the plan covers Rust workspace structure, shared API crate,
  Leptos UI crate, Trunk entrypoint, daemon static serving, React deletion,
  API/SSE/policy tests, UI state tests, and final quality gates.
- Scope: desktop/Tauri remains out of scope, matching the approved design.
- Placeholder scan: no `TBD`, `TODO`, or intentionally vague task remains. HTML
  `placeholder` attributes are used only as UI input hints, not plan
  placeholders.
- Type consistency: shared DTO names match the approved design and the current
  TypeScript DTO names, with Rust enum casing handled through Serde.
