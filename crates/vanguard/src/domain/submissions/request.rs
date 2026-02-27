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
///
/// If `contest_id` is provided, the problem must be in that contest and
/// the user must be a participant/collaborator/admin. If omitted, this is
/// a standalone (practice) submission — only the problem is validated.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateSubmissionRequest {
    /// Optional contest scope. `None` = standalone submission.
    pub contest_id: Option<Uuid>,
    pub problem_id: Uuid,
    
    pub language: Language,
    
    #[validate(length(min = 1, max = 65536, message = "Source code must be 1-65536 characters"))]
    pub source_code: String,
}

/// ZIP submission upload query parameters
/// Used with multipart/form-data file upload
///
/// If `contest_id` is provided, contest validation applies.
/// If omitted, this is a standalone (practice) submission.
#[derive(Debug, Deserialize)]
pub struct ZipSubmissionParams {
    /// Optional contest scope. `None` = standalone submission.
    pub contest_id: Option<Uuid>,
    pub problem_id: Uuid,
    /// Optional language hint so Sisyphus can set up the correct compiler
    /// toolchain before running compile.sh. If omitted, Sisyphus relies
    /// entirely on compile.sh to handle compilation.
    pub language: Option<Language>,
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
