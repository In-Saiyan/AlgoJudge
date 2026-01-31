//! Problem repository

use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppResult, models::{Problem, TestCase}};

/// Repository for problem database operations
pub struct ProblemRepository;

impl ProblemRepository {
    /// Create a new problem
    pub async fn create(
        pool: &PgPool,
        title: &str,
        description: &str,
        input_format: Option<&str>,
        output_format: Option<&str>,
        constraints: Option<&str>,
        samples: Option<serde_json::Value>,
        notes: Option<&str>,
        time_limit_ms: i32,
        memory_limit_kb: i32,
        difficulty: Option<&str>,
        tags: &[String],
        is_public: bool,
        author_id: &Uuid,
    ) -> AppResult<Problem> {
        let problem = sqlx::query_as::<_, Problem>(
            r#"
            INSERT INTO problems (
                title, description, input_format, output_format, constraints,
                samples, notes, time_limit_ms, memory_limit_kb, difficulty,
                tags, is_public, author_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(title)
        .bind(description)
        .bind(input_format)
        .bind(output_format)
        .bind(constraints)
        .bind(samples)
        .bind(notes)
        .bind(time_limit_ms)
        .bind(memory_limit_kb)
        .bind(difficulty)
        .bind(tags)
        .bind(is_public)
        .bind(author_id)
        .fetch_one(pool)
        .await?;

        Ok(problem)
    }

    /// Find problem by ID
    pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> AppResult<Option<Problem>> {
        let problem = sqlx::query_as::<_, Problem>(r#"SELECT * FROM problems WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(problem)
    }

    /// Update problem
    pub async fn update(
        pool: &PgPool,
        id: &Uuid,
        title: Option<&str>,
        description: Option<&str>,
        input_format: Option<&str>,
        output_format: Option<&str>,
        constraints: Option<&str>,
        samples: Option<serde_json::Value>,
        notes: Option<&str>,
        time_limit_ms: Option<i32>,
        memory_limit_kb: Option<i32>,
        difficulty: Option<&str>,
        tags: Option<&[String]>,
        is_public: Option<bool>,
    ) -> AppResult<Problem> {
        let problem = sqlx::query_as::<_, Problem>(
            r#"
            UPDATE problems
            SET 
                title = COALESCE($2, title),
                description = COALESCE($3, description),
                input_format = COALESCE($4, input_format),
                output_format = COALESCE($5, output_format),
                constraints = COALESCE($6, constraints),
                samples = COALESCE($7, samples),
                notes = COALESCE($8, notes),
                time_limit_ms = COALESCE($9, time_limit_ms),
                memory_limit_kb = COALESCE($10, memory_limit_kb),
                difficulty = COALESCE($11, difficulty),
                tags = COALESCE($12, tags),
                is_public = COALESCE($13, is_public),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(description)
        .bind(input_format)
        .bind(output_format)
        .bind(constraints)
        .bind(samples)
        .bind(notes)
        .bind(time_limit_ms)
        .bind(memory_limit_kb)
        .bind(difficulty)
        .bind(tags)
        .bind(is_public)
        .fetch_one(pool)
        .await?;

        Ok(problem)
    }

    /// Delete problem
    pub async fn delete(pool: &PgPool, id: &Uuid) -> AppResult<()> {
        sqlx::query(r#"DELETE FROM problems WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// List problems with pagination
    pub async fn list(
        pool: &PgPool,
        offset: i64,
        limit: i64,
        search: Option<&str>,
        difficulty: Option<&str>,
        tag: Option<&str>,
        show_all: bool,
    ) -> AppResult<(Vec<Problem>, i64)> {
        let search_pattern = search.map(|s| format!("%{}%", s));

        let problems = sqlx::query_as::<_, Problem>(
            r#"
            SELECT * FROM problems
            WHERE 
                ($1 OR is_public = true)
                AND ($2::text IS NULL OR title ILIKE $2)
                AND ($3::text IS NULL OR difficulty = $3)
                AND ($4::text IS NULL OR $4 = ANY(tags))
            ORDER BY created_at DESC
            OFFSET $5 LIMIT $6
            "#,
        )
        .bind(show_all)
        .bind(&search_pattern)
        .bind(difficulty)
        .bind(tag)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM problems
            WHERE 
                ($1 OR is_public = true)
                AND ($2::text IS NULL OR title ILIKE $2)
                AND ($3::text IS NULL OR difficulty = $3)
                AND ($4::text IS NULL OR $4 = ANY(tags))
            "#,
        )
        .bind(show_all)
        .bind(&search_pattern)
        .bind(difficulty)
        .bind(tag)
        .fetch_one(pool)
        .await?;

        Ok((problems, count))
    }

    /// Create test case
    pub async fn create_test_case(
        pool: &PgPool,
        problem_id: &Uuid,
        input: &str,
        expected_output: &str,
        is_sample: bool,
        points: Option<i32>,
        order: i32,
    ) -> AppResult<TestCase> {
        let test_case = sqlx::query_as::<_, TestCase>(
            r#"
            INSERT INTO test_cases (problem_id, input, expected_output, is_sample, points, "order")
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(problem_id)
        .bind(input)
        .bind(expected_output)
        .bind(is_sample)
        .bind(points)
        .bind(order)
        .fetch_one(pool)
        .await?;

        Ok(test_case)
    }

    /// Get test cases for problem
    pub async fn get_test_cases(pool: &PgPool, problem_id: &Uuid) -> AppResult<Vec<TestCase>> {
        let test_cases = sqlx::query_as::<_, TestCase>(
            r#"SELECT * FROM test_cases WHERE problem_id = $1 ORDER BY "order""#,
        )
        .bind(problem_id)
        .fetch_all(pool)
        .await?;

        Ok(test_cases)
    }

    /// Update test case
    pub async fn update_test_case(
        pool: &PgPool,
        id: &Uuid,
        input: Option<&str>,
        expected_output: Option<&str>,
        is_sample: Option<bool>,
        points: Option<i32>,
        order: Option<i32>,
    ) -> AppResult<TestCase> {
        let test_case = sqlx::query_as::<_, TestCase>(
            r#"
            UPDATE test_cases
            SET 
                input = COALESCE($2, input),
                expected_output = COALESCE($3, expected_output),
                is_sample = COALESCE($4, is_sample),
                points = COALESCE($5, points),
                "order" = COALESCE($6, "order")
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(input)
        .bind(expected_output)
        .bind(is_sample)
        .bind(points)
        .bind(order)
        .fetch_one(pool)
        .await?;

        Ok(test_case)
    }

    /// Delete test case
    pub async fn delete_test_case(pool: &PgPool, id: &Uuid) -> AppResult<()> {
        sqlx::query(r#"DELETE FROM test_cases WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Count total problems
    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM problems"#)
            .fetch_one(pool)
            .await?;

        Ok(count)
    }
}
