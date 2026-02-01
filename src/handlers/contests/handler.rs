//! Contest handler implementations

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    middleware::auth::AuthenticatedUser,
    services::ContestService,
    state::AppState,
    constants::roles,
};

use super::{
    request::{
        AddCollaboratorRequest, AddProblemRequest, CreateContestRequest, LeaderboardQuery, 
        ListContestsQuery, UpdateContestRequest,
    },
    response::{
        CollaboratorResponse, ContestProblemsResponse, ContestResponse, ContestsListResponse, 
        LeaderboardResponse, ParticipantsListResponse, RegistrationResponse, 
        VirtualParticipationResponse,
    },
};

/// List all contests (with filtering)
pub async fn list_contests(
    State(state): State<AppState>,
    Query(query): Query<ListContestsQuery>,
) -> AppResult<Json<ContestsListResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let (contests, total) = ContestService::list_contests(
        state.db(),
        page,
        per_page,
        query.status.as_deref(),
        query.visibility.as_deref(),
        query.search.as_deref(),
    )
    .await?;

    Ok(Json(ContestsListResponse {
        contests,
        total,
        page,
        per_page,
    }))
}

/// Create a new contest
pub async fn create_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateContestRequest>,
) -> AppResult<(StatusCode, Json<ContestResponse>)> {
    // Validate request
    payload.validate()?;

    // Only organizers and admins can create contests
    if auth_user.role != roles::ADMIN && auth_user.role != roles::ORGANIZER {
        return Err(AppError::Forbidden(
            "Only organizers can create contests".to_string(),
        ));
    }

    // Validate time constraints
    if payload.end_time <= payload.start_time {
        return Err(AppError::Validation(
            "End time must be after start time".to_string(),
        ));
    }

    let contest = ContestService::create_contest(state.db(), &auth_user.id, payload).await?;

    Ok((StatusCode::CREATED, Json(contest)))
}

/// Get a specific contest
pub async fn get_contest(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContestResponse>> {
    let contest = ContestService::get_contest(state.db(), &id).await?;
    Ok(Json(contest))
}

/// Update a contest
pub async fn update_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateContestRequest>,
) -> AppResult<Json<ContestResponse>> {
    payload.validate()?;

    let contest =
        ContestService::update_contest(state.db(), &id, &auth_user.id, &auth_user.role, payload)
            .await?;

    Ok(Json(contest))
}

/// Delete a contest
pub async fn delete_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    ContestService::delete_contest(state.db(), &id, &auth_user.id, &auth_user.role).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Register for a contest
pub async fn register_for_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<RegistrationResponse>> {
    let registration =
        ContestService::register_participant(state.db(), &id, &auth_user.id).await?;
    Ok(Json(registration))
}

/// Unregister from a contest
pub async fn unregister_from_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    ContestService::unregister_participant(state.db(), &id, &auth_user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List contest participants
pub async fn list_participants(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListContestsQuery>,
) -> AppResult<Json<ParticipantsListResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50).min(100);

    let (participants, total) =
        ContestService::list_participants(state.db(), &id, page, per_page).await?;

    Ok(Json(ParticipantsListResponse {
        participants,
        total,
        page,
        per_page,
    }))
}

/// List problems in a contest
pub async fn list_contest_problems(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContestProblemsResponse>> {
    let problems = ContestService::list_contest_problems(state.db(), &id).await?;
    Ok(Json(ContestProblemsResponse { problems }))
}

/// Add a problem to a contest
pub async fn add_problem_to_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddProblemRequest>,
) -> AppResult<StatusCode> {
    payload.validate()?;

    ContestService::add_problem_to_contest(
        state.db(),
        &id,
        &auth_user.id,
        &auth_user.role,
        payload,
    )
    .await?;

    Ok(StatusCode::CREATED)
}

/// Remove a problem from a contest
pub async fn remove_problem_from_contest(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((contest_id, problem_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    ContestService::remove_problem_from_contest(
        state.db(),
        &contest_id,
        &problem_id,
        &auth_user.id,
        &auth_user.role,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Collaborator Management
// ============================================================================

/// List collaborators for a contest
pub async fn list_collaborators(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<CollaboratorResponse>>> {
    // Only owner, collaborators, or admin can see collaborators
    if !ContestService::can_view_submissions(state.db(), &auth_user.id, &auth_user.role, &id).await? {
        return Err(AppError::Forbidden(
            "You don't have permission to view collaborators".to_string(),
        ));
    }

    let collaborators = ContestService::list_collaborators(state.db(), &id).await?;
    Ok(Json(collaborators))
}

/// Add a collaborator to a contest
pub async fn add_collaborator(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddCollaboratorRequest>,
) -> AppResult<StatusCode> {
    let role = crate::models::CollaboratorRole::from_str(&payload.role)
        .ok_or_else(|| AppError::Validation("Invalid role. Use 'editor' or 'viewer'".to_string()))?;

    ContestService::add_collaborator(
        state.db(),
        &id,
        &payload.user_id,
        role,
        &auth_user.id,
        &auth_user.role,
    )
    .await?;

    Ok(StatusCode::CREATED)
}

/// Remove a collaborator from a contest
pub async fn remove_collaborator(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    ContestService::remove_collaborator(
        state.db(),
        &id,
        &user_id,
        &auth_user.id,
        &auth_user.role,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Leaderboard and Virtual Participation
// ============================================================================

/// Get contest leaderboard
pub async fn get_leaderboard(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<LeaderboardQuery>,
) -> AppResult<Json<LeaderboardResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50).min(100);
    let include_frozen = query.include_frozen.unwrap_or(false);

    let leaderboard =
        ContestService::get_leaderboard(state.db(), &id, page, per_page, include_frozen).await?;

    Ok(Json(leaderboard))
}

/// Start virtual participation
pub async fn start_virtual_participation(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<VirtualParticipationResponse>> {
    let virtual_participation =
        ContestService::start_virtual_participation(state.db(), &id, &auth_user.id).await?;
    Ok(Json(virtual_participation))
}
