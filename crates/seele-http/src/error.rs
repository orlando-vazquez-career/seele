//! HTTP error type with conversions from inner SEELE errors + canonical
//! `axum::IntoResponse` mapping.
//!
//! Handlers return `Result<_, ApiError>`; `ApiError` is the bridge from the
//! storage / search / embedder error tree to a stable JSON error envelope.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        };
        (
            status,
            Json(ErrorBody {
                code,
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

impl From<seele_storage::StorageError> for ApiError {
    fn from(e: seele_storage::StorageError) -> Self {
        use seele_storage::StorageError::*;
        match e {
            NotFound(msg) => Self::NotFound(msg),
            InvalidInput(msg) => Self::BadRequest(msg),
            Conflict(msg) => Self::Conflict(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<seele_search::SearchError> for ApiError {
    fn from(e: seele_search::SearchError) -> Self {
        use seele_search::SearchError::*;
        match e {
            InvalidInput(msg) => Self::BadRequest(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<seele_embedder::EmbedderError> for ApiError {
    fn from(e: seele_embedder::EmbedderError) -> Self {
        Self::Internal(format!("embedder: {e}"))
    }
}

impl From<seele_core::SeeleError> for ApiError {
    fn from(e: seele_core::SeeleError) -> Self {
        use seele_core::SeeleError::*;
        match e {
            InvalidInput(msg) => Self::BadRequest(msg),
            NotFound(msg) => Self::NotFound(msg),
            Conflict(msg) => Self::Conflict(msg),
            other => Self::Internal(other.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
