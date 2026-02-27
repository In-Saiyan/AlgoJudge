//! Submission response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// User info for submissions
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
}

/// Problem info for submissions
#[derive(Debug, Serialize)]
pub struct ProblemInfo {
    pub id: Uuid,
    pub title: String,
    pub problem_code: Option<String>,
}

/// Contest info for submissions
#[derive(Debug, Serialize)]
pub struct ContestInfo {
    pub id: Uuid,
    pub title: String,
}

/// Submission summary for list responses
#[derive(Debug, Serialize)]
pub struct SubmissionSummary {
    pub id: Uuid,
    pub user: UserInfo,
    pub problem: ProblemInfo,
    /// `None` for standalone (practice) submissions.
    pub contest: Option<ContestInfo>,
    pub language: Option<String>,
    pub status: String,
    pub score: Option<i32>,
    pub max_time_ms: Option<i32>,
    pub max_memory_kb: Option<i32>,
    pub submitted_at: DateTime<Utc>,
}

/// Submission list response
#[derive(Debug, Serialize)]
pub struct SubmissionListResponse {
    pub submissions: Vec<SubmissionSummary>,
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

/// Full submission details
#[derive(Debug, Serialize)]
pub struct SubmissionDetailResponse {
    pub id: Uuid,
    pub user: UserInfo,
    pub problem: ProblemInfo,
    /// `None` for standalone (practice) submissions.
    pub contest: Option<ContestInfo>,
    pub submission_type: String,
    pub language: Option<String>,
    pub status: String,
    pub score: Option<i32>,
    pub total_test_cases: Option<i32>,
    pub passed_test_cases: Option<i32>,
    pub max_time_ms: Option<i32>,
    pub max_memory_kb: Option<i32>,
    pub compilation_log: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub compiled_at: Option<DateTime<Utc>>,
    pub judged_at: Option<DateTime<Utc>>,
    pub is_owner: bool,
}

/// Submission created response
#[derive(Debug, Serialize)]
pub struct SubmissionResponse {
    pub id: Uuid,
    /// `None` for standalone (practice) submissions.
    pub contest_id: Option<Uuid>,
    pub problem_id: Uuid,
    pub submission_type: String,
    pub language: Option<String>,
    pub status: String,
    pub submitted_at: DateTime<Utc>,
    pub message: String,
}

/// Test case result
#[derive(Debug, Serialize)]
pub struct TestCaseResult {
    pub test_case_number: i32,
    pub verdict: String,
    pub time_ms: Option<i32>,
    pub memory_kb: Option<i32>,
    pub checker_score: Option<f64>,
}

/// Submission results response
#[derive(Debug, Serialize)]
pub struct SubmissionResultsResponse {
    pub submission_id: Uuid,
    pub status: String,
    pub score: Option<i32>,
    pub total_test_cases: Option<i32>,
    pub passed_test_cases: Option<i32>,
    pub results: Vec<TestCaseResult>,
}

/// Source code response
#[derive(Debug, Serialize)]
pub struct SourceCodeResponse {
    pub submission_id: Uuid,
    pub language: Option<String>,
    pub source_code: Option<String>,
    pub submission_type: String,
}

/// Leaderboard entry
#[derive(Debug, Serialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub user: UserInfo,
    pub total_score: i32,
    pub total_penalty: i32,
    pub problems_solved: i32,
    pub problem_scores: Vec<ProblemScore>,
    pub last_submission_at: Option<DateTime<Utc>>,
}

/// Problem score for leaderboard
#[derive(Debug, Serialize)]
pub struct ProblemScore {
    pub problem_code: String,
    pub score: Option<i32>,
    pub attempts: i32,
    pub solved: bool,
    pub first_solved_at: Option<DateTime<Utc>>,
}

/// Leaderboard response
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub contest_id: Uuid,
    pub contest_title: String,
    pub scoring_type: String,
    pub entries: Vec<LeaderboardEntry>,
    pub pagination: Pagination,
    pub frozen: bool,
    pub problems: Vec<LeaderboardProblem>,
}

/// Problem info for leaderboard header
#[derive(Debug, Serialize)]
pub struct LeaderboardProblem {
    pub problem_code: String,
    pub title: String,
    pub max_score: i32,
}

/// Simple message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
