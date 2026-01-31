//! Submission request DTOs

use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

/// Create submission request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateSubmissionRequest {
    /// Problem ID to submit for
    pub problem_id: Uuid,

    /// Contest ID (optional - for contest submissions)
    pub contest_id: Option<Uuid>,

    /// Programming language
    #[validate(length(min = 1, max = 20))]
    pub language: String,

    /// Source code
    #[validate(length(min = 1, max = 1048576))] // 1MB max
    pub source_code: String,
}

/// List submissions query parameters
#[derive(Debug, Deserialize)]
pub struct ListSubmissionsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub problem_id: Option<Uuid>,
    pub contest_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub language: Option<String>,
    pub verdict: Option<String>,
}
