use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agent::profile::{
        AgentProfile, AgentProfileError, CreateAgentProfile, ExecutionPolicy, ProviderConfig,
    },
    store::agent_profiles::{AgentProfileFilters, AgentStoreError, PatchAgentProfile},
};

use super::{AppState, ErrorBody, ErrorResponse};

pub type AgentResponse = AgentProfile;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AgentListResponse {
    pub agents: Vec<AgentResponse>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SingleAgentResponse {
    pub agent: AgentResponse,
}

#[derive(Debug, Deserialize)]
pub struct AgentListQuery {
    pub owner_product_id: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub id: String,
    pub name: String,
    pub owner_product_id: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub execution_policy: ExecutionPolicy,
    #[serde(default)]
    pub provider_config: ProviderConfig,
}

#[derive(Debug, Deserialize)]
pub struct PatchAgentRequest {
    pub name: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<Option<String>>,
    pub execution_policy: Option<ExecutionPolicy>,
    pub provider_config: Option<ProviderConfig>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> Result<Json<AgentListResponse>, ApiError> {
    let agents = state.agent_profile_store().list(AgentProfileFilters {
        owner_product_id: query.owner_product_id,
        provider_id: query.provider_id,
    })?;

    Ok(Json(AgentListResponse { agents }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<(StatusCode, Json<SingleAgentResponse>), ApiError> {
    let input = CreateAgentProfile::from(request);
    validate_against_registry(&state, &input)?;
    let agent = state.agent_profile_store().create(input)?;

    Ok((StatusCode::CREATED, Json(SingleAgentResponse { agent })))
}

pub async fn get(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<SingleAgentResponse>, ApiError> {
    let agent = state.agent_profile_store().get(&agent_id)?;

    Ok(Json(SingleAgentResponse { agent }))
}

pub async fn patch(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<SingleAgentResponse>, ApiError> {
    reject_immutable_fields(&body)?;
    let request: PatchAgentRequest = serde_json::from_value(body)
        .map_err(|_| ApiError::Profile(AgentProfileError::InvalidAgentProfile))?;
    let current = state.agent_profile_store().get(&agent_id)?;
    let validation_input = CreateAgentProfile {
        id: current.id.clone(),
        name: request.name.clone().unwrap_or_else(|| current.name.clone()),
        owner_product_id: current.owner_product_id.clone(),
        provider_id: request
            .provider_id
            .clone()
            .unwrap_or_else(|| current.provider_id.clone()),
        model: request
            .model
            .clone()
            .unwrap_or_else(|| current.model.clone()),
        instructions: request
            .instructions
            .clone()
            .unwrap_or_else(|| current.instructions.clone()),
        execution_policy: request
            .execution_policy
            .clone()
            .unwrap_or_else(|| current.execution_policy.clone()),
        provider_config: request
            .provider_config
            .clone()
            .unwrap_or_else(|| current.provider_config.clone()),
    };
    validate_against_registry(&state, &validation_input)?;
    let agent = state
        .agent_profile_store()
        .patch(&agent_id, PatchAgentProfile::from(request))?;

    Ok(Json(SingleAgentResponse { agent }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.agent_profile_store().delete(&agent_id)?;

    Ok(StatusCode::NO_CONTENT)
}

fn validate_against_registry(state: &AppState, input: &CreateAgentProfile) -> Result<(), ApiError> {
    let registry = state.load_registry().map_err(ApiError::Registry)?;
    input
        .validate_against_registry(&registry)
        .map_err(ApiError::Profile)
}

fn reject_immutable_fields(body: &Value) -> Result<(), ApiError> {
    let Some(object) = body.as_object() else {
        return Err(ApiError::Profile(AgentProfileError::InvalidAgentProfile));
    };
    if object.is_empty()
        || object.keys().any(|key| {
            matches!(
                key.as_str(),
                "id" | "owner_product_id" | "created_at" | "updated_at"
            )
        })
    {
        return Err(ApiError::Profile(AgentProfileError::InvalidAgentProfile));
    }
    Ok(())
}

impl From<CreateAgentRequest> for CreateAgentProfile {
    fn from(request: CreateAgentRequest) -> Self {
        Self {
            id: request.id,
            name: request.name,
            owner_product_id: request.owner_product_id,
            provider_id: request.provider_id,
            model: request.model,
            instructions: request.instructions,
            execution_policy: request.execution_policy,
            provider_config: request.provider_config,
        }
    }
}

impl From<PatchAgentRequest> for PatchAgentProfile {
    fn from(request: PatchAgentRequest) -> Self {
        Self {
            name: request.name,
            provider_id: request.provider_id,
            model: request.model,
            instructions: request.instructions,
            execution_policy: request.execution_policy,
            provider_config: request.provider_config,
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    Profile(AgentProfileError),
    Store(anyhow::Error),
    Registry(anyhow::Error),
}

impl From<AgentStoreError> for ApiError {
    fn from(error: AgentStoreError) -> Self {
        match error {
            AgentStoreError::Profile(error) => Self::Profile(error),
            AgentStoreError::Store(error) => Self::Store(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Profile(error) => (
                status_for_profile_error(&error),
                error.code(),
                error.message().to_owned(),
            ),
            Self::Store(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
                error.to_string(),
            ),
            Self::Registry(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "registry_error",
                error.to_string(),
            ),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

fn status_for_profile_error(error: &AgentProfileError) -> StatusCode {
    match error {
        AgentProfileError::AgentNotFound => StatusCode::NOT_FOUND,
        AgentProfileError::AgentAuthorizationFailed => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_REQUEST,
    }
}
