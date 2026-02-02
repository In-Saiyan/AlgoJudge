//! Contest request DTOs.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use validator::Validate;

/// Scoring type for contests
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ScoringType {
    Icpc,
    Ioi,
    Custom,
}

impl Default for ScoringType {
    fn default() -> Self {
        Self::Icpc
    }
}

impl std::fmt::Display for ScoringType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScoringType::Icpc => write!(f, "icpc"),
            ScoringType::Ioi => write!(f, "ioi"),
            ScoringType::Custom => write!(f, "custom"),
        }
    }
}

/// Create contest request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateContestRequest {
    #[validate(length(min = 3, max = 255, message = "Title must be 3-255 characters"))]
    pub title: String,

    pub description: Option<String>,

    #[validate(length(max = 500, message = "Short description must be at most 500 characters"))]
    pub short_description: Option<String>,

    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub freeze_time: Option<DateTime<Utc>>,

    #[serde(default)]
    pub scoring_type: ScoringType,

    #[serde(default = "default_true")]
    pub is_public: bool,

    #[serde(default)]
    pub is_rated: bool,

    #[serde(default = "default_true")]
    pub registration_required: bool,

    pub max_participants: Option<i32>,

    pub allowed_languages: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

/// Update contest request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateContestRequest {
    #[validate(length(min = 3, max = 255, message = "Title must be 3-255 characters"))]
    pub title: Option<String>,

    pub description: Option<String>,

    #[validate(length(max = 500, message = "Short description must be at most 500 characters"))]
    pub short_description: Option<String>,

    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub freeze_time: Option<DateTime<Utc>>,

    pub scoring_type: Option<ScoringType>,

    pub is_public: Option<bool>,
    pub is_rated: Option<bool>,
    pub registration_required: Option<bool>,

    pub max_participants: Option<i32>,

    pub allowed_languages: Option<Vec<String>>,
}

/// List contests query parameters
#[derive(Debug, Deserialize)]
pub struct ListContestsQuery {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,

    /// Filter: upcoming, ongoing, past, all
    pub status: Option<String>,

    /// Filter by owner_id
    pub owner_id: Option<uuid::Uuid>,

    /// Search by title
    pub search: Option<String>,

    /// Only public contests
    #[serde(default = "default_true")]
    pub public_only: bool,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Add collaborator request
#[derive(Debug, Deserialize, Validate)]
pub struct AddCollaboratorRequest {
    pub user_id: uuid::Uuid,

    #[validate(custom(function = "validate_collaborator_role"))]
    pub role: String,

    #[serde(default)]
    pub can_edit_contest: bool,

    #[serde(default)]
    pub can_add_problems: bool,

    #[serde(default = "default_true")]
    pub can_view_submissions: bool,
}

fn validate_collaborator_role(role: &str) -> Result<(), validator::ValidationError> {
    match role {
        "co-owner" | "problem-setter" | "tester" => Ok(()),
        _ => {
            let mut err = validator::ValidationError::new("invalid_role");
            err.message = Some("Role must be co-owner, problem-setter, or tester".into());
            Err(err)
        }
    }
}

/// List participants query
#[derive(Debug, Deserialize)]
pub struct ListParticipantsQuery {
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,

    /// Sort by: registered_at, score, username
    pub sort_by: Option<String>,

    /// Sort order: asc, desc
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_sort_order() -> String {
    "desc".to_string()
}
