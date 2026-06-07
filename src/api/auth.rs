use std::collections::BTreeSet;

use axum::{
    Json,
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};

use crate::{
    config::AuthConfig,
    product::{ApiScope, AuthenticatedProductToken},
    store::products::ProductStoreError,
};

use super::{AppState, ErrorBody, ErrorResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthContext {
    Bootstrap,
    Product(ProductAuthContext),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductAuthContext {
    pub token_id: String,
    pub product_id: String,
    pub scopes: BTreeSet<ApiScope>,
}

#[derive(Debug)]
pub enum AuthError {
    MissingAuthentication,
    InvalidToken,
    BootstrapTokenRequired,
    ProductTokenRequired,
    InsufficientScope,
    ProductScopeMismatch,
    ProductDisabled,
}

impl AuthError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingAuthentication => "missing_authentication",
            Self::InvalidToken => "invalid_token",
            Self::BootstrapTokenRequired => "bootstrap_token_required",
            Self::ProductTokenRequired => "product_token_required",
            Self::InsufficientScope => "insufficient_scope",
            Self::ProductScopeMismatch => "product_scope_mismatch",
            Self::ProductDisabled => "product_disabled",
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::MissingAuthentication => "missing authentication",
            Self::InvalidToken => "invalid token",
            Self::BootstrapTokenRequired => "bootstrap token required",
            Self::ProductTokenRequired => "product token required",
            Self::InsufficientScope => "insufficient scope",
            Self::ProductScopeMismatch => "product scope mismatch",
            Self::ProductDisabled => "product disabled",
        }
    }

    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::MissingAuthentication
            | Self::InvalidToken
            | Self::BootstrapTokenRequired
            | Self::ProductTokenRequired
            | Self::ProductDisabled => StatusCode::UNAUTHORIZED,
            Self::InsufficientScope | Self::ProductScopeMismatch => StatusCode::FORBIDDEN,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            self.status(),
            Json(ErrorResponse {
                error: ErrorBody {
                    code: self.code(),
                    message: self.message().to_owned(),
                },
            }),
        )
            .into_response()
    }
}

#[derive(Clone)]
pub struct BootstrapAuth;

#[derive(Clone)]
pub struct ProductAuth(pub ProductAuthContext);

impl ProductAuth {
    pub fn require_scope(&self, scope: ApiScope) -> Result<(), AuthError> {
        if self.0.scopes.contains(&scope) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope)
        }
    }

    pub fn require_scopes(&self, scopes: &[ApiScope]) -> Result<(), AuthError> {
        if scopes.iter().all(|scope| self.0.scopes.contains(scope)) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope)
        }
    }

    pub fn ensure_product(&self, product_id: &str) -> Result<(), AuthError> {
        if self.0.product_id == product_id {
            Ok(())
        } else {
            Err(AuthError::ProductScopeMismatch)
        }
    }

    #[must_use]
    pub fn product_id(&self) -> &str {
        &self.0.product_id
    }
}

impl<S> FromRequestParts<S> for BootstrapAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match authenticate_request(parts, &AppState::from_ref(state))? {
            AuthContext::Bootstrap => Ok(Self),
            AuthContext::Product(_) => Err(AuthError::BootstrapTokenRequired),
        }
    }
}

impl<S> FromRequestParts<S> for ProductAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match authenticate_request(parts, &AppState::from_ref(state))? {
            AuthContext::Bootstrap => Err(AuthError::ProductTokenRequired),
            AuthContext::Product(context) => Ok(Self(context)),
        }
    }
}

fn authenticate_request(parts: &Parts, state: &AppState) -> Result<AuthContext, AuthError> {
    let token = bearer_token(parts)?;
    if is_bootstrap_token(&state.auth_config, token) {
        return Ok(AuthContext::Bootstrap);
    }

    match state.product_store.authenticate_product_token(token) {
        Ok(Some(AuthenticatedProductToken {
            token_id,
            product_id,
            scopes,
        })) => Ok(AuthContext::Product(ProductAuthContext {
            token_id,
            product_id,
            scopes,
        })),
        Ok(None) => Err(AuthError::InvalidToken),
        Err(ProductStoreError::ProductDisabled) => Err(AuthError::ProductDisabled),
        Err(ProductStoreError::Product(_)) | Err(ProductStoreError::Store(_)) => {
            Err(AuthError::InvalidToken)
        }
    }
}

fn bearer_token(parts: &Parts) -> Result<&str, AuthError> {
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .ok_or(AuthError::MissingAuthentication)?;
    let value = header.to_str().map_err(|_| AuthError::InvalidToken)?;
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or(AuthError::InvalidToken)
}

fn is_bootstrap_token(config: &AuthConfig, token: &str) -> bool {
    config
        .bootstrap_token
        .as_deref()
        .is_some_and(|value| value == token)
}
