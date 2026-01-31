//! Admin handler implementations

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    constants::roles,
    error::{AppError, AppResult},
    handlers::users::request::ListUsersQuery,
    middleware::auth::AuthenticatedUser,
    services::AdminService,
    state::AppState,
};

use super::{
    request::{BanUserRequest, UpdateUserRoleRequest},
    response::{
        AdminUsersListResponse, ContainersListResponse, SubmissionQueueResponse,
        SystemStatsResponse,
    },
};

/// Verify user is admin
fn require_admin(auth_user: &AuthenticatedUser) -> AppResult<()> {
    if auth_user.role != roles::ADMIN {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    Ok(())
}

/// List all users with admin details
pub async fn list_all_users(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<ListUsersQuery>,
) -> AppResult<Json<AdminUsersListResponse>> {
    require_admin(&auth_user)?;

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let (users, total) = AdminService::list_all_users(
        state.db(),
        page,
        per_page,
        query.search.as_deref(),
        query.role.as_deref(),
    )
    .await?;

    Ok(Json(AdminUsersListResponse {
        users,
        total,
        page,
        per_page,
    }))
}

/// Update a user's role
pub async fn update_user_role(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> AppResult<StatusCode> {
    require_admin(&auth_user)?;
    payload.validate()?;

    // Validate role
    if !roles::ALL.contains(&payload.role.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid role: {}. Valid roles: {:?}",
            payload.role,
            roles::ALL
        )));
    }

    AdminService::update_user_role(state.db(), &id, &payload.role).await?;

    Ok(StatusCode::OK)
}

/// Ban a user
pub async fn ban_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<BanUserRequest>,
) -> AppResult<StatusCode> {
    require_admin(&auth_user)?;

    // Cannot ban yourself
    if id == auth_user.id {
        return Err(AppError::Validation("Cannot ban yourself".to_string()));
    }

    AdminService::ban_user(
        state.db(),
        &id,
        payload.reason.as_deref(),
        payload.duration_hours,
    )
    .await?;

    Ok(StatusCode::OK)
}

/// Unban a user
pub async fn unban_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&auth_user)?;

    AdminService::unban_user(state.db(), &id).await?;

    Ok(StatusCode::OK)
}

/// Get system statistics
pub async fn get_system_stats(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<SystemStatsResponse>> {
    require_admin(&auth_user)?;

    let stats = AdminService::get_system_stats(state.db(), state.docker()).await?;

    Ok(Json(stats))
}

/// List active benchmark containers
pub async fn list_benchmark_containers(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<ContainersListResponse>> {
    require_admin(&auth_user)?;

    let containers = AdminService::list_benchmark_containers(state.docker()).await?;

    Ok(Json(ContainersListResponse { containers }))
}

/// Stop a benchmark container
pub async fn stop_container(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    require_admin(&auth_user)?;

    AdminService::stop_container(state.docker(), &id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get submission queue status
pub async fn get_submission_queue(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<SubmissionQueueResponse>> {
    require_admin(&auth_user)?;

    let queue = AdminService::get_submission_queue(state.db()).await?;

    Ok(Json(queue))
}

/// Rejudge a submission
pub async fn rejudge_submission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&auth_user)?;

    AdminService::rejudge_submission(state.db(), state.redis(), &id).await?;

    Ok(StatusCode::ACCEPTED)
}
