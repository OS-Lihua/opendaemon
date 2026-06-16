use serde::{Serialize, de::DeserializeOwned};

use crate::{
    dto::{
        AgentEnvelope, AgentProfile, AgentProfileFormPayload, AgentsEnvelope, CreateProductPayload,
        CreateProductTokenPayload, CreatedProductToken, CreatedProductTokenEnvelope, DaemonStatus,
        DirectoriesEnvelope, DirectoryEnvelope, DirectoryGrant, DirectoryGrantFormPayload,
        PermissionsEnvelope, Product, ProductEnvelope, ProductTokensEnvelope, ProductsEnvelope,
        Provider, ProvidersEnvelope, RuntimeView, RuntimesEnvelope, Session, Task,
        TaskCreatePayload, TaskEnvelope, TasksEnvelope, UpdateProductPayload,
    },
    error::ConsoleApiError,
};

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

    async fn patch_json<I: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        input: &I,
    ) -> Result<T, ConsoleApiError> {
        let response = gloo_net::http::Request::patch(&self.url(path))
            .header("authorization", &format!("Bearer {}", self.token))
            .json(input)
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?
            .send()
            .await
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?;
        decode_response(response).await
    }

    async fn delete(&self, path: &str) -> Result<(), ConsoleApiError> {
        let response = gloo_net::http::Request::delete(&self.url(path))
            .header("authorization", &format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|error| ConsoleApiError::Request(error.to_string()))?;
        if !(200..300).contains(&response.status()) {
            return Err(ConsoleApiError::Api {
                status: response.status(),
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".to_owned()),
            });
        }
        Ok(())
    }

    pub async fn session(&self) -> Result<Session, ConsoleApiError> {
        self.get_json("/v1/session").await
    }

    pub async fn daemon_status(&self) -> Result<DaemonStatus, ConsoleApiError> {
        self.get_json("/v1/daemon/status").await
    }

    pub async fn products(&self) -> Result<Vec<Product>, ConsoleApiError> {
        self.get_json::<ProductsEnvelope>("/v1/products")
            .await
            .map(|response| response.products)
    }

    pub async fn create_product(
        &self,
        input: &CreateProductPayload,
    ) -> Result<Product, ConsoleApiError> {
        self.post_json::<_, ProductEnvelope>("/v1/products", input)
            .await
            .map(|response| response.product)
    }

    pub async fn update_product(
        &self,
        product_id: &str,
        input: &UpdateProductPayload,
    ) -> Result<Product, ConsoleApiError> {
        self.patch_json::<_, ProductEnvelope>(&format!("/v1/products/{product_id}"), input)
            .await
            .map(|response| response.product)
    }

    pub async fn product_tokens(
        &self,
        product_id: &str,
    ) -> Result<Vec<crate::dto::ProductToken>, ConsoleApiError> {
        self.get_json::<ProductTokensEnvelope>(&format!("/v1/products/{product_id}/tokens"))
            .await
            .map(|response| response.tokens)
    }

    pub async fn create_product_token(
        &self,
        product_id: &str,
        input: &CreateProductTokenPayload,
    ) -> Result<CreatedProductToken, ConsoleApiError> {
        self.post_json::<_, CreatedProductTokenEnvelope>(
            &format!("/v1/products/{product_id}/tokens"),
            input,
        )
        .await
        .map(|response| response.token)
    }

    pub async fn revoke_product_token(
        &self,
        product_id: &str,
        token_id: &str,
    ) -> Result<(), ConsoleApiError> {
        self.delete(&format!("/v1/products/{product_id}/tokens/{token_id}"))
            .await
    }

    pub async fn providers(&self) -> Result<Vec<Provider>, ConsoleApiError> {
        self.get_json::<ProvidersEnvelope>("/v1/providers")
            .await
            .map(|response| response.providers)
    }

    pub async fn runtimes(&self) -> Result<Vec<RuntimeView>, ConsoleApiError> {
        self.get_json::<RuntimesEnvelope>("/v1/runtimes")
            .await
            .map(|response| response.runtimes)
    }

    pub async fn detect_runtimes(&self) -> Result<Vec<RuntimeView>, ConsoleApiError> {
        self.post_json::<_, RuntimesEnvelope>("/v1/runtimes/detect", &())
            .await
            .map(|response| response.runtimes)
    }

    pub async fn agents(&self) -> Result<Vec<AgentProfile>, ConsoleApiError> {
        self.get_json::<AgentsEnvelope>("/v1/agents")
            .await
            .map(|response| response.agents)
    }

    pub async fn create_agent(
        &self,
        input: &AgentProfileFormPayload,
    ) -> Result<AgentProfile, ConsoleApiError> {
        self.post_json::<_, AgentEnvelope>("/v1/agents", input)
            .await
            .map(|response| response.agent)
    }

    pub async fn directories(&self) -> Result<Vec<DirectoryGrant>, ConsoleApiError> {
        self.get_json::<DirectoriesEnvelope>("/v1/directories")
            .await
            .map(|response| response.directories)
    }

    pub async fn create_directory(
        &self,
        input: &DirectoryGrantFormPayload,
    ) -> Result<DirectoryGrant, ConsoleApiError> {
        self.post_json::<_, DirectoryEnvelope>("/v1/directories/grant", input)
            .await
            .map(|response| response.directory)
    }

    pub async fn tasks(&self) -> Result<Vec<Task>, ConsoleApiError> {
        self.get_json::<TasksEnvelope>("/v1/tasks")
            .await
            .map(|response| response.tasks)
    }

    pub async fn create_task(&self, input: &TaskCreatePayload) -> Result<Task, ConsoleApiError> {
        self.post_json::<_, TaskEnvelope>("/v1/tasks", input)
            .await
            .map(|response| response.task)
    }

    pub async fn permissions(&self) -> Result<Vec<crate::dto::PermissionRequest>, ConsoleApiError> {
        self.get_json::<PermissionsEnvelope>("/v1/permissions?status=pending")
            .await
            .map(|response| response.permissions)
    }
}

async fn decode_response<T: DeserializeOwned>(
    response: gloo_net::http::Response,
) -> Result<T, ConsoleApiError> {
    let status = response.status();
    if !(200..300).contains(&status) {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "request failed".to_owned());
        return Err(ConsoleApiError::Api { status, message });
    }
    response
        .json()
        .await
        .map_err(|error| ConsoleApiError::Decode(error.to_string()))
}
