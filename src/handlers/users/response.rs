//! User response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// User public profile response
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// User list response
#[derive(Debug, Serialize)]
pub struct UsersListResponse {
    pub users: Vec<UserProfileResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// User statistics response
#[derive(Debug, Serialize)]
pub struct UserStatsResponse {
    pub user_id: Uuid,
    pub total_submissions: i64,
    pub accepted_submissions: i64,
    pub problems_solved: i64,
    pub contests_participated: i64,
    pub rating: Option<i32>,
}

/// User submission history response
#[derive(Debug, Serialize)]
pub struct UserSubmissionsResponse {
    pub submissions: Vec<SubmissionSummary>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// Brief submission info
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SubmissionSummary {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub problem_title: String,
    pub language: String,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    pub submitted_at: DateTime<Utc>,
}
