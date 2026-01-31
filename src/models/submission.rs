//! Submission model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Submission database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Submission {
    pub id: Uuid,
    pub user_id: Uuid,
    pub problem_id: Uuid,
    pub contest_id: Option<Uuid>,
    pub language: String,
    #[serde(skip_serializing)]
    pub source_code: String,
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
    TimeLimitExceeded,
    MemoryLimitExceeded,
    RuntimeError,
    CompilationError,
    InternalError,
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
            Self::TimeLimitExceeded => "time_limit_exceeded",
            Self::MemoryLimitExceeded => "memory_limit_exceeded",
            Self::RuntimeError => "runtime_error",
            Self::CompilationError => "compilation_error",
            Self::InternalError => "internal_error",
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
            "time_limit_exceeded" => Some(Self::TimeLimitExceeded),
            "memory_limit_exceeded" => Some(Self::MemoryLimitExceeded),
            "runtime_error" => Some(Self::RuntimeError),
            "compilation_error" => Some(Self::CompilationError),
            "internal_error" => Some(Self::InternalError),
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
    pub test_case_id: Uuid,
    pub verdict: String,
    pub execution_time_ms: Option<f64>,
    pub memory_usage_kb: Option<i64>,
    pub actual_output: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}
