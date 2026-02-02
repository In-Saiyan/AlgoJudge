//! Contest handlers.

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use super::{
    request::{
        AddCollaboratorRequest, CreateContestRequest, ListContestsQuery,
        ListParticipantsQuery, UpdateContestRequest,
    },
    response::{
        CollaboratorInfo, CollaboratorListResponse, ContestDetailResponse,
        ContestListResponse, ContestResponse, ContestSummary, MessageResponse,
        OwnerInfo, Pagination, ParticipantInfo, ParticipantListResponse,
        RegistrationResponse,
    },
};

/// Database row for contest with owner info
#[derive(Debug, FromRow)]
struct ContestRow {
    id: Uuid,
    title: String,
    description: Option<String>,
    short_description: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    freeze_time: Option<DateTime<Utc>>,
    scoring_type: String,
    is_public: bool,
    is_rated: bool,
    registration_required: bool,
    max_participants: Option<i32>,
    allowed_languages: Option<Vec<String>>,
    owner_id: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Helper to determine contest status
fn get_contest_status(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> String {
    let now = Utc::now();
    if now < start_time {
        "upcoming".to_string()
    } else if now >= start_time && now <= end_time {
        "ongoing".to_string()
    } else {
        "past".to_string()
    }
}

// =============================================================================
// Contest CRUD
// =============================================================================

/// GET /api/v1/contests
/// 
/// List contests with pagination and filtering.
pub async fn list_contests(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<ListContestsQuery>,
) -> ApiResult<Json<ContestListResponse>> {
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;
    let now = Utc::now();

    // Build the base query with status filtering
    let status_filter = match query.status.as_deref() {
        Some("upcoming") => "AND c.start_time > $3",
        Some("ongoing") => "AND c.start_time <= $3 AND c.end_time >= $3",
        Some("past") => "AND c.end_time < $3",
        _ => "",
    };

    let visibility_filter = if query.public_only {
        "AND c.is_public = true"
    } else {
        ""
    };

    let search_filter = if query.search.is_some() {
        "AND c.title ILIKE $4"
    } else {
        ""
    };

    let owner_filter = if query.owner_id.is_some() {
        if query.search.is_some() {
            "AND c.owner_id = $5"
        } else {
            "AND c.owner_id = $4"
        }
    } else {
        ""
    };

    let sql = format!(
        r#"
        SELECT 
            c.id, c.title, c.short_description, c.start_time, c.end_time,
            c.scoring_type, c.is_public, c.is_rated,
            u.id as owner_id, u.username as owner_username, u.display_name as owner_display_name,
            COALESCE(p.participant_count, 0) as participant_count
        FROM contests c
        JOIN users u ON c.owner_id = u.id
        LEFT JOIN (
            SELECT contest_id, COUNT(*) as participant_count
            FROM contest_participants
            GROUP BY contest_id
        ) p ON c.id = p.contest_id
        WHERE 1=1 {} {} {} {}
        ORDER BY c.start_time DESC
        LIMIT $1 OFFSET $2
        "#,
        visibility_filter, status_filter, search_filter, owner_filter
    );

    let count_sql = format!(
        r#"
        SELECT COUNT(*) as count
        FROM contests c
        WHERE 1=1 {} {} {} {}
        "#,
        visibility_filter, status_filter, search_filter, owner_filter
    );

    // Execute queries based on parameters
    let search_pattern = query.search.as_ref().map(|s| format!("%{}%", s));

    let rows: Vec<(Uuid, String, Option<String>, DateTime<Utc>, DateTime<Utc>, String, bool, bool, Uuid, String, Option<String>, i64)> = 
        match (&query.status, &search_pattern, &query.owner_id) {
            (Some(_), Some(search), Some(owner)) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(now)
                    .bind(search)
                    .bind(owner)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (Some(_), Some(search), None) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(now)
                    .bind(search)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (Some(_), None, Some(owner)) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(now)
                    .bind(owner)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (Some(_), None, None) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(now)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (None, Some(search), Some(owner)) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(search)
                    .bind(owner)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (None, Some(search), None) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(search)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (None, None, Some(owner)) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .bind(owner)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
            (None, None, None) => {
                sqlx::query_as(&sql)
                    .bind(per_page as i64)
                    .bind(offset)
                    .fetch_all(&state.db)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            }
        };

    // Get total count (simplified - just count with visibility filter)
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM contests WHERE is_public = true OR $1::boolean = false"
    )
        .bind(query.public_only)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contests: Vec<ContestSummary> = rows
        .into_iter()
        .map(|row| ContestSummary {
            id: row.0,
            title: row.1,
            short_description: row.2,
            start_time: row.3,
            end_time: row.4,
            scoring_type: row.5,
            is_public: row.6,
            is_rated: row.7,
            participant_count: row.11,
            owner: OwnerInfo {
                id: row.8,
                username: row.9,
                display_name: row.10,
            },
            status: get_contest_status(row.3, row.4),
        })
        .collect();

    let total_pages = ((total.0 as f64) / (per_page as f64)).ceil() as u32;

    Ok(Json(ContestListResponse {
        contests,
        pagination: Pagination {
            page,
            per_page,
            total: total.0,
            total_pages,
        },
    }))
}

/// POST /api/v1/contests
/// 
/// Create a new contest (organizer/admin only).
pub async fn create_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<CreateContestRequest>,
) -> ApiResult<(StatusCode, Json<ContestResponse>)> {
    // Validate request
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check user role (must be organizer or admin)
    if user.role != "admin" && user.role != "organizer" {
        return Err(ApiError::Forbidden);
    }

    // Validate times
    if payload.end_time <= payload.start_time {
        return Err(ApiError::Validation("End time must be after start time".to_string()));
    }

    if let Some(freeze) = payload.freeze_time {
        if freeze < payload.start_time || freeze > payload.end_time {
            return Err(ApiError::Validation("Freeze time must be between start and end time".to_string()));
        }
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    // Insert contest
    sqlx::query(
        r#"
        INSERT INTO contests (
            id, title, description, short_description, start_time, end_time, freeze_time,
            scoring_type, is_public, is_rated, registration_required, max_participants,
            allowed_languages, owner_id, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $15)
        "#
    )
        .bind(id)
        .bind(&payload.title)
        .bind(&payload.description)
        .bind(&payload.short_description)
        .bind(payload.start_time)
        .bind(payload.end_time)
        .bind(payload.freeze_time)
        .bind(payload.scoring_type.to_string())
        .bind(payload.is_public)
        .bind(payload.is_rated)
        .bind(payload.registration_required)
        .bind(payload.max_participants)
        .bind(&payload.allowed_languages)
        .bind(user.id)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create contest: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(ContestResponse {
            id,
            title: payload.title,
            description: payload.description,
            short_description: payload.short_description,
            start_time: payload.start_time,
            end_time: payload.end_time,
            freeze_time: payload.freeze_time,
            scoring_type: payload.scoring_type.to_string(),
            is_public: payload.is_public,
            is_rated: payload.is_rated,
            registration_required: payload.registration_required,
            max_participants: payload.max_participants,
            allowed_languages: payload.allowed_languages,
            owner_id: user.id,
            created_at: now,
            updated_at: now,
        }),
    ))
}

