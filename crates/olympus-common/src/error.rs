//! Application error types for Olympus services.

use thiserror::Error;

/// Main application error type used across all Olympus services.
#[derive(Error, Debug)]
pub enum AppError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    /// Authorization failed - user lacks permission
    #[error("Access denied: {0}")]
    Forbidden(String),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Request validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Conflict - e.g., duplicate entry
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Redis error
    #[error("Cache error: {0}")]
    CacheError(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(String),

    /// External service error
    #[error("External service error: {0}")]
    ExternalServiceError(String),

    /// File I/O error
    #[error("File error: {0}")]
    FileError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Timeout error
    #[error("Operation timed out: {0}")]
    TimeoutError(String),

    /// Queue error
    #[error("Queue error: {0}")]
    QueueError(String),
}

impl AppError {
    /// Returns the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::ValidationError(_) => 422,
            AppError::Conflict(_) => 409,
            AppError::RateLimitExceeded => 429,
            AppError::DatabaseError(_) => 500,
            AppError::CacheError(_) => 500,
            AppError::InternalError(_) => 500,
            AppError::ExternalServiceError(_) => 502,
            AppError::FileError(_) => 500,
            AppError::SerializationError(_) => 500,
            AppError::TimeoutError(_) => 504,
            AppError::QueueError(_) => 500,
        }
    }

    /// Returns the error code string for this error
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::Forbidden(_) => "FORBIDDEN",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::ValidationError(_) => "VALIDATION_ERROR",
            AppError::Conflict(_) => "CONFLICT",
            AppError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            AppError::DatabaseError(_) => "DATABASE_ERROR",
            AppError::CacheError(_) => "CACHE_ERROR",
            AppError::InternalError(_) => "INTERNAL_ERROR",
            AppError::ExternalServiceError(_) => "EXTERNAL_SERVICE_ERROR",
            AppError::FileError(_) => "FILE_ERROR",
            AppError::SerializationError(_) => "SERIALIZATION_ERROR",
            AppError::TimeoutError(_) => "TIMEOUT_ERROR",
            AppError::QueueError(_) => "QUEUE_ERROR",
        }
    }
}

/// Result type alias using AppError
pub type AppResult<T> = Result<T, AppError>;
