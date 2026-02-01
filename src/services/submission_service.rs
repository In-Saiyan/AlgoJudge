//! Submission service

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    constants::verdicts,
    db::repositories::SubmissionRepository,
    error::{AppError, AppResult},
    handlers::submissions::{
        request::CreateSubmissionRequest,
        response::{
            SubmissionResponse, SubmissionResultsResponse, SubmissionSourceResponse,
            TestCaseResult, BenchmarkSummary,
        },
    },
    models::Submission,
};

/// Submission service for business logic
pub struct SubmissionService;

impl SubmissionService {
    /// Create a new submission
    pub async fn create_submission(
        pool: &PgPool,
        mut redis: ConnectionManager,
        user_id: &Uuid,
        payload: CreateSubmissionRequest,
    ) -> AppResult<Submission> {
        // Verify problem exists
        let problem_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM problems WHERE id = $1)"#,
        )
        .bind(payload.problem_id)
        .fetch_one(pool)
        .await?;

        if !problem_exists {
            return Err(AppError::NotFound("Problem not found".to_string()));
        }

        // If contest submission, verify contest and participation
        if let Some(contest_id) = payload.contest_id {
            #[derive(sqlx::FromRow)]
            struct ContestInfo {
                start_time: chrono::DateTime<chrono::Utc>,
                end_time: chrono::DateTime<chrono::Utc>,
                allowed_languages: Vec<String>,
            }

            let contest = sqlx::query_as::<_, ContestInfo>(
                r#"SELECT start_time, end_time, allowed_languages FROM contests WHERE id = $1"#,
            )
            .bind(contest_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

            // Check if contest is ongoing
            let now = chrono::Utc::now();
            if now < contest.start_time || now > contest.end_time {
                return Err(AppError::Validation("Contest is not active".to_string()));
            }

            // Check if user is registered
            let is_participant: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM contest_participants 
                    WHERE contest_id = $1 AND user_id = $2
                )
                "#,
            )
            .bind(contest_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;

            if !is_participant {
                return Err(AppError::Forbidden(
                    "Not registered for this contest".to_string(),
                ));
            }

            // Check allowed languages
            if !contest.allowed_languages.is_empty()
                && !contest.allowed_languages.contains(&payload.language)
            {
                return Err(AppError::Validation(format!(
                    "Language {} not allowed in this contest",
                    payload.language
                )));
            }
        }

        // Create submission
        let submission = SubmissionRepository::create(
            pool,
            user_id,
            &payload.problem_id,
            payload.contest_id.as_ref(),
            &payload.language,
            &payload.source_code,
            verdicts::PENDING,
        )
        .await?;

        // Add to judging queue
        redis
            .lpush::<_, _, ()>("judge_queue", submission.id.to_string())
            .await?;

        Ok(submission)
    }

    /// Create a new ZIP-based submission for algorithmic benchmarking
    pub async fn create_zip_submission(
        pool: &PgPool,
        mut redis: ConnectionManager,
        user_id: &Uuid,
        problem_id: &Uuid,
        contest_id: Option<&Uuid>,
        runtime: &str,
        zip_data: Vec<u8>,
        custom_generator: Option<Vec<u8>>,
        custom_generator_filename: Option<String>,
    ) -> AppResult<Submission> {
        // Verify problem exists and get allowed runtimes
        #[derive(sqlx::FromRow)]
        struct ProblemInfo {
            allowed_runtimes: Vec<String>,
        }

        let problem = sqlx::query_as::<_, ProblemInfo>(
            r#"SELECT allowed_runtimes FROM problems WHERE id = $1"#,
        )
        .bind(problem_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check if runtime is allowed
        if !problem.allowed_runtimes.is_empty()
            && !problem.allowed_runtimes.iter().any(|r| r == runtime)
        {
            return Err(AppError::Validation(format!(
                "Runtime '{}' not allowed for this problem. Allowed: {:?}",
                runtime, problem.allowed_runtimes
            )));
        }

        // Get runtime ID
        let runtime_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM runtimes WHERE name = $1 AND is_active = true"#,
        )
        .bind(runtime)
        .fetch_optional(pool)
        .await?;

        if runtime_id.is_none() {
            return Err(AppError::Validation(format!(
                "Unknown or inactive runtime: {}", runtime
            )));
        }

        // If contest submission, verify contest and participation
        if let Some(cid) = contest_id {
            #[derive(sqlx::FromRow)]
            struct ContestInfo {
                start_time: chrono::DateTime<chrono::Utc>,
                end_time: chrono::DateTime<chrono::Utc>,
            }

            let contest = sqlx::query_as::<_, ContestInfo>(
                r#"SELECT start_time, end_time FROM contests WHERE id = $1"#,
            )
            .bind(cid)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

            // Check if contest is ongoing
            let now = chrono::Utc::now();
            if now < contest.start_time || now > contest.end_time {
                return Err(AppError::Validation("Contest is not active".to_string()));
            }

            // Check if user is registered
            let is_participant: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM contest_participants 
                    WHERE contest_id = $1 AND user_id = $2
                )
                "#,
            )
            .bind(cid)
            .bind(user_id)
            .fetch_one(pool)
            .await?;

            if !is_participant {
                return Err(AppError::Forbidden(
                    "Not registered for this contest".to_string(),
                ));
            }
        }

        // Create ZIP submission
        let submission = SubmissionRepository::create_zip_submission(
            pool,
            user_id,
            problem_id,
            contest_id,
            runtime,
            runtime_id,
            zip_data,
            custom_generator,
            custom_generator_filename,
            verdicts::PENDING,
        )
        .await?;

        // Add to judging queue
        redis
            .lpush::<_, _, ()>("judge_queue", submission.id.to_string())
            .await?;

        Ok(submission)
    }

    /// Get submission by ID
    pub async fn get_submission(pool: &PgPool, id: &Uuid) -> AppResult<SubmissionResponse> {
        let submission = SubmissionRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Submission not found".to_string()))?;

        Self::to_submission_response(pool, submission).await
    }

    /// List submissions
    pub async fn list_submissions(
        pool: &PgPool,
        page: u32,
        per_page: u32,
        user_id: Option<&Uuid>,
        problem_id: Option<&Uuid>,
        contest_id: Option<&Uuid>,
        language: Option<&str>,
        verdict: Option<&str>,
    ) -> AppResult<(Vec<SubmissionResponse>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        let (submissions, total) = SubmissionRepository::list(
            pool, offset, limit, user_id, problem_id, contest_id, language, verdict,
        )
        .await?;

        let responses: Vec<SubmissionResponse> = futures::future::try_join_all(
            submissions
                .into_iter()
                .map(|s| Self::to_submission_response(pool, s)),
        )
        .await?;

        Ok((responses, total))
    }

    /// Get submission results
    pub async fn get_submission_results(
        pool: &PgPool,
        id: &Uuid,
        show_full: bool,
    ) -> AppResult<SubmissionResultsResponse> {
        let submission = SubmissionRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Submission not found".to_string()))?;

        // Get test case results
        let test_results = sqlx::query_as::<_, TestCaseResult>(
            r#"
            SELECT 
                tcr.test_case_id,
                tc."order" as test_case_order,
                tcr.verdict,
                tcr.execution_time_ms,
                tcr.memory_usage_kb,
                CASE WHEN tc.is_sample OR $2 THEN LEFT(tc.input, 1000) ELSE NULL END as input_preview,
                CASE WHEN tc.is_sample OR $2 THEN LEFT(tc.expected_output, 1000) ELSE NULL END as expected_output_preview,
                CASE WHEN tc.is_sample OR $2 THEN LEFT(tcr.actual_output, 1000) ELSE NULL END as actual_output_preview,
                tcr.error_message
            FROM test_case_results tcr
            JOIN test_cases tc ON tcr.test_case_id = tc.id
            WHERE tcr.submission_id = $1
            ORDER BY tc."order"
            "#,
        )
        .bind(id)
        .bind(show_full)
        .fetch_all(pool)
        .await?;

        // Get benchmark summary if available
        let benchmark_summary = sqlx::query_as::<_, BenchmarkSummary>(
            r#"
            SELECT 
                iterations,
                time_avg_ms,
                time_median_ms,
                time_min_ms,
                time_max_ms,
                time_stddev_ms,
                memory_avg_kb,
                memory_peak_kb,
                time_outliers
            FROM benchmark_results
            WHERE submission_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(SubmissionResultsResponse {
            submission_id: *id,
            verdict: submission.verdict,
            score: submission.score,
            compilation_output: submission.compilation_output,
            test_results,
            benchmark_summary,
        })
    }

    /// Get submission source code
    pub async fn get_submission_source(
        pool: &PgPool,
        id: &Uuid,
    ) -> AppResult<SubmissionSourceResponse> {
        let submission = SubmissionRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Submission not found".to_string()))?;

        Ok(SubmissionSourceResponse {
            submission_id: submission.id,
            language: submission.language,
            source_code: submission.source_code,
            submitted_at: submission.submitted_at,
        })
    }

    // Helper function
    async fn to_submission_response(
        pool: &PgPool,
        submission: Submission,
    ) -> AppResult<SubmissionResponse> {
        let username: Option<String> = sqlx::query_scalar(
            r#"SELECT username FROM users WHERE id = $1"#,
        )
        .bind(submission.user_id)
        .fetch_optional(pool)
        .await?;

        let problem_title: Option<String> = sqlx::query_scalar(
            r#"SELECT title FROM problems WHERE id = $1"#,
        )
        .bind(submission.problem_id)
        .fetch_optional(pool)
        .await?;

        Ok(SubmissionResponse {
            id: submission.id,
            user_id: submission.user_id,
            username: username.unwrap_or_default(),
            problem_id: submission.problem_id,
            problem_title: problem_title.unwrap_or_default(),
            contest_id: submission.contest_id,
            language: submission.language,
            verdict: submission.verdict,
            execution_time_ms: submission.execution_time_ms,
            memory_usage_kb: submission.memory_usage_kb,
            score: submission.score,
            submitted_at: submission.submitted_at,
            judged_at: submission.judged_at,
        })
    }
}
