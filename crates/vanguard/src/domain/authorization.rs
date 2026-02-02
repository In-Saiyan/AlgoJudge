//! Authorization helpers using olympus-rules specifications.
//!
//! This module provides convenient functions to check user permissions
//! using the composable rules defined in olympus-rules.

use std::sync::Arc;
use uuid::Uuid;

use olympus_rules::{
    auth_rules::{
        CanAccessProblemBinaries, CanAddProblems, IsAdmin, IsCollaborator, IsContestOwner,
        IsOrganizer, IsParticipant, IsProblemOwner, IsSubmissionOwner, IsValidUser, NotRateLimited,
    },
    context::AuthContext,
    specification::Specification,
};

use crate::error::{ApiError, ApiResult};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;

/// Build an AuthContext from the current request state and user.
pub fn build_auth_context(state: &AppState, user: &AuthUser) -> AuthContext {
    AuthContext::new(
        user.id,
        user.role.clone(),
        false, // is_banned check done separately
        Arc::new(state.db.clone()),
        Arc::new(state.redis.clone()),
    )
}

/// Build an AuthContext with contest scope.
pub fn build_contest_context(state: &AppState, user: &AuthUser, contest_id: Uuid) -> AuthContext {
    build_auth_context(state, user).with_contest(contest_id)
}

/// Build an AuthContext with problem scope.
pub fn build_problem_context(state: &AppState, user: &AuthUser, problem_id: Uuid) -> AuthContext {
    build_auth_context(state, user).with_problem(problem_id)
}

/// Build an AuthContext with submission scope.
pub fn build_submission_context(
    state: &AppState,
    user: &AuthUser,
    submission_id: Uuid,
) -> AuthContext {
    build_auth_context(state, user).with_submission(submission_id)
}

/// Build an AuthContext with contest and problem scope.
pub fn build_contest_problem_context(
    state: &AppState,
    user: &AuthUser,
    contest_id: Uuid,
    problem_id: Uuid,
) -> AuthContext {
    build_auth_context(state, user)
        .with_contest(contest_id)
        .with_problem(problem_id)
}

// =============================================================================
// Authorization check functions
// =============================================================================

/// Check if user is a valid (non-banned) user.
pub async fn require_valid_user(ctx: &AuthContext) -> ApiResult<()> {
    if !IsValidUser.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user is an admin.
pub async fn require_admin(ctx: &AuthContext) -> ApiResult<()> {
    if !IsAdmin.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user is an organizer (or admin).
pub async fn require_organizer(ctx: &AuthContext) -> ApiResult<()> {
    if !IsOrganizer.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user is a participant in the given contest.
pub async fn require_participant(ctx: &AuthContext) -> ApiResult<()> {
    if !IsParticipant.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user is a collaborator of the given contest.
pub async fn require_collaborator(ctx: &AuthContext) -> ApiResult<()> {
    if !IsCollaborator.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user is the owner of the given contest.
pub async fn require_contest_owner(ctx: &AuthContext) -> ApiResult<()> {
    if !IsContestOwner.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user can modify the contest (owner, collaborator, or admin).
pub async fn require_contest_modify_access(ctx: &AuthContext) -> ApiResult<()> {
    // Admin always has access
    if IsAdmin.is_satisfied_by(ctx).await {
        return Ok(());
    }
    // Contest owner has access
    if IsContestOwner.is_satisfied_by(ctx).await {
        return Ok(());
    }
    // Collaborator has access
    if IsCollaborator.is_satisfied_by(ctx).await {
        return Ok(());
    }
    Err(ApiError::Forbidden)
}

/// Check if user can add problems to a contest.
pub async fn require_can_add_problems(ctx: &AuthContext) -> ApiResult<()> {
    if !CanAddProblems.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user owns the given problem.
pub async fn require_problem_owner(ctx: &AuthContext) -> ApiResult<()> {
    if !IsProblemOwner.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user can modify the problem (owner or admin).
pub async fn require_problem_modify_access(ctx: &AuthContext) -> ApiResult<()> {
    if IsAdmin.is_satisfied_by(ctx).await {
        return Ok(());
    }
    if IsProblemOwner.is_satisfied_by(ctx).await {
        return Ok(());
    }
    Err(ApiError::Forbidden)
}

/// Check if user can access problem binaries.
pub async fn require_problem_binary_access(ctx: &AuthContext) -> ApiResult<()> {
    if !CanAccessProblemBinaries.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user owns the given submission.
pub async fn require_submission_owner(ctx: &AuthContext) -> ApiResult<()> {
    if !IsSubmissionOwner.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Check if user can view the submission (owner, admin, or collaborator).
pub async fn require_submission_view_access(ctx: &AuthContext) -> ApiResult<()> {
    if IsAdmin.is_satisfied_by(ctx).await {
        return Ok(());
    }
    if IsSubmissionOwner.is_satisfied_by(ctx).await {
        return Ok(());
    }
    // For viewing submissions, collaborators of the contest should also have access
    if IsCollaborator.is_satisfied_by(ctx).await {
        return Ok(());
    }
    Err(ApiError::Forbidden)
}

/// Check if user is not rate limited.
pub async fn require_not_rate_limited(ctx: &AuthContext) -> ApiResult<()> {
    let rule = NotRateLimited::api_authenticated();
    if !rule.is_satisfied_by(ctx).await {
        return Err(ApiError::RateLimitExceeded);
    }
    Ok(())
}

/// Check if user can submit to a contest.
/// 
/// Rule: IsValidUser AND ((IsParticipant AND NotRateLimited) OR IsAdmin OR IsCollaborator)
pub async fn require_can_submit(ctx: &AuthContext) -> ApiResult<()> {
    // Must be valid user
    let valid_user = IsValidUser;
    if !valid_user.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden);
    }

    // Admin always can submit
    let admin = IsAdmin;
    if admin.is_satisfied_by(ctx).await {
        return Ok(());
    }

    // Collaborator can submit
    let collaborator = IsCollaborator;
    if collaborator.is_satisfied_by(ctx).await {
        return Ok(());
    }

    // Participant can submit if not rate limited
    let participant = IsParticipant;
    if participant.is_satisfied_by(ctx).await {
        let rate_limit = NotRateLimited::submission();
        if rate_limit.is_satisfied_by(ctx).await {
            return Ok(());
        } else {
            return Err(ApiError::RateLimitExceeded);
        }
    }

    Err(ApiError::Forbidden)
}
