//! Problem response DTOs.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Owner information
#[derive(Debug, Serialize)]
pub struct OwnerInfo {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
}

/// Problem summary for list responses
#[derive(Debug, Serialize)]
pub struct ProblemSummary {
    pub id: Uuid,
    pub title: String,
    pub difficulty: Option<String>,
    pub tags: Option<Vec<String>>,
    pub time_limit_ms: i32,
    pub memory_limit_kb: i32,
    pub max_score: i32,
    pub is_public: bool,
    pub owner: OwnerInfo,
    pub created_at: DateTime<Utc>,
}

/// Problem list response
#[derive(Debug, Serialize)]
pub struct ProblemListResponse {
    pub problems: Vec<ProblemSummary>,
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

/// Full problem details (for owner/admin view)
#[derive(Debug, Serialize)]
pub struct ProblemDetailResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,
    pub sample_input: Option<String>,
    pub sample_output: Option<String>,
    pub sample_explanation: Option<String>,
    pub difficulty: Option<String>,
    pub tags: Option<Vec<String>>,
    pub time_limit_ms: i32,
    pub memory_limit_kb: i32,
    pub num_test_cases: i32,
    pub generator_path: Option<String>,
    pub checker_path: Option<String>,
    pub max_score: i32,
    pub partial_scoring: bool,
    pub is_public: bool,
    pub allowed_languages: Option<Vec<String>>,
    pub owner: OwnerInfo,
    pub is_owner: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Problem created/updated response
#[derive(Debug, Serialize)]
pub struct ProblemResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,
    pub sample_input: Option<String>,
    pub sample_output: Option<String>,
    pub sample_explanation: Option<String>,
    pub difficulty: Option<String>,
    pub tags: Option<Vec<String>>,
    pub time_limit_ms: i32,
    pub memory_limit_kb: i32,
    pub num_test_cases: i32,
    /// Status of problem: "draft" until both generator and checker are uploaded, then "ready"
    pub status: String,
    /// Whether generator binary has been uploaded
    pub generator_uploaded: bool,
    /// Whether checker binary has been uploaded
    pub checker_uploaded: bool,
    pub max_score: i32,
    pub partial_scoring: bool,
    pub is_public: bool,
    pub allowed_languages: Option<Vec<String>>,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Message with next steps for problem setup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Contest problem info (problem within a contest)
#[derive(Debug, Serialize)]
pub struct ContestProblemInfo {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub problem_code: String,
    pub title: String,
    pub difficulty: Option<String>,
    pub time_limit_ms: i32,
    pub memory_limit_kb: i32,
    pub max_score: i32,
    pub sort_order: i32,
}

/// Contest problems list response
#[derive(Debug, Serialize)]
pub struct ContestProblemsResponse {
    pub problems: Vec<ContestProblemInfo>,
}

/// Simple message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}
