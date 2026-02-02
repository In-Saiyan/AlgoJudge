//! Context types for specification evaluation.
//!
//! Contexts carry the necessary information for specifications to evaluate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
