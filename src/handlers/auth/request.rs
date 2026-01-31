//! Authentication request DTOs

use serde::Deserialize;
use validator::Validate;

use crate::constants::{MAX_PASSWORD_LENGTH, MAX_USERNAME_LENGTH, MIN_PASSWORD_LENGTH, MIN_USERNAME_LENGTH};

/// User registration request
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = MIN_USERNAME_LENGTH, max = MAX_USERNAME_LENGTH))]
    pub username: String,

    #[validate(email)]
    pub email: String,

    #[validate(length(min = MIN_PASSWORD_LENGTH, max = MAX_PASSWORD_LENGTH))]
    pub password: String,

    #[validate(length(max = 100))]
    pub display_name: Option<String>,
}

/// User login request
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    /// Can be either username or email
    #[validate(length(min = 1))]
    pub identifier: String,

    #[validate(length(min = 1))]
    pub password: String,
}

/// Token refresh request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Logout request (optional, can invalidate specific token)
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    /// Optional: invalidate all sessions if true
    pub all_sessions: Option<bool>,
}
