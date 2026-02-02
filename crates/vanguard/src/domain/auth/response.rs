//! Authentication response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Authentication tokens response
#[derive(Debug, Serialize)]
pub struct AuthTokensResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

impl AuthTokensResponse {
    pub fn new(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer",
            expires_in,
        }
    }
}

/// User response
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user: UserResponse,
    pub tokens: AuthTokensResponse,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: UserResponse,
    pub tokens: AuthTokensResponse,
}

/// Logout response
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub message: &'static str,
}

impl Default for LogoutResponse {
    fn default() -> Self {
        Self {
            message: "Successfully logged out",
        }
    }
}
