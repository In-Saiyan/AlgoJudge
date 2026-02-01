//! Submission request DTOs

use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

/// Create submission request (legacy - source code)
#[derive(Debug, Deserialize, Validate)]
pub struct CreateSubmissionRequest {
    /// Problem ID to submit for
    pub problem_id: Uuid,

    /// Contest ID (optional - for contest submissions)
    pub contest_id: Option<Uuid>,

    /// Programming language / runtime
    #[validate(length(min = 1, max = 20))]
    pub language: String,

    /// Source code (for legacy submissions)
    #[validate(length(min = 1, max = 1048576))] // 1MB max
    pub source_code: String,
}

/// Create ZIP submission request (new algorithmic benchmarking)
/// 
/// ZIP must contain:
/// - compile.sh: Script to compile the solution
/// - run.sh: Script to run the compiled binary
/// 
/// The compiled binary should be named after the problem code (A, B, etc.)
#[derive(Debug, Deserialize, Validate)]
pub struct CreateZipSubmissionRequest {
    /// Problem ID to submit for
    pub problem_id: Uuid,

    /// Contest ID (optional - for contest submissions)
    pub contest_id: Option<Uuid>,

    /// Runtime environment name (e.g., "cpp", "rust", "go")
    #[validate(length(min = 1, max = 50))]
    pub runtime: String,

    /// Base64 encoded ZIP file
    #[validate(length(min = 1))]
    pub submission_zip_base64: String,

    /// Optional: Custom test case generator (base64 encoded binary)
    /// If provided, overrides the problem's default generator
    pub custom_generator_base64: Option<String>,
    
    /// Original filename of custom generator
    pub custom_generator_filename: Option<String>,
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
