//! Submission request DTOs.

use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

/// Supported programming languages
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Cpp,
    C,
    Rust,
    Go,
    Python,
    Zig,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Cpp => write!(f, "cpp"),
            Language::C => write!(f, "c"),
            Language::Rust => write!(f, "rust"),
            Language::Go => write!(f, "go"),
            Language::Python => write!(f, "python"),
            Language::Zig => write!(f, "zig"),
        }
    }
}

/// Create submission request (legacy source code)
#[derive(Debug, Deserialize, Validate)]
pub struct CreateSubmissionRequest {
    pub contest_id: Uuid,
    pub problem_id: Uuid,
    
    pub language: Language,
    
    #[validate(length(min = 1, max = 65536, message = "Source code must be 1-65536 characters"))]
    pub source_code: String,
}

/// List submissions query parameters
#[derive(Debug, Deserialize)]
pub struct ListSubmissionsQuery {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,

    /// Filter by contest
    pub contest_id: Option<Uuid>,

    /// Filter by problem
    pub problem_id: Option<Uuid>,

    /// Filter by user
    pub user_id: Option<Uuid>,

    /// Filter by status
    pub status: Option<String>,

    /// Filter by language
    pub language: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Leaderboard query parameters
#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_leaderboard_per_page")]
    pub per_page: u32,
}

fn default_leaderboard_per_page() -> u32 {
    50
}
