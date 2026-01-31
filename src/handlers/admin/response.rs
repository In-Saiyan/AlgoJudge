//! Admin response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// System statistics response
#[derive(Debug, Serialize)]
pub struct SystemStatsResponse {
    pub total_users: i64,
    pub total_contests: i64,
    pub total_problems: i64,
    pub total_submissions: i64,
    pub pending_submissions: i64,
    pub active_containers: i64,
    pub uptime_seconds: u64,
}

/// Container info response
#[derive(Debug, Serialize)]
pub struct ContainerInfoResponse {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub submission_id: Option<Uuid>,
}

/// Containers list response
#[derive(Debug, Serialize)]
pub struct ContainersListResponse {
    pub containers: Vec<ContainerInfoResponse>,
}

/// Submission queue response
#[derive(Debug, Serialize)]
pub struct SubmissionQueueResponse {
    pub pending: Vec<QueuedSubmission>,
    pub running: Vec<QueuedSubmission>,
    pub total_pending: i64,
    pub total_running: i64,
}

/// Queued submission info
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct QueuedSubmission {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub problem_id: Uuid,
    pub problem_title: String,
    pub language: String,
    pub status: String,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
}

/// Admin user view response
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminUserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub ban_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// Admin users list response
#[derive(Debug, Serialize)]
pub struct AdminUsersListResponse {
    pub users: Vec<AdminUserResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}
