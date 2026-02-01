//! Test case model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Test case database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TestCase {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub input: String,
    pub expected_output: String,
    pub is_sample: bool,
    pub points: Option<i32>,
    pub order: i32,
    pub created_at: DateTime<Utc>,
}

impl TestCase {
    /// Get a preview of the input (truncated)
    pub fn input_preview(&self, max_len: usize) -> String {
        if self.input.len() <= max_len {
            self.input.clone()
        } else {
            format!("{}...", &self.input[..max_len])
        }
    }

    /// Get a preview of the expected output (truncated)
    pub fn output_preview(&self, max_len: usize) -> String {
        if self.expected_output.len() <= max_len {
            self.expected_output.clone()
        } else {
            format!("{}...", &self.expected_output[..max_len])
        }
    }
}
