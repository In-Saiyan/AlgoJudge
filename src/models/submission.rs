//! Submission model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Submission database model
/// 
/// Users submit a ZIP file containing:
/// - compile.sh: Script to compile the solution
/// - run.sh: Script to run the compiled solution
/// 
/// The compiled binary should be named after the problem code (e.g., A, B, C)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Submission {
    pub id: Uuid,
    pub user_id: Uuid,
    pub problem_id: Uuid,
    pub contest_id: Option<Uuid>,
    /// Legacy field for simple submissions
    pub language: String,
    /// Legacy field - use submission_zip for new submissions
    #[serde(skip_serializing)]
    pub source_code: String,
    /// The submitted ZIP file containing compile.sh and run.sh
    #[sqlx(default)]
    #[serde(skip_serializing)]
    pub submission_zip: Option<Vec<u8>>,
    /// Runtime environment ID
    pub runtime_id: Option<Uuid>,
    /// Optional custom test case generator (overrides problem's generator)
    #[sqlx(default)]
    #[serde(skip_serializing)]
    pub custom_generator_binary: Option<Vec<u8>>,
    pub custom_generator_filename: Option<String>,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    pub score: Option<i32>,
    pub compilation_output: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub judged_at: Option<DateTime<Utc>>,
}

/// Submission verdict enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pending,
    Compiling,
    Running,
    Accepted,
    WrongAnswer,
    Partial,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    RuntimeError,
    CompilationError,
    InvalidFormat,
    SystemError,
}

impl Verdict {
    /// Get verdict as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Compiling => "compiling",
            Self::Running => "running",
            Self::Accepted => "accepted",
            Self::WrongAnswer => "wrong_answer",
            Self::Partial => "partial",
            Self::TimeLimitExceeded => "time_limit_exceeded",
            Self::MemoryLimitExceeded => "memory_limit_exceeded",
            Self::RuntimeError => "runtime_error",
            Self::CompilationError => "compilation_error",
            Self::InvalidFormat => "invalid_format",
            Self::SystemError => "system_error",
        }
    }

    /// Parse verdict from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "compiling" => Some(Self::Compiling),
            "running" => Some(Self::Running),
            "accepted" => Some(Self::Accepted),
            "wrong_answer" => Some(Self::WrongAnswer),
            "partial" => Some(Self::Partial),
            "time_limit_exceeded" => Some(Self::TimeLimitExceeded),
            "memory_limit_exceeded" => Some(Self::MemoryLimitExceeded),
            "runtime_error" => Some(Self::RuntimeError),
            "compilation_error" => Some(Self::CompilationError),
            "invalid_format" => Some(Self::InvalidFormat),
            "system_error" => Some(Self::SystemError),
            // Legacy mapping
            "internal_error" => Some(Self::SystemError),
            _ => None,
        }
    }

    /// Check if this is a final verdict (judging complete)
    pub fn is_final(&self) -> bool {
        !matches!(self, Self::Pending | Self::Compiling | Self::Running)
    }

    /// Check if this verdict means the solution was accepted
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
    
    /// Check if this verdict means partial credit
    pub fn is_partial(&self) -> bool {
        matches!(self, Self::Partial)
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Test case result for a submission
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TestCaseResult {
    pub id: Uuid,
    pub submission_id: Uuid,
    /// Optional - for legacy static test cases
    pub test_case_id: Option<Uuid>,
    /// Test case number (1, 2, 3...) for generated test cases
    pub test_case_number: Option<i32>,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    pub actual_output: Option<String>,
    pub error_message: Option<String>,
    /// Match percentage returned by verifier (0.0 - 100.0)
    pub match_percentage: Option<f64>,
    /// Raw output from the verifier
    pub verifier_output: Option<String>,
}
