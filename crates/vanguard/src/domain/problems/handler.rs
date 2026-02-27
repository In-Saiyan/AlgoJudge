//! Problem handlers.

use axum::{
    extract::{Extension, Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use deadpool_redis::redis;
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use super::{
    request::{
        AddProblemToContestRequest, CreateProblemRequest, ListProblemsQuery,
        UpdateProblemRequest,
    },
    response::{
        ContestProblemInfo, ContestProblemsResponse, MessageResponse, OwnerInfo,
        Pagination, ProblemDetailResponse, ProblemListResponse, ProblemResponse,
        ProblemSummary,
    },
};

/// Database row for problem
#[derive(Debug, FromRow)]
struct ProblemRow {
    id: Uuid,
    title: String,
    description: String,
    input_format: Option<String>,
    output_format: Option<String>,
    constraints: Option<String>,
    sample_input: Option<String>,
    sample_output: Option<String>,
    sample_explanation: Option<String>,
    difficulty: Option<String>,
    tags: Option<Vec<String>>,
    time_limit_ms: i32,
    memory_limit_kb: i32,
    num_test_cases: i32,
    generator_path: Option<String>,
    checker_path: Option<String>,
    max_score: i32,
    partial_scoring: bool,
    is_public: bool,
    allowed_languages: Option<Vec<String>>,
    owner_id: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// =============================================================================
// Problem CRUD
// =============================================================================

/// GET /api/v1/problems
/// 
/// List problems with pagination and filtering.
pub async fn list_problems(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Query(query): Query<ListProblemsQuery>,
) -> ApiResult<Json<ProblemListResponse>> {
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let visibility_filter = if query.public_only {
        "AND p.is_public = true"
    } else {
        ""
    };

    let sql = format!(
        r#"
        SELECT 
            p.id, p.title, p.difficulty, p.tags, p.time_limit_ms, p.memory_limit_kb,
            p.max_score, p.is_public, p.created_at,
            u.id as owner_id, u.username, u.display_name
        FROM problems p
        JOIN users u ON p.owner_id = u.id
        WHERE 1=1 {}
        ORDER BY p.created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        visibility_filter
    );

    let rows: Vec<(Uuid, String, Option<String>, Option<Vec<String>>, i32, i32, i32, bool, DateTime<Utc>, Uuid, String, Option<String>)> = 
        sqlx::query_as(&sql)
            .bind(per_page as i64)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM problems WHERE is_public = true OR $1::boolean = false"
    )
        .bind(query.public_only)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problems: Vec<ProblemSummary> = rows
        .into_iter()
        .map(|row| ProblemSummary {
            id: row.0,
            title: row.1,
            difficulty: row.2,
            tags: row.3,
            time_limit_ms: row.4,
            memory_limit_kb: row.5,
            max_score: row.6,
            is_public: row.7,
            created_at: row.8,
            owner: OwnerInfo {
                id: row.9,
                username: row.10,
                display_name: row.11,
            },
        })
        .collect();

    let total_pages = ((total.0 as f64) / (per_page as f64)).ceil() as u32;

    Ok(Json(ProblemListResponse {
        problems,
        pagination: Pagination {
            page,
            per_page,
            total: total.0,
            total_pages,
        },
    }))
}

/// POST /api/v1/problems
/// 
/// Create a new problem.
pub async fn create_problem(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<CreateProblemRequest>,
) -> ApiResult<(StatusCode, Json<ProblemResponse>)> {
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check user role (must be organizer or admin)
    if user.role != "admin" && user.role != "organizer" {
        return Err(ApiError::Forbidden);
    }

    let id = Uuid::new_v4();
    let now = Utc::now();
    let difficulty = payload.difficulty.as_ref().map(|d| d.to_string());

    // Note: generator_path and checker_path are set to NULL on creation
    // They will be populated when binaries are uploaded via separate endpoints
    sqlx::query(
        r#"
        INSERT INTO problems (
            id, title, description, input_format, output_format, constraints,
            sample_input, sample_output, sample_explanation, difficulty, tags,
            time_limit_ms, memory_limit_kb, num_test_cases, generator_path, checker_path,
            max_score, partial_scoring, is_public, allowed_languages, owner_id,
            created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL, NULL,
            $15, $16, $17, $18, $19, $20, $20
        )
        "#
    )
        .bind(id)
        .bind(&payload.title)
        .bind(&payload.description)
        .bind(&payload.input_format)
        .bind(&payload.output_format)
        .bind(&payload.constraints)
        .bind(&payload.sample_input)
        .bind(&payload.sample_output)
        .bind(&payload.sample_explanation)
        .bind(&difficulty)
        .bind(&payload.tags)
        .bind(payload.time_limit_ms)
        .bind(payload.memory_limit_kb)
        .bind(payload.num_test_cases)
        .bind(payload.max_score)
        .bind(payload.partial_scoring)
        .bind(payload.is_public)
        .bind(&payload.allowed_languages)
        .bind(user.id)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create problem: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(ProblemResponse {
            id,
            title: payload.title,
            description: payload.description,
            input_format: payload.input_format,
            output_format: payload.output_format,
            constraints: payload.constraints,
            sample_input: payload.sample_input,
            sample_output: payload.sample_output,
            sample_explanation: payload.sample_explanation,
            difficulty,
            tags: payload.tags,
            time_limit_ms: payload.time_limit_ms,
            memory_limit_kb: payload.memory_limit_kb,
            num_test_cases: payload.num_test_cases,
            status: "draft".to_string(),
            generator_uploaded: false,
            checker_uploaded: false,
            max_score: payload.max_score,
            partial_scoring: payload.partial_scoring,
            is_public: payload.is_public,
            allowed_languages: payload.allowed_languages,
            owner_id: user.id,
            created_at: now,
            updated_at: now,
            message: Some("Problem created. Upload generator and checker binaries to activate.".to_string()),
        }),
    ))
}