/// GET /api/v1/contests/{id}
/// 
/// Get contest details.
pub async fn get_contest(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<Json<ContestDetailResponse>> {
    // Fetch contest with owner info
    let contest: Option<ContestRow> = sqlx::query_as(
        r#"
        SELECT id, title, description, short_description, start_time, end_time, freeze_time,
               scoring_type, is_public, is_rated, registration_required, max_participants,
               allowed_languages, owner_id, created_at, updated_at
        FROM contests WHERE id = $1
        "#
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Check visibility
    let user_id = user.as_ref().map(|u| u.id);
    if !contest.is_public && user_id != Some(contest.owner_id) {
        // Check if user is a collaborator
        if let Some(uid) = user_id {
            let is_collaborator: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
            )
                .bind(contest_id)
                .bind(uid)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

            if is_collaborator.is_none() {
                return Err(ApiError::NotFound("Contest not found".to_string()));
            }
        } else {
            return Err(ApiError::NotFound("Contest not found".to_string()));
        }
    }

    // Get owner info
    let owner: (Uuid, String, Option<String>) = sqlx::query_as(
        "SELECT id, username, display_name FROM users WHERE id = $1"
    )
        .bind(contest.owner_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    // Get counts
    let participant_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM contest_participants WHERE contest_id = $1"
    )
        .bind(contest_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problem_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM contest_problems WHERE contest_id = $1"
    )
        .bind(contest_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    // Check user's relationship to contest
    let (is_registered, is_collaborator) = if let Some(uid) = user_id {
        let reg: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM contest_participants WHERE contest_id = $1 AND user_id = $2"
        )
            .bind(contest_id)
            .bind(uid)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        let collab: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
        )
            .bind(contest_id)
            .bind(uid)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        (reg.is_some(), collab.is_some())
    } else {
        (false, false)
    };

    let is_owner = user_id == Some(contest.owner_id);

    Ok(Json(ContestDetailResponse {
        id: contest.id,
        title: contest.title,
        description: contest.description,
        short_description: contest.short_description,
        start_time: contest.start_time,
        end_time: contest.end_time,
        freeze_time: contest.freeze_time,
        scoring_type: contest.scoring_type,
        is_public: contest.is_public,
        is_rated: contest.is_rated,
        registration_required: contest.registration_required,
        max_participants: contest.max_participants,
        allowed_languages: contest.allowed_languages,
        owner: OwnerInfo {
            id: owner.0,
            username: owner.1,
            display_name: owner.2,
        },
        participant_count: participant_count.0,
        problem_count: problem_count.0,
        status: get_contest_status(contest.start_time, contest.end_time),
        is_registered,
        is_collaborator,
        is_owner,
        created_at: contest.created_at,
        updated_at: contest.updated_at,
    }))
}

