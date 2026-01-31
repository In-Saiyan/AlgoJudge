//! Contest request DTOs

use chrono::{DateTime, Utc};
use serde::Deserialize;
use validator::Validate;

use crate::constants::{MAX_CONTEST_DESCRIPTION_LENGTH, MAX_CONTEST_TITLE_LENGTH};

/// Create contest request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateContestRequest {
    #[validate(length(min = 1, max = MAX_CONTEST_TITLE_LENGTH))]
    pub title: String,

    #[validate(length(max = MAX_CONTEST_DESCRIPTION_LENGTH))]
    pub description: Option<String>,

    /// Scoring mode: icpc, codeforces, ioi, practice
    pub scoring_mode: String,

    /// Contest visibility: public, private, hidden
    pub visibility: String,

    /// Registration mode: open, closed, invite_only
    pub registration_mode: String,

    /// Contest start time
    pub start_time: DateTime<Utc>,

    /// Contest end time
    pub end_time: DateTime<Utc>,

    /// Registration opens at (optional)
    pub registration_start: Option<DateTime<Utc>>,

    /// Registration closes at (optional)
    pub registration_end: Option<DateTime<Utc>>,

    /// Allowed programming languages (empty = all)
    pub allowed_languages: Option<Vec<String>>,

    /// Freeze leaderboard N minutes before end (optional)
    pub freeze_time_minutes: Option<i32>,

    /// Allow virtual participation after contest ends
    pub allow_virtual: Option<bool>,
}

/// Update contest request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateContestRequest {
    #[validate(length(min = 1, max = MAX_CONTEST_TITLE_LENGTH))]
    pub title: Option<String>,

    #[validate(length(max = MAX_CONTEST_DESCRIPTION_LENGTH))]
    pub description: Option<String>,

    pub scoring_mode: Option<String>,
    pub visibility: Option<String>,
    pub registration_mode: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub registration_start: Option<DateTime<Utc>>,
    pub registration_end: Option<DateTime<Utc>>,
    pub allowed_languages: Option<Vec<String>>,
    pub freeze_time_minutes: Option<i32>,
    pub allow_virtual: Option<bool>,
}

/// List contests query parameters
#[derive(Debug, Deserialize)]
pub struct ListContestsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub status: Option<String>, // upcoming, ongoing, ended
    pub visibility: Option<String>,
    pub search: Option<String>,
}

/// Add problem to contest request
#[derive(Debug, Deserialize, Validate)]
pub struct AddProblemRequest {
    pub problem_id: uuid::Uuid,

    /// Order/index within the contest
    pub order: Option<i32>,

    /// Custom time limit for this contest (overrides problem default)
    pub time_limit_ms: Option<i64>,

    /// Custom memory limit for this contest (overrides problem default)
    pub memory_limit_kb: Option<i64>,

    /// Points for this problem (Codeforces mode)
    pub points: Option<i32>,
}

/// Leaderboard query parameters
#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    /// Include frozen standings
    pub include_frozen: Option<bool>,
}
