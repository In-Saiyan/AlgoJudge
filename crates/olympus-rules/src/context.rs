//! Context types for specification evaluation.
//!
//! Contexts carry the necessary information for specifications to evaluate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(feature = "auth")]
use std::sync::Arc;
#[cfg(feature = "auth")]
use uuid::Uuid;

/// Generic evaluation context that can hold arbitrary values.
#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    /// String values
    pub strings: HashMap<String, String>,
    /// Integer values
    pub integers: HashMap<String, i64>,
    /// Boolean values
    pub booleans: HashMap<String, bool>,
}

impl EvalContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_string(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.strings.insert(key.into(), value.into());
        self
    }

    pub fn with_int(mut self, key: impl Into<String>, value: i64) -> Self {
        self.integers.insert(key.into(), value);
        self
    }

    pub fn with_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.booleans.insert(key.into(), value);
        self
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(|s| s.as_str())
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.integers.get(key).copied()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.booleans.get(key).copied()
    }
}

/// File metadata context for cleanup rules (Horus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: String,
    pub is_file: bool,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub created_at: i64,      // Unix timestamp
    pub modified_at: i64,     // Unix timestamp
    pub accessed_at: i64,     // Unix timestamp
}

/// Execution result context for judge rules (Minos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub submission_id: String,
    pub problem_id: String,
    pub test_case_id: String,
    pub exit_code: i32,
    pub time_ms: u64,
    pub memory_kb: u64,
    pub time_limit_ms: u64,
    pub memory_limit_kb: u64,
    pub output_matches: bool,
}

/// Authorization context for Vanguard access control.
/// 
/// This context carries user identity and database/cache access for
/// evaluating authorization rules asynchronously.
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct AuthContext {
    /// Current user ID
    pub user_id: Uuid,
    /// Current user's role (admin, organizer, participant, spectator)
    pub role: String,
    /// Is the user currently banned?
    pub is_banned: bool,
    /// Database pool for async lookups (wrapped in Arc for Clone)
    pub db: Arc<sqlx::PgPool>,
    /// Redis pool for rate limiting checks (wrapped in Arc for Clone)
    pub redis: Arc<deadpool_redis::Pool>,
    /// Optional: Target contest ID for contest-scoped rules
    pub contest_id: Option<Uuid>,
    /// Optional: Target problem ID for problem-scoped rules
    pub problem_id: Option<Uuid>,
    /// Optional: Target submission ID for submission-scoped rules
    pub submission_id: Option<Uuid>,
}

#[cfg(feature = "auth")]
impl std::fmt::Debug for AuthContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthContext")
            .field("user_id", &self.user_id)
            .field("role", &self.role)
            .field("is_banned", &self.is_banned)
            .field("contest_id", &self.contest_id)
            .field("problem_id", &self.problem_id)
            .field("submission_id", &self.submission_id)
            .finish()
    }
}

#[cfg(feature = "auth")]
impl AuthContext {
    /// Create a new authorization context
    pub fn new(
        user_id: Uuid,
        role: String,
        is_banned: bool,
        db: Arc<sqlx::PgPool>,
        redis: Arc<deadpool_redis::Pool>,
    ) -> Self {
        Self {
            user_id,
            role,
            is_banned,
            db,
            redis,
            contest_id: None,
            problem_id: None,
            submission_id: None,
        }
    }

    /// Set target contest for evaluation
    pub fn with_contest(mut self, contest_id: Uuid) -> Self {
        self.contest_id = Some(contest_id);
        self
    }

    /// Set target problem for evaluation
    pub fn with_problem(mut self, problem_id: Uuid) -> Self {
        self.problem_id = Some(problem_id);
        self
    }

    /// Set target submission for evaluation
    pub fn with_submission(mut self, submission_id: Uuid) -> Self {
        self.submission_id = Some(submission_id);
        self
    }
}
