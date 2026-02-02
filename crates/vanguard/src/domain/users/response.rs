//! User management response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// User list response
#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub users: Vec<UserSummary>,
    pub pagination: Pagination,
}

/// User summary for list responses
#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Pagination info
#[derive(Debug, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: u32,
}

/// User profile response (public view)
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// User stats response
#[derive(Debug, Serialize)]
pub struct UserStatsResponse {
    pub user_id: Uuid,
    pub total_submissions: i64,
    pub accepted_submissions: i64,
    pub contests_participated: i64,
    pub problems_solved: i64,
}

/// Update user response
#[derive(Debug, Serialize)]
pub struct UpdateUserResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub updated_at: DateTime<Utc>,
}
