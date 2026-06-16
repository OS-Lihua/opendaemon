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
