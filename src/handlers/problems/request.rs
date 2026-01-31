//! Problem request DTOs

use serde::Deserialize;
use validator::Validate;

use crate::constants::{MAX_PROBLEM_DESCRIPTION_LENGTH, MAX_PROBLEM_TITLE_LENGTH};

/// Create problem request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateProblemRequest {
    #[validate(length(min = 1, max = MAX_PROBLEM_TITLE_LENGTH))]
    pub title: String,

    #[validate(length(max = MAX_PROBLEM_DESCRIPTION_LENGTH))]
    pub description: String,

    /// Input format description
    pub input_format: Option<String>,

    /// Output format description
    pub output_format: Option<String>,

    /// Constraints description
    pub constraints: Option<String>,

    /// Sample input/output pairs for display
    pub samples: Option<Vec<SampleIO>>,

    /// Notes/hints
    pub notes: Option<String>,

    /// Time limit in milliseconds
    pub time_limit_ms: i64,

    /// Memory limit in kilobytes
    pub memory_limit_kb: i64,

    /// Problem difficulty (optional)
    pub difficulty: Option<String>,

    /// Tags for categorization
    pub tags: Option<Vec<String>>,

    /// Is this problem visible to participants?
    pub is_public: Option<bool>,
}

/// Sample input/output pair
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct SampleIO {
    pub input: String,
    pub output: String,
    pub explanation: Option<String>,
}

/// Update problem request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProblemRequest {
    #[validate(length(min = 1, max = MAX_PROBLEM_TITLE_LENGTH))]
    pub title: Option<String>,

    #[validate(length(max = MAX_PROBLEM_DESCRIPTION_LENGTH))]
    pub description: Option<String>,

    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,
    pub samples: Option<Vec<SampleIO>>,
    pub notes: Option<String>,
    pub time_limit_ms: Option<i64>,
    pub memory_limit_kb: Option<i64>,
    pub difficulty: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_public: Option<bool>,
}

/// List problems query parameters
#[derive(Debug, Deserialize)]
pub struct ListProblemsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub difficulty: Option<String>,
    pub tag: Option<String>,
}

/// Create test case request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateTestCaseRequest {
    /// Input data (can be raw text or base64 for binary)
    pub input: String,

    /// Expected output
    pub expected_output: String,

    /// Is this test case visible to participants (sample)?
    pub is_sample: Option<bool>,

    /// Points for this test case (IOI mode)
    pub points: Option<i32>,

    /// Test case order
    pub order: Option<i32>,
}

/// Update test case request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTestCaseRequest {
    pub input: Option<String>,
    pub expected_output: Option<String>,
    pub is_sample: Option<bool>,
    pub points: Option<i32>,
    pub order: Option<i32>,
}
