//! Contest service

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    constants::roles,
    db::repositories::ContestRepository,
    error::{AppError, AppResult},
    handlers::contests::{
        request::{AddProblemRequest, CreateContestRequest, UpdateContestRequest},
        response::{
            ContestProblemResponse, ContestResponse, ContestSummary, LeaderboardResponse,
            ParticipantResponse, RegistrationResponse, VirtualParticipationResponse,
        },
    },
    models::Contest,
};

/// Contest service for business logic
pub struct ContestService;

impl ContestService {
    /// Create a new contest
    pub async fn create_contest(
        pool: &PgPool,
        organizer_id: &Uuid,
        payload: CreateContestRequest,
    ) -> AppResult<ContestResponse> {
        let contest = ContestRepository::create(
            pool,
            &payload.title,
            payload.description.as_deref(),
            organizer_id,
            &payload.scoring_mode,
            &payload.visibility,
            &payload.registration_mode,
            payload.start_time,
            payload.end_time,
            payload.registration_start,
            payload.registration_end,
            &payload.allowed_languages.unwrap_or_default(),
            payload.freeze_time_minutes,
            payload.allow_virtual.unwrap_or(false),
        )
        .await?;

        Self::to_contest_response(pool, contest).await
    }

    /// Get contest by ID
    pub async fn get_contest(pool: &PgPool, id: &Uuid) -> AppResult<ContestResponse> {
        let contest = ContestRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        Self::to_contest_response(pool, contest).await
    }

