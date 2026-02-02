//! Problem request DTOs.

use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

/// Problem difficulty levels
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "easy"),
            Difficulty::Medium => write!(f, "medium"),
            Difficulty::Hard => write!(f, "hard"),
            Difficulty::Expert => write!(f, "expert"),
        }
    }
}

/// Create problem request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateProblemRequest {
    #[validate(length(min = 3, max = 255, message = "Title must be 3-255 characters"))]
    pub title: String,

    #[validate(length(min = 10, message = "Description must be at least 10 characters"))]
    pub description: String,

    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,

    pub sample_input: Option<String>,
    pub sample_output: Option<String>,
    pub sample_explanation: Option<String>,

    pub difficulty: Option<Difficulty>,
    pub tags: Option<Vec<String>>,

    #[validate(range(min = 100, max = 30000, message = "Time limit must be 100-30000 ms"))]
    #[serde(default = "default_time_limit")]
    pub time_limit_ms: i32,

    #[validate(range(min = 16384, max = 1048576, message = "Memory limit must be 16-1024 MB"))]
    #[serde(default = "default_memory_limit")]
    pub memory_limit_kb: i32,

    #[validate(range(min = 1, max = 100, message = "Number of test cases must be 1-100"))]
    #[serde(default = "default_num_test_cases")]
    pub num_test_cases: i32,

    // Note: generator and checker binaries are uploaded separately via
    // POST /api/v1/problems/{id}/generator and POST /api/v1/problems/{id}/checker

    #[validate(range(min = 1, max = 10000, message = "Max score must be 1-10000"))]
    #[serde(default = "default_max_score")]
    pub max_score: i32,

    #[serde(default)]
    pub partial_scoring: bool,

    #[serde(default)]
    pub is_public: bool,

    pub allowed_languages: Option<Vec<String>>,
}

fn default_time_limit() -> i32 {
    1000
}

fn default_memory_limit() -> i32 {
    262144 // 256 MB
}

fn default_num_test_cases() -> i32 {
    10
}

fn default_max_score() -> i32 {
    100
}

/// Update problem request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProblemRequest {
    #[validate(length(min = 3, max = 255, message = "Title must be 3-255 characters"))]
    pub title: Option<String>,

    #[validate(length(min = 10, message = "Description must be at least 10 characters"))]
    pub description: Option<String>,

    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,

    pub sample_input: Option<String>,
    pub sample_output: Option<String>,
    pub sample_explanation: Option<String>,

    pub difficulty: Option<Difficulty>,
    pub tags: Option<Vec<String>>,

    #[validate(range(min = 100, max = 30000, message = "Time limit must be 100-30000 ms"))]
    pub time_limit_ms: Option<i32>,

    #[validate(range(min = 16384, max = 1048576, message = "Memory limit must be 16-1024 MB"))]
    pub memory_limit_kb: Option<i32>,

    #[validate(range(min = 1, max = 100, message = "Number of test cases must be 1-100"))]
    pub num_test_cases: Option<i32>,

    // Note: generator and checker binaries are uploaded separately via
    // POST /api/v1/problems/{id}/generator and POST /api/v1/problems/{id}/checker

    #[validate(range(min = 1, max = 10000, message = "Max score must be 1-10000"))]
    pub max_score: Option<i32>,

    pub partial_scoring: Option<bool>,

    pub is_public: Option<bool>,

    pub allowed_languages: Option<Vec<String>>,
}

/// List problems query parameters
#[derive(Debug, Deserialize)]
pub struct ListProblemsQuery {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,

    /// Filter by difficulty
    pub difficulty: Option<String>,

    /// Filter by tag
    pub tag: Option<String>,

    /// Filter by owner
    pub owner_id: Option<Uuid>,

    /// Search by title
    pub search: Option<String>,

    /// Only public problems
    #[serde(default = "default_true")]
    pub public_only: bool,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

fn default_true() -> bool {
    true
}

/// Add problem to contest request
#[derive(Debug, Deserialize, Validate)]
pub struct AddProblemToContestRequest {
    pub problem_id: Uuid,

    #[validate(length(min = 1, max = 10, message = "Problem code must be 1-10 characters"))]
    pub problem_code: String,

    pub sort_order: Option<i32>,

    /// Override max score for this contest
    pub max_score: Option<i32>,

    /// Override time limit for this contest
    pub time_limit_ms: Option<i32>,

    /// Override memory limit for this contest
    pub memory_limit_kb: Option<i32>,
}
