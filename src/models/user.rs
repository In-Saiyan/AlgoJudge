//! User model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub display_name: Option<String>,
    pub role: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub ban_expires_at: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Check if the user is currently banned
    pub fn is_currently_banned(&self) -> bool {
        if !self.is_banned {
            return false;
        }

        // Check if ban has expired
        if let Some(expires_at) = self.ban_expires_at {
            if expires_at < Utc::now() {
                return false;
            }
        }

        true
    }

    /// Check if user has admin privileges
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    /// Check if user can manage contests
    pub fn can_manage_contests(&self) -> bool {
        matches!(self.role.as_str(), "admin" | "organizer")
    }
}
