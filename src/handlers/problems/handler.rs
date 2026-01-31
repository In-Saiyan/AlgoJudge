//! Problem handler implementations

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
    middleware::auth::{AuthenticatedUser, OptionalAuth},
    services::ProblemService,
    state::AppState,
};

use super::{
    request::{
        CreateProblemRequest, CreateTestCaseRequest, ListProblemsQuery, UpdateProblemRequest,
        UpdateTestCaseRequest,
    },
    response::{
        ProblemResponse, ProblemsListResponse, TestCaseFullResponse, TestCasesListResponse,
    },
};

/// List all problems (paginated)
pub async fn list_problems(
    State(state): State<AppState>,
    OptionalAuth(auth_user): OptionalAuth,
    Query(query): Query<ListProblemsQuery>,
) -> AppResult<Json<ProblemsListResponse>> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    // Regular users only see public problems, admins see all
    let show_all = auth_user
        .as_ref()
        .map(|u| u.role == roles::ADMIN || u.role == roles::ORGANIZER)
        .unwrap_or(false);

    let (problems, total) = ProblemService::list_problems(
        state.db(),
        page,
        per_page,
        query.search.as_deref(),
        query.difficulty.as_deref(),
        query.tag.as_deref(),
        show_all,
    )
    .await?;

    Ok(Json(ProblemsListResponse {
        problems,
        total,
        page,
        per_page,
    }))
}

/// Create a new problem
pub async fn create_problem(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateProblemRequest>,
) -> AppResult<(StatusCode, Json<ProblemResponse>)> {
    payload.validate()?;

    // Only organizers and admins can create problems
    if auth_user.role != roles::ADMIN && auth_user.role != roles::ORGANIZER {
        return Err(AppError::Forbidden(
            "Only organizers can create problems".to_string(),
        ));
    }

    let problem = ProblemService::create_problem(state.db(), &auth_user.id, payload).await?;

    Ok((StatusCode::CREATED, Json(problem)))
}

/// Get a specific problem
pub async fn get_problem(
    State(state): State<AppState>,
    OptionalAuth(auth_user): OptionalAuth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ProblemResponse>> {
    let can_view_private = auth_user
        .as_ref()
        .map(|u| u.role == roles::ADMIN || u.role == roles::ORGANIZER)
        .unwrap_or(false);

    let problem = ProblemService::get_problem(state.db(), &id, can_view_private).await?;
    Ok(Json(problem))
}

/// Update a problem
pub async fn update_problem(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProblemRequest>,
) -> AppResult<Json<ProblemResponse>> {
    payload.validate()?;

    let problem = ProblemService::update_problem(
        state.db(),
        &id,
        &auth_user.id,
        &auth_user.role,
        payload,
    )
    .await?;

    Ok(Json(problem))
}

/// Delete a problem
pub async fn delete_problem(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    ProblemService::delete_problem(state.db(), &id, &auth_user.id, &auth_user.role).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List test cases for a problem
pub async fn list_test_cases(
    State(state): State<AppState>,
    OptionalAuth(auth_user): OptionalAuth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TestCasesListResponse>> {
    // Only show full test case data to admins/organizers
    let show_full = auth_user
        .as_ref()
        .map(|u| u.role == roles::ADMIN || u.role == roles::ORGANIZER)
        .unwrap_or(false);

    let (test_cases, total) = ProblemService::list_test_cases(state.db(), &id, show_full).await?;

    Ok(Json(TestCasesListResponse { test_cases, total }))
}

/// Add a test case to a problem
pub async fn add_test_case(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateTestCaseRequest>,
) -> AppResult<(StatusCode, Json<TestCaseFullResponse>)> {
    payload.validate()?;

    if auth_user.role != roles::ADMIN && auth_user.role != roles::ORGANIZER {
        return Err(AppError::Forbidden(
            "Only organizers can add test cases".to_string(),
        ));
    }

    let test_case = ProblemService::add_test_case(
        state.db(),
        &id,
        &auth_user.id,
        &auth_user.role,
        payload,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(test_case)))
}

/// Update a test case
pub async fn update_test_case(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((problem_id, tc_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateTestCaseRequest>,
) -> AppResult<Json<TestCaseFullResponse>> {
    payload.validate()?;

    let test_case = ProblemService::update_test_case(
        state.db(),
        &problem_id,
        &tc_id,
        &auth_user.id,
        &auth_user.role,
        payload,
    )
    .await?;

    Ok(Json(test_case))
}

/// Delete a test case
pub async fn delete_test_case(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((problem_id, tc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    ProblemService::delete_test_case(
        state.db(),
        &problem_id,
        &tc_id,
        &auth_user.id,
        &auth_user.role,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
