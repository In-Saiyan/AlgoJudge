//! Common types used across Olympus services.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User ID type
pub type UserId = Uuid;

/// Contest ID type
pub type ContestId = Uuid;

/// Problem ID type
pub type ProblemId = Uuid;

/// Submission ID type
pub type SubmissionId = Uuid;

/// Test case ID type
pub type TestCaseId = Uuid;

/// User role in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    /// Full system access
    Admin,
    /// Can create/manage contests and problems
    Organizer,
    /// Can participate in contests and submit solutions
    Participant,
    /// Can view public contests and leaderboards
    Spectator,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Participant
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Organizer => write!(f, "organizer"),
            UserRole::Participant => write!(f, "participant"),
            UserRole::Spectator => write!(f, "spectator"),
        }
    }
}

/// Submission status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmissionStatus {
    /// Waiting in queue
    Pending,
    /// Currently compiling
    Compiling,
    /// Compilation successful
    Compiled,
    /// Compilation failed
    CompilationError,
    /// Currently running tests
    Running,
    /// All tests passed
    Accepted,
    /// Output mismatch
    WrongAnswer,
    /// Exceeded time limit
    TimeLimitExceeded,
    /// Exceeded memory limit
    MemoryLimitExceeded,
    /// Program crashed
    RuntimeError,
    /// Internal judge error
    InternalError,
}

impl std::fmt::Display for SubmissionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmissionStatus::Pending => write!(f, "PENDING"),
            SubmissionStatus::Compiling => write!(f, "COMPILING"),
            SubmissionStatus::Compiled => write!(f, "COMPILED"),
            SubmissionStatus::CompilationError => write!(f, "COMPILATION_ERROR"),
            SubmissionStatus::Running => write!(f, "RUNNING"),
            SubmissionStatus::Accepted => write!(f, "ACCEPTED"),
            SubmissionStatus::WrongAnswer => write!(f, "WRONG_ANSWER"),
            SubmissionStatus::TimeLimitExceeded => write!(f, "TIME_LIMIT_EXCEEDED"),
            SubmissionStatus::MemoryLimitExceeded => write!(f, "MEMORY_LIMIT_EXCEEDED"),
            SubmissionStatus::RuntimeError => write!(f, "RUNTIME_ERROR"),
            SubmissionStatus::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

/// Verdict for a single test case
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// Test passed
    Accepted,
    /// Output mismatch
    WrongAnswer,
    /// Exceeded time limit
    TimeLimitExceeded,
    /// Exceeded memory limit
    MemoryLimitExceeded,
    /// Program crashed
    RuntimeError,
    /// Presentation error (whitespace issues)
    PresentationError,
}

/// Supported runtime/language for submissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Cpp,
    C,
    Rust,
    Go,
    Python,
    Zig,
}

impl std::fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runtime::Cpp => write!(f, "cpp"),
            Runtime::C => write!(f, "c"),
            Runtime::Rust => write!(f, "rust"),
            Runtime::Go => write!(f, "go"),
            Runtime::Python => write!(f, "python"),
            Runtime::Zig => write!(f, "zig"),
        }
    }
}

/// Contest status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContestStatus {
    /// Contest is being prepared
    Draft,
    /// Contest is open for registration
    Registration,
    /// Contest is active
    Running,
    /// Contest has ended
    Finished,
    /// Contest is archived
    Archived,
}

impl Default for ContestStatus {
    fn default() -> Self {
        ContestStatus::Draft
    }
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Pagination {
            page: 1,
            per_page: 20,
        }
    }
}

impl Pagination {
    /// Calculate offset for SQL queries
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.per_page
    }

    /// Get limit for SQL queries
    pub fn limit(&self) -> u32 {
        self.per_page
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        PaginatedResponse {
            data,
            page,
            per_page,
            total,
            total_pages,
        }
    }
}
