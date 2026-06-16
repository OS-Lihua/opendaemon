use axum::Json;
use serde::Serialize;

use crate::product::{ApiScope, ProductStatus};

use super::{
    ProductAuthContext,
    auth::{AnyAuth, AuthContext},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SessionResponse {
    pub credential_type: CredentialType,
    pub product_id: Option<String>,
    pub scopes: Vec<ApiScope>,
    pub product_status: Option<ProductStatus>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    Bootstrap,
    Product,
}

pub async fn get(auth: AnyAuth) -> Json<SessionResponse> {
    Json(match auth.0 {
        AuthContext::Bootstrap => SessionResponse {
            credential_type: CredentialType::Bootstrap,
            product_id: None,
            scopes: Vec::new(),
            product_status: None,
        },
        AuthContext::Product(ProductAuthContext {
            product_id, scopes, ..
        }) => SessionResponse {
            credential_type: CredentialType::Product,
            product_id: Some(product_id),
            scopes: scopes.into_iter().collect(),
            product_status: Some(ProductStatus::Active),
        },
    })
}
