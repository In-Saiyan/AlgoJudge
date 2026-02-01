//! Submission handler implementations

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use base64::Engine;
use uuid::Uuid;
use validator::Validate;

use crate::{
    constants::{languages, roles},
    error::{AppError, AppResult},
    middleware::auth::{AuthenticatedUser, OptionalAuth},
    services::SubmissionService,
    state::AppState,
};

use super::{
    request::{CreateSubmissionRequest, CreateZipSubmissionRequest, ListSubmissionsQuery},
    response::{
        CreateSubmissionResponse, SubmissionResponse, SubmissionResultsResponse,
        SubmissionSourceResponse, SubmissionsListResponse,
    },
};

/// Create a new submission
pub async fn create_submission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateSubmissionRequest>,
) -> AppResult<(StatusCode, Json<CreateSubmissionResponse>)> {
    payload.validate()?;

    // Validate language
    if !languages::ALL.contains(&payload.language.as_str()) {
        return Err(AppError::Validation(format!(
            "Unsupported language: {}. Supported languages: {:?}",
            payload.language,
            languages::ALL
        )));
    }

    let submission = SubmissionService::create_submission(
        state.db(),
        state.redis(),
        &auth_user.id,
        payload,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateSubmissionResponse {
            id: submission.id,
            message: "Submission received and queued for judging".to_string(),
            status: submission.verdict,
        }),
    ))
}

/// Create a new ZIP-based submission for algorithmic benchmarking
/// 
/// ZIP must contain:
/// - compile.sh: Script to compile the solution
/// - run.sh: Script to run the compiled binary
/// 
/// The compiled binary should be named after the problem code (A, B, etc.)
pub async fn create_zip_submission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateZipSubmissionRequest>,
) -> AppResult<(StatusCode, Json<CreateSubmissionResponse>)> {
    payload.validate()?;

    // Decode base64 ZIP
    let zip_data = base64::engine::general_purpose::STANDARD
        .decode(&payload.submission_zip_base64)
        .map_err(|e| AppError::Validation(format!("Invalid base64 for ZIP: {}", e)))?;

    // Decode optional custom generator
    let custom_generator = if let Some(ref gen_b64) = payload.custom_generator_base64 {
        Some(
            base64::engine::general_purpose::STANDARD
                .decode(gen_b64)
                .map_err(|e| AppError::Validation(format!("Invalid base64 for generator: {}", e)))?,
        )
    } else {
        None
    };

    let submission = SubmissionService::create_zip_submission(
        state.db(),
        state.redis(),
        &auth_user.id,
        &payload.problem_id,
        payload.contest_id.as_ref(),
        &payload.runtime,
        zip_data,
        custom_generator,
        payload.custom_generator_filename.clone(),
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateSubmissionResponse {
            id: submission.id,
            message: "ZIP submission received and queued for algorithmic benchmarking".to_string(),
            status: submission.verdict,
        }),
    ))
}

/// List submissions
pub async fn list_submissions(
    State(state): State<AppState>,
    OptionalAuth(auth_user): OptionalAuth,
    Query(query): Query<ListSubmissionsQuery>,
) -> AppResult<Json<SubmissionsListResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    // Regular users can only see their own submissions or public submissions
    let filter_user_id = match &auth_user {
        Some(u) if u.role == roles::ADMIN || u.role == roles::ORGANIZER => query.user_id,
        Some(u) => Some(u.id),
        None => return Err(AppError::Unauthorized),
    };

    let (submissions, total) = SubmissionService::list_submissions(
        state.db(),
        page,
        per_page,
        filter_user_id.as_ref(),
        query.problem_id.as_ref(),
        query.contest_id.as_ref(),
        query.language.as_deref(),
        query.verdict.as_deref(),
    )
    .await?;

    Ok(Json(SubmissionsListResponse {
        submissions,
        total,
        page,
        per_page,
    }))
}

/// Get a specific submission
pub async fn get_submission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SubmissionResponse>> {
    let submission = SubmissionService::get_submission(state.db(), &id).await?;

    // Users can only view their own submissions (unless admin/organizer)
    if submission.user_id != auth_user.id
        && auth_user.role != roles::ADMIN
        && auth_user.role != roles::ORGANIZER
    {
        return Err(AppError::Forbidden(
            "Cannot view other users' submissions".to_string(),
        ));
    }

    Ok(Json(submission))
}

/// Get detailed results for a submission
pub async fn get_submission_results(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SubmissionResultsResponse>> {
    // First check if user can view this submission
    let submission = SubmissionService::get_submission(state.db(), &id).await?;

    if submission.user_id != auth_user.id
        && auth_user.role != roles::ADMIN
        && auth_user.role != roles::ORGANIZER
    {
        return Err(AppError::Forbidden(
            "Cannot view other users' submission results".to_string(),
        ));
    }

    let show_full = auth_user.role == roles::ADMIN || auth_user.role == roles::ORGANIZER;

    let results = SubmissionService::get_submission_results(state.db(), &id, show_full).await?;

    Ok(Json(results))
}

/// Get source code for a submission
pub async fn get_submission_source(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SubmissionSourceResponse>> {
    let submission = SubmissionService::get_submission(state.db(), &id).await?;

    // Users can only view their own source code (unless admin)
    if submission.user_id != auth_user.id && auth_user.role != roles::ADMIN {
        return Err(AppError::Forbidden(
            "Cannot view other users' source code".to_string(),
        ));
    }

    let source = SubmissionService::get_submission_source(state.db(), &id).await?;

    Ok(Json(source))
}
