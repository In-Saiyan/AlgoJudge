//! Admin service

use bollard::Docker;
use chrono::{Duration, Utc};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::repositories::{ContestRepository, ProblemRepository, SubmissionRepository, UserRepository},
    error::AppResult,
    handlers::admin::response::{
        AdminUserResponse, ContainerInfoResponse, QueuedSubmission, SubmissionQueueResponse,
        SystemStatsResponse,
    },
};

/// Admin service for system management
pub struct AdminService;

impl AdminService {
    /// List all users with admin details
    pub async fn list_all_users(
        pool: &PgPool,
        page: u32,
        per_page: u32,
        search: Option<&str>,
        role: Option<&str>,
    ) -> AppResult<(Vec<AdminUserResponse>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;
        let search_pattern = search.map(|s| format!("%{}%", s));

        let users = sqlx::query_as::<_, AdminUserResponse>(
            r#"
            SELECT 
                id, username, email, display_name, role,
                is_banned, ban_reason, ban_expires_at,
                created_at, last_login_at
            FROM users
            WHERE 
                ($1::text IS NULL OR username ILIKE $1 OR email ILIKE $1)
                AND ($2::text IS NULL OR role = $2)
            ORDER BY created_at DESC
            OFFSET $3 LIMIT $4
            "#,
        )
        .bind(&search_pattern)
        .bind(role)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let total = UserRepository::count(pool).await?;

        Ok((users, total))
    }

    /// Update user role
    pub async fn update_user_role(pool: &PgPool, user_id: &Uuid, role: &str) -> AppResult<()> {
        UserRepository::update_role(pool, user_id, role).await?;
        Ok(())
    }

    /// Ban a user
    pub async fn ban_user(
        pool: &PgPool,
        user_id: &Uuid,
        reason: Option<&str>,
        duration_hours: Option<i64>,
    ) -> AppResult<()> {
        let expires_at = duration_hours.map(|h| Utc::now() + Duration::hours(h));
        UserRepository::ban(pool, user_id, reason, expires_at).await
    }

    /// Unban a user
    pub async fn unban_user(pool: &PgPool, user_id: &Uuid) -> AppResult<()> {
        UserRepository::unban(pool, user_id).await
    }

    /// Get system statistics
    pub async fn get_system_stats(pool: &PgPool, docker: &Docker) -> AppResult<SystemStatsResponse> {
        let total_users = UserRepository::count(pool).await?;
        let total_contests = ContestRepository::count(pool).await?;
        let total_problems = ProblemRepository::count(pool).await?;
        let total_submissions = SubmissionRepository::count(pool).await?;
        let pending_submissions = SubmissionRepository::count_by_verdict(pool, "pending").await?;

        // Get Docker container count
        let containers = docker
            .list_containers(None::<bollard::query_parameters::ListContainersOptions>)
            .await
            .unwrap_or_default();

        let active_containers = containers
            .iter()
            .filter(|c| {
                c.names
                    .as_ref()
                    .map(|names| names.iter().any(|n| n.contains("algojudge")))
                    .unwrap_or(false)
            })
            .count() as i64;

        // Uptime would normally come from a startup timestamp
        let uptime_seconds = 0u64;

        Ok(SystemStatsResponse {
            total_users,
            total_contests,
            total_problems,
            total_submissions,
            pending_submissions,
            active_containers,
            uptime_seconds,
        })
    }

    /// List benchmark containers
    pub async fn list_benchmark_containers(docker: &Docker) -> AppResult<Vec<ContainerInfoResponse>> {
        let containers = docker
            .list_containers(None::<bollard::query_parameters::ListContainersOptions>)
            .await
            .unwrap_or_default();

        let benchmark_containers: Vec<ContainerInfoResponse> = containers
            .into_iter()
            .filter(|c| {
                c.names
                    .as_ref()
                    .map(|names| names.iter().any(|n| n.contains("algojudge")))
                    .unwrap_or(false)
            })
            .map(|c| ContainerInfoResponse {
                id: c.id.unwrap_or_default(),
                name: c
                    .names
                    .and_then(|n| n.first().cloned())
                    .unwrap_or_default(),
                image: c.image.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                created_at: chrono::DateTime::from_timestamp(c.created.unwrap_or(0), 0)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
                submission_id: None, // Would parse from container labels
            })
            .collect();

        Ok(benchmark_containers)
    }

    /// Stop a container
    pub async fn stop_container(docker: &Docker, container_id: &str) -> AppResult<()> {
        docker
            .stop_container(container_id, None::<bollard::query_parameters::StopContainerOptions>)
            .await?;
        Ok(())
    }

    /// Get submission queue status
    pub async fn get_submission_queue(pool: &PgPool) -> AppResult<SubmissionQueueResponse> {
        let pending = sqlx::query_as::<_, QueuedSubmission>(
            r#"
            SELECT 
                s.id, s.user_id, u.username, s.problem_id, p.title as problem_title,
                s.language, s.verdict as status, s.submitted_at as queued_at,
                NULL::timestamptz as started_at
            FROM submissions s
            JOIN users u ON s.user_id = u.id
            JOIN problems p ON s.problem_id = p.id
            WHERE s.verdict = 'pending'
            ORDER BY s.submitted_at
            LIMIT 100
            "#,
        )
        .fetch_all(pool)
        .await?;

        let running = sqlx::query_as::<_, QueuedSubmission>(
            r#"
            SELECT 
                s.id, s.user_id, u.username, s.problem_id, p.title as problem_title,
                s.language, s.verdict as status, s.submitted_at as queued_at,
                s.submitted_at as started_at
            FROM submissions s
            JOIN users u ON s.user_id = u.id
            JOIN problems p ON s.problem_id = p.id
            WHERE s.verdict IN ('compiling', 'running')
            ORDER BY s.submitted_at
            LIMIT 100
            "#,
        )
        .fetch_all(pool)
        .await?;

        let total_pending = SubmissionRepository::count_by_verdict(pool, "pending").await?;
        let total_running = SubmissionRepository::count_by_verdict(pool, "running").await?
            + SubmissionRepository::count_by_verdict(pool, "compiling").await?;

        Ok(SubmissionQueueResponse {
            pending,
            running,
            total_pending,
            total_running,
        })
    }

    /// Rejudge a submission
    pub async fn rejudge_submission(
        pool: &PgPool,
        mut redis: ConnectionManager,
        submission_id: &Uuid,
    ) -> AppResult<()> {
        // Reset submission status
        sqlx::query(
            r#"
            UPDATE submissions 
            SET verdict = 'pending', 
                execution_time_ms = NULL, 
                memory_usage_kb = NULL,
                score = NULL,
                compilation_output = NULL,
                judged_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(submission_id)
        .execute(pool)
        .await?;

        // Delete old test case results
        sqlx::query(r#"DELETE FROM test_case_results WHERE submission_id = $1"#)
            .bind(submission_id)
            .execute(pool)
            .await?;

        // Add to queue
        redis
            .lpush::<_, _, ()>("judge_queue", submission_id.to_string())
            .await?;

        Ok(())
    }
}
