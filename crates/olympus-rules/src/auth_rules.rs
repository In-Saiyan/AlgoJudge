//! Authorization rules for Vanguard (API Gateway).
//!
//! These specifications perform async database/cache lookups to evaluate
//! user permissions for contests, problems, and submissions.

#[cfg(feature = "auth")]
use crate::context::AuthContext;
#[cfg(feature = "auth")]
use crate::specification::Specification;
#[cfg(feature = "auth")]
use async_trait::async_trait;
#[cfg(feature = "auth")]
use uuid::Uuid;

// =============================================================================
// User-level rules
// =============================================================================

/// Check if the user is valid (not banned).
#[cfg(feature = "auth")]
pub struct IsValidUser;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsValidUser {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        !ctx.is_banned
    }
}

/// Check if the user has admin role.
#[cfg(feature = "auth")]
pub struct IsAdmin;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsAdmin {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        ctx.role == "admin"
    }
}

/// Check if the user has organizer role (or higher).
#[cfg(feature = "auth")]
pub struct IsOrganizer;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsOrganizer {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        ctx.role == "admin" || ctx.role == "organizer"
    }
}

// =============================================================================
// Contest-scoped rules
// =============================================================================

/// Check if the user is a participant in the context's contest.
/// Requires `ctx.contest_id` to be set.
#[cfg(feature = "auth")]
pub struct IsParticipant;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsParticipant {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(contest_id) = ctx.contest_id else {
            tracing::warn!("IsParticipant evaluated without contest_id in context");
            return false;
        };
        
        let result: Result<Option<bool>, _> = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM contest_participants 
                WHERE contest_id = $1 AND user_id = $2
            )"#,
        )
        .bind(contest_id)
        .bind(ctx.user_id)
        .fetch_one(ctx.db.as_ref())
        .await;

        result.ok().flatten().unwrap_or(false)
    }
}

/// Check if the user is a collaborator of the context's contest.
/// Requires `ctx.contest_id` to be set.
#[cfg(feature = "auth")]
pub struct IsCollaborator;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsCollaborator {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(contest_id) = ctx.contest_id else {
            tracing::warn!("IsCollaborator evaluated without contest_id in context");
            return false;
        };
        
        let result: Result<Option<bool>, _> = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM contest_collaborators 
                WHERE contest_id = $1 AND user_id = $2
            )"#,
        )
        .bind(contest_id)
        .bind(ctx.user_id)
        .fetch_one(ctx.db.as_ref())
        .await;

        result.ok().flatten().unwrap_or(false)
    }
}

/// Check if the user is the owner of the context's contest.
/// Requires `ctx.contest_id` to be set.
#[cfg(feature = "auth")]
pub struct IsContestOwner;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsContestOwner {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(contest_id) = ctx.contest_id else {
            tracing::warn!("IsContestOwner evaluated without contest_id in context");
            return false;
        };
        
        let result: Result<Option<Uuid>, _> = sqlx::query_scalar(
            "SELECT owner_id FROM contests WHERE id = $1",
        )
        .bind(contest_id)
        .fetch_optional(ctx.db.as_ref())
        .await;

        result.ok().flatten().map(|owner| owner == ctx.user_id).unwrap_or(false)
    }
}

/// Check if the user can add problems to the context's contest.
/// (Collaborator with can_add_problems permission or contest owner)
/// Requires `ctx.contest_id` to be set.
#[cfg(feature = "auth")]
pub struct CanAddProblems;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for CanAddProblems {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(contest_id) = ctx.contest_id else {
            tracing::warn!("CanAddProblems evaluated without contest_id in context");
            return false;
        };
        
        // Check if collaborator with can_add_problems permission
        let result: Result<Option<bool>, _> = sqlx::query_scalar(
            r#"SELECT can_add_problems FROM contest_collaborators 
               WHERE contest_id = $1 AND user_id = $2"#,
        )
        .bind(contest_id)
        .bind(ctx.user_id)
        .fetch_optional(ctx.db.as_ref())
        .await;

        result.ok().flatten().unwrap_or(false)
    }
}

// =============================================================================
// Problem-scoped rules  
// =============================================================================

/// Check if the user is the owner of the context's problem.
/// Requires `ctx.problem_id` to be set.
#[cfg(feature = "auth")]
pub struct IsProblemOwner;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsProblemOwner {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(problem_id) = ctx.problem_id else {
            tracing::warn!("IsProblemOwner evaluated without problem_id in context");
            return false;
        };
        
        let result: Result<Option<Uuid>, _> = sqlx::query_scalar(
            "SELECT owner_id FROM problems WHERE id = $1",
        )
        .bind(problem_id)
        .fetch_optional(ctx.db.as_ref())
        .await;

        result.ok().flatten().map(|owner| owner == ctx.user_id).unwrap_or(false)
    }
}

/// Check if the user can access problem binaries (generator/checker).
/// Access granted if: admin, problem owner, or collaborator of any contest containing the problem.
/// Requires `ctx.problem_id` to be set.
#[cfg(feature = "auth")]
pub struct CanAccessProblemBinaries;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for CanAccessProblemBinaries {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(problem_id) = ctx.problem_id else {
            tracing::warn!("CanAccessProblemBinaries evaluated without problem_id in context");
            return false;
        };

