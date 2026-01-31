//! Contest response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Contest response
#[derive(Debug, Serialize)]
pub struct ContestResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub scoring_mode: String,
    pub visibility: String,
    pub registration_mode: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub registration_start: Option<DateTime<Utc>>,
    pub registration_end: Option<DateTime<Utc>>,
    pub allowed_languages: Vec<String>,
    pub freeze_time_minutes: Option<i32>,
    pub allow_virtual: bool,
    pub organizer_id: Uuid,
    pub organizer_name: String,
    pub participant_count: i64,
    pub problem_count: i64,
    pub status: String, // upcoming, ongoing, ended
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Contest list response
#[derive(Debug, Serialize)]
pub struct ContestsListResponse {
    pub contests: Vec<ContestSummary>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// Contest summary for list views
#[derive(Debug, Serialize)]
pub struct ContestSummary {
    pub id: Uuid,
    pub title: String,
    pub scoring_mode: String,
    pub visibility: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub participant_count: i64,
    pub problem_count: i64,
    pub status: String,
}

/// Contest problem response
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ContestProblemResponse {
    pub id: Uuid,
    pub contest_id: Uuid,
    pub problem_id: Uuid,
    pub title: String,
    pub order: i32,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub points: Option<i32>,
    pub solved_count: i64,
    pub attempt_count: i64,
}

/// Contest problems list response
#[derive(Debug, Serialize)]
pub struct ContestProblemsResponse {
    pub problems: Vec<ContestProblemResponse>,
}

/// Participant response
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ParticipantResponse {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub is_virtual: bool,
}

/// Participants list response
#[derive(Debug, Serialize)]
pub struct ParticipantsListResponse {
    pub participants: Vec<ParticipantResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// Leaderboard entry
#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub score: i64,
    pub penalty: i64, // Penalty time in minutes (ICPC) or negative points (CF)
    pub problems_solved: i32,
    pub problem_results: Vec<ProblemResult>,
    pub last_accepted_at: Option<DateTime<Utc>>,
}

/// Problem result in leaderboard
#[derive(Debug, Serialize)]
pub struct ProblemResult {
    pub problem_id: Uuid,
    pub solved: bool,
    pub attempts: i32,
    pub points: Option<i32>,
    pub time_to_solve_minutes: Option<i64>,
    pub is_first_solve: bool,
}

/// Leaderboard response
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub contest_id: Uuid,
    pub entries: Vec<LeaderboardEntry>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
    pub is_frozen: bool,
    pub frozen_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    pub message: String,
    pub contest_id: Uuid,
    pub registered_at: DateTime<Utc>,
}

/// Virtual participation response
#[derive(Debug, Serialize)]
pub struct VirtualParticipationResponse {
    pub message: String,
    pub contest_id: Uuid,
    pub virtual_start: DateTime<Utc>,
    pub virtual_end: DateTime<Utc>,
}
