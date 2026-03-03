//! Admin request DTOs.

use serde::Deserialize;
use validator::Validate;

/// Query parameters for admin user listing
#[derive(Debug, Deserialize, Default)]
pub struct AdminListUsersQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    /// Filter by role
    pub role: Option<String>,
    /// Filter by banned status
    pub is_banned: Option<bool>,
    /// Search by username or email
    pub search: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Update user role request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRoleRequest {
    #[validate(custom(function = "validate_role"))]
    pub role: String,
}

fn validate_role(role: &str) -> Result<(), validator::ValidationError> {
    match role {
        "admin" | "organizer" | "participant" | "spectator" => Ok(()),
        _ => {
            let mut err = validator::ValidationError::new("invalid_role");
            err.message =
                Some("Role must be one of: admin, organizer, participant, spectator".into());
            Err(err)
        }
    }
}

/// Ban user request
#[derive(Debug, Deserialize, Validate)]
pub struct BanUserRequest {
    #[validate(length(min = 1, max = 500, message = "Reason must be 1-500 characters"))]
    pub reason: String,
}

/// Query for queue listing
#[derive(Debug, Deserialize, Default)]
pub struct QueueQuery {
    /// Stream name: compile_queue or run_queue
    pub stream: Option<String>,
    /// Maximum entries to return
    #[serde(default = "default_queue_count")]
    pub count: u32,
}

fn default_queue_count() -> u32 {
    50
}

/// Rejudge request
#[derive(Debug, Deserialize)]
pub struct RejudgeRequest {
    pub submission_id: uuid::Uuid,
}

/// Save rule config request
#[derive(Debug, Deserialize, Validate)]
pub struct SaveRuleConfigRequest {
    /// Unique rule name within the service
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    /// Target service: vanguard, minos, horus
    #[validate(custom(function = "validate_service"))]
    pub service: String,
    /// Human-readable description
    pub description: Option<String>,
    /// The JSON rule tree
    pub config: serde_json::Value,
    /// Whether this rule is active
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Semantic version
    #[serde(default = "default_version")]
    pub version: String,
}

fn validate_service(service: &str) -> Result<(), validator::ValidationError> {
    match service {
        "vanguard" | "minos" | "horus" => Ok(()),
        _ => {
            let mut err = validator::ValidationError::new("invalid_service");
            err.message = Some("Service must be one of: vanguard, minos, horus".into());
            Err(err)
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Update rule config request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRuleConfigRequest {
    /// Human-readable description
    pub description: Option<String>,
    /// The JSON rule tree (if changing)
    pub config: Option<serde_json::Value>,
    /// Whether this rule is active
    pub enabled: Option<bool>,
    /// Semantic version
    pub version: Option<String>,
}

/// Query for listing rules
#[derive(Debug, Deserialize, Default)]
pub struct ListRulesQuery {
    /// Filter by service
    pub service: Option<String>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
}
