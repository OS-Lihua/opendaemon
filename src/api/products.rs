use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
    product::{
        CreateProduct, CreateProductToken, CreatedProductToken, PatchProduct, Product,
        ProductError, ProductStatus, ProductToken,
    },
    store::products::ProductStoreError,
};

use super::{AppState, ErrorBody, ErrorResponse, auth::BootstrapAuth};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProductListResponse {
    pub products: Vec<Product>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SingleProductResponse {
    pub product: Product,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProductTokenListResponse {
    pub tokens: Vec<ProductToken>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CreatedProductTokenResponse {
    pub token: CreatedProductToken,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchProductRequest {
    pub display_name: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<ProductStatus>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductTokenRequest {
    pub label: String,
    pub scopes: Vec<crate::product::ApiScope>,
}

pub async fn list(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
) -> Result<Json<ProductListResponse>, ApiError> {
    let products = state.product_store.list_products()?;
    Ok(Json(ProductListResponse { products }))
}

pub async fn create(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateProductRequest>,
) -> Result<(StatusCode, Json<SingleProductResponse>), ApiError> {
    let product = state.product_store.create_product(CreateProduct {
        id: request.id,
        display_name: request.display_name,
        description: request.description,
    })?;
    Ok((StatusCode::CREATED, Json(SingleProductResponse { product })))
}

pub async fn get(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Path(product_id): Path<String>,
) -> Result<Json<SingleProductResponse>, ApiError> {
    let product = state.product_store.get_product(&product_id)?;
    Ok(Json(SingleProductResponse { product }))
}

pub async fn patch(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Path(product_id): Path<String>,
    Json(request): Json<PatchProductRequest>,
) -> Result<Json<SingleProductResponse>, ApiError> {
    let product = state.product_store.patch_product(
        &product_id,
        PatchProduct {
            display_name: request.display_name,
            description: request.description,
            status: request.status,
        },
    )?;
    Ok(Json(SingleProductResponse { product }))
}

pub async fn list_tokens(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Path(product_id): Path<String>,
) -> Result<Json<ProductTokenListResponse>, ApiError> {
    let tokens = state.product_store.list_tokens(&product_id)?;
    Ok(Json(ProductTokenListResponse { tokens }))
}

pub async fn create_token(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Path(product_id): Path<String>,
    Json(request): Json<CreateProductTokenRequest>,
) -> Result<(StatusCode, Json<CreatedProductTokenResponse>), ApiError> {
    let token = state.product_store.create_token(
        &product_id,
        CreateProductToken {
            label: request.label,
            scopes: request.scopes,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedProductTokenResponse { token }),
    ))
}

pub async fn revoke_token(
    _auth: BootstrapAuth,
    State(state): State<AppState>,
    Path((product_id, token_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state.product_store.revoke_token(&product_id, &token_id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug)]
pub enum ApiError {
    Store(ProductStoreError),
}

impl From<ProductStoreError> for ApiError {
    fn from(error: ProductStoreError) -> Self {
        Self::Store(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Store(ProductStoreError::Product(error)) => (
                status_for_product_error(error),
                error.code(),
                error.message().to_owned(),
            ),
            Self::Store(ProductStoreError::ProductDisabled) => (
                StatusCode::UNAUTHORIZED,
                "product_disabled",
                "product disabled".to_owned(),
            ),
            Self::Store(ProductStoreError::Store(error)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_error",
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

fn status_for_product_error(error: ProductError) -> StatusCode {
    match error {
        ProductError::ProductNotFound => StatusCode::NOT_FOUND,
        ProductError::ProductAlreadyExists => StatusCode::BAD_REQUEST,
        ProductError::InvalidProduct
        | ProductError::InvalidProductId
        | ProductError::InvalidProductToken => StatusCode::BAD_REQUEST,
    }
}
