//! Authentication request DTOs.

use serde::Deserialize;
use validator::Validate;

/// Registration request
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 3, max = 32, message = "Username must be 3-32 characters"))]
    #[validate(regex(path = *USERNAME_REGEX, message = "Username can only contain letters, numbers, and underscores"))]
    pub username: String,

    #[validate(email(message = "Invalid email address"))]
    pub email: String,

    #[validate(length(min = 8, max = 128, message = "Password must be 8-128 characters"))]
    pub password: String,

    #[validate(length(max = 64, message = "Display name must be at most 64 characters"))]
    pub display_name: Option<String>,
}

/// Login request
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    /// Can be username or email
    #[validate(length(min = 1, message = "Username or email is required"))]
    pub identifier: String,

    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Update user profile request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(max = 64, message = "Display name must be at most 64 characters"))]
    pub display_name: Option<String>,

    #[validate(length(max = 500, message = "Bio must be at most 500 characters"))]
    pub bio: Option<String>,
}

lazy_static::lazy_static! {
    static ref USERNAME_REGEX: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
}
