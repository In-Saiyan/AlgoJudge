//! Error handling and API error responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// API error response
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Application error type
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Access denied")]
    Forbidden,

    #[error("{0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] deadpool_redis::PoolError),

    #[error("Redis command error: {0}")]
    RedisCmd(#[from] redis::RedisError),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Token error: {0}")]
    Token(String),
}

impl ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Redis(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::RedisCmd(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Token(_) => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            ApiError::Unauthorized => "UNAUTHORIZED",
            ApiError::InvalidCredentials => "INVALID_CREDENTIALS",
            ApiError::Forbidden => "FORBIDDEN",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Validation(_) => "VALIDATION_ERROR",
            ApiError::Conflict(_) => "CONFLICT",
            ApiError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            ApiError::Database(_) => "DATABASE_ERROR",
            ApiError::Redis(_) => "CACHE_ERROR",
            ApiError::RedisCmd(_) => "CACHE_ERROR",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::Token(_) => "TOKEN_ERROR",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.error_code();

        // Don't expose internal error details in production
        let message = match &self {
            ApiError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                "A database error occurred".to_string()
            }
            ApiError::Redis(e) => {
                tracing::error!("Redis pool error: {:?}", e);
                "A cache error occurred".to_string()
            }
            ApiError::RedisCmd(e) => {
                tracing::error!("Redis command error: {:?}", e);
                "A cache error occurred".to_string()
            }
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                "An internal error occurred".to_string()
            }
            _ => self.to_string(),
        };

        let body = ApiErrorResponse {
            error: ApiErrorBody {
                code,
                message,
                details: None,
            },
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for API handlers
pub type ApiResult<T> = Result<T, ApiError>;
