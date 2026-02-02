//! User management request DTOs.

use serde::Deserialize;
use validator::Validate;

/// Query parameters for listing users
#[derive(Debug, Deserialize, Default)]
pub struct ListUsersQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    /// Filter by role
    pub role: Option<String>,
    /// Search by username or display name
    pub search: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Update user profile request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(max = 64, message = "Display name must be at most 64 characters"))]
    pub display_name: Option<String>,

    #[validate(length(max = 500, message = "Bio must be at most 500 characters"))]
    pub bio: Option<String>,
}