    /// Update contest
    pub async fn update_contest(
        pool: &PgPool,
        id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
        payload: UpdateContestRequest,
    ) -> AppResult<ContestResponse> {
        let contest = ContestRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Check permissions
        if contest.organizer_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot update other users' contests".to_string(),
            ));
        }

        let updated = ContestRepository::update(
            pool,
            id,
            payload.title.as_deref(),
            payload.description.as_deref(),
            payload.scoring_mode.as_deref(),
            payload.visibility.as_deref(),
            payload.registration_mode.as_deref(),
            payload.start_time,
            payload.end_time,
            payload.registration_start,
            payload.registration_end,
            payload.allowed_languages.as_deref(),
            payload.freeze_time_minutes,
            payload.allow_virtual,
        )
        .await?;

        Self::to_contest_response(pool, updated).await
    }

    /// Delete contest
    pub async fn delete_contest(
        pool: &PgPool,
        id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
    ) -> AppResult<()> {
        let contest = ContestRepository::find_by_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Check permissions
        if contest.organizer_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot delete other users' contests".to_string(),
            ));
        }

        ContestRepository::delete(pool, id).await
    }

    /// List contests with pagination
    pub async fn list_contests(
        pool: &PgPool,
        page: u32,
        per_page: u32,
        status: Option<&str>,
        visibility: Option<&str>,
        search: Option<&str>,
    ) -> AppResult<(Vec<ContestSummary>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        let (contests, total) =
            ContestRepository::list(pool, offset, limit, status, visibility, search).await?;

        let summaries: Vec<ContestSummary> = futures::future::try_join_all(
            contests.into_iter().map(|c| Self::to_contest_summary(pool, c)),
        )
        .await?;

        Ok((summaries, total))
    }

    /// Register participant for contest
    pub async fn register_participant(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
    ) -> AppResult<RegistrationResponse> {
        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Check if registration is open
        if !contest.is_registration_open() {
            return Err(AppError::Validation("Registration is not open".to_string()));
        }

        // Check if already registered
        if ContestRepository::is_participant(pool, contest_id, user_id).await? {
            return Err(AppError::AlreadyExists(
                "Already registered for this contest".to_string(),
            ));
        }

        ContestRepository::register_participant(pool, contest_id, user_id, false).await?;

        Ok(RegistrationResponse {
            message: "Successfully registered for contest".to_string(),
            contest_id: *contest_id,
            registered_at: Utc::now(),
        })
    }

    /// Unregister participant from contest
    pub async fn unregister_participant(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
    ) -> AppResult<()> {
        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Can't unregister after contest starts
        if contest.status() != crate::models::ContestStatus::Upcoming {
            return Err(AppError::Validation(
                "Cannot unregister after contest starts".to_string(),
            ));
        }

        ContestRepository::unregister_participant(pool, contest_id, user_id).await
    }

    /// List participants
    pub async fn list_participants(
        pool: &PgPool,
        contest_id: &Uuid,
        page: u32,
        per_page: u32,
    ) -> AppResult<(Vec<ParticipantResponse>, i64)> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        let participants = sqlx::query_as::<_, ParticipantResponse>(
            r#"
            SELECT 
                cp.user_id,
                u.username,
                u.display_name,
                cp.registered_at,
                cp.is_virtual
            FROM contest_participants cp
            JOIN users u ON cp.user_id = u.id
            WHERE cp.contest_id = $1
            ORDER BY cp.registered_at
            OFFSET $2 LIMIT $3
            "#,
        )
        .bind(contest_id)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let total = ContestRepository::get_participant_count(pool, contest_id).await?;

        Ok((participants, total))
    }

    /// List problems in contest
    pub async fn list_contest_problems(
        pool: &PgPool,
        contest_id: &Uuid,
    ) -> AppResult<Vec<ContestProblemResponse>> {
        let problems = sqlx::query_as::<_, ContestProblemResponse>(
            r#"
            SELECT 
                cp.id,
                cp.contest_id,
                cp.problem_id,
                p.title,
                cp."order",
                COALESCE(cp.time_limit_ms, p.time_limit_ms) as time_limit_ms,
                COALESCE(cp.memory_limit_kb, p.memory_limit_kb) as memory_limit_kb,
                cp.points,
                COUNT(DISTINCT s.user_id) FILTER (WHERE s.verdict = 'accepted') as solved_count,
                COUNT(s.id) as attempt_count
            FROM contest_problems cp
            JOIN problems p ON cp.problem_id = p.id
            LEFT JOIN submissions s ON s.problem_id = p.id AND s.contest_id = cp.contest_id
            WHERE cp.contest_id = $1
            GROUP BY cp.id, cp.contest_id, cp.problem_id, p.title, cp."order", 
                     cp.time_limit_ms, p.time_limit_ms, cp.memory_limit_kb, p.memory_limit_kb, cp.points
            ORDER BY cp."order"
            "#,
        )
        .bind(contest_id)
        .fetch_all(pool)
        .await?;

        Ok(problems)
    }

    /// Add problem to contest
    pub async fn add_problem_to_contest(
        pool: &PgPool,
        contest_id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
        payload: AddProblemRequest,
    ) -> AppResult<()> {
        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Check permissions
        if contest.organizer_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot modify other users' contests".to_string(),
            ));
        }

        // Get next order if not specified
        let order = payload.order.unwrap_or_else(|| {
            // Would query max order + 1
            1
        });

        sqlx::query(
            r#"
            INSERT INTO contest_problems (contest_id, problem_id, "order", time_limit_ms, memory_limit_kb, points)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(contest_id)
        .bind(payload.problem_id)
        .bind(order)
        .bind(payload.time_limit_ms)
        .bind(payload.memory_limit_kb)
        .bind(payload.points)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Remove problem from contest
    pub async fn remove_problem_from_contest(
        pool: &PgPool,
        contest_id: &Uuid,
        problem_id: &Uuid,
        requester_id: &Uuid,
        requester_role: &str,
    ) -> AppResult<()> {
        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        // Check permissions
        if contest.organizer_id != *requester_id && requester_role != roles::ADMIN {
            return Err(AppError::Forbidden(
                "Cannot modify other users' contests".to_string(),
            ));
        }

        sqlx::query(
            r#"DELETE FROM contest_problems WHERE contest_id = $1 AND problem_id = $2"#,
        )
        .bind(contest_id)
        .bind(problem_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get leaderboard
    pub async fn get_leaderboard(
        pool: &PgPool,
        contest_id: &Uuid,
        page: u32,
        per_page: u32,
        _include_frozen: bool,
    ) -> AppResult<LeaderboardResponse> {
        // This is a simplified version - real implementation would be more complex
        // and depend on scoring_mode (ICPC, Codeforces, IOI)

        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        let is_frozen = contest.is_leaderboard_frozen();

        Ok(LeaderboardResponse {
            contest_id: *contest_id,
            entries: vec![], // Would be populated based on scoring mode
            total: 0,
            page,
            per_page,
            is_frozen,
            frozen_at: if is_frozen {
                Some(contest.end_time - chrono::Duration::minutes(contest.freeze_time_minutes.unwrap_or(0) as i64))
            } else {
                None
            },
            updated_at: Utc::now(),
        })
    }

    /// Start virtual participation
    pub async fn start_virtual_participation(
        pool: &PgPool,
        contest_id: &Uuid,
        user_id: &Uuid,
    ) -> AppResult<VirtualParticipationResponse> {
        let contest = ContestRepository::find_by_id(pool, contest_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contest not found".to_string()))?;

        if !contest.can_start_virtual() {
            return Err(AppError::Validation(
                "Virtual participation not available".to_string(),
            ));
        }

        let virtual_start = Utc::now();
        let duration = contest.end_time - contest.start_time;
        let virtual_end = virtual_start + duration;

        // Register as virtual participant
        ContestRepository::register_participant(pool, contest_id, user_id, true).await?;

        // Update virtual start time
        sqlx::query(
            r#"
            UPDATE contest_participants 
            SET virtual_start = $3 
            WHERE contest_id = $1 AND user_id = $2
            "#,
        )
        .bind(contest_id)
        .bind(user_id)
        .bind(virtual_start)
        .execute(pool)
        .await?;

        Ok(VirtualParticipationResponse {
            message: "Virtual participation started".to_string(),
            contest_id: *contest_id,
            virtual_start,
            virtual_end,
        })
    }

    // Helper functions
    async fn to_contest_response(pool: &PgPool, contest: Contest) -> AppResult<ContestResponse> {
        let organizer: Option<String> = sqlx::query_scalar(
            r#"SELECT username FROM users WHERE id = $1"#,
        )
        .bind(contest.organizer_id)
        .fetch_optional(pool)
        .await?;

        let participant_count =
            ContestRepository::get_participant_count(pool, &contest.id).await?;

        let problem_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM contest_problems WHERE contest_id = $1"#,
        )
        .bind(contest.id)
        .fetch_one(pool)
        .await?;

        let status = contest.status().to_string();

        Ok(ContestResponse {
            id: contest.id,
            title: contest.title,
            description: contest.description,
            scoring_mode: contest.scoring_mode,
            visibility: contest.visibility,
            registration_mode: contest.registration_mode,
            start_time: contest.start_time,
            end_time: contest.end_time,
            registration_start: contest.registration_start,
            registration_end: contest.registration_end,
            allowed_languages: contest.allowed_languages,
            freeze_time_minutes: contest.freeze_time_minutes,
            allow_virtual: contest.allow_virtual,
            organizer_id: contest.organizer_id,
            organizer_name: organizer.unwrap_or_default(),
            participant_count,
            problem_count,
            status,
            created_at: contest.created_at,
            updated_at: contest.updated_at,
        })
    }

    async fn to_contest_summary(pool: &PgPool, contest: Contest) -> AppResult<ContestSummary> {
        let participant_count =
            ContestRepository::get_participant_count(pool, &contest.id).await?;

        let problem_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM contest_problems WHERE contest_id = $1"#,
        )
        .bind(contest.id)
        .fetch_one(pool)
        .await?;

        let status = contest.status().to_string();

        Ok(ContestSummary {
            id: contest.id,
            title: contest.title,
            scoring_mode: contest.scoring_mode,
            visibility: contest.visibility,
            start_time: contest.start_time,
            end_time: contest.end_time,
            participant_count,
            problem_count,
            status,
        })
    }
}
