//! User service

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    constants::roles,
    db::repositories::UserRepository,
    error::{AppError, AppResult},
    handlers::users::response::{SubmissionSummary, UserStatsResponse},
    models::User,
};

/// User service for business logic
pub struct UserService;

impl UserService {
    /// Get user by ID
    pub async fn get_user_by_id(pool: &PgPool, id: &Uuid) -> AppResult<User> {
        UserRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))
    }

    /// List users with pagination
    pub async fn list_users(
        pool: &PgPool,
        page: u32,
        per_page: u32,
        search: Option<&str>,
        role: Option<&str>,
    ) -> AppResult<(Vec<User>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        UserRepository::list(pool, offset, limit, search, role).await
    }

    /// Update user profile
    pub async fn update_user(
        pool: &PgPool,
        requester_id: &Uuid,
        target_id: &Uuid,
        requester_role: &str,
        display_name: Option<&str>,
        email: Option<&str>,
        current_password: Option<&str>,
        new_password: Option<&str>,
    ) -> AppResult<User> {
        // Check permissions
        if requester_id != target_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot update other users' profiles".to_string(),
            ));
        }

        // If changing password, verify current password
        let password_hash = if let Some(new_pwd) = new_password {
            let current_pwd = current_password
                .ok_or_else(|| AppError::Validation("Current password required".to_string()))?;

            let user = UserRepository::find_by_id(pool, target_id)
                .await?
                .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

            // Verify current password
            let argon2 = argon2::Argon2::default();
            let parsed_hash = argon2::PasswordHash::new(&user.password_hash)
                .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid password hash")))?;

            if argon2::PasswordVerifier::verify_password(&argon2, current_pwd.as_bytes(), &parsed_hash).is_err() {
                return Err(AppError::InvalidCredentials);
            }

            // Hash new password
            use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
            let salt = SaltString::generate(&mut OsRng);
            Some(
                argon2
                    .hash_password(new_pwd.as_bytes(), &salt)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hashing failed: {}", e)))?
                    .to_string(),
            )
        } else {
            None
        };

        UserRepository::update(
            pool,
            target_id,
            email,
            display_name,
            password_hash.as_deref(),
        )
        .await
    }

    /// Get user's submission history
    pub async fn get_user_submissions(
        pool: &PgPool,
        user_id: &Uuid,
        page: u32,
        per_page: u32,
    ) -> AppResult<(Vec<SubmissionSummary>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        // This would normally join with problems table
        let submissions = sqlx::query_as::<_, SubmissionSummary>(
            r#"
            SELECT 
                s.id,
                s.problem_id,
                p.title as problem_title,
                s.language,
                s.verdict,
                s.execution_time_ms,
                s.memory_usage_kb,
                s.submitted_at
            FROM submissions s
            JOIN problems p ON s.problem_id = p.id
            WHERE s.user_id = $1
            ORDER BY s.submitted_at DESC
            OFFSET $2 LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM submissions WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok((submissions, total))
    }

    /// Get user statistics
    pub async fn get_user_stats(pool: &PgPool, user_id: &Uuid) -> AppResult<UserStatsResponse> {
        #[derive(sqlx::FromRow)]
        struct UserStats {
            total_submissions: i64,
            accepted_submissions: i64,
            problems_solved: i64,
        }

        let stats = sqlx::query_as::<_, UserStats>(
            r#"
            SELECT 
                COUNT(*) as total_submissions,
                COUNT(*) FILTER (WHERE verdict = 'accepted') as accepted_submissions,
                COUNT(DISTINCT problem_id) FILTER (WHERE verdict = 'accepted') as problems_solved
            FROM submissions
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let contests_participated: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) 
            FROM contest_participants 
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(UserStatsResponse {
            user_id: *user_id,
            total_submissions: stats.total_submissions,
            accepted_submissions: stats.accepted_submissions,
            problems_solved: stats.problems_solved,
            contests_participated,
            rating: None, // TODO: Implement rating system
        })
    }
}
