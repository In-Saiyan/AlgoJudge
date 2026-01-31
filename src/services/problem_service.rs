//! Problem service

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    constants::roles,
    db::repositories::ProblemRepository,
    error::{AppError, AppResult},
    handlers::problems::{
        request::{CreateProblemRequest, CreateTestCaseRequest, UpdateProblemRequest, UpdateTestCaseRequest},
        response::{ProblemResponse, ProblemSummary, TestCaseFullResponse, TestCaseResponse},
    },
    models::Problem,
};

/// Problem service for business logic
pub struct ProblemService;

impl ProblemService {
    /// Create a new problem
    pub async fn create_problem(
        pool: &PgPool,
        author_id: &Uuid,
        payload: CreateProblemRequest,
    ) -> AppResult<ProblemResponse> {
        let samples_json = payload
            .samples
            .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null));

        let problem = ProblemRepository::create(
            pool,
            &payload.title,
            &payload.description,
            payload.input_format.as_deref(),
            payload.output_format.as_deref(),
            payload.constraints.as_deref(),
            samples_json,
            payload.notes.as_deref(),
            payload.time_limit_ms as i32,
            payload.memory_limit_kb as i32,
            payload.difficulty.as_deref(),
            &payload.tags.unwrap_or_default(),
            payload.is_public.unwrap_or(false),
            author_id,
        )
        .await?;

        Self::to_problem_response(pool, problem).await
    }

    /// Get problem by ID
    pub async fn get_problem(
        pool: &PgPool,
        id: &Uuid,
        can_view_private: bool,
    ) -> AppResult<ProblemResponse> {
        let problem = ProblemRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        if !problem.is_public && !can_view_private {
            return Err(AppError::NotFound("Problem not found".to_string()));
        }

        Self::to_problem_response(pool, problem).await
    }

    /// Update problem
    pub async fn update_problem(
        pool: &PgPool,
        id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
        payload: UpdateProblemRequest,
    ) -> AppResult<ProblemResponse> {
        let problem = ProblemRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check permissions
        if problem.author_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot update other users' problems".to_string(),
            ));
        }

        let samples_json = payload
            .samples
            .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null));

        let updated = ProblemRepository::update(
            pool,
            id,
            payload.title.as_deref(),
            payload.description.as_deref(),
            payload.input_format.as_deref(),
            payload.output_format.as_deref(),
            payload.constraints.as_deref(),
            samples_json,
            payload.notes.as_deref(),
            payload.time_limit_ms.map(|v| v as i32),
            payload.memory_limit_kb.map(|v| v as i32),
            payload.difficulty.as_deref(),
            payload.tags.as_deref(),
            payload.is_public,
        )
        .await?;

        Self::to_problem_response(pool, updated).await
    }

    /// Delete problem
    pub async fn delete_problem(
        pool: &PgPool,
        id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
    ) -> AppResult<()> {
        let problem = ProblemRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check permissions
        if problem.author_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot delete other users' problems".to_string(),
            ));
        }

        ProblemRepository::delete(pool, id).await
    }

    /// List problems
    pub async fn list_problems(
        pool: &PgPool,
        page: u32,
        per_page: u32,
        search: Option<&str>,
        difficulty: Option<&str>,
        tag: Option<&str>,
        show_all: bool,
    ) -> AppResult<(Vec<ProblemSummary>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        let (problems, total) =
            ProblemRepository::list(pool, offset, limit, search, difficulty, tag, show_all).await?;

        let summaries: Vec<ProblemSummary> = futures::future::try_join_all(
            problems.into_iter().map(|p| Self::to_problem_summary(pool, p)),
        )
        .await?;

        Ok((summaries, total))
    }

    /// List test cases for a problem
    pub async fn list_test_cases(
        pool: &PgPool,
        problem_id: &Uuid,
        show_full: bool,
    ) -> AppResult<(Vec<TestCaseResponse>, i64)> {
        let test_cases = ProblemRepository::get_test_cases(pool, problem_id).await?;
        let total = test_cases.len() as i64;

        let responses: Vec<TestCaseResponse> = test_cases
            .into_iter()
            .map(|tc| TestCaseResponse {
                id: tc.id,
                problem_id: tc.problem_id,
                order: tc.order,
                is_sample: tc.is_sample,
                points: tc.points,
                input: if tc.is_sample || show_full {
                    Some(tc.input)
                } else {
                    None
                },
                expected_output: if tc.is_sample || show_full {
                    Some(tc.expected_output)
                } else {
                    None
                },
                created_at: tc.created_at,
            })
            .collect();

        Ok((responses, total))
    }

    /// Add test case
    pub async fn add_test_case(
        pool: &PgPool,
        problem_id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
        payload: CreateTestCaseRequest,
    ) -> AppResult<TestCaseFullResponse> {
        let problem = ProblemRepository::find_by_id(pool, problem_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check permissions
        if problem.author_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot add test cases to other users' problems".to_string(),
            ));
        }

        let order = payload.order.unwrap_or_else(|| {
            // Would query max order + 1
            1
        });

        let test_case = ProblemRepository::create_test_case(
            pool,
            problem_id,
            &payload.input,
            &payload.expected_output,
            payload.is_sample.unwrap_or(false),
            payload.points,
            order,
        )
        .await?;

        Ok(TestCaseFullResponse {
            id: test_case.id,
            problem_id: test_case.problem_id,
            order: test_case.order,
            is_sample: test_case.is_sample,
            points: test_case.points,
            input: test_case.input,
            expected_output: test_case.expected_output,
            created_at: test_case.created_at,
        })
    }

    /// Update test case
    pub async fn update_test_case(
        pool: &PgPool,
        problem_id: &Uuid,
        tc_id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
        payload: UpdateTestCaseRequest,
    ) -> AppResult<TestCaseFullResponse> {
        let problem = ProblemRepository::find_by_id(pool, problem_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check permissions
        if problem.author_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot update test cases on other users' problems".to_string(),
            ));
        }

        let test_case = ProblemRepository::update_test_case(
            pool,
            tc_id,
            payload.input.as_deref(),
            payload.expected_output.as_deref(),
            payload.is_sample,
            payload.points,
            payload.order,
        )
        .await?;

        Ok(TestCaseFullResponse {
            id: test_case.id,
            problem_id: test_case.problem_id,
            order: test_case.order,
            is_sample: test_case.is_sample,
            points: test_case.points,
            input: test_case.input,
            expected_output: test_case.expected_output,
            created_at: test_case.created_at,
        })
    }

    /// Delete test case
    pub async fn delete_test_case(
        pool: &PgPool,
        problem_id: &Uuid,
        tc_id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
    ) -> AppResult<()> {
        let problem = ProblemRepository::find_by_id(pool, problem_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Check permissions
        if problem.author_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot delete test cases from other users' problems".to_string(),
            ));
        }

        ProblemRepository::delete_test_case(pool, tc_id).await
    }

    // Helper functions
    async fn to_problem_response(pool: &PgPool, problem: Problem) -> AppResult<ProblemResponse> {
        let author: Option<String> = sqlx::query_scalar(
            r#"SELECT username FROM users WHERE id = $1"#,
        )
        .bind(problem.author_id)
        .fetch_optional(pool)
        .await?;

        #[derive(sqlx::FromRow)]
        struct ProblemStats {
            solved_count: i64,
            attempt_count: i64,
        }

        let stats = sqlx::query_as::<_, ProblemStats>(
            r#"
            SELECT 
                COUNT(DISTINCT user_id) FILTER (WHERE verdict = 'accepted') as solved_count,
                COUNT(*) as attempt_count
            FROM submissions
            WHERE problem_id = $1
            "#,
        )
        .bind(problem.id)
        .fetch_one(pool)
        .await?;

        let samples = problem
            .samples
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        Ok(ProblemResponse {
            id: problem.id,
            title: problem.title,
            description: problem.description,
            input_format: problem.input_format,
            output_format: problem.output_format,
            constraints: problem.constraints,
            samples,
            notes: problem.notes,
            time_limit_ms: problem.time_limit_ms,
            memory_limit_kb: problem.memory_limit_kb,
            difficulty: problem.difficulty,
            tags: problem.tags,
            is_public: problem.is_public,
            author_id: problem.author_id,
            author_name: author.unwrap_or_default(),
            solved_count: stats.solved_count,
            attempt_count: stats.attempt_count,
            created_at: problem.created_at,
            updated_at: problem.updated_at,
        })
    }

    async fn to_problem_summary(pool: &PgPool, problem: Problem) -> AppResult<ProblemSummary> {
        #[derive(sqlx::FromRow)]
        struct ProblemStats {
            solved_count: i64,
            attempt_count: i64,
        }

        let stats = sqlx::query_as::<_, ProblemStats>(
            r#"
            SELECT 
                COUNT(DISTINCT user_id) FILTER (WHERE verdict = 'accepted') as solved_count,
                COUNT(*) as attempt_count
            FROM submissions
            WHERE problem_id = $1
            "#,
        )
        .bind(problem.id)
        .fetch_one(pool)
        .await?;

        Ok(ProblemSummary {
            id: problem.id,
            title: problem.title,
            difficulty: problem.difficulty,
            tags: problem.tags,
            time_limit_ms: problem.time_limit_ms,
            memory_limit_kb: problem.memory_limit_kb,
            solved_count: stats.solved_count,
            attempt_count: stats.attempt_count,
        })
    }
}
