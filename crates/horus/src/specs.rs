//! Cleanup policy specifications for Horus

use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use tokio::fs;
use uuid::Uuid;

/// Context for evaluating cleanup specifications
pub struct CleanupContext<'a> {
    /// Path being evaluated
    pub path: &'a Path,

    /// File metadata (if available)
    pub metadata: Option<std::fs::Metadata>,

    /// Database pool for lookups
    pub db_pool: &'a PgPool,
}

impl<'a> CleanupContext<'a> {
    pub fn new(path: &'a Path, db_pool: &'a PgPool) -> Self {
        let metadata = std::fs::metadata(path).ok();
        Self {
            path,
            metadata,
            db_pool,
        }
    }
}

/// Trait for cleanup specifications
#[async_trait]
pub trait CleanupSpec: Send + Sync {
    /// Check if the path satisfies this specification
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool;

    /// Get description of this specification
    fn description(&self) -> &'static str;
}

// ============================================================================
// File/Directory Type Specifications
// ============================================================================

/// Specification that matches files
pub struct IsFile;

#[async_trait]
impl CleanupSpec for IsFile {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        ctx.metadata.as_ref().map(|m| m.is_file()).unwrap_or(false)
    }

    fn description(&self) -> &'static str {
        "is a file"
    }
}

/// Specification that matches directories
pub struct IsDirectory;

#[async_trait]
impl CleanupSpec for IsDirectory {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        ctx.metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false)
    }

    fn description(&self) -> &'static str {
        "is a directory"
    }
}

// ============================================================================
// Time-based Specifications
// ============================================================================

/// Specification that matches files/dirs not accessed within duration
pub struct LastAccessOlderThan {
    pub duration: Duration,
}

impl LastAccessOlderThan {
    pub fn hours(hours: u64) -> Self {
        Self {
            duration: Duration::from_secs(hours * 3600),
        }
    }
}

#[async_trait]
impl CleanupSpec for LastAccessOlderThan {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        // First check for .last_access marker file (used by Minos)
        let marker_path = if ctx.path.is_dir() {
            ctx.path.join(".last_access")
        } else {
            ctx.path.with_extension("last_access")
        };

        if marker_path.exists() {
            if let Ok(content) = fs::read_to_string(&marker_path).await {
                if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(content.trim()) {
                    let age = chrono::Utc::now().signed_duration_since(timestamp);
                    return age.num_seconds() > self.duration.as_secs() as i64;
                }
            }
        }

        // Fall back to filesystem atime
        if let Some(ref metadata) = ctx.metadata {
            if let Ok(accessed) = metadata.accessed() {
                if let Ok(age) = SystemTime::now().duration_since(accessed) {
                    return age > self.duration;
                }
            }
        }

        false
    }

    fn description(&self) -> &'static str {
        "last access older than threshold"
    }
}

/// Specification that matches files/dirs created before duration
pub struct CreatedOlderThan {
    pub duration: Duration,
}

impl CreatedOlderThan {
    pub fn hours(hours: u64) -> Self {
        Self {
            duration: Duration::from_secs(hours * 3600),
        }
    }

    pub fn days(days: u64) -> Self {
        Self {
            duration: Duration::from_secs(days * 24 * 3600),
        }
    }
}

#[async_trait]
impl CleanupSpec for CreatedOlderThan {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        if let Some(ref metadata) = ctx.metadata {
            // Try created time first, fall back to modified time
            let created = metadata.created().or_else(|_| metadata.modified());
            if let Ok(created) = created {
                if let Ok(age) = SystemTime::now().duration_since(created) {
                    return age > self.duration;
                }
            }
        }
        false
    }

    fn description(&self) -> &'static str {
        "created older than threshold"
    }
}

// ============================================================================
// Database-backed Specifications
// ============================================================================

/// Specification that checks if a submission is still active/pending
pub struct HasActiveSubmission;

#[async_trait]
impl CleanupSpec for HasActiveSubmission {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        // Extract submission ID from path (e.g., /temp/{submission_id}/)
        let submission_id = ctx
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| {
                // Handle both "{uuid}" and "{uuid}_bin" formats
                let clean = s.trim_end_matches("_bin");
                Uuid::parse_str(clean).ok()
            });

        if let Some(id) = submission_id {
            // Check if submission is in a non-final state
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM submissions WHERE id = $1 AND status IN ('PENDING', 'COMPILING', 'JUDGING')"
            )
            .bind(id)
            .fetch_one(ctx.db_pool)
            .await;

            return result.map(|count| count > 0).unwrap_or(false);
        }

        false
    }

    fn description(&self) -> &'static str {
        "has active submission in database"
    }
}

/// Specification that checks if a binary has a corresponding submission
pub struct HasSubmissionRecord;

#[async_trait]
impl CleanupSpec for HasSubmissionRecord {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        let submission_id = ctx
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| {
                let clean = s.trim_end_matches("_bin");
                Uuid::parse_str(clean).ok()
            });

        if let Some(id) = submission_id {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM submissions WHERE id = $1",
            )
            .bind(id)
            .fetch_one(ctx.db_pool)
            .await;

            return result.map(|count| count > 0).unwrap_or(false);
        }

        false
    }

    fn description(&self) -> &'static str {
        "has submission record in database"
    }
}

/// Specification that checks if a problem still exists
pub struct HasProblemRecord;

#[async_trait]
impl CleanupSpec for HasProblemRecord {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        let problem_id = ctx
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        if let Some(id) = problem_id {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM problems WHERE id = $1",
            )
            .bind(id)
            .fetch_one(ctx.db_pool)
            .await;

            return result.map(|count| count > 0).unwrap_or(false);
        }

        false
    }

    fn description(&self) -> &'static str {
        "has problem record in database"
    }
}

// ============================================================================
// Combinator Specifications
// ============================================================================

/// AND combinator for cleanup specs
pub struct And<A, B> {
    pub left: A,
    pub right: B,
}

#[async_trait]
impl<A: CleanupSpec, B: CleanupSpec> CleanupSpec for And<A, B> {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        self.left.is_satisfied_by(ctx).await && self.right.is_satisfied_by(ctx).await
    }

    fn description(&self) -> &'static str {
        "AND combination"
    }
}

/// OR combinator for cleanup specs
pub struct Or<A, B> {
    pub left: A,
    pub right: B,
}

#[async_trait]
impl<A: CleanupSpec, B: CleanupSpec> CleanupSpec for Or<A, B> {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        self.left.is_satisfied_by(ctx).await || self.right.is_satisfied_by(ctx).await
    }

    fn description(&self) -> &'static str {
        "OR combination"
    }
}

/// NOT combinator for cleanup specs
pub struct Not<A> {
    pub inner: A,
}

#[async_trait]
impl<A: CleanupSpec> CleanupSpec for Not<A> {
    async fn is_satisfied_by(&self, ctx: &CleanupContext<'_>) -> bool {
        !self.inner.is_satisfied_by(ctx).await
    }

    fn description(&self) -> &'static str {
        "NOT combination"
    }
}

// ============================================================================
// Helper trait for building specs
// ============================================================================

pub trait CleanupSpecExt: CleanupSpec + Sized {
    fn and<B: CleanupSpec>(self, other: B) -> And<Self, B> {
        And {
            left: self,
            right: other,
        }
    }

    fn or<B: CleanupSpec>(self, other: B) -> Or<Self, B> {
        Or {
            left: self,
            right: other,
        }
    }

    fn not(self) -> Not<Self> {
        Not { inner: self }
    }
}

impl<T: CleanupSpec + Sized> CleanupSpecExt for T {}
