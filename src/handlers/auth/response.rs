//! Authentication response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Authentication token response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

/// User information in auth response
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Registration success response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub message: String,
    pub user: UserResponse,
}

/// Token refresh response
#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Logout response
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub message: String,
}

/// Current user response (for /me endpoint)
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user: UserResponse,
}
