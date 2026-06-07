use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use rand::random;
use rusqlite::{Connection, OptionalExtension, Row, params};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    config::StoreConfig,
    product::{
        ApiScope, AuthenticatedProductToken, CreateProduct, CreateProductToken,
        CreatedProductToken, PatchProduct, Product, ProductError, ProductStatus, ProductToken,
        ProductTokenStatus, normalize_scopes, now_rfc3339,
    },
};

use super::sqlite;

const TOKEN_PREFIX_LEN: usize = 9;
const PRODUCT_TOKEN_PREFIX: &str = "odpk_";

#[derive(Debug, Clone)]
pub struct ProductStore {
    sqlite_path: Arc<PathBuf>,
}

#[derive(Debug)]
pub enum ProductStoreError {
    Product(ProductError),
    ProductDisabled,
    Store(anyhow::Error),
}

impl ProductStore {
    #[must_use]
    pub fn configured(config: StoreConfig) -> Self {
        Self {
            sqlite_path: Arc::new(config.sqlite_path),
        }
    }

    pub fn open(config: StoreConfig) -> Result<Self, ProductStoreError> {
        sqlite::open_connection(&config.sqlite_path).map_err(store_error)?;
        Ok(Self::configured(config))
    }

    pub fn create_product(&self, input: CreateProduct) -> Result<Product, ProductStoreError> {
        input.validate().map_err(ProductStoreError::Product)?;
        let now = now_rfc3339().map_err(ProductStoreError::Product)?;
        let product = Product {
            id: input.id,
            display_name: input.display_name,
            status: ProductStatus::Active,
            description: input.description,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "INSERT INTO products (
                    id,
                    display_name,
                    status,
                    description,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    product.id,
                    product.display_name,
                    serialize_json(&product.status)?,
                    product.description,
                    product.created_at,
                    product.updated_at,
                ],
            )
            .map_err(map_sqlite_product_error)?;
        transaction.commit().map_err(store_error)?;
        self.get_product(&product.id)
    }

    pub fn list_products(&self) -> Result<Vec<Product>, ProductStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT * FROM products ORDER BY rowid ASC")
            .map_err(store_error)?;
        let products = statement
            .query_map([], row_to_product)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?;
        Ok(products)
    }

    pub fn get_product(&self, product_id: &str) -> Result<Product, ProductStoreError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT * FROM products WHERE id = ?1",
                params![product_id],
                row_to_product,
            )
            .optional()
            .map_err(store_error)?
            .ok_or(ProductStoreError::Product(ProductError::ProductNotFound))
    }

    pub fn patch_product(
        &self,
        product_id: &str,
        patch: PatchProduct,
    ) -> Result<Product, ProductStoreError> {
        patch.validate().map_err(ProductStoreError::Product)?;
        let current = self.get_product(product_id)?;
        let updated = Product {
            id: current.id,
            display_name: patch.display_name.unwrap_or(current.display_name),
            status: patch.status.unwrap_or(current.status),
            description: patch.description.unwrap_or(current.description),
            created_at: current.created_at,
            updated_at: now_rfc3339().map_err(ProductStoreError::Product)?,
        };
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "UPDATE products
                 SET display_name = ?1,
                     status = ?2,
                     description = ?3,
                     updated_at = ?4
                 WHERE id = ?5",
                params![
                    updated.display_name,
                    serialize_json(&updated.status)?,
                    updated.description,
                    updated.updated_at,
                    product_id,
                ],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        self.get_product(product_id)
    }

    pub fn list_tokens(&self, product_id: &str) -> Result<Vec<ProductToken>, ProductStoreError> {
        self.get_product(product_id)?;
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT * FROM product_tokens
                 WHERE product_id = ?1
                 ORDER BY rowid ASC",
            )
            .map_err(store_error)?;
        let tokens = statement
            .query_map(params![product_id], row_to_token)
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?;
        Ok(tokens)
    }

    pub fn create_token(
        &self,
        product_id: &str,
        input: CreateProductToken,
    ) -> Result<CreatedProductToken, ProductStoreError> {
        self.get_product(product_id)?;
        input.validate().map_err(ProductStoreError::Product)?;
        let scopes = normalize_scopes(&input.scopes).map_err(ProductStoreError::Product)?;
        let token = generate_token();
        let token_prefix = token_prefix(&token);
        let token_digest_hex = token_digest_hex(&token);
        let now = now_rfc3339().map_err(ProductStoreError::Product)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(store_error)?;
        transaction
            .execute(
                "INSERT INTO product_tokens (
                    id,
                    product_id,
                    label,
                    scopes_json,
                    token_prefix,
                    token_digest_hex,
                    created_at,
                    last_used_at,
                    revoked_at
                ) VALUES ('', ?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
                params![
                    product_id,
                    input.label,
                    serialize_json(&scopes)?,
                    token_prefix,
                    token_digest_hex,
                    now,
                ],
            )
            .map_err(map_sqlite_product_error)?;
        let row_id = transaction.last_insert_rowid();
        let id = format!("ptok_{row_id}");
        transaction
            .execute(
                "UPDATE product_tokens SET id = ?1 WHERE rowid = ?2",
                params![id, row_id],
            )
            .map_err(store_error)?;
        transaction.commit().map_err(store_error)?;
        Ok(CreatedProductToken {
            id,
            product_id: product_id.to_owned(),
            label: input.label,
            scopes,
            token_prefix: token_prefix.to_owned(),
            token,
            created_at: now,
        })
    }

    pub fn revoke_token(&self, product_id: &str, token_id: &str) -> Result<(), ProductStoreError> {
        self.get_product(product_id)?;
        let now = now_rfc3339().map_err(ProductStoreError::Product)?;
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE product_tokens
                 SET revoked_at = COALESCE(revoked_at, ?1)
                 WHERE product_id = ?2 AND id = ?3",
                params![now, product_id, token_id],
            )
            .map_err(store_error)?;
        Ok(())
    }

    pub fn authenticate_product_token(
        &self,
        token: &str,
    ) -> Result<Option<AuthenticatedProductToken>, ProductStoreError> {
        if !token.starts_with(PRODUCT_TOKEN_PREFIX) || token.len() < TOKEN_PREFIX_LEN {
            return Ok(None);
        }
        let digest = token_digest(token);
        let digest_hex = hex::encode(&digest);
        let prefix = token_prefix(token);
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT
                    pt.id,
                    pt.product_id,
                    pt.scopes_json,
                    pt.token_digest_hex,
                    p.status AS product_status
                 FROM product_tokens pt
                 JOIN products p ON p.id = pt.product_id
                 WHERE pt.token_prefix = ?1 AND pt.revoked_at IS NULL",
            )
            .map_err(store_error)?;
        let rows = statement
            .query_map(params![prefix], |row| {
                Ok((
                    row.get::<_, String>("id")?,
                    row.get::<_, String>("product_id")?,
                    deserialize_json::<Vec<ApiScope>>(&row.get::<_, String>("scopes_json")?)?,
                    row.get::<_, String>("token_digest_hex")?,
                    deserialize_json::<ProductStatus>(&row.get::<_, String>("product_status")?)?,
                ))
            })
            .map_err(store_error)?
            .collect::<Result<Vec<_>, rusqlite::Error>>()
            .map_err(store_error)?;

        for (token_id, product_id, scopes, stored_digest_hex, product_status) in rows {
            let stored_digest = match hex::decode(stored_digest_hex) {
                Ok(value) => value,
                Err(error) => return Err(ProductStoreError::Store(error.into())),
            };
            if stored_digest.ct_eq(digest.as_slice()).into() {
                if product_status == ProductStatus::Disabled {
                    return Err(ProductStoreError::ProductDisabled);
                }
                if digest_hex != token_digest_hex(token) {
                    continue;
                }
                self.touch_last_used(&token_id)?;
                return Ok(Some(AuthenticatedProductToken {
                    token_id,
                    product_id,
                    scopes: scopes.into_iter().collect::<BTreeSet<_>>(),
                }));
            }
        }
        Ok(None)
    }

    fn touch_last_used(&self, token_id: &str) -> Result<(), ProductStoreError> {
        let now = now_rfc3339().map_err(ProductStoreError::Product)?;
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE product_tokens SET last_used_at = ?1 WHERE id = ?2",
                params![now, token_id],
            )
            .map_err(store_error)?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection, ProductStoreError> {
        sqlite::open_connection(&self.sqlite_path).map_err(store_error)
    }
}

