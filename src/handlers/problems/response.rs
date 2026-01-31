//! Problem response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::request::SampleIO;

/// Problem response
#[derive(Debug, Serialize)]
pub struct ProblemResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,
    pub samples: Vec<SampleIO>,
    pub notes: Option<String>,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub difficulty: Option<String>,
    pub tags: Vec<String>,
    pub is_public: bool,
    pub author_id: Uuid,
    pub author_name: String,
    pub solved_count: i64,
    pub attempt_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Problem list response
#[derive(Debug, Serialize)]
pub struct ProblemsListResponse {
    pub problems: Vec<ProblemSummary>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// Problem summary for list views
#[derive(Debug, Serialize)]
pub struct ProblemSummary {
    pub id: Uuid,
    pub title: String,
    pub difficulty: Option<String>,
    pub tags: Vec<String>,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub solved_count: i64,
    pub attempt_count: i64,
}

/// Test case response
#[derive(Debug, Serialize)]
pub struct TestCaseResponse {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub order: i32,
    pub is_sample: bool,
    pub points: Option<i32>,
    /// Input is only shown for sample test cases
    pub input: Option<String>,
    /// Output is only shown for sample test cases
    pub expected_output: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Test cases list response
#[derive(Debug, Serialize)]
pub struct TestCasesListResponse {
    pub test_cases: Vec<TestCaseResponse>,
    pub total: i64,
}

/// Test case response for admins/organizers (full data)
#[derive(Debug, Serialize)]
pub struct TestCaseFullResponse {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub order: i32,
    pub is_sample: bool,
    pub points: Option<i32>,
    pub input: String,
    pub expected_output: String,
    pub created_at: DateTime<Utc>,
}
