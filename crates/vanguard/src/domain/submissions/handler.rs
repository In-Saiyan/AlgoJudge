//! Submission handlers.

use axum::{
    extract::{Path, Query, State, Multipart},
    Extension, Json,
};
use chrono::Utc;
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::middleware::auth::AuthUser;

use super::request::{CreateSubmissionRequest, LeaderboardQuery, ListSubmissionsQuery, ZipSubmissionParams};
use super::response::*;

/// POST /api/v1/submissions - Submit source code
pub async fn create_submission(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<CreateSubmissionRequest>,
) -> ApiResult<Json<SubmissionResponse>> {
    payload.validate().map_err(|e| ApiError::Validation(format!("{}", e)))?;

    let user_id = user.id;

    // Check contest exists and is active
    let contest = sqlx::query_as::<_, ContestCheckRow>(
        r#"
        SELECT id, status, starts_at, ends_at, allowed_languages
        FROM contests WHERE id = $1
        "#,
    )
    .bind(payload.contest_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Contest not found".to_string()))?;

    // Check contest is running
    let now = Utc::now();
    if contest.status != "active" {
        return Err(ApiError::Validation("Contest is not active".to_string()));
    }
    if now < contest.starts_at {
        return Err(ApiError::Validation("Contest has not started yet".to_string()));
    }
    if now > contest.ends_at {
        return Err(ApiError::Validation("Contest has ended".to_string()));
    }

    // Check language is allowed
    let lang_str = payload.language.to_string();
    if let Some(allowed) = &contest.allowed_languages {
        if !allowed.contains(&lang_str) {
            return Err(ApiError::Validation(format!(
                "Language '{}' is not allowed in this contest",
                lang_str
            )));
        }
    }

    // Check user is participant
    let is_participant: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
        r#"SELECT EXISTS(SELECT 1 FROM contest_participants WHERE contest_id = $1 AND user_id = $2)"#,
    )
    .bind(payload.contest_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let is_participant = is_participant.unwrap_or(false);

    if !is_participant {
        // Check if admin or collaborator
        let is_admin = user.role == "admin";
        let is_collaborator: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
            r#"SELECT EXISTS(SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2)"#,
        )
        .bind(payload.contest_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
        let is_collaborator = is_collaborator.unwrap_or(false);

        if !is_admin && !is_collaborator {
            return Err(ApiError::Forbidden);
        }
    }

    // Check problem exists and is in contest
    let problem_in_contest: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM problems p
            JOIN contest_problems cp ON cp.problem_id = p.id
            WHERE p.id = $1 AND cp.contest_id = $2
        )
        "#,
    )
    .bind(payload.problem_id)
    .bind(payload.contest_id)
    .fetch_one(&state.db)
    .await?;
    let problem_in_contest = problem_in_contest.unwrap_or(false);

    if !problem_in_contest {
        return Err(ApiError::NotFound("Problem not found in this contest".to_string()));
    }

    // Create submission
    let submission_id = Uuid::new_v4();
    let submitted_at = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO submissions (
            id, contest_id, problem_id, user_id,
            submission_type, language, source_code,
            status, submitted_at
        )
        VALUES ($1, $2, $3, $4, 'source', $5, $6, 'pending', $7)
        "#,
    )
    .bind(submission_id)
    .bind(payload.contest_id)
    .bind(payload.problem_id)
    .bind(user_id)
    .bind(&lang_str)
    .bind(&payload.source_code)
    .bind(submitted_at)
    .execute(&state.db)
    .await?;

    // Queue for compilation via Redis Stream
    let mut conn = state.redis.get().await?;

    let stream_id: String = redis::cmd("XADD")
        .arg("compile_queue")
        .arg("*")
        .arg("submission_id")
        .arg(submission_id.to_string())
        .arg("type")
        .arg("source")
        .arg("language")
        .arg(&lang_str)
        .query_async(&mut *conn)
        .await?;

    tracing::info!(
        submission_id = %submission_id,
        stream_id = %stream_id,
        "Submission queued for compilation"
    );

    Ok(Json(SubmissionResponse {
        id: submission_id,
        contest_id: payload.contest_id,
        problem_id: payload.problem_id,
        submission_type: "source".to_string(),
        language: Some(lang_str),
        status: "pending".to_string(),
        submitted_at,
        message: "Submission queued for compilation".to_string(),
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct ContestCheckRow {
    #[allow(dead_code)]
    id: Uuid,
    status: String,
    starts_at: chrono::DateTime<Utc>,
    ends_at: chrono::DateTime<Utc>,
    allowed_languages: Option<Vec<String>>,
}

/// POST /api/v1/submissions/zip - Submit ZIP file (algorithmic benchmark)
pub async fn create_zip_submission(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(params): Query<ZipSubmissionParams>,
    mut multipart: Multipart,
) -> ApiResult<Json<SubmissionResponse>> {
    let user_id = user.id;

    // Check contest exists and is active
    let contest = sqlx::query_as::<_, ContestCheckRowSimple>(
        r#"
        SELECT id, status, starts_at, ends_at
        FROM contests WHERE id = $1
        "#,
    )
    .bind(params.contest_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Contest not found".to_string()))?;

    // Check contest is running
    let now = Utc::now();
    if contest.status != "active" {
        return Err(ApiError::Validation("Contest is not active".to_string()));
    }
    if now < contest.starts_at {
        return Err(ApiError::Validation("Contest has not started yet".to_string()));
    }
    if now > contest.ends_at {
        return Err(ApiError::Validation("Contest has ended".to_string()));
    }

    // Check user is participant
    let is_participant: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
        r#"SELECT EXISTS(SELECT 1 FROM contest_participants WHERE contest_id = $1 AND user_id = $2)"#,
    )
    .bind(params.contest_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let is_participant = is_participant.unwrap_or(false);

    if !is_participant {
        let is_admin = user.role == "admin";
        let is_collaborator: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
            r#"SELECT EXISTS(SELECT 1 FROM contest_collaborators WHERE contest_id = $1 AND user_id = $2)"#,
        )
        .bind(params.contest_id)
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
        let is_collaborator = is_collaborator.unwrap_or(false);

        if !is_admin && !is_collaborator {
            return Err(ApiError::Forbidden);
        }
    }

    // Check problem exists and is in contest
    let problem_in_contest: Option<bool> = sqlx::query_scalar::<_, Option<bool>>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM problems p
            JOIN contest_problems cp ON cp.problem_id = p.id
            WHERE p.id = $1 AND cp.contest_id = $2
        )
        "#,
    )
    .bind(params.problem_id)
    .bind(params.contest_id)
    .fetch_one(&state.db)
    .await?;
    let problem_in_contest = problem_in_contest.unwrap_or(false);

    if !problem_in_contest {
        return Err(ApiError::NotFound("Problem not found in this contest".to_string()));
    }

    // Get contest-specific upload size limit
    let max_size = get_contest_upload_limit(&state.db, params.contest_id).await?;

    // Process multipart form with streaming size validation
    let mut zip_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::Validation(format!("Failed to read multipart: {}", e))
    })? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                ApiError::Validation(format!("Failed to read file: {}", e))
            })?;

            // Contest-specific size limit (default 10MB, max 100MB)
            if data.len() > max_size {
                return Err(ApiError::Validation(
                    format!(
                        "File size ({:.2}MB) exceeds contest limit ({:.2}MB)",
                        data.len() as f64 / 1024.0 / 1024.0,
                        max_size as f64 / 1024.0 / 1024.0
                    )
                ));
            }

            zip_data = Some(data.to_vec());
        }
    }

    let zip_data = zip_data.ok_or_else(|| {
        ApiError::Validation("No file uploaded".to_string())
    })?;

    // Validate ZIP structure (includes security checks)
    validate_zip_structure(&zip_data)?;

    // Create submission
    let submission_id = Uuid::new_v4();
    let submitted_at = Utc::now();
    let file_size = zip_data.len() as i64;

    // Save ZIP to storage
    let storage_path = format!(
        "/mnt/data/submissions/{}/{}/{}.zip",
        params.contest_id, user_id, submission_id
    );

    // Create directory structure
    if let Some(parent) = std::path::Path::new(&storage_path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ApiError::Internal(format!("Failed to create directory: {}", e))
        })?;
    }

    // Write file
    tokio::fs::write(&storage_path, &zip_data).await.map_err(|e| {
        ApiError::Internal(format!("Failed to save submission: {}", e))
    })?;

    // Insert into database (including file_size for tracking)
    sqlx::query(
        r#"
        INSERT INTO submissions (
            id, contest_id, problem_id, user_id,
            submission_type, file_path, file_size_bytes,
            status, submitted_at
        )
        VALUES ($1, $2, $3, $4, 'zip', $5, $6, 'pending', $7)
        "#,
    )
    .bind(submission_id)
    .bind(params.contest_id)
    .bind(params.problem_id)
    .bind(user_id)
    .bind(&storage_path)
    .bind(file_size)
    .bind(submitted_at)
    .execute(&state.db)
    .await?;

    // Queue for compilation via Redis Stream
    let mut conn = state.redis.get().await?;

    let stream_id: String = redis::cmd("XADD")
        .arg("compile_queue")
        .arg("*")
        .arg("submission_id")
        .arg(submission_id.to_string())
        .arg("type")
        .arg("zip")
        .arg("file_path")
        .arg(&storage_path)
        .query_async(&mut *conn)
        .await?;

    tracing::info!(
        submission_id = %submission_id,
        stream_id = %stream_id,
        "ZIP submission queued for compilation"
    );

    Ok(Json(SubmissionResponse {
        id: submission_id,
        contest_id: params.contest_id,
        problem_id: params.problem_id,
        submission_type: "zip".to_string(),
        language: None,
        status: "pending".to_string(),
        submitted_at,
        message: "Submission queued for compilation".to_string(),
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct ContestCheckRowSimple {
    #[allow(dead_code)]
    id: Uuid,
    status: String,
    starts_at: chrono::DateTime<Utc>,
    ends_at: chrono::DateTime<Utc>,
}

/// Default submission size limit in bytes (10MB)
const DEFAULT_MAX_SUBMISSION_SIZE: usize = 10 * 1024 * 1024;

/// Maximum allowed submission size in bytes (100MB)
const MAX_ALLOWED_SUBMISSION_SIZE: usize = 100 * 1024 * 1024;

/// Get contest-specific upload limit or return default
async fn get_contest_upload_limit(
    db: &sqlx::PgPool,
    contest_id: Uuid,
) -> Result<usize, ApiError> {
    let limit: Option<i32> = sqlx::query_scalar(
        "SELECT max_submission_size_mb FROM contests WHERE id = $1"
    )
    .bind(contest_id)
    .fetch_optional(db)
    .await?
    .flatten();
    
    Ok(limit
        .map(|mb| (mb as usize * 1024 * 1024).min(MAX_ALLOWED_SUBMISSION_SIZE))
        .unwrap_or(DEFAULT_MAX_SUBMISSION_SIZE))
}

/// Validate ZIP structure with security checks
/// - Must contain compile.sh and run.sh
/// - No symlinks pointing outside archive
/// - No absolute paths
/// - Total uncompressed size < 5x compressed size (zip bomb protection)
fn validate_zip_structure(data: &[u8]) -> Result<(), ApiError> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let compressed_size = data.len();
    let reader = Cursor::new(data);
    let mut archive = ZipArchive::new(reader).map_err(|e| {
        ApiError::Validation(format!("Invalid ZIP file: {}", e))
    })?;

    let mut has_compile = false;
    let mut has_run = false;
    let mut total_uncompressed: u64 = 0;
    let max_uncompressed = (compressed_size as u64) * 5; // Zip bomb protection

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| {
            ApiError::Validation(format!("Failed to read ZIP entry: {}", e))
        })?;
        
        let name = file.name();
        
        // Security: Check for path traversal
        if name.contains("..") {
            return Err(ApiError::Validation(
                "ZIP contains path traversal (..): rejected for security".to_string(),
            ));
        }
        
        // Security: Check for absolute paths
        if name.starts_with('/') {
            return Err(ApiError::Validation(
                "ZIP contains absolute path: rejected for security".to_string(),
            ));
        }
        
        // Security: Check for symlinks
        if file.is_symlink() {
            return Err(ApiError::Validation(
                "ZIP contains symlinks: rejected for security".to_string(),
            ));
        }
        
        // Track uncompressed size for zip bomb detection
        total_uncompressed += file.size();
        if total_uncompressed > max_uncompressed {
            return Err(ApiError::Validation(
                "ZIP uncompressed size exceeds limit (potential zip bomb)".to_string(),
            ));
        }

        // Check for required files
        let normalized = name.trim_start_matches("./");
        if normalized == "compile.sh" {
            has_compile = true;
        }
        if normalized == "run.sh" {
            has_run = true;
        }
    }

    if !has_compile {
        return Err(ApiError::Validation(
            "ZIP must contain compile.sh".to_string(),
        ));
    }
    if !has_run {
        return Err(ApiError::Validation(
            "ZIP must contain run.sh".to_string(),
        ));
    }

    Ok(())
}

