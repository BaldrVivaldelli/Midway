use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    Serialization(String),
    #[error("{0}")]
    InvalidJson(String),
    #[error("{0}")]
    Http(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Secrets(String),
    #[error("{0}")]
    Runtime(String),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub kind: String,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(value: AppError) -> Self {
        match value {
            AppError::Validation(message) => Self {
                kind: "validation".to_string(),
                message,
            },
            AppError::Database(message) => Self {
                kind: "database".to_string(),
                message,
            },
            AppError::Serialization(message) => Self {
                kind: "serialization".to_string(),
                message,
            },
            AppError::InvalidJson(message) => Self {
                kind: "invalidJson".to_string(),
                message,
            },
            AppError::Http(message) => Self {
                kind: "http".to_string(),
                message,
            },
            AppError::NotFound(message) => Self {
                kind: "notFound".to_string(),
                message,
            },
            AppError::Io(message) => Self {
                kind: "io".to_string(),
                message,
            },
            AppError::Secrets(message) => Self {
                kind: "secrets".to_string(),
                message,
            },
            AppError::Runtime(message) => Self {
                kind: "runtime".to_string(),
                message,
            },
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Serialization(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        AppError::Http(value.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(value: keyring::Error) -> Self {
        AppError::Secrets(value.to_string())
    }
}
