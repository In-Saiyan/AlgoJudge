//! Custom error types and handling
//!
//! This module defines the application's error types and implements
//! conversion to HTTP responses for the Axum framework.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Application-wide error type
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // Authentication errors
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    // Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // Resource errors
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    // Database errors
    #[error("Database error: {0}")]
    Database(String),

    // External service errors
    #[error("Docker error: {0}")]
    Docker(String),

    #[error("Redis error: {0}")]
    Redis(String),

    // Benchmark errors
    #[error("Compilation error: {0}")]
    CompilationError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Time limit exceeded")]
    TimeLimitExceeded,

    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,

    // Rate limiting
    #[error("Too many requests")]
    TooManyRequests,

    // Internal errors
    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Error response body
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetails,
}

/// Error details in response
#[derive(Debug, Serialize)]
pub struct ErrorDetails {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AppError {
    /// Get the error code for this error type
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::NotFound(_) => "NOT_FOUND",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",
            Self::Conflict(_) => "CONFLICT",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Docker(_) => "DOCKER_ERROR",
            Self::Redis(_) => "REDIS_ERROR",
            Self::CompilationError(_) => "COMPILATION_ERROR",
            Self::RuntimeError(_) => "RUNTIME_ERROR",
            Self::TimeLimitExceeded => "TIME_LIMIT_EXCEEDED",
            Self::MemoryLimitExceeded => "MEMORY_LIMIT_EXCEEDED",
            Self::TooManyRequests => "TOO_MANY_REQUESTS",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Configuration(_) => "CONFIGURATION_ERROR",
        }
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials | Self::InvalidToken | Self::TokenExpired => {
                StatusCode::UNAUTHORIZED
            }
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Validation(_) | Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::AlreadyExists(_) | Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::CompilationError(_) | Self::RuntimeError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TimeLimitExceeded | Self::MemoryLimitExceeded => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Database(_) | Self::Docker(_) | Self::Redis(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Create error response with additional details
    pub fn with_details(self, details: serde_json::Value) -> AppErrorWithDetails {
        AppErrorWithDetails {
            error: self,
            details: Some(details),
        }
    }
}

/// Error with additional details
pub struct AppErrorWithDetails {
    pub error: AppError,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        // Log internal errors but don't expose details to clients
        let message = match &self {
            AppError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                tracing::debug!("Internal error details: {:#?}", e);
                "An internal error occurred".to_string()
            }
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                tracing::debug!("Database error details: {}", e);
                "A database error occurred".to_string()
            }
            _ => self.to_string(),
        };

        let body = ErrorResponse {
            error: ErrorDetails {
                code: self.error_code().to_string(),
                message,
                details: None,
            },
        };

        (status, Json(body)).into_response()
    }
}

impl IntoResponse for AppErrorWithDetails {
    fn into_response(self) -> Response {
        let status = self.error.status_code();
        let code = self.error.error_code().to_string();
        let message = self.error.to_string();

        let body = ErrorResponse {
            error: ErrorDetails {
                code,
                message,
                details: self.details,
            },
        };

        (status, Json(body)).into_response()
    }
}

// Implement From for common error types
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".to_string()),
            sqlx::Error::Database(db_err) => {
                // Check for unique constraint violations
                if db_err.is_unique_violation() {
                    AppError::AlreadyExists("Resource already exists".to_string())
                } else {
                    AppError::Database(db_err.to_string())
                }
            }
            _ => AppError::Database(err.to_string()),
        }
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis(err.to_string())
    }
}

impl From<bollard::errors::Error> for AppError {
    fn from(err: bollard::errors::Error) -> Self {
        AppError::Docker(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
            _ => AppError::InvalidToken,
        }
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        AppError::Validation(err.to_string())
    }
}

/// Result type alias using AppError
pub type AppResult<T> = Result<T, AppError>;
