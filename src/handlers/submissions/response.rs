//! Submission response DTOs

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Submission response
#[derive(Debug, Serialize)]
pub struct SubmissionResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub problem_id: Uuid,
    pub problem_title: String,
    pub contest_id: Option<Uuid>,
    pub language: String,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    pub score: Option<i32>,
    pub submitted_at: DateTime<Utc>,
    pub judged_at: Option<DateTime<Utc>>,
}

/// Submission list response
#[derive(Debug, Serialize)]
pub struct SubmissionsListResponse {
    pub submissions: Vec<SubmissionResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// Detailed submission results
#[derive(Debug, Serialize)]
pub struct SubmissionResultsResponse {
    pub submission_id: Uuid,
    pub verdict: String,
    pub score: Option<i32>,
    pub compilation_output: Option<String>,
    pub test_results: Vec<TestCaseResult>,
    pub benchmark_summary: Option<BenchmarkSummary>,
}

/// Result for a single test case
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TestCaseResult {
    pub test_case_id: Uuid,
    pub test_case_order: i32,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    /// Only shown for sample test cases or to admins
    pub input_preview: Option<String>,
    pub expected_output_preview: Option<String>,
    pub actual_output_preview: Option<String>,
    pub error_message: Option<String>,
}

/// Benchmark summary across all iterations
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BenchmarkSummary {
    /// Number of iterations run (excluding warm-up)
    pub iterations: i32,

    // Time metrics (in milliseconds)
    pub time_avg_ms: f64,
    pub time_median_ms: f64,
    pub time_min_ms: f64,
    pub time_max_ms: f64,
    pub time_stddev_ms: f64,

    // Memory metrics (in kilobytes)
    pub memory_avg_kb: i64,
    pub memory_peak_kb: i64,

    // Outlier information (stored as JSON)
    pub time_outliers: serde_json::Value,
}

/// Information about an outlier measurement
#[derive(Debug, Serialize)]
pub struct OutlierInfo {
    pub iteration: u32,
    pub value_ms: f64,
    pub deviation_percent: f64,
}

/// Source code response
#[derive(Debug, Serialize)]
pub struct SubmissionSourceResponse {
    pub submission_id: Uuid,
    pub language: String,
    pub source_code: String,
    pub submitted_at: DateTime<Utc>,
}

/// Create submission response
#[derive(Debug, Serialize)]
pub struct CreateSubmissionResponse {
    pub id: Uuid,
    pub message: String,
    pub status: String,
}
