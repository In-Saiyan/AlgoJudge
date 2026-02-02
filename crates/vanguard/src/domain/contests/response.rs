//! Contest response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Contest summary for list responses
#[derive(Debug, Serialize)]
pub struct ContestSummary {
    pub id: Uuid,
    pub title: String,
    pub short_description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub scoring_type: String,
    pub is_public: bool,
    pub is_rated: bool,
    pub participant_count: i64,
    pub owner: OwnerInfo,
    pub status: String,
}

/// Owner information
#[derive(Debug, Serialize)]
pub struct OwnerInfo {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
}

/// Contest list response
#[derive(Debug, Serialize)]
pub struct ContestListResponse {
    pub contests: Vec<ContestSummary>,
    pub pagination: Pagination,
}

/// Pagination info
#[derive(Debug, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: u32,
}

/// Full contest details
#[derive(Debug, Serialize)]
pub struct ContestDetailResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub short_description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub freeze_time: Option<DateTime<Utc>>,
    pub scoring_type: String,
    pub is_public: bool,
    pub is_rated: bool,
    pub registration_required: bool,
    pub max_participants: Option<i32>,
    pub allowed_languages: Option<Vec<String>>,
    pub owner: OwnerInfo,
    pub participant_count: i64,
    pub problem_count: i64,
    pub status: String,
    pub is_registered: bool,
    pub is_collaborator: bool,
    pub is_owner: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Contest created/updated response
#[derive(Debug, Serialize)]
pub struct ContestResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub short_description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub freeze_time: Option<DateTime<Utc>>,
    pub scoring_type: String,
    pub is_public: bool,
    pub is_rated: bool,
    pub registration_required: bool,
    pub max_participants: Option<i32>,
    pub allowed_languages: Option<Vec<String>>,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Collaborator info
#[derive(Debug, Serialize)]
pub struct CollaboratorInfo {
    pub id: Uuid,
    pub user: OwnerInfo,
    pub role: String,
    pub can_edit_contest: bool,
    pub can_add_problems: bool,
    pub can_view_submissions: bool,
    pub added_at: DateTime<Utc>,
}

/// Collaborator list response
#[derive(Debug, Serialize)]
pub struct CollaboratorListResponse {
    pub collaborators: Vec<CollaboratorInfo>,
}

/// Participant info
#[derive(Debug, Serialize)]
pub struct ParticipantInfo {
    pub id: Uuid,
    pub user: OwnerInfo,
    pub status: String,
    pub total_score: i32,
    pub total_penalty: i32,
    pub problems_solved: i32,
    pub registered_at: DateTime<Utc>,
    pub last_submission_at: Option<DateTime<Utc>>,
}

/// Participant list response
#[derive(Debug, Serialize)]
pub struct ParticipantListResponse {
    pub participants: Vec<ParticipantInfo>,
    pub pagination: Pagination,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    pub message: String,
    pub contest_id: Uuid,
    pub registered_at: DateTime<Utc>,
}

/// Simple message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