/// GET /api/v1/problems/{id}
/// 
/// Get problem details.
pub async fn get_problem(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(problem_id): Path<Uuid>,
) -> ApiResult<Json<ProblemDetailResponse>> {
    let problem: Option<ProblemRow> = sqlx::query_as(
        "SELECT * FROM problems WHERE id = $1"
    )
        .bind(problem_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problem = problem.ok_or(ApiError::NotFound("Problem not found".to_string()))?;

    // Check visibility
    let user_id = user.as_ref().map(|u| u.id);
    let user_role = user.as_ref().map(|u| u.role.as_str());

    if !problem.is_public && user_id != Some(problem.owner_id) && user_role != Some("admin") {
        return Err(ApiError::NotFound("Problem not found".to_string()));
    }

    // Get owner info
    let owner: (Uuid, String, Option<String>) = sqlx::query_as(
        "SELECT id, username, display_name FROM users WHERE id = $1"
    )
        .bind(problem.owner_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let is_owner = user_id == Some(problem.owner_id);

    Ok(Json(ProblemDetailResponse {
        id: problem.id,
        title: problem.title,
        description: problem.description,
        input_format: problem.input_format,
        output_format: problem.output_format,
        constraints: problem.constraints,
        sample_input: problem.sample_input,
        sample_output: problem.sample_output,
        sample_explanation: problem.sample_explanation,
        difficulty: problem.difficulty,
        tags: problem.tags,
        time_limit_ms: problem.time_limit_ms,
        memory_limit_kb: problem.memory_limit_kb,
        num_test_cases: problem.num_test_cases,
        generator_path: if is_owner || user_role == Some("admin") { problem.generator_path } else { None },
        checker_path: if is_owner || user_role == Some("admin") { problem.checker_path } else { None },
        max_score: problem.max_score,
        partial_scoring: problem.partial_scoring,
        is_public: problem.is_public,
        allowed_languages: problem.allowed_languages,
        owner: OwnerInfo {
            id: owner.0,
            username: owner.1,
            display_name: owner.2,
        },
        is_owner,
        created_at: problem.created_at,
        updated_at: problem.updated_at,
    }))
}

/// PUT /api/v1/problems/{id}
/// 
/// Update problem (owner or admin only).
pub async fn update_problem(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
    Json(payload): Json<UpdateProblemRequest>,
) -> ApiResult<Json<ProblemResponse>> {
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    let problem: Option<ProblemRow> = sqlx::query_as(
        "SELECT * FROM problems WHERE id = $1"
    )
        .bind(problem_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problem = problem.ok_or(ApiError::NotFound("Problem not found".to_string()))?;

    // Check permission
    if problem.owner_id != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    // Build updated values
    let title = payload.title.unwrap_or(problem.title);
    let description = payload.description.unwrap_or(problem.description);
    let input_format = payload.input_format.or(problem.input_format);
    let output_format = payload.output_format.or(problem.output_format);
    let constraints = payload.constraints.or(problem.constraints);
    let sample_input = payload.sample_input.or(problem.sample_input);
    let sample_output = payload.sample_output.or(problem.sample_output);
    let sample_explanation = payload.sample_explanation.or(problem.sample_explanation);
    let difficulty = payload.difficulty.map(|d| d.to_string()).or(problem.difficulty);
    let tags = payload.tags.or(problem.tags);
    let time_limit_ms = payload.time_limit_ms.unwrap_or(problem.time_limit_ms);
    let memory_limit_kb = payload.memory_limit_kb.unwrap_or(problem.memory_limit_kb);
    let num_test_cases = payload.num_test_cases.unwrap_or(problem.num_test_cases);
    // Note: generator_path and checker_path are not updated here
    // They are updated via the dedicated upload endpoints
    let max_score = payload.max_score.unwrap_or(problem.max_score);
    let partial_scoring = payload.partial_scoring.unwrap_or(problem.partial_scoring);
    let is_public = payload.is_public.unwrap_or(problem.is_public);
    let allowed_languages = payload.allowed_languages.or(problem.allowed_languages);

    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE problems SET
            title = $2, description = $3, input_format = $4, output_format = $5,
            constraints = $6, sample_input = $7, sample_output = $8, sample_explanation = $9,
            difficulty = $10, tags = $11, time_limit_ms = $12, memory_limit_kb = $13,
            num_test_cases = $14, max_score = $15,
            partial_scoring = $16, is_public = $17, allowed_languages = $18, updated_at = $19
        WHERE id = $1
        "#
    )
        .bind(problem_id)
        .bind(&title)
        .bind(&description)
        .bind(&input_format)
        .bind(&output_format)
        .bind(&constraints)
        .bind(&sample_input)
        .bind(&sample_output)
        .bind(&sample_explanation)
        .bind(&difficulty)
        .bind(&tags)
        .bind(time_limit_ms)
        .bind(memory_limit_kb)
        .bind(num_test_cases)
        .bind(max_score)
        .bind(partial_scoring)
        .bind(is_public)
        .bind(&allowed_languages)
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update problem: {}", e)))?;

    // Determine problem status based on binary uploads
    let generator_uploaded = problem.generator_path.is_some();
    let checker_uploaded = problem.checker_path.is_some();
    let status = if generator_uploaded && checker_uploaded { "ready" } else { "draft" };

    Ok(Json(ProblemResponse {
        id: problem_id,
        title,
        description,
        input_format,
        output_format,
        constraints,
        sample_input,
        sample_output,
        sample_explanation,
        difficulty,
        tags,
        time_limit_ms,
        memory_limit_kb,
        num_test_cases,
        status: status.to_string(),
        generator_uploaded,
        checker_uploaded,
        max_score,
        partial_scoring,
        is_public,
        allowed_languages,
        owner_id: problem.owner_id,
        created_at: problem.created_at,
        updated_at: now,
        message: None,
    }))
}

/// DELETE /api/v1/problems/{id}
/// 
/// Delete problem (owner or admin only).
pub async fn delete_problem(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let problem: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM problems WHERE id = $1"
    )
        .bind(problem_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problem = problem.ok_or(ApiError::NotFound("Problem not found".to_string()))?;

    if problem.0 != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    sqlx::query("DELETE FROM problems WHERE id = $1")
        .bind(problem_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete problem: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Contest Problems
// =============================================================================

/// GET /api/v1/contests/{id}/problems
/// 
/// List problems in a contest.
pub async fn list_contest_problems(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<Json<ContestProblemsResponse>> {
    // Check contest exists
    let contest: Option<(bool, DateTime<Utc>, Uuid)> = sqlx::query_as(
        "SELECT is_public, start_time, owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    let user_id = user.as_ref().map(|u| u.id);
    let user_role = user.as_ref().map(|u| u.role.as_str());
    let now = Utc::now();

    // Check if user can view problems
    // Problems visible if: contest started, or user is owner/admin/collaborator
    let can_view = now >= contest.1 
        || user_id == Some(contest.2) 
        || user_role == Some("admin");

    if !can_view {
        // Check if collaborator
        if let Some(uid) = user_id {
            let is_collab: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
            )
                .bind(contest_id)
                .bind(uid)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

            if is_collab.is_none() {
                return Err(ApiError::Forbidden);
            }
        } else {
            return Err(ApiError::Forbidden);
        }
    }

    let rows: Vec<(Uuid, Uuid, String, String, Option<String>, i32, i32, i32, i32)> = sqlx::query_as(
        r#"
        SELECT 
            cp.id, cp.problem_id, cp.problem_code, p.title, p.difficulty,
            COALESCE(cp.time_limit_ms, p.time_limit_ms) as time_limit_ms,
            COALESCE(cp.memory_limit_kb, p.memory_limit_kb) as memory_limit_kb,
            COALESCE(cp.max_score, p.max_score) as max_score,
            cp.sort_order
        FROM contest_problems cp
        JOIN problems p ON cp.problem_id = p.id
        WHERE cp.contest_id = $1
        ORDER BY cp.sort_order, cp.problem_code
        "#
    )
        .bind(contest_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problems: Vec<ContestProblemInfo> = rows
        .into_iter()
        .map(|row| ContestProblemInfo {
            id: row.0,
            problem_id: row.1,
            problem_code: row.2,
            title: row.3,
            difficulty: row.4,
            time_limit_ms: row.5,
            memory_limit_kb: row.6,
            max_score: row.7,
            sort_order: row.8,
        })
        .collect();

    Ok(Json(ContestProblemsResponse { problems }))
}

/// POST /api/v1/contests/{id}/problems
/// 
/// Add a problem to contest.
pub async fn add_problem_to_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
    Json(payload): Json<AddProblemToContestRequest>,
) -> ApiResult<(StatusCode, Json<ContestProblemInfo>)> {
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check contest exists and user has permission
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Check permission (owner, admin, or collaborator with can_add_problems)
    let has_permission = if contest.0 == user.id || user.role == "admin" {
        true
    } else {
        let collab: Option<(bool,)> = sqlx::query_as(
            "SELECT can_add_problems FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
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

    // Check problem exists
    let problem: Option<(String, Option<String>, i32, i32, i32)> = sqlx::query_as(
        "SELECT title, difficulty, time_limit_ms, memory_limit_kb, max_score FROM problems WHERE id = $1"
    )
        .bind(payload.problem_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let problem = problem.ok_or(ApiError::NotFound("Problem not found".to_string()))?;

    // Check if problem already in contest
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_problems WHERE contest_id = $1 AND problem_id = $2"
    )
        .bind(contest_id)
        .bind(payload.problem_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(ApiError::Validation("Problem already in contest".to_string()));
    }

    // Check if problem code already used
    let code_exists: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM contest_problems WHERE contest_id = $1 AND problem_code = $2"
    )
        .bind(contest_id)
        .bind(&payload.problem_code)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if code_exists.is_some() {
        return Err(ApiError::Validation("Problem code already used in contest".to_string()));
    }

    let id = Uuid::new_v4();
    let sort_order = payload.sort_order.unwrap_or(0);
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO contest_problems (
            id, contest_id, problem_id, problem_code, sort_order,
            max_score, time_limit_ms, memory_limit_kb, added_at, added_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#
    )
        .bind(id)
        .bind(contest_id)
        .bind(payload.problem_id)
        .bind(&payload.problem_code)
        .bind(sort_order)
        .bind(payload.max_score)
        .bind(payload.time_limit_ms)
        .bind(payload.memory_limit_kb)
        .bind(now)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add problem: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(ContestProblemInfo {
            id,
            problem_id: payload.problem_id,
            problem_code: payload.problem_code,
            title: problem.0,
            difficulty: problem.1,
            time_limit_ms: payload.time_limit_ms.unwrap_or(problem.2),
            memory_limit_kb: payload.memory_limit_kb.unwrap_or(problem.3),
            max_score: payload.max_score.unwrap_or(problem.4),
            sort_order,
        }),
    ))
}

/// DELETE /api/v1/contests/{id}/problems/{problem_id}
/// 
/// Remove a problem from contest.
pub async fn remove_problem_from_contest(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((contest_id, problem_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // Check contest exists and user has permission
    let contest: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM contests WHERE id = $1"
    )
        .bind(contest_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let contest = contest.ok_or(ApiError::NotFound("Contest not found".to_string()))?;

    // Check permission
    let has_permission = if contest.0 == user.id || user.role == "admin" {
        true
    } else {
        let collab: Option<(bool,)> = sqlx::query_as(
            "SELECT can_add_problems FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2"
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

    let result = sqlx::query(
        "DELETE FROM contest_problems WHERE contest_id = $1 AND problem_id = $2"
    )
        .bind(contest_id)
        .bind(problem_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove problem: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("Problem not in contest".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Binary Upload Handlers (Generator & Checker)
// =============================================================================

/// Maximum binary file size (50MB)
const MAX_BINARY_SIZE: usize = 50 * 1024 * 1024;

/// Validate that uploaded file is a valid ELF executable
fn validate_elf_binary(data: &[u8]) -> Result<(), ApiError> {
    // ELF magic number: 0x7F 'E' 'L' 'F'
    if data.len() < 4 || &data[0..4] != b"\x7fELF" {
        return Err(ApiError::Validation(
            "Uploaded file is not a valid Linux ELF executable".to_string(),
        ));
    }
    Ok(())
}

/// Check if user has permission to access/modify problem binaries.
/// 
/// Access is granted to:
/// 1. Admins (role = "admin")
/// 2. Problem owner (owner_id matches user.id)
/// 3. Organizers (role = "organizer") who own a contest containing this problem
/// 4. Collaborators of any contest that contains this problem
async fn check_problem_binary_permission(
    state: &AppState,
    problem_id: Uuid,
    user: &AuthUser,
) -> Result<(), ApiError> {
    // Admins can always access
    if user.role == "admin" {
        return Ok(());
    }

    // Check if user is the problem owner
    let owner_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT owner_id FROM problems WHERE id = $1"
    )
    .bind(problem_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    match owner_id {
        Some(id) if id == user.id => return Ok(()),
        None => return Err(ApiError::NotFound("Problem not found".to_string())),
        _ => {}
    }

    // Check if user is an organizer who owns a contest containing this problem
    let is_contest_owner: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM contest_problems cp
            JOIN contests c ON c.id = cp.contest_id
            WHERE cp.problem_id = $1 AND c.owner_id = $2
        )
        "#
    )
    .bind(problem_id)
    .bind(user.id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if is_contest_owner.unwrap_or(false) {
        return Ok(());
    }

    // Check if user is a collaborator of any contest containing this problem
    // Collaborators with can_add_problems permission can manage problem binaries
    let is_collaborator: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM contest_problems cp
            JOIN contest_collaborators cc ON cc.contest_id = cp.contest_id
            WHERE cp.problem_id = $1 
              AND cc.user_id = $2
              AND cc.can_add_problems = true
        )
        "#
    )
    .bind(problem_id)
    .bind(user.id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if is_collaborator.unwrap_or(false) {
        return Ok(());
    }

    Err(ApiError::Forbidden)
}

/// When both generator and checker binaries exist for a problem,
/// find all submissions in `queue_pending` status for that problem
/// and re-queue them on the `run_queue` Redis Stream for judging.
async fn requeue_pending_submissions(
    state: &AppState,
    problem_id: Uuid,
) -> ApiResult<u64> {
    // Check if both binaries now exist
    let dir_path = format!("/mnt/data/binaries/problems/{}", problem_id);
    let generator_exists = tokio::fs::metadata(format!("{}/generator", dir_path)).await.is_ok();
    let checker_exists = tokio::fs::metadata(format!("{}/checker", dir_path)).await.is_ok();

    if !generator_exists || !checker_exists {
        return Ok(0);
    }

    // Fetch problem limits for the run_queue message
    let problem = sqlx::query_as::<_, ProblemLimitsRow>(
        r#"
        SELECT id, time_limit_ms, memory_limit_kb, num_test_cases
        FROM problems WHERE id = $1
        "#,
    )
    .bind(problem_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Problem not found".to_string()))?;

    // Find queue_pending submissions
    let pending_submissions = sqlx::query_as::<_, PendingSubmissionRow>(
        r#"
        SELECT s.id as submission_id, s.contest_id
        FROM submissions s
        WHERE s.problem_id = $1 AND s.status = 'queue_pending'
        "#,
    )
    .bind(problem_id)
    .fetch_all(&state.db)
    .await?;

    if pending_submissions.is_empty() {
        return Ok(0);
    }

    let mut conn = state.redis.get().await?;
    let mut requeued: u64 = 0;

    for sub in &pending_submissions {
        // Look up the user binary path
        let binary_path = format!(
            "/mnt/data/binaries/users/{}_bin",
            sub.submission_id
        );

        // Push to run_queue
        let _: String = redis::cmd("XADD")
            .arg("run_queue")
            .arg("*")
            .arg("submission_id")
            .arg(sub.submission_id.to_string())
            .arg("problem_id")
            .arg(problem_id.to_string())
            .arg("contest_id")
            .arg(sub.contest_id.map(|id| id.to_string()).unwrap_or_default())
            .arg("time_limit_ms")
            .arg(problem.time_limit_ms.to_string())
            .arg("memory_limit_kb")
            .arg(problem.memory_limit_kb.to_string())
            .arg("num_testcases")
            .arg(problem.num_test_cases.to_string())
            .arg("binary_path")
            .arg(&binary_path)
            .query_async(&mut *conn)
            .await?;

        // Update status back to compiled
        sqlx::query(
            "UPDATE submissions SET status = 'compiled' WHERE id = $1"
        )
        .bind(sub.submission_id)
        .execute(&state.db)
        .await?;

        requeued += 1;
    }

    tracing::info!(
        problem_id = %problem_id,
        requeued = requeued,
        "Re-queued pending submissions after binary upload"
    );

    Ok(requeued)
}

#[derive(Debug, FromRow)]
struct ProblemLimitsRow {
    #[allow(dead_code)]
    id: Uuid,
    time_limit_ms: i32,
    memory_limit_kb: i32,
    num_test_cases: i32,
}

#[derive(Debug, FromRow)]
struct PendingSubmissionRow {
    submission_id: Uuid,
    contest_id: Option<Uuid>,
}

/// POST /api/v1/problems/{id}/generator
/// 
/// Upload generator binary for a problem.
/// The binary will be stored at /mnt/data/binaries/problems/{problem_id}/generator
pub async fn upload_generator(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<MessageResponse>> {
    // Check permission
    check_problem_binary_permission(&state, problem_id, &user).await?;

    // Process multipart upload
    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::Validation(format!("Failed to read multipart: {}", e))
    })? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                ApiError::Validation(format!("Failed to read file: {}", e))
            })?;

            if data.len() > MAX_BINARY_SIZE {
                return Err(ApiError::Validation(
                    format!("File size exceeds {}MB limit", MAX_BINARY_SIZE / 1024 / 1024)
                ));
            }

            file_data = Some(data.to_vec());
        }
    }

    let file_data = file_data.ok_or_else(|| {
        ApiError::Validation("No file uploaded".to_string())
    })?;

    // Validate it's an ELF binary
    validate_elf_binary(&file_data)?;

    // Create directory and save file
    let dir_path = format!("/mnt/data/binaries/problems/{}", problem_id);
    let file_path = format!("{}/generator", dir_path);

    tokio::fs::create_dir_all(&dir_path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to create directory: {}", e))
    })?;

    tokio::fs::write(&file_path, &file_data).await.map_err(|e| {
        ApiError::Internal(format!("Failed to save generator: {}", e))
    })?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        tokio::fs::set_permissions(&file_path, perms).await.map_err(|e| {
            ApiError::Internal(format!("Failed to set permissions: {}", e))
        })?;
    }

    // Update database with path
    sqlx::query(
        "UPDATE problems SET generator_path = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&file_path)
    .bind(problem_id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update problem: {}", e)))?;

    tracing::info!(
        problem_id = %problem_id,
        user_id = %user.id,
        file_size = file_data.len(),
        "Generator binary uploaded"
    );

    // Re-queue any queue_pending submissions now that binaries may be ready
    let requeued = requeue_pending_submissions(&state, problem_id).await?;

    let msg = if requeued > 0 {
        format!("Generator uploaded successfully. {} pending submission(s) re-queued for judging.", requeued)
    } else {
        "Generator uploaded successfully".to_string()
    };

    Ok(Json(MessageResponse {
        message: msg,
    }))
}

/// POST /api/v1/problems/{id}/checker
/// 
/// Upload checker/verifier binary for a problem.
/// The binary will be stored at /mnt/data/binaries/problems/{problem_id}/checker
pub async fn upload_checker(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<MessageResponse>> {
    // Check permission
    check_problem_binary_permission(&state, problem_id, &user).await?;

    // Process multipart upload
    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::Validation(format!("Failed to read multipart: {}", e))
    })? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                ApiError::Validation(format!("Failed to read file: {}", e))
            })?;

            if data.len() > MAX_BINARY_SIZE {
                return Err(ApiError::Validation(
                    format!("File size exceeds {}MB limit", MAX_BINARY_SIZE / 1024 / 1024)
                ));
            }

            file_data = Some(data.to_vec());
        }
    }

    let file_data = file_data.ok_or_else(|| {
        ApiError::Validation("No file uploaded".to_string())
    })?;

    // Validate it's an ELF binary
    validate_elf_binary(&file_data)?;

    // Create directory and save file
    let dir_path = format!("/mnt/data/binaries/problems/{}", problem_id);
    let file_path = format!("{}/checker", dir_path);

    tokio::fs::create_dir_all(&dir_path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to create directory: {}", e))
    })?;

    tokio::fs::write(&file_path, &file_data).await.map_err(|e| {
        ApiError::Internal(format!("Failed to save checker: {}", e))
    })?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        tokio::fs::set_permissions(&file_path, perms).await.map_err(|e| {
            ApiError::Internal(format!("Failed to set permissions: {}", e))
        })?;
    }

    // Update database with path
    sqlx::query(
        "UPDATE problems SET checker_path = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(&file_path)
    .bind(problem_id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update problem: {}", e)))?;

    tracing::info!(
        problem_id = %problem_id,
        user_id = %user.id,
        file_size = file_data.len(),
        "Checker binary uploaded"
    );

    // Re-queue any queue_pending submissions now that binaries may be ready
    let requeued = requeue_pending_submissions(&state, problem_id).await?;

    let msg = if requeued > 0 {
        format!("Checker uploaded successfully. {} pending submission(s) re-queued for judging.", requeued)
    } else {
        "Checker uploaded successfully".to_string()
    };

    Ok(Json(MessageResponse {
        message: msg,
    }))
}

/// GET /api/v1/problems/{id}/generator
/// 
/// Download generator binary for a problem.
pub async fn download_generator(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
) -> ApiResult<axum::response::Response> {
    use axum::body::Body;
    use axum::response::IntoResponse;
    
    // Check permission
    check_problem_binary_permission(&state, problem_id, &user).await?;

    // Get file path from database
    let path: Option<String> = sqlx::query_scalar(
        "SELECT generator_path FROM problems WHERE id = $1"
    )
    .bind(problem_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .flatten();

    let path = path.ok_or_else(|| {
        ApiError::NotFound("Generator not uploaded for this problem".to_string())
    })?;

    // Read file
    let data = tokio::fs::read(&path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to read generator: {}", e))
    })?;

    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream"),
         (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"generator\"")],
        Body::from(data)
    ).into_response())
}