impl From<StoreConfig> for ProductStore {
    fn from(config: StoreConfig) -> Self {
        Self::configured(config)
    }
}

fn row_to_product(row: &Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        status: deserialize_json(&row.get::<_, String>("status")?)?,
        description: row.get("description")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_token(row: &Row<'_>) -> rusqlite::Result<ProductToken> {
    let revoked_at = row.get::<_, Option<String>>("revoked_at")?;
    Ok(ProductToken {
        id: row.get("id")?,
        product_id: row.get("product_id")?,
        label: row.get("label")?,
        scopes: deserialize_json(&row.get::<_, String>("scopes_json")?)?,
        token_prefix: row.get("token_prefix")?,
        status: if revoked_at.is_some() {
            ProductTokenStatus::Revoked
        } else {
            ProductTokenStatus::Active
        },
        created_at: row.get("created_at")?,
        last_used_at: row.get("last_used_at")?,
        revoked_at,
    })
}

fn generate_token() -> String {
    let bytes: [u8; 24] = random();
    format!("{PRODUCT_TOKEN_PREFIX}{}", hex::encode(bytes))
}

fn token_prefix(token: &str) -> &str {
    &token[..TOKEN_PREFIX_LEN]
}

fn token_digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn token_digest_hex(token: &str) -> String {
    hex::encode(token_digest(token))
}

fn serialize_json<T: serde::Serialize>(value: &T) -> Result<String, ProductStoreError> {
    serde_json::to_string(value).map_err(store_error)
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn map_sqlite_product_error(error: rusqlite::Error) -> ProductStoreError {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("products.id")
                || message.contains("product_tokens.token_digest_hex") =>
        {
            ProductStoreError::Product(ProductError::ProductAlreadyExists)
        }
        other => store_error(other),
    }
}

fn store_error(error: impl Into<anyhow::Error>) -> ProductStoreError {
    ProductStoreError::Store(error.into())
}