/// PUT /api/v1/contests/{id}
/// 
/// Update contest (owner or collaborator with edit permission).
pub async fn update_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
    Json(payload): Json<UpdateContestRequest>,
) -> ApiResult<Json<ContestResponse>> {
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check contest exists and user has permission
    let contest: Option<ContestRow> = sqlx::query_as(
        "SELECT * FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Check permission
    let has_permission = if contest.owner_id == user.id || user.role == "admin" {
        true
    } else {
        // Check collaborator permission
        let collab: Option<(bool,)> = sqlx::query_as(
            "SELECT can_edit_contest FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
        )
            .bind(contest_id)
            .bind(user.id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        collab.map(|c| c.0).unwrap_or(false)
    };

    if !has_permission {
        return Err(ApiError::Forbidden);
    }

    // Build update
    let title = payload.title.unwrap_or(contest.title);
    let description = payload.description.or(contest.description);
    let short_description = payload.short_description.or(contest.short_description);
    let start_time = payload.start_time.unwrap_or(contest.start_time);
    let end_time = payload.end_time.unwrap_or(contest.end_time);
    let freeze_time = payload.freeze_time.or(contest.freeze_time);
    let scoring_type = payload.scoring_type.map(|s| s.to_string()).unwrap_or(contest.scoring_type);
    let is_public = payload.is_public.unwrap_or(contest.is_public);
    let is_rated = payload.is_rated.unwrap_or(contest.is_rated);
    let registration_required = payload.registration_required.unwrap_or(contest.registration_required);
    let max_participants = payload.max_participants.or(contest.max_participants);
    let allowed_languages = payload.allowed_languages.or(contest.allowed_languages);

    // Validate times
    if end_time <= start_time {
        return Err(ApiError::Validation("End time must be after start time".to_string()));
    }

    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE contests SET
            title = $2, description = $3, short_description = $4,
            start_time = $5, end_time = $6, freeze_time = $7,
            scoring_type = $8, is_public = $9, is_rated = $10,
            registration_required = $11, max_participants = $12,
            allowed_languages = $13, updated_at = $14
        WHERE id = $1
        "#
    )
        .bind(contest_id)
        .bind(&title)
        .bind(&description)
        .bind(&short_description)
        .bind(start_time)
        .bind(end_time)
        .bind(freeze_time)
        .bind(&scoring_type)
        .bind(is_public)
        .bind(is_rated)
        .bind(registration_required)
        .bind(max_participants)
        .bind(&allowed_languages)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update contest: {}", e)))?;

    Ok(Json(ContestResponse {
        id: contest_id,
        title,
        description,
        short_description,
        start_time,
        end_time,
        freeze_time,
        scoring_type,
        is_public,
        is_rated,
        registration_required,
        max_participants,
        allowed_languages,
        owner_id: contest.owner_id,
        created_at: contest.created_at,
        updated_at: now,
    }))
}

/// DELETE /api/v1/contests/{id}
/// 
/// Delete contest (owner or admin only).
pub async fn delete_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Check contest exists
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Check permission
    if contest.0 != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    sqlx::query("DELETE FROM contests WHERE id = $1")
        .bind(contest_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete contest: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Registration
// =============================================================================

/// POST /api/v1/contests/{id}/register
/// 
/// Register for a contest.
pub async fn register_for_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<RegistrationResponse>)> {
    // Check contest exists and is open for registration
    let contest: Option<(DateTime<Utc>, DateTime<Utc>, bool, Option<i32>)> = sqlx::query_as(
        "SELECT start_time, end_time, registration_required, max_participants FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    let now = Utc::now();

    // Check if contest has ended
    if now > contest.1 {
        return Err(ApiError::Validation("Contest has already ended".to_string()));
    }

    // Check if already registered
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_participants WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(ApiError::Validation("Already registered for this contest".to_string()));
    }

    // Check max participants
    if let Some(max) = contest.3 {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM contest_participants WHERE contest_id = $1"
        )
            .bind(contest_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        if count.0 >= max as i64 {
            return Err(ApiError::Validation("Contest is full".to_string()));
        }
    }

    // Register
    sqlx::query(
        r#"
        INSERT INTO contest_participants (contest_id, user_id, registered_at, status)
        VALUES ($1, $2, $3, 'registered')
        "#
    )
        .bind(contest_id)
        .bind(user.id)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to register: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(RegistrationResponse {
            message: "Successfully registered for contest".to_string(),
            contest_id,
            registered_at: now,
        }),
    ))
}

