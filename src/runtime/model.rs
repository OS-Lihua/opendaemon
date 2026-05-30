use std::path::PathBuf;

use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    LocalCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatus {
    NotDetected,
    Available,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
}

impl RuntimeError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeView {
    pub id: String,
    pub provider_id: String,
    pub kind: RuntimeKind,
    pub status: RuntimeStatus,
    pub executable: Option<PathBuf>,
    pub version: Option<String>,
    pub detected_at: Option<String>,
    pub error: Option<RuntimeError>,
}

impl RuntimeView {
    #[must_use]
    pub fn not_detected(provider_id: impl Into<String>) -> Self {
        let provider_id = provider_id.into();
        Self {
            id: runtime_id(&provider_id),
            provider_id,
            kind: RuntimeKind::LocalCli,
            status: RuntimeStatus::NotDetected,
            executable: None,
            version: None,
            detected_at: None,
            error: None,
        }
    }

    #[must_use]
    pub fn available(
        provider_id: impl Into<String>,
        executable: PathBuf,
        version: Option<String>,
    ) -> Self {
        let provider_id = provider_id.into();
        Self {
            id: runtime_id(&provider_id),
            provider_id,
            kind: RuntimeKind::LocalCli,
            status: RuntimeStatus::Available,
            executable: Some(executable),
            version,
            detected_at: Some(now_rfc3339()),
            error: None,
        }
    }

    #[must_use]
    pub fn unavailable(provider_id: impl Into<String>, error: RuntimeError) -> Self {
        let provider_id = provider_id.into();
        Self {
            id: runtime_id(&provider_id),
            provider_id,
            kind: RuntimeKind::LocalCli,
            status: RuntimeStatus::Unavailable,
            executable: None,
            version: None,
            detected_at: Some(now_rfc3339()),
            error: Some(error),
        }
    }

    #[must_use]
    pub fn error(
        provider_id: impl Into<String>,
        executable: Option<PathBuf>,
        error: RuntimeError,
    ) -> Self {
        let provider_id = provider_id.into();
        Self {
            id: runtime_id(&provider_id),
            provider_id,
            kind: RuntimeKind::LocalCli,
            status: RuntimeStatus::Error,
            executable,
            version: None,
            detected_at: Some(now_rfc3339()),
            error: Some(error),
        }
    }
}

#[must_use]
pub fn runtime_id(provider_id: &str) -> String {
    format!(
        "rt_{}_local_cli",
        normalize_provider_id(provider_id, LetterCase::Lower)
    )
}

#[must_use]
pub fn override_env_var_name(provider_id: &str) -> String {
    format!(
        "OPENDAEMON_PROVIDER_{}_PATH",
        normalize_provider_id(provider_id, LetterCase::Upper)
    )
}

fn normalize_provider_id(provider_id: &str, letter_case: LetterCase) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;

    for character in provider_id.chars() {
        if character.is_ascii_alphanumeric() {
            let normalized_character = match letter_case {
                LetterCase::Lower => character.to_ascii_lowercase(),
                LetterCase::Upper => character.to_ascii_uppercase(),
            };
            normalized.push(normalized_character);
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }

    normalized
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamps must format as RFC3339")
}

enum LetterCase {
    Lower,
    Upper,
}
