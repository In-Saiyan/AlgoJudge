//! Admin request DTOs

use serde::Deserialize;
use validator::Validate;

/// Update user role request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRoleRequest {
    #[validate(length(min = 1))]
    pub role: String,
}

/// Ban user request
#[derive(Debug, Deserialize)]
pub struct BanUserRequest {
    pub reason: Option<String>,
    /// Duration in hours (None = permanent)
    pub duration_hours: Option<i64>,
}
