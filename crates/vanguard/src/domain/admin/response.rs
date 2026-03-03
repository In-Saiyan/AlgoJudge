//! Admin response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

// =============================================================================
// Pagination
// =============================================================================

/// Pagination info
#[derive(Debug, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: u32,
}

// =============================================================================
// User Management
// =============================================================================

/// Admin view of a user (includes sensitive fields)
#[derive(Debug, Serialize)]
pub struct AdminUserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub is_banned: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub banned_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Admin user list response
#[derive(Debug, Serialize)]
pub struct AdminUserListResponse {
    pub users: Vec<AdminUserResponse>,
    pub pagination: Pagination,
}

/// Response after updating a user's role
#[derive(Debug, Serialize)]
pub struct UpdateRoleResponse {
    pub id: Uuid,
    pub username: String,
    pub role: String,
    pub updated_at: DateTime<Utc>,
}

/// Response after banning/unbanning a user
#[derive(Debug, Serialize)]
pub struct BanResponse {
    pub id: Uuid,
    pub username: String,
    pub is_banned: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub banned_reason: Option<String>,
}

// =============================================================================
// System Stats
// =============================================================================

/// System-wide statistics
#[derive(Debug, Serialize)]
pub struct SystemStatsResponse {
    pub users: UserStats,
    pub contests: ContestStats,
    pub submissions: SubmissionStats,
    pub storage: StorageStats,
}

#[derive(Debug, Serialize)]
pub struct UserStats {
    pub total: i64,
    pub active: i64,
    pub banned: i64,
    pub by_role: Vec<RoleCount>,
}

#[derive(Debug, Serialize)]
pub struct RoleCount {
    pub role: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ContestStats {
    pub total: i64,
    pub active: i64,
    pub draft: i64,
    pub finished: i64,
}

#[derive(Debug, Serialize)]
pub struct SubmissionStats {
    pub total: i64,
    pub pending: i64,
    pub judging: i64,
    pub accepted: i64,
    pub rejected: i64,
}

#[derive(Debug, Serialize)]
pub struct StorageStats {
    pub submissions_count: i64,
    pub results_count: i64,
}

// =============================================================================
// Queue Management
// =============================================================================

/// Queue info response
#[derive(Debug, Serialize)]
pub struct QueueInfoResponse {
    pub queues: Vec<QueueDetail>,
}

#[derive(Debug, Serialize)]
pub struct QueueDetail {
    pub name: String,
    pub length: i64,
    pub consumer_groups: Vec<ConsumerGroupInfo>,
    pub pending_entries: Vec<PendingEntry>,
}

#[derive(Debug, Serialize)]
pub struct ConsumerGroupInfo {
    pub name: String,
    pub consumers: i64,
    pub pending: i64,
    pub last_delivered_id: String,
}

#[derive(Debug, Serialize)]
pub struct PendingEntry {
    pub id: String,
    pub consumer: String,
    pub idle_ms: i64,
    pub delivery_count: i64,
}

/// Response after rejudge
#[derive(Debug, Serialize)]
pub struct RejudgeResponse {
    pub submission_id: Uuid,
    pub status: String,
    pub message: String,
}

/// Response after contest-wide rejudge
#[derive(Debug, Serialize)]
pub struct ContestRejudgeResponse {
    pub contest_id: Uuid,
    pub rejudged_count: usize,
    pub skipped_count: usize,
    pub message: String,
}

// =============================================================================
// Container Management
// =============================================================================

/// Info about a running Docker container
#[derive(Debug, Serialize)]
pub struct ContainerInfo {
    pub container_id: String,
    pub image: String,
    pub status: String,
    pub created: String,
    pub state: String,
    /// CPU usage percentage (from docker stats)
    pub cpu_percent: Option<String>,
    /// Memory usage string (from docker stats)
    pub memory_usage: Option<String>,
    /// Network I/O string (from docker stats)
    pub net_io: Option<String>,
    /// PIDs inside the container
    pub pids: Option<String>,
}

/// Response for container listing
#[derive(Debug, Serialize)]
pub struct ContainerListResponse {
    pub containers: Vec<ContainerInfo>,
    pub total: usize,
}

// =============================================================================
// Rule Configuration
// =============================================================================

/// Rule config response
#[derive(Debug, Serialize)]
pub struct RuleConfigResponse {
    pub id: Uuid,
    pub name: String,
    pub service: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub version: String,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rule config list response
#[derive(Debug, Serialize)]
pub struct RuleConfigListResponse {
    pub rules: Vec<RuleConfigResponse>,
}

/// Success response after saving a rule
#[derive(Debug, Serialize)]
pub struct SaveRuleResponse {
    pub id: Uuid,
    pub name: String,
    pub service: String,
    pub success: bool,
    pub message: String,
}
