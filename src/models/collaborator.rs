//! Contest collaborator model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Contest collaborator - users who can help manage a contest
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ContestCollaborator {
    pub id: Uuid,
    pub contest_id: Uuid,
    pub user_id: Uuid,
    /// Role: 'editor' can modify problems, 'viewer' can only view
    pub role: String,
    /// User who added this collaborator
    pub added_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Collaborator role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollaboratorRole {
    /// Can add/modify problems and view submissions
    Editor,
    /// Can only view submissions
    Viewer,
}

impl CollaboratorRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "editor" => Some(Self::Editor),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }
    
    /// Check if this role can modify contest problems
    pub fn can_modify(&self) -> bool {
        matches!(self, Self::Editor)
    }
}

impl std::fmt::Display for CollaboratorRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
