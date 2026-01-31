//! Submission repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppResult, models::Submission};

/// Repository for submission database operations
pub struct SubmissionRepository;

impl SubmissionRepository {
    /// Create a new submission
    pub async fn create(
        pool: &PgPool,
        user_id: &Uuid,
        problem_id: &Uuid,
        contest_id: Option<&Uuid>,
        language: &str,
        source_code: &str,
        verdict: &str,
    ) -> AppResult<Submission> {
        let submission = sqlx::query_as::<_, Submission>(
            r#"
            INSERT INTO submissions (user_id, problem_id, contest_id, language, source_code, verdict)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(problem_id)
        .bind(contest_id)
        .bind(language)
        .bind(source_code)
        .bind(verdict)
        .fetch_one(pool)
        .await?;

        Ok(submission)
    }

    /// Find submission by ID
    pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> AppResult<Option<Submission>> {
        let submission = sqlx::query_as::<_, Submission>(r#"SELECT * FROM submissions WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(submission)
    }

    /// Update submission verdict and metrics
    pub async fn update_result(
        pool: &PgPool,
        id: &Uuid,
        verdict: &str,
        execution_time_ms: Option<f64>,
        memory_usage_kb: Option<i64>,
        score: Option<i32>,
        compilation_output: Option<&str>,
    ) -> AppResult<Submission> {
        let submission = sqlx::query_as::<_, Submission>(
            r#"
            UPDATE submissions
            SET 
                verdict = $2,
                execution_time_ms = $3,
                memory_usage_kb = $4,
                score = $5,
                compilation_output = $6,
                judged_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(verdict)
        .bind(execution_time_ms)
        .bind(memory_usage_kb)
        .bind(score)
        .bind(compilation_output)
        .fetch_one(pool)
        .await?;

        Ok(submission)
    }

    /// Update submission verdict (simpler version for benchmark runner)
    pub async fn update_verdict(
        pool: &PgPool,
        id: &Uuid,
        verdict: &str,
        execution_time_ms: Option<i32>,
        memory_usage_kb: Option<i32>,
        score: Option<i32>,
        compilation_output: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE submissions
            SET 
                verdict = $2,
                execution_time_ms = $3,
                memory_usage_kb = $4,
                score = $5,
                compilation_output = $6,
                judged_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(verdict)
        .bind(execution_time_ms)
        .bind(memory_usage_kb)
        .bind(score)
        .bind(compilation_output)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// List submissions with pagination and filters
    pub async fn list(
        pool: &PgPool,
        offset: i64,
        limit: i64,
        user_id: Option<&Uuid>,
        problem_id: Option<&Uuid>,
        contest_id: Option<&Uuid>,
        language: Option<&str>,
        verdict: Option<&str>,
    ) -> AppResult<(Vec<Submission>, i64)> {
        let submissions = sqlx::query_as::<_, Submission>(
            r#"
            SELECT * FROM submissions
            WHERE 
                ($1::uuid IS NULL OR user_id = $1)
                AND ($2::uuid IS NULL OR problem_id = $2)
                AND ($3::uuid IS NULL OR contest_id = $3)
                AND ($4::text IS NULL OR language = $4)
                AND ($5::text IS NULL OR verdict = $5)
            ORDER BY submitted_at DESC
            OFFSET $6 LIMIT $7
            "#,
        )
        .bind(user_id)
        .bind(problem_id)
        .bind(contest_id)
        .bind(language)
        .bind(verdict)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM submissions
            WHERE 
                ($1::uuid IS NULL OR user_id = $1)
                AND ($2::uuid IS NULL OR problem_id = $2)
                AND ($3::uuid IS NULL OR contest_id = $3)
                AND ($4::text IS NULL OR language = $4)
                AND ($5::text IS NULL OR verdict = $5)
            "#,
        )
        .bind(user_id)
        .bind(problem_id)
        .bind(contest_id)
        .bind(language)
        .bind(verdict)
        .fetch_one(pool)
        .await?;

        Ok((submissions, count))
    }

    /// Get pending submissions
    pub async fn get_pending(pool: &PgPool, limit: i64) -> AppResult<Vec<Submission>> {
        let submissions = sqlx::query_as::<_, Submission>(
            r#"
            SELECT * FROM submissions 
            WHERE verdict = 'pending' 
            ORDER BY submitted_at 
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(submissions)
    }

    /// Count total submissions
    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM submissions"#)
            .fetch_one(pool)
            .await?;

        Ok(count)
    }

    /// Count submissions by verdict
    pub async fn count_by_verdict(pool: &PgPool, verdict: &str) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM submissions WHERE verdict = $1"#,
        )
        .bind(verdict)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }
}
