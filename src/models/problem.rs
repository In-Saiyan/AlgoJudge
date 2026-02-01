//! Problem model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Problem database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Problem {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub constraints: Option<String>,
    pub samples: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub time_limit_ms: i32,
    pub memory_limit_kb: i32,
    pub difficulty: Option<String>,
    pub tags: Vec<String>,
    pub is_public: bool,
    pub author_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Problem {
    /// Get time limit in seconds
    pub fn time_limit_seconds(&self) -> f64 {
        self.time_limit_ms as f64 / 1000.0
    }

    /// Get memory limit in megabytes
    pub fn memory_limit_mb(&self) -> i32 {
        self.memory_limit_kb / 1024
    }
}

/// Problem difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Easy => write!(f, "easy"),
            Self::Medium => write!(f, "medium"),
            Self::Hard => write!(f, "hard"),
            Self::Expert => write!(f, "expert"),
        }
    }
}
