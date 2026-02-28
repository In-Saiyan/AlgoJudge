//! Verdict types and determination logic

use serde::{Deserialize, Serialize};

/// Verdict for a single test case or entire submission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// Passed all checks
    Accepted,
    /// Output does not match expected
    WrongAnswer,
    /// Exceeded time limit
    TimeLimitExceeded,
    /// Exceeded memory limit
    MemoryLimitExceeded,
    /// Program crashed or non-zero exit
    RuntimeError,
    /// Output too large
    OutputLimitExceeded,
    /// Internal judge error
    JudgeError,
    /// Currently being judged
    Judging,
    /// Waiting in queue
    Pending,
}

impl Verdict {
    /// Get short code for verdict
    pub fn code(&self) -> &'static str {
        match self {
            Verdict::Accepted => "AC",
            Verdict::WrongAnswer => "WA",
            Verdict::TimeLimitExceeded => "TLE",
            Verdict::MemoryLimitExceeded => "MLE",
            Verdict::RuntimeError => "RE",
            Verdict::OutputLimitExceeded => "OLE",
            Verdict::JudgeError => "JE",
            Verdict::Judging => "JG",
            Verdict::Pending => "PD",
        }
    }

    /// Check if verdict is a failure (not accepted)
    pub fn is_failure(&self) -> bool {
        !matches!(self, Verdict::Accepted | Verdict::Pending | Verdict::Judging)
    }

    /// Check if verdict is final (not pending/judging)
    pub fn is_final(&self) -> bool {
        !matches!(self, Verdict::Pending | Verdict::Judging)
    }

    /// Convert to database string representation (must match CHECK constraints).
    pub fn to_db_string(&self) -> &'static str {
        match self {
            Verdict::Accepted => "accepted",
            Verdict::WrongAnswer => "wrong_answer",
            Verdict::TimeLimitExceeded => "time_limit",
            Verdict::MemoryLimitExceeded => "memory_limit",
            Verdict::RuntimeError => "runtime_error",
            Verdict::OutputLimitExceeded => "runtime_error", // mapped to runtime_error in DB
            Verdict::JudgeError => "system_error",
            Verdict::Judging => "judging",
            Verdict::Pending => "pending",
        }
    }

    /// Create from database string
    pub fn from_db_string(s: &str) -> Option<Self> {
        match s {
            "accepted" => Some(Verdict::Accepted),
            "wrong_answer" => Some(Verdict::WrongAnswer),
            "time_limit" => Some(Verdict::TimeLimitExceeded),
            "memory_limit" => Some(Verdict::MemoryLimitExceeded),
            "runtime_error" => Some(Verdict::RuntimeError),
            "system_error" => Some(Verdict::JudgeError),
            "judging" => Some(Verdict::Judging),
            "pending" => Some(Verdict::Pending),
            _ => None,
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Result of executing a single test case
#[derive(Debug, Clone)]
pub struct TestCaseResult {
    /// Test case number (1-indexed)
    pub testcase_number: i32,

    /// Verdict for this test case
    pub verdict: Verdict,

    /// Execution time in milliseconds
    pub time_ms: u64,

    /// Peak memory usage in KB
    pub memory_kb: u64,

    /// Exit code (if applicable)
    pub exit_code: Option<i32>,

    /// Error message (for RE, JE)
    pub error_message: Option<String>,

    /// Checker output/comment (if any)
    pub checker_comment: Option<String>,
}

impl TestCaseResult {
    /// Create a new accepted result
    pub fn accepted(testcase_number: i32, time_ms: u64, memory_kb: u64) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::Accepted,
            time_ms,
            memory_kb,
            exit_code: Some(0),
            error_message: None,
            checker_comment: None,
        }
    }

    /// Create a wrong answer result
    pub fn wrong_answer(testcase_number: i32, time_ms: u64, memory_kb: u64, comment: Option<String>) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::WrongAnswer,
            time_ms,
            memory_kb,
            exit_code: Some(0),
            error_message: None,
            checker_comment: comment,
        }
    }

    /// Create a time limit exceeded result
    pub fn time_limit_exceeded(testcase_number: i32, time_limit_ms: u64, memory_kb: u64) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::TimeLimitExceeded,
            time_ms: time_limit_ms,
            memory_kb,
            exit_code: None,
            error_message: Some("Time limit exceeded".to_string()),
            checker_comment: None,
        }
    }

    /// Create a memory limit exceeded result
    pub fn memory_limit_exceeded(testcase_number: i32, time_ms: u64, memory_limit_kb: u64) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::MemoryLimitExceeded,
            time_ms,
            memory_kb: memory_limit_kb,
            exit_code: None,
            error_message: Some("Memory limit exceeded".to_string()),
            checker_comment: None,
        }
    }

    /// Create a runtime error result
    pub fn runtime_error(testcase_number: i32, time_ms: u64, memory_kb: u64, exit_code: i32, message: String) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::RuntimeError,
            time_ms,
            memory_kb,
            exit_code: Some(exit_code),
            error_message: Some(message),
            checker_comment: None,
        }
    }

    /// Create a judge error result
    pub fn judge_error(testcase_number: i32, message: String) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::JudgeError,
            time_ms: 0,
            memory_kb: 0,
            exit_code: None,
            error_message: Some(message),
            checker_comment: None,
        }
    }

    /// Create an output limit exceeded result
    pub fn output_limit_exceeded(testcase_number: i32, time_ms: u64, memory_kb: u64) -> Self {
        Self {
            testcase_number,
            verdict: Verdict::OutputLimitExceeded,
            time_ms,
            memory_kb,
            exit_code: None,
            error_message: Some("Output limit exceeded".to_string()),
            checker_comment: None,
        }
    }
}

/// Aggregated result for entire submission
#[derive(Debug, Clone)]
pub struct SubmissionResult {
    /// Overall verdict
    pub verdict: Verdict,

    /// Results for each test case
    pub testcase_results: Vec<TestCaseResult>,

    /// Number of passed test cases
    pub passed_count: i32,

    /// Total number of test cases
    pub total_count: i32,

    /// Maximum time across all test cases (ms)
    pub max_time_ms: u64,

    /// Maximum memory across all test cases (KB)
    pub max_memory_kb: u64,

    /// First failing test case number (if any)
    pub first_failure: Option<i32>,

    /// Score (0-100)
    pub score: f64,
}

impl SubmissionResult {
    /// Create submission result from test case results
    pub fn from_testcases(results: Vec<TestCaseResult>) -> Self {
        let total_count = results.len() as i32;
        let passed_count = results.iter().filter(|r| r.verdict == Verdict::Accepted).count() as i32;

        let max_time_ms = results.iter().map(|r| r.time_ms).max().unwrap_or(0);
        let max_memory_kb = results.iter().map(|r| r.memory_kb).max().unwrap_or(0);

        // Find first failure
        let first_failure = results
            .iter()
            .find(|r| r.verdict.is_failure())
            .map(|r| r.testcase_number);

        // Determine overall verdict
        let verdict = if passed_count == total_count {
            Verdict::Accepted
        } else if let Some(first_fail) = results.iter().find(|r| r.verdict.is_failure()) {
            first_fail.verdict
        } else {
            Verdict::JudgeError
        };

        // Calculate score
        let score = if total_count > 0 {
            (passed_count as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };

        Self {
            verdict,
            testcase_results: results,
            passed_count,
            total_count,
            max_time_ms,
            max_memory_kb,
            first_failure,
            score,
        }
    }
}
