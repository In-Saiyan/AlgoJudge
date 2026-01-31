//! User handler implementations

use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::AppResult,
    middleware::auth::AuthenticatedUser,
    services::UserService,
    state::AppState,
};

use super::{
    request::{ListUsersQuery, UpdateUserRequest},
    response::{UserProfileResponse, UserStatsResponse, UserSubmissionsResponse, UsersListResponse},
};

/// List all users (paginated)
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> AppResult<Json<UsersListResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let (users, total) = UserService::list_users(
        state.db(),
        page,
        per_page,
        query.search.as_deref(),
        query.role.as_deref(),
    )
    .await?;

    let users = users
        .into_iter()
        .map(|u| UserProfileResponse {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            role: u.role,
            created_at: u.created_at,
        })
        .collect();

    Ok(Json(UsersListResponse {
        users,
        total,
        page,
        per_page,
    }))
}

/// Get a specific user by ID
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserProfileResponse>> {
    let user = UserService::get_user_by_id(state.db(), &id).await?;

    Ok(Json(UserProfileResponse {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: user.role,
        created_at: user.created_at,
    }))
}

/// Update user profile
pub async fn update_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> AppResult<Json<UserProfileResponse>> {
    // Validate request
    payload.validate()?;

    // Users can only update their own profile (unless admin)
    let user = UserService::update_user(
        state.db(),
        &auth_user.id,
        &id,
        &auth_user.role,
        payload.display_name.as_deref(),
        payload.email.as_deref(),
        payload.current_password.as_deref(),
        payload.new_password.as_deref(),
    )
    .await?;

    Ok(Json(UserProfileResponse {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: user.role,
        created_at: user.created_at,
    }))
}

/// Get user's submission history
pub async fn get_user_submissions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListUsersQuery>,
) -> AppResult<Json<UserSubmissionsResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let (submissions, total) = UserService::get_user_submissions(state.db(), &id, page, per_page).await?;

    Ok(Json(UserSubmissionsResponse {
        submissions,
        total,
        page,
        per_page,
    }))
}

/// Get user statistics
pub async fn get_user_stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserStatsResponse>> {
    let stats = UserService::get_user_stats(state.db(), &id).await?;
    Ok(Json(stats))
}