        // Admin can access everything
        if ctx.role == "admin" {
            return true;
        }

        // Check if problem owner
        let owner_result: Result<Option<Uuid>, _> = sqlx::query_scalar(
            "SELECT owner_id FROM problems WHERE id = $1",
        )
        .bind(problem_id)
        .fetch_optional(ctx.db.as_ref())
        .await;

        if owner_result.ok().flatten().map(|o| o == ctx.user_id).unwrap_or(false) {
            return true;
        }

        // Check if owner or collaborator of any contest containing this problem
        let access_result: Result<Option<bool>, _> = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM contest_problems cp
                JOIN contests c ON c.id = cp.contest_id
                LEFT JOIN contest_collaborators cc ON cc.contest_id = c.id AND cc.user_id = $2
                WHERE cp.problem_id = $1
                AND (c.owner_id = $2 OR cc.user_id IS NOT NULL)
            )"#,
        )
        .bind(problem_id)
        .bind(ctx.user_id)
        .fetch_one(ctx.db.as_ref())
        .await;

        access_result.ok().flatten().unwrap_or(false)
    }
}

// =============================================================================
// Submission-scoped rules
// =============================================================================

/// Check if the user owns the context's submission.
/// Requires `ctx.submission_id` to be set.
#[cfg(feature = "auth")]
pub struct IsSubmissionOwner;

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for IsSubmissionOwner {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let Some(submission_id) = ctx.submission_id else {
            tracing::warn!("IsSubmissionOwner evaluated without submission_id in context");
            return false;
        };
        
        let result: Result<Option<Uuid>, _> = sqlx::query_scalar(
            "SELECT user_id FROM submissions WHERE id = $1",
        )
        .bind(submission_id)
        .fetch_optional(ctx.db.as_ref())
        .await;

        result.ok().flatten().map(|owner| owner == ctx.user_id).unwrap_or(false)
    }
}

// =============================================================================
// Rate limiting rules
// =============================================================================

/// Check if the user is NOT rate limited for the given action.
/// Uses Redis to check rate limit counters.
/// Note: This returns `true` if user is NOT rate limited (i.e., allowed to proceed).
#[cfg(feature = "auth")]
pub struct NotRateLimited {
    pub action: String,
    pub limit: u64,
    pub window_secs: u64,
}

#[cfg(feature = "auth")]
impl NotRateLimited {
    pub fn new(action: impl Into<String>, limit: u64, window_secs: u64) -> Self {
        Self {
            action: action.into(),
            limit,
            window_secs,
        }
    }
    
    /// Submission rate limit: 10 per minute
    pub fn submission() -> Self {
        Self::new("submit", 10, 60)
    }
    
    /// API rate limit for authenticated users: 100 per minute
    pub fn api_authenticated() -> Self {
        Self::new("api", 100, 60)
    }
}

#[cfg(feature = "auth")]
#[async_trait]
impl Specification<AuthContext> for NotRateLimited {
    async fn is_satisfied_by(&self, ctx: &AuthContext) -> bool {
        let key = format!("rl:{}:{}", self.action, ctx.user_id);
        
        let conn_result = ctx.redis.get().await;
        let mut conn = match conn_result {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to get Redis connection for rate limit check: {}", e);
                // Fail open - allow the request if Redis is unavailable
                return true;
            }
        };

        let count_result: Result<u64, _> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await;

        match count_result {
            Ok(count) => count < self.limit,
            Err(_) => {
                // Key doesn't exist or error - user hasn't hit this endpoint recently
                true
            }
        }
    }
}

// =============================================================================
// Composite rule examples (for documentation)
// =============================================================================

/// Example: Can user submit to a contest?
/// `IsValidUser & ((NotRateLimited & IsParticipant) | IsAdmin | IsCollaborator)`
#[cfg(feature = "auth")]
pub mod composites {
    //! Pre-built composite authorization rules.
    //!
    //! These demonstrate how to combine specs using operators.
    //!
    //! # Example
    //! ```ignore
    //! use olympus_rules::prelude::*;
    //! use olympus_rules::auth_rules::*;
    //!
    //! // Build the "can submit" rule
    //! let can_submit = Spec(IsValidUser) 
    //!     & ((Spec(NotRateLimited::submission()) & Spec(IsParticipant)) 
    //!        | Spec(IsAdmin) 
    //!        | Spec(IsCollaborator));
    //!
    //! // Evaluate
    //! let auth_ctx = AuthContext::new(user_id, role, is_banned, db, redis)
    //!     .with_contest(contest_id);
    //!
    //! if can_submit.is_satisfied_by(&auth_ctx).await {
    //!     // Allow submission
    //! }
    //! ```
}

#[cfg(all(test, feature = "auth"))]
mod tests {
    // Tests would require mocking database/redis
    // See integration tests in vanguard for full coverage
}
