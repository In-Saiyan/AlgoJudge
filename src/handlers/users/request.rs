//! User request DTOs

use serde::Deserialize;
use validator::Validate;

/// Update user request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(max = 100))]
    pub display_name: Option<String>,

    #[validate(email)]
    pub email: Option<String>,

    /// Current password (required for sensitive changes)
    pub current_password: Option<String>,

    /// New password
    #[validate(length(min = 8, max = 128))]
    pub new_password: Option<String>,
}

/// List users query parameters
#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub role: Option<String>,
}
