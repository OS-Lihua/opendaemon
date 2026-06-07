use std::collections::BTreeSet;

use regex::Regex;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Product {
    pub id: String,
    pub display_name: String,
    pub status: ProductStatus,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProduct {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchProduct {
    pub display_name: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<ProductStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiScope {
    #[serde(rename = "providers:read")]
    ProvidersRead,
    #[serde(rename = "runtimes:read")]
    RuntimesRead,
    #[serde(rename = "agents:read")]
    AgentsRead,
    #[serde(rename = "agents:write")]
    AgentsWrite,
    #[serde(rename = "directories:read")]
    DirectoriesRead,
    #[serde(rename = "directories:grant")]
    DirectoriesGrant,
    #[serde(rename = "directories:direct")]
    DirectoriesDirect,
    #[serde(rename = "tasks:create")]
    TasksCreate,
    #[serde(rename = "tasks:read")]
    TasksRead,
    #[serde(rename = "tasks:cancel")]
    TasksCancel,
    #[serde(rename = "tasks:remote_execution")]
    TasksRemoteExecution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductTokenStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProductToken {
    pub id: String,
    pub product_id: String,
    pub label: String,
    pub scopes: Vec<ApiScope>,
    pub token_prefix: String,
    pub status: ProductTokenStatus,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProductToken {
    pub label: String,
    pub scopes: Vec<ApiScope>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreatedProductToken {
    pub id: String,
    pub product_id: String,
    pub label: String,
    pub scopes: Vec<ApiScope>,
    pub token_prefix: String,
    pub token: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedProductToken {
    pub token_id: String,
    pub product_id: String,
    pub scopes: BTreeSet<ApiScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductError {
    ProductNotFound,
    ProductAlreadyExists,
    InvalidProduct,
    InvalidProductId,
    InvalidProductToken,
}

impl ProductError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProductNotFound => "product_not_found",
            Self::ProductAlreadyExists => "product_already_exists",
            Self::InvalidProduct => "invalid_product",
            Self::InvalidProductId => "invalid_product_id",
            Self::InvalidProductToken => "invalid_product_token",
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::ProductNotFound => "product not found",
            Self::ProductAlreadyExists => "product already exists",
            Self::InvalidProduct => "invalid product",
            Self::InvalidProductId => "invalid product id",
            Self::InvalidProductToken => "invalid product token",
        }
    }
}

impl Product {
    pub fn validate_id(id: &str) -> Result<(), ProductError> {
        let id_pattern =
            Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$").expect("valid product id regex");
        if id_pattern.is_match(id) {
            Ok(())
        } else {
            Err(ProductError::InvalidProductId)
        }
    }
}

impl CreateProduct {
    pub fn validate(&self) -> Result<(), ProductError> {
        Product::validate_id(&self.id)?;
        validate_required_string(&self.display_name)?;
        if let Some(description) = &self.description {
            validate_optional_string(description)?;
        }
        Ok(())
    }
}

impl PatchProduct {
    pub fn validate(&self) -> Result<(), ProductError> {
        if self.display_name.is_none() && self.description.is_none() && self.status.is_none() {
            return Err(ProductError::InvalidProduct);
        }
        if let Some(display_name) = &self.display_name {
            validate_required_string(display_name)?;
        }
        if let Some(Some(description)) = &self.description {
            validate_optional_string(description)?;
        }
        Ok(())
    }
}

impl CreateProductToken {
    pub fn validate(&self) -> Result<(), ProductError> {
        validate_required_string(&self.label)?;
        normalize_scopes(&self.scopes).map(|_| ())
    }
}

pub fn normalize_scopes(scopes: &[ApiScope]) -> Result<Vec<ApiScope>, ProductError> {
    let scopes = scopes.iter().copied().collect::<BTreeSet<_>>();
    if scopes.is_empty() {
        return Err(ProductError::InvalidProductToken);
    }
    Ok(scopes.into_iter().collect())
}

pub fn now_rfc3339() -> Result<String, ProductError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ProductError::InvalidProduct)
}

fn validate_required_string(value: &str) -> Result<(), ProductError> {
    if value.trim().is_empty() {
        Err(ProductError::InvalidProduct)
    } else {
        Ok(())
    }
}

fn validate_optional_string(value: &str) -> Result<(), ProductError> {
    if value.contains('\0') {
        Err(ProductError::InvalidProduct)
    } else {
        Ok(())
    }
}