/// GET /api/v1/problems/{id}/checker
/// 
/// Download checker binary for a problem.
pub async fn download_checker(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(problem_id): Path<Uuid>,
) -> ApiResult<axum::response::Response> {
    use axum::body::Body;
    use axum::response::IntoResponse;
    
    // Check permission
    check_problem_binary_permission(&state, problem_id, &user).await?;

    // Get file path from database
    let path: Option<String> = sqlx::query_scalar(
        "SELECT checker_path FROM problems WHERE id = $1"
    )
    .bind(problem_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    .flatten();

    let path = path.ok_or_else(|| {
        ApiError::NotFound("Checker not uploaded for this problem".to_string())
    })?;

    // Read file
    let data = tokio::fs::read(&path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to read checker: {}", e))
    })?;

    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream"),
         (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"checker\"")],
        Body::from(data)
    ).into_response())
}

/// Create routes for problems
pub fn problem_routes() -> axum::Router<AppState> {
    use axum::routing::get;

    axum::Router::new()
        .route("/", get(list_problems))
        .route("/{id}", get(get_problem))
}

pub fn protected_problem_routes() -> axum::Router<AppState> {
    use axum::routing::{delete, get, post, put};

    axum::Router::new()
        .route("/", post(create_problem))
        .route("/{id}", put(update_problem))
        .route("/{id}", delete(delete_problem))
        .route("/{id}/generator", post(upload_generator))
        .route("/{id}/generator", get(download_generator))
        .route("/{id}/checker", post(upload_checker))
        .route("/{id}/checker", get(download_checker))
}
