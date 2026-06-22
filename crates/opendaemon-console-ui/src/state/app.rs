use leptos::prelude::*;
use opendaemon_console_api::{
    ConsoleApiClient,
    dto::{
        AgentProfile, AgentProfileFormPayload, CreateProductPayload, CreateProductTokenPayload,
        CreatedProductToken, DaemonStatus, DirectoryGrant, DirectoryGrantFormPayload,
        ExecutionPolicy, PermissionDecision, PermissionRequest, Product, Provider, ProviderConfig,
        RuntimeView, Session, Task, TaskCreatePayload, WorkspaceMode,
    },
};
use serde_json::Value;
use wasm_bindgen::JsCast;

use crate::state::session::{StoredSession, storage_key};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthState {
    pub stored: StoredSession,
    pub session: Session,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ConsoleResources {
    pub status: Option<DaemonStatus>,
    pub products: Vec<Product>,
    pub providers: Vec<Provider>,
    pub runtimes: Vec<RuntimeView>,
    pub agents: Vec<AgentProfile>,
    pub directories: Vec<DirectoryGrant>,
    pub tasks: Vec<Task>,
    pub permissions: Vec<PermissionRequest>,
}

#[derive(Clone)]
pub struct AppState {
    pub auth: RwSignal<Option<AuthState>>,
    pub resources: RwSignal<ConsoleResources>,
    pub active_task_id: RwSignal<Option<String>>,
    pub loading: RwSignal<bool>,
    pub notice: RwSignal<Option<String>>,
    pub error: RwSignal<Option<String>>,
    pub created_token: RwSignal<Option<CreatedProductToken>>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            auth: RwSignal::new(None),
            resources: RwSignal::new(ConsoleResources::default()),
            active_task_id: RwSignal::new(None),
            loading: RwSignal::new(false),
            notice: RwSignal::new(None),
            error: RwSignal::new(None),
            created_token: RwSignal::new(None),
        }
    }

    #[must_use]
    pub fn client(&self) -> Option<ConsoleApiClient> {
        self.auth.with(|auth| {
            auth.as_ref()
                .map(|auth| ConsoleApiClient::new(&auth.stored.base_url, &auth.stored.bearer_token))
        })
    }

    pub fn set_error(&self, message: impl Into<String>) {
        self.error.set(Some(message.into()));
        self.notice.set(None);
    }

    pub fn set_notice(&self, message: impl Into<String>) {
        self.notice.set(Some(message.into()));
        self.error.set(None);
    }

    pub fn sign_out(&self) {
        remove_stored_session();
        self.auth.set(None);
        self.resources.set(ConsoleResources::default());
        self.active_task_id.set(None);
        self.created_token.set(None);
        self.set_notice("Signed out");
    }

    pub fn bootstrap_from_storage(&self) {
        let Some(stored) = load_stored_session() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            let client = ConsoleApiClient::new(&stored.base_url, &stored.bearer_token);
            match client.session().await {
                Ok(session) => {
                    state.active_task_id.set(stored.active_task_id.clone());
                    state.auth.set(Some(AuthState { stored, session }));
                    state.refresh_all().await;
                }
                Err(error) => {
                    remove_stored_session();
                    state.set_error(format!("Stored session expired: {error}"));
                }
            }
        });
    }

    pub fn connect(&self, base_url: String, credential_mode: String, bearer_token: String) {
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            state.error.set(None);
            state.notice.set(None);
            let stored = StoredSession {
                base_url: normalize_base_url(&base_url),
                credential_mode,
                bearer_token,
                last_route: current_route(),
                active_task_id: None,
            };
            let client = ConsoleApiClient::new(&stored.base_url, &stored.bearer_token);
            match client.session().await {
                Ok(session) => {
                    save_stored_session(&stored);
                    state.auth.set(Some(AuthState { stored, session }));
                    state.set_notice("Connected");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Login failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub async fn refresh_all(&self) {
        let Some(client) = self.client() else {
            return;
        };
        self.loading.set(true);
        self.error.set(None);
        let mut resources = ConsoleResources::default();
        let mut errors = Vec::new();
        let session = self
            .auth
            .with(|auth| auth.as_ref().map(|auth| auth.session.clone()));
        let is_bootstrap = session
            .as_ref()
            .is_some_and(|session| session.credential_type == "bootstrap");

        match client.daemon_status().await {
            Ok(value) => resources.status = Some(value),
            Err(error) => errors.push(format!("status: {error}")),
        }
        if is_bootstrap {
            match client.products().await {
                Ok(value) => resources.products = value,
                Err(error) => errors.push(format!("products: {error}")),
            }
        }
        match client.providers().await {
            Ok(value) => resources.providers = value,
            Err(error) => errors.push(format!("providers: {error}")),
        }
        match client.runtimes().await {
            Ok(value) => resources.runtimes = value,
            Err(error) => errors.push(format!("runtimes: {error}")),
        }
        if !is_bootstrap {
            match client.agents().await {
                Ok(value) => resources.agents = value,
                Err(error) => errors.push(format!("agents: {error}")),
            }
            match client.directories().await {
                Ok(value) => resources.directories = value,
                Err(error) => errors.push(format!("directories: {error}")),
            }
            match client.tasks().await {
                Ok(value) => resources.tasks = value,
                Err(error) => errors.push(format!("tasks: {error}")),
            }
            match client.permissions().await {
                Ok(value) => resources.permissions = value,
                Err(error) => errors.push(format!("permissions: {error}")),
            }
        }

        self.resources.set(resources);
        if errors.is_empty() {
            self.set_notice("Refreshed");
        } else {
            self.set_error(errors.join("; "));
        }
        self.loading.set(false);
    }

    pub fn refresh(&self) {
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.refresh_all().await;
        });
    }

    pub fn create_product(&self, payload: CreateProductPayload) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.create_product(&payload).await {
                Ok(_) => {
                    state.set_notice("Product created");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Create product failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn create_product_token(&self, product_id: String, label: String, scopes: Vec<String>) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            let payload = CreateProductTokenPayload { label, scopes };
            match client.create_product_token(&product_id, &payload).await {
                Ok(token) => {
                    state.created_token.set(Some(token));
                    state.set_notice("Product token created");
                }
                Err(error) => state.set_error(format!("Create token failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn detect_runtimes(&self) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.detect_runtimes().await {
                Ok(runtimes) => {
                    state
                        .resources
                        .update(|resources| resources.runtimes = runtimes);
                    state.set_notice("Runtime detection finished");
                }
                Err(error) => state.set_error(format!("Detect runtimes failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn create_agent(&self, payload: AgentProfileFormPayload) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.create_agent(&payload).await {
                Ok(_) => {
                    state.set_notice("Agent saved");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Save agent failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn create_directory(&self, payload: DirectoryGrantFormPayload) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.create_directory(&payload).await {
                Ok(_) => {
                    state.set_notice("Directory grant saved");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Save directory failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn create_task(&self, payload: TaskCreatePayload) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.create_task(&payload).await {
                Ok(task) => {
                    state.active_task_id.set(Some(task.id));
                    state.set_notice("Task created");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Create task failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn cancel_task(&self, task_id: String) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client.cancel_task(&task_id).await {
                Ok(task) => {
                    state.active_task_id.set(Some(task.id));
                    state.set_notice("Task cancelled");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Cancel task failed: {error}")),
            }
            state.loading.set(false);
        });
    }

    pub fn respond_to_permission(
        &self,
        task_id: String,
        request_id: String,
        decision: PermissionDecision,
        reason: Option<String>,
    ) {
        let Some(client) = self.client() else {
            return;
        };
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.loading.set(true);
            match client
                .respond_to_permission(&task_id, &request_id, decision, reason)
                .await
            {
                Ok(task) => {
                    state.active_task_id.set(Some(task.id));
                    state.set_notice("Permission response sent");
                    state.refresh_all().await;
                }
                Err(error) => state.set_error(format!("Permission response failed: {error}")),
            }
            state.loading.set(false);
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn use_app_state() -> AppState {
    use_context::<AppState>().expect("AppState context missing")
}

#[must_use]
pub fn input_value(event: leptos::ev::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

#[must_use]
pub fn textarea_value(event: leptos::ev::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

#[must_use]
pub fn select_value(event: leptos::ev::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

#[must_use]
pub fn checkbox_checked(event: leptos::ev::Event) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.checked())
        .unwrap_or(false)
}

#[must_use]
pub fn csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[must_use]
pub fn workspace_mode(value: &str) -> WorkspaceMode {
    if value == "direct" {
        WorkspaceMode::Direct
    } else {
        WorkspaceMode::Worktree
    }
}

#[must_use]
pub fn optional_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

#[must_use]
pub fn optional_json(value: &str) -> Option<Value> {
    optional_string(value).and_then(|value| serde_json::from_str(&value).ok())
}

#[must_use]
pub fn default_execution_policy(allow_direct_directory: bool) -> ExecutionPolicy {
    ExecutionPolicy {
        default_workspace_mode: if allow_direct_directory {
            WorkspaceMode::Direct
        } else {
            WorkspaceMode::Worktree
        },
        allow_direct_directory,
    }
}

#[must_use]
pub fn provider_config(
    permission_mode: String,
    custom_args: String,
    custom_env_keys: String,
    mcp_config: String,
) -> ProviderConfig {
    ProviderConfig {
        permission_mode: optional_string(&permission_mode),
        custom_args: csv(&custom_args),
        custom_env_keys: csv(&custom_env_keys),
        mcp_config: optional_json(&mcp_config),
    }
}

fn normalize_base_url(input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        "http://127.0.0.1:19514".to_owned()
    } else {
        input.trim_end_matches('/').to_owned()
    }
}

fn current_route() -> String {
    web_sys::window()
        .and_then(|window| window.location().pathname().ok())
        .unwrap_or_else(|| "/console/".to_owned())
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok())
        .flatten()
}

fn load_stored_session() -> Option<StoredSession> {
    storage()
        .and_then(|storage| storage.get_item(storage_key()).ok())
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
}

fn save_stored_session(session: &StoredSession) {
    if let (Some(storage), Ok(value)) = (storage(), serde_json::to_string(session)) {
        let _ = storage.set_item(storage_key(), &value);
    }
}

fn remove_stored_session() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(storage_key());
    }
}