/// POST /api/v1/contests/{id}/unregister
/// 
/// Unregister from a contest.
pub async fn unregister_from_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<Json<MessageResponse>> {
    // Check if registered
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_participants WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if existing.is_none() {
        return Err(ApiError::NotFound("Not registered for this contest".to_string()));
    }

    // Check if contest has started (cannot unregister after start)
    let contest: Option<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT start_time FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    if Utc::now() >= contest.0 {
        return Err(ApiError::Validation("Cannot unregister after contest has started".to_string()));
    }

    sqlx::query(
        "DELETE FROM contest_participants WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to unregister: {}", e)))?;

    Ok(Json(MessageResponse {
        message: "Successfully unregistered from contest".to_string(),
    }))
}

/// GET /api/v1/contests/{id}/participants
/// 
/// List contest participants.
pub async fn list_participants(
    State(state): State<AppState>,
    Path(contest_id): Path<Uuid>,
    Query(query): Query<ListParticipantsQuery>,
) -> ApiResult<Json<ParticipantListResponse>> {
    // Check contest exists
    let exists: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Contest not found".to_string()));
    }

    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let order_by = match query.sort_by.as_deref() {
        Some("score") => "cp.total_score",
        Some("username") => "u.username",
        _ => "cp.registered_at",
    };

    let order = if query.sort_order == "asc" { "ASC" } else { "DESC" };

    let sql = format!(
        r#"
        SELECT 
            cp.id, u.id as user_id, u.username, u.display_name,
            cp.status, cp.total_score, cp.total_penalty, cp.problems_solved,
            cp.registered_at, cp.last_submission_at
        FROM contest_participants cp
        JOIN users u ON cp.user_id = u.id
        WHERE cp.contest_id = $1
        ORDER BY {} {}
        LIMIT $2 OFFSET $3
        "#,
        order_by, order
    );

    let rows: Vec<(Uuid, Uuid, String, Option<String>, String, i32, i32, i32, DateTime<Utc>, Option<DateTime<Utc>>)> = 
        sqlx::query_as(&sql)
            .bind(contest_id)
            .bind(per_page as i64)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM contest_participants WHERE contest_id = $1"
    )
        .bind(contest_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let participants: Vec<ParticipantInfo> = rows
        .into_iter()
        .map(|row| ParticipantInfo {
            id: row.0,
            user: OwnerInfo {
                id: row.1,
                username: row.2,
                display_name: row.3,
            },
            status: row.4,
            total_score: row.5,
            total_penalty: row.6,
            problems_solved: row.7,
            registered_at: row.8,
            last_submission_at: row.9,
        })
        .collect();

    let total_pages = ((total.0 as f64) / (per_page as f64)).ceil() as u32;

    Ok(Json(ParticipantListResponse {
        participants,
        pagination: Pagination {
            page,
            per_page,
            total: total.0,
            total_pages,
        },
    }))
}

