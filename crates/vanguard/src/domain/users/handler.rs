//! User management handlers.

use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, Row};
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use super::{
    request::{ListUsersQuery, UpdateUserRequest},
    response::{Pagination, UpdateUserResponse, UserListResponse, UserProfileResponse, UserStatsResponse, UserSummary},
};

/// User summary row from database
#[derive(Debug, FromRow)]
struct UserSummaryRow {
    id: Uuid,
    username: String,
    display_name: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
}

/// User profile row from database
#[derive(Debug, FromRow)]
struct UserProfileRow {
    id: Uuid,
    username: String,
    display_name: Option<String>,
    bio: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// GET /api/v1/users
/// 
/// List users with pagination and optional filtering.
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> ApiResult<Json<UserListResponse>> {
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    // Build dynamic query based on filters
    let mut sql = String::from(
        "SELECT id, username, display_name, role, created_at FROM users WHERE 1=1"
    );
    let mut count_sql = String::from("SELECT COUNT(*) FROM users WHERE 1=1");

    if let Some(ref role) = query.role {
        sql.push_str(" AND role = $3");
        count_sql.push_str(" AND role = $1");
    }

    if let Some(ref search) = query.search {
        if query.role.is_some() {
            sql.push_str(" AND (username ILIKE $4 OR display_name ILIKE $4)");
            count_sql.push_str(" AND (username ILIKE $2 OR display_name ILIKE $2)");
        } else {
            sql.push_str(" AND (username ILIKE $3 OR display_name ILIKE $3)");
            count_sql.push_str(" AND (username ILIKE $1 OR display_name ILIKE $1)");
        }
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT $1 OFFSET $2");

    // Execute queries based on filters
    let (users, total): (Vec<UserSummaryRow>, i64) = match (&query.role, &query.search) {
        (Some(role), Some(search)) => {
            let search_pattern = format!("%{}%", search);
            let users: Vec<UserSummaryRow> = sqlx::query_as(&sql)
                .bind(per_page as i64)
                .bind(offset)
                .bind(role)
                .bind(&search_pattern)
                .fetch_all(&state.db)
                .await?;
            let total: (i64,) = sqlx::query_as(&count_sql)
                .bind(role)
                .bind(&search_pattern)
                .fetch_one(&state.db)
                .await?;
            (users, total.0)
        }
        (Some(role), None) => {
            let users: Vec<UserSummaryRow> = sqlx::query_as(&sql)
                .bind(per_page as i64)
                .bind(offset)
                .bind(role)
                .fetch_all(&state.db)
                .await?;
            let total: (i64,) = sqlx::query_as(&count_sql)
                .bind(role)
                .fetch_one(&state.db)
                .await?;
            (users, total.0)
        }
        (None, Some(search)) => {
            let search_pattern = format!("%{}%", search);
            let users: Vec<UserSummaryRow> = sqlx::query_as(&sql)
                .bind(per_page as i64)
                .bind(offset)
                .bind(&search_pattern)
                .fetch_all(&state.db)
                .await?;
            let total: (i64,) = sqlx::query_as(&count_sql)
                .bind(&search_pattern)
                .fetch_one(&state.db)
                .await?;
            (users, total.0)
        }
        (None, None) => {
            let sql = "SELECT id, username, display_name, role, created_at FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2";
            let count_sql = "SELECT COUNT(*) FROM users";
            let users: Vec<UserSummaryRow> = sqlx::query_as(sql)
                .bind(per_page as i64)
                .bind(offset)
                .fetch_all(&state.db)
                .await?;
            let total: (i64,) = sqlx::query_as(count_sql)
                .fetch_one(&state.db)
                .await?;
            (users, total.0)
        }
    };

    let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;

    Ok(Json(UserListResponse {
        users: users
            .into_iter()
            .map(|u| UserSummary {
                id: u.id,
                username: u.username,
                display_name: u.display_name,
                role: u.role,
                created_at: u.created_at,
            })
            .collect(),
        pagination: Pagination {
            page,
            per_page,
            total,
            total_pages,
        },
    }))
}

/// GET /api/v1/users/{id}
/// 
/// Get a user's public profile.
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<UserProfileResponse>> {
    let user: UserProfileRow = sqlx::query_as(
        "SELECT id, username, display_name, bio, role, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    Ok(Json(UserProfileResponse {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        bio: user.bio,
        role: user.role,
        created_at: user.created_at,
    }))
}

/// PUT /api/v1/users/{id}
/// 
/// Update a user's profile. Only the owner can update their profile.
pub async fn update_user(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> ApiResult<Json<UpdateUserResponse>> {
    // Check if user is updating their own profile
    if auth_user.id != user_id {
        return Err(ApiError::Forbidden);
    }

    // Validate request
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Update user
    let now = Utc::now();
    let row = sqlx::query(
        r#"
        UPDATE users
        SET display_name = COALESCE($1, display_name),
            bio = COALESCE($2, bio),
            updated_at = $3
        WHERE id = $4
        RETURNING id, username, display_name, bio, role, updated_at
        "#
    )
    .bind(&payload.display_name)
    .bind(&payload.bio)
    .bind(now)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    Ok(Json(UpdateUserResponse {
        id: row.get("id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        bio: row.get("bio"),
        role: row.get("role"),
        updated_at: row.get("updated_at"),
    }))
}

/// GET /api/v1/users/{id}/stats
/// 
/// Get a user's statistics.
pub async fn get_user_stats(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<UserStatsResponse>> {
    // Verify user exists
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    if !exists.0 {
        return Err(ApiError::NotFound("User not found".to_string()));
    }

    // For now, return placeholder stats since submissions table doesn't exist yet
    // This will be updated when submissions are implemented in Phase 3
    Ok(Json(UserStatsResponse {
        user_id,
        total_submissions: 0,
        accepted_submissions: 0,
        contests_participated: 0,
        problems_solved: 0,
    }))
}
