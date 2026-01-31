//! User repository

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppResult, models::User};

/// Repository for user database operations
pub struct UserRepository;

impl UserRepository {
    /// Create a new user
    pub async fn create(
        pool: &PgPool,
        username: &str,
        email: &str,
        password_hash: &str,
        display_name: Option<&str>,
        role: &str,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (username, email, password_hash, display_name, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .bind(role)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Find user by ID
    pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM users WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(user)
    }

    /// Find user by username
    pub async fn find_by_username(pool: &PgPool, username: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM users WHERE username = $1"#)
            .bind(username)
            .fetch_optional(pool)
            .await?;

        Ok(user)
    }

    /// Find user by email
    pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM users WHERE email = $1"#)
            .bind(email)
            .fetch_optional(pool)
            .await?;

        Ok(user)
    }

    /// Find user by username or email (for login)
    pub async fn find_by_identifier(pool: &PgPool, identifier: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT * FROM users WHERE username = $1 OR email = $1"#,
        )
        .bind(identifier)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Update user
    pub async fn update(
        pool: &PgPool,
        id: &Uuid,
        email: Option<&str>,
        display_name: Option<&str>,
        password_hash: Option<&str>,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET 
                email = COALESCE($2, email),
                display_name = COALESCE($3, display_name),
                password_hash = COALESCE($4, password_hash),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(email)
        .bind(display_name)
        .bind(password_hash)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Update user role
    pub async fn update_role(pool: &PgPool, id: &Uuid, role: &str) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET role = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(role)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// List users with pagination
    pub async fn list(
        pool: &PgPool,
        offset: i64,
        limit: i64,
        search: Option<&str>,
        role: Option<&str>,
    ) -> AppResult<(Vec<User>, i64)> {
        let search_pattern = search.map(|s| format!("%{}%", s));

        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT * FROM users
            WHERE 
                ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1)
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

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM users
            WHERE 
                ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1)
                AND ($2::text IS NULL OR role = $2)
            "#,
        )
        .bind(&search_pattern)
        .bind(role)
        .fetch_one(pool)
        .await?;

        Ok((users, count))
    }

    /// Ban user
    pub async fn ban(
        pool: &PgPool,
        id: &Uuid,
        reason: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET is_banned = true, ban_reason = $2, ban_expires_at = $3, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(reason)
        .bind(expires_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Unban user
    pub async fn unban(pool: &PgPool, id: &Uuid) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET is_banned = false, ban_reason = NULL, ban_expires_at = NULL, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update last login time
    pub async fn update_last_login(pool: &PgPool, id: &Uuid) -> AppResult<()> {
        sqlx::query(r#"UPDATE users SET last_login_at = NOW() WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Count total users
    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM users"#)
            .fetch_one(pool)
            .await?;

        Ok(count)
    }
}