// =============================================================================
// Collaborators
// =============================================================================

/// GET /api/v1/contests/{id}/collaborators
/// 
/// List contest collaborators.
pub async fn list_collaborators(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<Json<CollaboratorListResponse>> {
    // Check contest exists and user has permission to view
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Only owner, admin, or collaborators can view
    let is_collaborator: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if contest.0 != user.id && user.role != "admin" && is_collaborator.is_none() {
        return Err(ApiError::Forbidden);
    }

    let rows: Vec<(Uuid, Uuid, String, Option<String>, String, bool, bool, bool, DateTime<Utc>)> = 
        sqlx::query_as(
            r#"
            SELECT 
                cc.id, u.id as user_id, u.username, u.display_name,
                cc.role, cc.can_edit_contest, cc.can_add_problems, cc.can_view_submissions,
                cc.added_at
            FROM contest_collaborators cc
            JOIN users u ON cc.user_id = u.id
            WHERE cc.contest_id = $1
            ORDER BY cc.added_at DESC
            "#
        )
            .bind(contest_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let collaborators: Vec<CollaboratorInfo> = rows
        .into_iter()
        .map(|row| CollaboratorInfo {
            id: row.0,
            user: OwnerInfo {
                id: row.1,
                username: row.2,
                display_name: row.3,
            },
            role: row.4,
            can_edit_contest: row.5,
            can_add_problems: row.6,
            can_view_submissions: row.7,
            added_at: row.8,
        })
        .collect();

    Ok(Json(CollaboratorListResponse { collaborators }))
}

/// POST /api/v1/contests/{id}/collaborators
/// 
/// Add a collaborator to contest.
pub async fn add_collaborator(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
    Json(payload): Json<AddCollaboratorRequest>,
) -> ApiResult<(StatusCode, Json<CollaboratorInfo>)> {
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check contest exists and user is owner or admin
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    if contest.0 != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    // Check target user exists
    let target_user: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, username, display_name FROM users WHERE id = $1"
    )
        .bind(payload.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let target_user = target_user.ok_or(ApiError::NotFound("User not found".to_string()))?;

    // Check not already a collaborator
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(payload.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(ApiError::Validation("User is already a collaborator".to_string()));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO contest_collaborators (
            id, contest_id, user_id, role, can_edit_contest, can_add_problems,
            can_view_submissions, added_at, added_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#
    )
        .bind(id)
        .bind(contest_id)
        .bind(payload.user_id)
        .bind(&payload.role)
        .bind(payload.can_edit_contest)
        .bind(payload.can_add_problems)
        .bind(payload.can_view_submissions)
        .bind(now)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add collaborator: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(CollaboratorInfo {
            id,
            user: OwnerInfo {
                id: target_user.0,
                username: target_user.1,
                display_name: target_user.2,
            },
            role: payload.role,
            can_edit_contest: payload.can_edit_contest,
            can_add_problems: payload.can_add_problems,
            can_view_submissions: payload.can_view_submissions,
            added_at: now,
        }),
    ))
}

/// DELETE /api/v1/contests/{id}/collaborators/{user_id}
/// 
/// Remove a collaborator from contest.
pub async fn remove_collaborator(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((contest_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // Check contest exists and user is owner or admin
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    if contest.0 != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    let result = sqlx::query(
        "DELETE FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
    )
        .bind(contest_id)
        .bind(target_user_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove collaborator: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Collaborator not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Create routes for contests
pub fn contest_routes() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post, put};

    axum::Router::new()
        .route("/", get(list_contests))
        .route("/{id}", get(get_contest))
        .route("/{id}/participants", get(list_participants))
}

pub fn protected_contest_routes() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post, put};

    axum::Router::new()
        .route("/", post(create_contest))
        .route("/{id}", put(update_contest))
        .route("/{id}", delete(delete_contest))
        .route("/{id}/register", post(register_for_contest))
        .route("/{id}/unregister", post(unregister_from_contest))
        .route("/{id}/collaborators", get(list_collaborators))
        .route("/{id}/collaborators", post(add_collaborator))
        .route("/{id}/collaborators/{user_id}", delete(remove_collaborator))
}
