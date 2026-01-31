//! Contest model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Contest database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Contest {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub organizer_id: Uuid,
    pub scoring_mode: String,
    pub visibility: String,
    pub registration_mode: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub registration_start: Option<DateTime<Utc>>,
    pub registration_end: Option<DateTime<Utc>>,
    pub allowed_languages: Vec<String>,
    pub freeze_time_minutes: Option<i32>,
    pub allow_virtual: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Contest {
    /// Get current status of the contest
    pub fn status(&self) -> ContestStatus {
        let now = Utc::now();
        if now < self.start_time {
            ContestStatus::Upcoming
        } else if now >= self.start_time && now < self.end_time {
            ContestStatus::Ongoing
        } else {
            ContestStatus::Ended
        }
    }

    /// Check if registration is open
    pub fn is_registration_open(&self) -> bool {
        let now = Utc::now();

        // Check registration mode
        if self.registration_mode == "closed" {
            return false;
        }

        // Check registration time window
        if let Some(start) = self.registration_start {
            if now < start {
                return false;
            }
        }

        if let Some(end) = self.registration_end {
            if now > end {
                return false;
            }
        }

        // Can't register after contest ends
        if now > self.end_time {
            return false;
        }

        true
    }

    /// Check if leaderboard is frozen
    pub fn is_leaderboard_frozen(&self) -> bool {
        if let Some(freeze_minutes) = self.freeze_time_minutes {
            let freeze_time = self.end_time - chrono::Duration::minutes(freeze_minutes as i64);
            Utc::now() >= freeze_time && Utc::now() < self.end_time
        } else {
            false
        }
    }

    /// Check if virtual participation is allowed
    pub fn can_start_virtual(&self) -> bool {
        self.allow_virtual && self.status() == ContestStatus::Ended
    }
}

/// Contest status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContestStatus {
    Upcoming,
    Ongoing,
    Ended,
}

impl std::fmt::Display for ContestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upcoming => write!(f, "upcoming"),
            Self::Ongoing => write!(f, "ongoing"),
            Self::Ended => write!(f, "ended"),
        }
    }
}

/// Contest participant model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ContestParticipant {
    pub id: Uuid,
    pub contest_id: Uuid,
    pub user_id: Uuid,
    pub is_virtual: bool,
    pub virtual_start: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
}

/// Contest problem association
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ContestProblem {
    pub id: Uuid,
    pub contest_id: Uuid,
    pub problem_id: Uuid,
    pub order: i32,
    pub time_limit_ms: Option<i64>,
    pub memory_limit_kb: Option<i64>,
    pub points: Option<i32>,
}