/// GET /api/v1/submissions - List submissions
pub async fn list_submissions(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Query(params): Query<ListSubmissionsQuery>,
) -> ApiResult<Json<SubmissionListResponse>> {
    let offset = ((params.page.max(1) - 1) * params.per_page) as i64;
    let limit = params.per_page.min(100) as i64;

    let submissions = sqlx::query_as::<_, SubmissionRow>(
        r#"
        SELECT 
            s.id, s.contest_id, s.problem_id, s.user_id,
            s.language, s.status, s.score,
            s.max_time_ms, s.max_memory_kb, s.submitted_at,
            u.username, u.display_name,
            p.title as problem_title, p.problem_code,
            c.title as contest_title
        FROM submissions s
        JOIN users u ON u.id = s.user_id
        JOIN problems p ON p.id = s.problem_id
        JOIN contests c ON c.id = s.contest_id
        ORDER BY s.submitted_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let total: Option<i64> = sqlx::query_scalar::<_, Option<i64>>("SELECT COUNT(*) FROM submissions")
        .fetch_one(&state.db)
        .await?;
    let total = total.unwrap_or(0);

    let total_pages = ((total as f64) / (params.per_page as f64)).ceil() as u32;

    let submissions: Vec<SubmissionSummary> = submissions
        .into_iter()
        .map(|row| SubmissionSummary {
            id: row.id,
            user: UserInfo {
                id: row.user_id,
                username: row.username,
                display_name: row.display_name,
            },
            problem: ProblemInfo {
                id: row.problem_id,
                title: row.problem_title,
                problem_code: row.problem_code,
            },
            contest: ContestInfo {
                id: row.contest_id,
                title: row.contest_title,
            },
            language: row.language,
            status: row.status,
            score: row.score,
            max_time_ms: row.max_time_ms,
            max_memory_kb: row.max_memory_kb,
            submitted_at: row.submitted_at,
        })
        .collect();

    Ok(Json(SubmissionListResponse {
        submissions,
        pagination: Pagination {
            page: params.page,
            per_page: params.per_page,
            total,
            total_pages,
        },
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct SubmissionRow {
    id: Uuid,
    contest_id: Uuid,
    problem_id: Uuid,
    user_id: Uuid,
    language: Option<String>,
    status: String,
    score: Option<i32>,
    max_time_ms: Option<i32>,
    max_memory_kb: Option<i32>,
    submitted_at: chrono::DateTime<Utc>,
    username: String,
    display_name: Option<String>,
    problem_title: String,
    problem_code: Option<String>,
    contest_title: String,
}

/// GET /api/v1/submissions/{id} - Get submission details
pub async fn get_submission(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SubmissionDetailResponse>> {
    let row = sqlx::query_as::<_, SubmissionDetailRow>(
        r#"
        SELECT 
            s.id, s.contest_id, s.problem_id, s.user_id,
            s.submission_type, s.language, s.status, s.score,
            s.total_test_cases, s.passed_test_cases,
            s.max_time_ms, s.max_memory_kb, s.compilation_log,
            s.submitted_at, s.compiled_at, s.judged_at,
            u.username, u.display_name,
            p.title as problem_title, p.problem_code,
            c.title as contest_title
        FROM submissions s
        JOIN users u ON u.id = s.user_id
        JOIN problems p ON p.id = s.problem_id
        JOIN contests c ON c.id = s.contest_id
        WHERE s.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Submission not found".to_string()))?;

    let is_owner = row.user_id == user.id || user.role == "admin";

    Ok(Json(SubmissionDetailResponse {
        id: row.id,
        user: UserInfo {
            id: row.user_id,
            username: row.username,
            display_name: row.display_name,
        },
        problem: ProblemInfo {
            id: row.problem_id,
            title: row.problem_title,
            problem_code: row.problem_code,
        },
        contest: ContestInfo {
            id: row.contest_id,
            title: row.contest_title,
        },
        submission_type: row.submission_type,
        language: row.language,
        status: row.status,
        score: row.score,
        total_test_cases: row.total_test_cases,
        passed_test_cases: row.passed_test_cases,
        max_time_ms: row.max_time_ms,
        max_memory_kb: row.max_memory_kb,
        compilation_log: if is_owner { row.compilation_log } else { None },
        submitted_at: row.submitted_at,
        compiled_at: row.compiled_at,
        judged_at: row.judged_at,
        is_owner,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct SubmissionDetailRow {
    id: Uuid,
    contest_id: Uuid,
    problem_id: Uuid,
    user_id: Uuid,
    submission_type: String,
    language: Option<String>,
    status: String,
    score: Option<i32>,
    total_test_cases: Option<i32>,
    passed_test_cases: Option<i32>,
    max_time_ms: Option<i32>,
    max_memory_kb: Option<i32>,
    compilation_log: Option<String>,
    submitted_at: chrono::DateTime<Utc>,
    compiled_at: Option<chrono::DateTime<Utc>>,
    judged_at: Option<chrono::DateTime<Utc>>,
    username: String,
    display_name: Option<String>,
    problem_title: String,
    problem_code: Option<String>,
    contest_title: String,
}

/// GET /api/v1/submissions/{id}/results - Get test case results
pub async fn get_submission_results(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SubmissionResultsResponse>> {
    // First check submission exists and get owner info
    let submission = sqlx::query_as::<_, SubmissionStatusRow>(
        r#"
        SELECT user_id, status, score, total_test_cases, passed_test_cases
        FROM submissions WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Submission not found".to_string()))?;

    // Only owner, admin, or after contest can see detailed results
    let is_owner = submission.user_id == user.id || user.role == "admin";

    if !is_owner {
        return Err(ApiError::Forbidden);
    }

    let results = sqlx::query_as::<_, TestCaseResultRow>(
        r#"
        SELECT test_case_number, verdict, time_ms, memory_kb, checker_score
        FROM submission_results
        WHERE submission_id = $1
        ORDER BY test_case_number
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(SubmissionResultsResponse {
        submission_id: id,
        status: submission.status,
        score: submission.score,
        total_test_cases: submission.total_test_cases,
        passed_test_cases: submission.passed_test_cases,
        results: results
            .into_iter()
            .map(|r| TestCaseResult {
                test_case_number: r.test_case_number,
                verdict: r.verdict,
                time_ms: r.time_ms,
                memory_kb: r.memory_kb,
                checker_score: r.checker_score,
            })
            .collect(),
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct SubmissionStatusRow {
    user_id: Uuid,
    status: String,
    score: Option<i32>,
    total_test_cases: Option<i32>,
    passed_test_cases: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct TestCaseResultRow {
    test_case_number: i32,
    verdict: String,
    time_ms: Option<i32>,
    memory_kb: Option<i32>,
    checker_score: Option<f64>,
}

/// GET /api/v1/submissions/{id}/source - Get source code
pub async fn get_submission_source(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SourceCodeResponse>> {
    let submission = sqlx::query_as::<_, SubmissionSourceRow>(
        r#"
        SELECT user_id, submission_type, language, source_code
        FROM submissions WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Submission not found".to_string()))?;

    // Only owner or admin can view source
    if submission.user_id != user.id && user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    Ok(Json(SourceCodeResponse {
        submission_id: id,
        language: submission.language,
        source_code: submission.source_code,
        submission_type: submission.submission_type,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct SubmissionSourceRow {
    user_id: Uuid,
    submission_type: String,
    language: Option<String>,
    source_code: Option<String>,
}

/// GET /api/v1/users/{id}/submissions - Get user's submissions
pub async fn get_user_submissions(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<ListSubmissionsQuery>,
) -> ApiResult<Json<SubmissionListResponse>> {
    let offset = ((params.page.max(1) - 1) * params.per_page) as i64;
    let limit = params.per_page.min(100) as i64;

    let submissions = sqlx::query_as::<_, SubmissionRow>(
        r#"
        SELECT 
            s.id, s.contest_id, s.problem_id, s.user_id,
            s.language, s.status, s.score,
            s.max_time_ms, s.max_memory_kb, s.submitted_at,
            u.username, u.display_name,
            p.title as problem_title, p.problem_code,
            c.title as contest_title
        FROM submissions s
        JOIN users u ON u.id = s.user_id
        JOIN problems p ON p.id = s.problem_id
        JOIN contests c ON c.id = s.contest_id
        WHERE s.user_id = $1
        ORDER BY s.submitted_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let total: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COUNT(*) FROM submissions WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let total = total.unwrap_or(0);

    let total_pages = ((total as f64) / (params.per_page as f64)).ceil() as u32;

    let submissions: Vec<SubmissionSummary> = submissions
        .into_iter()
        .map(|row| SubmissionSummary {
            id: row.id,
            user: UserInfo {
                id: row.user_id,
                username: row.username,
                display_name: row.display_name,
            },
            problem: ProblemInfo {
                id: row.problem_id,
                title: row.problem_title,
                problem_code: row.problem_code,
            },
            contest: ContestInfo {
                id: row.contest_id,
                title: row.contest_title,
            },
            language: row.language,
            status: row.status,
            score: row.score,
            max_time_ms: row.max_time_ms,
            max_memory_kb: row.max_memory_kb,
            submitted_at: row.submitted_at,
        })
        .collect();

    Ok(Json(SubmissionListResponse {
        submissions,
        pagination: Pagination {
            page: params.page,
            per_page: params.per_page,
            total,
            total_pages,
        },
    }))
}

/// GET /api/v1/contests/{id}/leaderboard - Get contest leaderboard
pub async fn get_contest_leaderboard(
    State(state): State<AppState>,
    Path(contest_id): Path<Uuid>,
    Query(params): Query<LeaderboardQuery>,
) -> ApiResult<Json<LeaderboardResponse>> {
    // Get contest info
    let contest = sqlx::query_as::<_, ContestLeaderboardRow>(
        r#"
        SELECT id, title, scoring_type, leaderboard_frozen
        FROM contests WHERE id = $1
        "#,
    )
    .bind(contest_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Contest not found".to_string()))?;

    // Get problems in contest
    let problems = sqlx::query_as::<_, ContestProblemRow>(
        r#"
        SELECT p.id, p.title, p.problem_code, cp.points
        FROM problems p
        JOIN contest_problems cp ON cp.problem_id = p.id
        WHERE cp.contest_id = $1
        ORDER BY cp.order_index
        "#,
    )
    .bind(contest_id)
    .fetch_all(&state.db)
    .await?;

    let leaderboard_problems: Vec<LeaderboardProblem> = problems
        .iter()
        .map(|p| LeaderboardProblem {
            problem_code: p.problem_code.clone().unwrap_or_default(),
            title: p.title.clone(),
            max_score: p.points.unwrap_or(100),
        })
        .collect();

    // Calculate standings
    let offset = ((params.page.max(1) - 1) * params.per_page) as i64;
    let limit = params.per_page.min(100) as i64;

    let standings = sqlx::query_as::<_, StandingRow>(
        r#"
        WITH user_problem_scores AS (
            SELECT 
                s.user_id,
                s.problem_id,
                MAX(CASE WHEN s.status = 'accepted' THEN s.score ELSE 0 END) as best_score,
                COUNT(*) as attempts,
                BOOL_OR(s.status = 'accepted') as solved,
                MIN(CASE WHEN s.status = 'accepted' THEN s.submitted_at END) as first_solved_at
            FROM submissions s
            WHERE s.contest_id = $1
            GROUP BY s.user_id, s.problem_id
        ),
        user_totals AS (
            SELECT 
                ups.user_id,
                SUM(ups.best_score) as total_score,
                COUNT(CASE WHEN ups.solved THEN 1 END) as problems_solved,
                SUM(ups.attempts) as total_attempts,
                MAX(ups.first_solved_at) as last_ac
            FROM user_problem_scores ups
            GROUP BY ups.user_id
        )
        SELECT 
            ut.user_id,
            u.username,
            u.display_name,
            COALESCE(ut.total_score, 0)::bigint as total_score,
            COALESCE(ut.problems_solved, 0)::bigint as problems_solved,
            ut.last_ac as last_submission_at
        FROM user_totals ut
        JOIN users u ON u.id = ut.user_id
        ORDER BY ut.total_score DESC, ut.last_ac ASC NULLS LAST
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(contest_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let total: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT COUNT(DISTINCT user_id) 
        FROM submissions 
        WHERE contest_id = $1
        "#,
    )
    .bind(contest_id)
    .fetch_one(&state.db)
    .await?;
    let total = total.unwrap_or(0);

    let total_pages = ((total as f64) / (params.per_page as f64)).ceil() as u32;

    // Build leaderboard entries
    let mut entries = Vec::new();
    let base_rank = offset as i32 + 1;

    for (idx, row) in standings.into_iter().enumerate() {
        // Get per-problem scores for this user
        let problem_scores = sqlx::query_as::<_, ProblemScoreRow>(
            r#"
            SELECT 
                p.problem_code,
                MAX(CASE WHEN s.status = 'accepted' THEN s.score ELSE 0 END) as score,
                COUNT(s.id) as attempts,
                BOOL_OR(s.status = 'accepted') as solved,
                MIN(CASE WHEN s.status = 'accepted' THEN s.submitted_at END) as first_solved_at
            FROM contest_problems cp
            JOIN problems p ON p.id = cp.problem_id
            LEFT JOIN submissions s ON s.problem_id = p.id AND s.user_id = $1 AND s.contest_id = $2
            WHERE cp.contest_id = $2
            GROUP BY p.id, p.problem_code, cp.order_index
            ORDER BY cp.order_index
            "#,
        )
        .bind(row.user_id)
        .bind(contest_id)
        .fetch_all(&state.db)
        .await?;

        entries.push(LeaderboardEntry {
            rank: base_rank + idx as i32,
            user: UserInfo {
                id: row.user_id,
                username: row.username,
                display_name: row.display_name,
            },
            total_score: row.total_score as i32,
            total_penalty: 0, // TODO: ICPC penalty calculation
            problems_solved: row.problems_solved as i32,
            problem_scores: problem_scores
                .into_iter()
                .map(|ps| ProblemScore {
                    problem_code: ps.problem_code.unwrap_or_default(),
                    score: ps.score.map(|s| s as i32),
                    attempts: ps.attempts as i32,
                    solved: ps.solved.unwrap_or(false),
                    first_solved_at: ps.first_solved_at,
                })
                .collect(),
            last_submission_at: row.last_submission_at,
        });
    }

    Ok(Json(LeaderboardResponse {
        contest_id,
        contest_title: contest.title,
        scoring_type: contest.scoring_type.unwrap_or_else(|| "ioi".to_string()),
        entries,
        pagination: Pagination {
            page: params.page,
            per_page: params.per_page,
            total,
            total_pages,
        },
        frozen: contest.leaderboard_frozen.unwrap_or(false),
        problems: leaderboard_problems,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct ContestLeaderboardRow {
    #[allow(dead_code)]
    id: Uuid,
    title: String,
    scoring_type: Option<String>,
    leaderboard_frozen: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct ContestProblemRow {
    #[allow(dead_code)]
    id: Uuid,
    title: String,
    problem_code: Option<String>,
    points: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct StandingRow {
    user_id: Uuid,
    username: String,
    display_name: Option<String>,
    total_score: i64,
    problems_solved: i64,
    last_submission_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProblemScoreRow {
    problem_code: Option<String>,
    score: Option<i64>,
    attempts: i64,
    solved: Option<bool>,
    first_solved_at: Option<chrono::DateTime<Utc>>,
}
