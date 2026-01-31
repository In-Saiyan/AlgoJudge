//! Contest repository

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppResult, models::Contest};

/// Repository for contest database operations
pub struct ContestRepository;

impl ContestRepository {
    /// Create a new contest
    pub async fn create(
        pool: &PgPool,
        title: &str,
        description: Option<&str>,
        organizer_id: &Uuid,
        scoring_mode: &str,
        visibility: &str,
        registration_mode: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        registration_start: Option<DateTime<Utc>>,
        registration_end: Option<DateTime<Utc>>,
        allowed_languages: &[String],
        freeze_time_minutes: Option<i32>,
        allow_virtual: bool,
    ) -> AppResult<Contest> {
        let contest = sqlx::query_as::<_, Contest>(
            r#"
            INSERT INTO contests (
                title, description, organizer_id, scoring_mode, visibility,
                registration_mode, start_time, end_time, registration_start,
                registration_end, allowed_languages, freeze_time_minutes, allow_virtual
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(title)
        .bind(description)
        .bind(organizer_id)
        .bind(scoring_mode)
        .bind(visibility)
        .bind(registration_mode)
        .bind(start_time)
        .bind(end_time)
        .bind(registration_start)
        .bind(registration_end)
        .bind(allowed_languages)
        .bind(freeze_time_minutes)
        .bind(allow_virtual)
        .fetch_one(pool)
        .await?;

        Ok(contest)
    }

    /// Find contest by ID
    pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> AppResult<Option<Contest>> {
        let contest = sqlx::query_as::<_, Contest>(r#"SELECT * FROM contests WHERE id = $1"#)
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(contest)
    }

    /// Update contest
    pub async fn update(
        pool: &PgPool,
        id: &Uuid,
        title: Option<&str>,
        description: Option<&str>,
        scoring_mode: Option<&str>,
        visibility: Option<&str>,
        registration_mode: Option<&str>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        registration_start: Option<DateTime<Utc>>,
        registration_end: Option<DateTime<Utc>>,
        allowed_languages: Option<&[String]>,
        freeze_time_minutes: Option<i32>,
        allow_virtual: Option<bool>,
    ) -> AppResult<Contest> {
        let contest = sqlx::query_as::<_, Contest>(
            r#"
            UPDATE contests
            SET 
                title = COALESCE($2, title),
                description = COALESCE($3, description),
                scoring_mode = COALESCE($4, scoring_mode),
                visibility = COALESCE($5, visibility),
                registration_mode = COALESCE($6, registration_mode),
                start_time = COALESCE($7, start_time),
                end_time = COALESCE($8, end_time),
                registration_start = COALESCE($9, registration_start),
                registration_end = COALESCE($10, registration_end),
                allowed_languages = COALESCE($11, allowed_languages),
                freeze_time_minutes = COALESCE($12, freeze_time_minutes),
                allow_virtual = COALESCE($13, allow_virtual),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(description)
        .bind(scoring_mode)
        .bind(visibility)
        .bind(registration_mode)
        .bind(start_time)
        .bind(end_time)
        .bind(registration_start)
        .bind(registration_end)
        .bind(allowed_languages)
        .bind(freeze_time_minutes)
        .bind(allow_virtual)
        .fetch_one(pool)
        .await?;

        Ok(contest)
    }

    /// Delete contest
    pub async fn delete(pool: &PgPool, id: &Uuid) -> AppResult<()> {
        sqlx::query(r#"DELETE FROM contests WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// List contests with pagination
    pub async fn list(
        pool: &PgPool,
        offset: i64,
        limit: i64,
        status: Option<&str>,
        visibility: Option<&str>,
        search: Option<&str>,
    ) -> AppResult<(Vec<Contest>, i64)> {
        let search_pattern = search.map(|s| format!("%{}%", s));
        let now = Utc::now();

        let contests = sqlx::query_as::<_, Contest>(
            r#"
            SELECT * FROM contests
            WHERE 
                ($1::text IS NULL OR visibility = $1)
                AND ($2::text IS NULL OR title ILIKE $2)
                AND (
                    $3::text IS NULL
                    OR ($3 = 'upcoming' AND start_time > $4)
                    OR ($3 = 'ongoing' AND start_time <= $4 AND end_time > $4)
                    OR ($3 = 'ended' AND end_time <= $4)
                )
            ORDER BY start_time DESC
            OFFSET $5 LIMIT $6
            "#,
        )
        .bind(visibility)
        .bind(&search_pattern)
        .bind(status)
        .bind(now)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM contests
            WHERE 
                ($1::text IS NULL OR visibility = $1)
                AND ($2::text IS NULL OR title ILIKE $2)
                AND (
                    $3::text IS NULL
                    OR ($3 = 'upcoming' AND start_time > $4)
                    OR ($3 = 'ongoing' AND start_time <= $4 AND end_time > $4)
                    OR ($3 = 'ended' AND end_time <= $4)
                )
            "#,
        )
        .bind(visibility)
        .bind(&search_pattern)
        .bind(status)
        .bind(now)
        .fetch_one(pool)
        .await?;

        Ok((contests, count))
    }

    /// Register participant for contest
    pub async fn register_participant(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
        is_virtual: bool,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO contest_participants (contest_id, user_id, is_virtual)
            VALUES ($1, $2, $3)
            ON CONFLICT (contest_id, user_id) DO NOTHING
            "#,
        )
        .bind(contest_id)
        .bind(user_id)
        .bind(is_virtual)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Unregister participant from contest
    pub async fn unregister_participant(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
    ) -> AppResult<()> {
        sqlx::query(
            r#"DELETE FROM contest_participants WHERE contest_id = $1 AND user_id = $2"#,
        )
        .bind(contest_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Check if user is registered for contest
    pub async fn is_participant(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
    ) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
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

        Ok(exists)
    }

    /// Get participant count for contest
    pub async fn get_participant_count(pool: &PgPool, contest_id: &Uuid) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM contest_participants WHERE contest_id = $1"#,
        )
        .bind(contest_id)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// Count total contests
    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM contests"#)
            .fetch_one(pool)
            .await?;

        Ok(count)
    }
}
