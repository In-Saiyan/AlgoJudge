//! Admin management handlers.
//!
//! All handlers require admin role (enforced by admin_middleware layer).

use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use chrono::Utc;
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;

use super::request::*;
use super::response::*;

// =============================================================================
// 7.1 User Management
// =============================================================================

/// Database row for admin user listing
#[derive(Debug, FromRow)]
struct AdminUserRow {
    id: Uuid,
    username: String,
    email: String,
    display_name: Option<String>,
    role: String,
    is_banned: bool,
    banned_at: Option<chrono::DateTime<Utc>>,
    banned_reason: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

/// GET /api/v1/admin/users
///
/// List all users with admin-level detail (includes email, ban status).
pub async fn admin_list_users(
    State(state): State<AppState>,
    Query(query): Query<AdminListUsersQuery>,
) -> ApiResult<Json<AdminUserListResponse>> {
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    // Build dynamic query
    let mut conditions = vec!["1=1".to_string()];
    let mut data_bind_idx = 3u32; // $1 = limit, $2 = offset for data query
    let mut count_bind_idx = 1u32; // count query has no limit/offset

    if query.role.is_some() {
        conditions.push(format!("role = ${data}", data = data_bind_idx));
        data_bind_idx += 1;
        count_bind_idx += 1;
    }
    if query.is_banned.is_some() {
        conditions.push(format!("is_banned = ${data}", data = data_bind_idx));
        data_bind_idx += 1;
        count_bind_idx += 1;
    }
    if query.search.is_some() {
        conditions.push(format!(
            "(username ILIKE ${data} OR email ILIKE ${data})",
            data = data_bind_idx
        ));
        // data_bind_idx += 1;
        // count_bind_idx += 1;
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, username, email, display_name, role, is_banned, banned_at, banned_reason, created_at, updated_at \
         FROM users WHERE {} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        where_clause
    );

    // Count query uses $1, $2, ... (no limit/offset)
    let count_where = {
        let mut conds = vec!["1=1".to_string()];
        let mut ci = 1u32;
        if query.role.is_some() {
            conds.push(format!("role = ${ci}"));
            ci += 1;
        }
        if query.is_banned.is_some() {
            conds.push(format!("is_banned = ${ci}"));
            ci += 1;
        }
        if query.search.is_some() {
            conds.push(format!("(username ILIKE ${ci} OR email ILIKE ${ci})"));
        }
        conds.join(" AND ")
    };
    let count_sql = format!("SELECT COUNT(*) FROM users WHERE {}", count_where);

    // Build and execute query with dynamic binds
    let mut q = sqlx::query_as::<_, AdminUserRow>(&sql)
        .bind(per_page as i64)
        .bind(offset);
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);

    if let Some(ref role) = query.role {
        q = q.bind(role.clone());
        cq = cq.bind(role.clone());
    }
    if let Some(is_banned) = query.is_banned {
        q = q.bind(is_banned);
        cq = cq.bind(is_banned);
    }
    if let Some(ref search) = query.search {
        let pattern = format!("%{}%", search);
        q = q.bind(pattern.clone());
        cq = cq.bind(pattern);
    }

    let users = q.fetch_all(&state.db).await?;
    let total = cq.fetch_one(&state.db).await?;
    let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;

    Ok(Json(AdminUserListResponse {
        users: users
            .into_iter()
            .map(|u| AdminUserResponse {
                id: u.id,
                username: u.username,
                email: u.email,
                display_name: u.display_name,
                role: u.role,
                is_banned: u.is_banned,
                banned_at: u.banned_at,
                banned_reason: u.banned_reason,
                created_at: u.created_at,
                updated_at: u.updated_at,
            })
            .collect(),
        pagination: Pagination {
            page,
            per_page,
            total,
            total_pages,
        },
    }))
}

/// PUT /api/v1/admin/users/{id}/role
///
/// Update a user's role.
pub async fn update_user_role(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> ApiResult<Json<UpdateRoleResponse>> {
    payload
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Prevent admins from changing their own role
    if admin.id == user_id {
        return Err(ApiError::Validation(
            "Cannot change your own role".to_string(),
        ));
    }

    let row = sqlx::query_as::<_, UpdateRoleRow>(
        r#"
        UPDATE users SET role = $1, updated_at = $2
        WHERE id = $3
        RETURNING id, username, role, updated_at
        "#,
    )
    .bind(&payload.role)
    .bind(Utc::now())
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    tracing::info!(
        admin_id = %admin.id,
        target_user = %user_id,
        new_role = %payload.role,
        "Admin updated user role"
    );

    Ok(Json(UpdateRoleResponse {
        id: row.id,
        username: row.username,
        role: row.role,
        updated_at: row.updated_at,
    }))
}

#[derive(Debug, FromRow)]
struct UpdateRoleRow {
    id: Uuid,
    username: String,
    role: String,
    updated_at: chrono::DateTime<Utc>,
}

/// POST /api/v1/admin/users/{id}/ban
///
/// Ban a user with a reason.
pub async fn ban_user(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<BanUserRequest>,
) -> ApiResult<Json<BanResponse>> {
    payload
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Prevent banning yourself
    if admin.id == user_id {
        return Err(ApiError::Validation("Cannot ban yourself".to_string()));
    }

    let now = Utc::now();
    let row = sqlx::query_as::<_, BanRow>(
        r#"
        UPDATE users
        SET is_banned = true, banned_at = $1, banned_reason = $2, updated_at = $1
        WHERE id = $3
        RETURNING id, username, is_banned, banned_at, banned_reason
        "#,
    )
    .bind(now)
    .bind(&payload.reason)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Invalidate all sessions for the banned user
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;

    tracing::info!(
        admin_id = %admin.id,
        target_user = %user_id,
        reason = %payload.reason,
        "Admin banned user"
    );

    Ok(Json(BanResponse {
        id: row.id,
        username: row.username,
        is_banned: row.is_banned,
        banned_at: row.banned_at,
        banned_reason: row.banned_reason,
    }))
}

/// POST /api/v1/admin/users/{id}/unban
///
/// Remove ban from a user.
pub async fn unban_user(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<BanResponse>> {
    let row = sqlx::query_as::<_, BanRow>(
        r#"
        UPDATE users
        SET is_banned = false, banned_at = NULL, banned_reason = NULL, updated_at = $1
        WHERE id = $2
        RETURNING id, username, is_banned, banned_at, banned_reason
        "#,
    )
    .bind(Utc::now())
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    tracing::info!(
        admin_id = %admin.id,
        target_user = %user_id,
        "Admin unbanned user"
    );

    Ok(Json(BanResponse {
        id: row.id,
        username: row.username,
        is_banned: row.is_banned,
        banned_at: row.banned_at,
        banned_reason: row.banned_reason,
    }))
}

#[derive(Debug, FromRow)]
struct BanRow {
    id: Uuid,
    username: String,
    is_banned: bool,
    banned_at: Option<chrono::DateTime<Utc>>,
    banned_reason: Option<String>,
}

// =============================================================================
// 7.2 System Management
// =============================================================================

/// Row type for role count aggregation
#[derive(Debug, FromRow)]
struct RoleCountRow {
    role: String,
    count: i64,
}

/// GET /api/v1/admin/stats
///
/// System-wide statistics dashboard.
pub async fn system_stats(State(state): State<AppState>) -> ApiResult<Json<SystemStatsResponse>> {
    // User stats
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    let banned_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_banned = true")
        .fetch_one(&state.db)
        .await?;

    let role_counts: Vec<RoleCountRow> = sqlx::query_as(
        "SELECT role, COUNT(*) as count FROM users GROUP BY role ORDER BY count DESC",
    )
    .fetch_all(&state.db)
    .await?;

    // Contest stats (derived from start_time / end_time, no status column)
    let total_contests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contests")
        .fetch_one(&state.db)
        .await?;
    let active_contests: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM contests WHERE start_time <= NOW() AND end_time > NOW()",
    )
    .fetch_one(&state.db)
    .await?;
    let draft_contests: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM contests WHERE start_time > NOW()")
            .fetch_one(&state.db)
            .await?;
    let finished_contests: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM contests WHERE end_time <= NOW()")
            .fetch_one(&state.db)
            .await?;

    // Submission stats
    let total_submissions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submissions")
        .fetch_one(&state.db)
        .await?;
    let pending_submissions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM submissions WHERE status IN ('pending', 'compiling', 'compiled')",
    )
    .fetch_one(&state.db)
    .await?;
    let judging_submissions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM submissions WHERE status = 'judging'")
            .fetch_one(&state.db)
            .await?;
    let accepted_submissions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM submissions WHERE status = 'accepted'")
            .fetch_one(&state.db)
            .await?;
    let rejected_submissions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM submissions WHERE status IN ('wrong_answer', 'time_limit', 'memory_limit', 'runtime_error', 'compilation_error')",
    )
    .fetch_one(&state.db)
    .await?;

    // Storage counts
    let results_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submission_results")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(SystemStatsResponse {
        users: UserStats {
            total: total_users,
            active: total_users - banned_users,
            banned: banned_users,
            by_role: role_counts
                .into_iter()
                .map(|r| RoleCount {
                    role: r.role,
                    count: r.count,
                })
                .collect(),
        },
        contests: ContestStats {
            total: total_contests,
            active: active_contests,
            draft: draft_contests,
            finished: finished_contests,
        },
        submissions: SubmissionStats {
            total: total_submissions,
            pending: pending_submissions,
            judging: judging_submissions,
            accepted: accepted_submissions,
            rejected: rejected_submissions,
        },
        storage: StorageStats {
            submissions_count: total_submissions,
            results_count,
        },
    }))
}

// =============================================================================
// 7.3 Queue Management
// =============================================================================

/// GET /api/v1/admin/queue
///
/// Get Redis Stream queue status for compile_queue and run_queue.
pub async fn get_queue_info(State(state): State<AppState>) -> ApiResult<Json<QueueInfoResponse>> {
    let mut conn = state.redis.get().await?;

    let mut queues = Vec::new();

    for stream_name in &["compile_queue", "run_queue"] {
        let detail = get_stream_info(&mut conn, stream_name).await;
        queues.push(detail);
    }

    Ok(Json(QueueInfoResponse { queues }))
}

/// Get info about a single Redis Stream.
async fn get_stream_info(conn: &mut deadpool_redis::Connection, stream_name: &str) -> QueueDetail {
    // Get stream length
    let length: i64 = redis::cmd("XLEN")
        .arg(stream_name)
        .query_async(conn)
        .await
        .unwrap_or(0);

    // Get consumer group info
    let groups = get_consumer_groups(conn, stream_name).await;

    // Get pending entries (from all groups)
    let mut pending = Vec::new();
    for group in &groups {
        if let Ok(entries) = get_pending_entries(conn, stream_name, &group.name).await {
            pending.extend(entries);
        }
    }

    QueueDetail {
        name: stream_name.to_string(),
        length,
        consumer_groups: groups,
        pending_entries: pending,
    }
}

/// Get consumer groups for a stream.
async fn get_consumer_groups(
    conn: &mut deadpool_redis::Connection,
    stream_name: &str,
) -> Vec<ConsumerGroupInfo> {
    // XINFO GROUPS <stream>
    let result: Result<Vec<redis::Value>, _> = redis::cmd("XINFO")
        .arg("GROUPS")
        .arg(stream_name)
        .query_async(conn)
        .await;

    match result {
        Ok(values) => parse_group_info(values),
        Err(_) => Vec::new(),
    }
}

/// Parse XINFO GROUPS response into ConsumerGroupInfo.
fn parse_group_info(values: Vec<redis::Value>) -> Vec<ConsumerGroupInfo> {
    let mut groups = Vec::new();

    for value in values {
        if let redis::Value::Array(fields) = value {
            let mut name = String::new();
            let mut consumers = 0i64;
            let mut pending = 0i64;
            let mut last_id = String::new();

            let mut iter = fields.iter();
            while let Some(key) = iter.next() {
                let key_str = match key {
                    redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                    redis::Value::SimpleString(s) => s.clone(),
                    _ => continue,
                };

                if let Some(val) = iter.next() {
                    match key_str.as_str() {
                        "name" => {
                            name = match val {
                                redis::Value::BulkString(b) => {
                                    String::from_utf8_lossy(b).to_string()
                                }
                                redis::Value::SimpleString(s) => s.clone(),
                                _ => String::new(),
                            };
                        }
                        "consumers" => {
                            consumers = match val {
                                redis::Value::Int(n) => *n,
                                _ => 0,
                            };
                        }
                        "pending" => {
                            pending = match val {
                                redis::Value::Int(n) => *n,
                                _ => 0,
                            };
                        }
                        "last-delivered-id" => {
                            last_id = match val {
                                redis::Value::BulkString(b) => {
                                    String::from_utf8_lossy(b).to_string()
                                }
                                redis::Value::SimpleString(s) => s.clone(),
                                _ => String::new(),
                            };
                        }
                        _ => {}
                    }
                }
            }

            if !name.is_empty() {
                groups.push(ConsumerGroupInfo {
                    name,
                    consumers,
                    pending,
                    last_delivered_id: last_id,
                });
            }
        }
    }

    groups
}

/// Get pending entries for a consumer group.
async fn get_pending_entries(
    conn: &mut deadpool_redis::Connection,
    stream_name: &str,
    group_name: &str,
) -> Result<Vec<PendingEntry>, redis::RedisError> {
    // XPENDING <stream> <group> - <+ 10
    let result: redis::Value = redis::cmd("XPENDING")
        .arg(stream_name)
        .arg(group_name)
        .arg("-")
        .arg("+")
        .arg(10)
        .query_async(conn)
        .await?;

    let mut entries = Vec::new();
    if let redis::Value::Array(items) = result {
        for item in items {
            if let redis::Value::Array(fields) = item {
                if fields.len() >= 4 {
                    let id = match &fields[0] {
                        redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let consumer = match &fields[1] {
                        redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                        _ => continue,
                    };
                    let idle_ms = match &fields[2] {
                        redis::Value::Int(n) => *n,
                        _ => 0,
                    };
                    let delivery_count = match &fields[3] {
                        redis::Value::Int(n) => *n,
                        _ => 0,
                    };

                    entries.push(PendingEntry {
                        id,
                        consumer,
                        idle_ms,
                        delivery_count,
                    });
                }
            }
        }
    }

    Ok(entries)
}

/// POST /api/v1/admin/queue/{id}/rejudge
///
/// Rejudge a submission by resetting its status and pushing to compile_queue.
pub async fn rejudge_submission(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(submission_id): Path<Uuid>,
) -> ApiResult<Json<RejudgeResponse>> {
    // Check submission exists
    let exists: Option<(String,)> = sqlx::query_as("SELECT status FROM submissions WHERE id = $1")
        .bind(submission_id)
        .fetch_optional(&state.db)
        .await?;

    let (current_status,) =
        exists.ok_or_else(|| ApiError::NotFound("Submission not found".to_string()))?;

    // Don't rejudge if already in progress
    if current_status == "compiling" || current_status == "judging" {
        return Err(ApiError::Validation(format!(
            "Submission is currently '{}', cannot rejudge",
            current_status
        )));
    }

    // Reset submission status
    sqlx::query(
        r#"
        UPDATE submissions
        SET status = 'pending',
            score = NULL,
            passed_test_cases = NULL,
            max_time_ms = NULL,
            max_memory_kb = NULL,
            compilation_log = NULL,
            compiled_at = NULL,
            judged_at = NULL
        WHERE id = $1
        "#,
    )
    .bind(submission_id)
    .execute(&state.db)
    .await?;

    // Delete old results
    sqlx::query("DELETE FROM submission_results WHERE submission_id = $1")
        .bind(submission_id)
        .execute(&state.db)
        .await?;

    // Look up file_path for the submission
    let file_path: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM submissions WHERE id = $1")
            .bind(submission_id)
            .fetch_optional(&state.db)
            .await?
            .flatten();

    // Push to compile_queue Redis Stream
    let mut conn = state.redis.get().await?;
    redis::cmd("XADD")
        .arg("compile_queue")
        .arg("*")
        .arg("submission_id")
        .arg(submission_id.to_string())
        .arg("file_path")
        .arg(file_path.unwrap_or_default())
        .arg("priority")
        .arg("1")
        .query_async::<String>(&mut conn)
        .await?;

    tracing::info!(
        admin_id = %admin.id,
        submission_id = %submission_id,
        "Admin requested rejudge"
    );

    Ok(Json(RejudgeResponse {
        submission_id,
        status: "pending".to_string(),
        message: "Submission queued for rejudging".to_string(),
    }))
}

// =============================================================================
// 7.4 Rule Configuration
// =============================================================================

/// Database row for rule_configs
#[derive(Debug, FromRow)]
struct RuleConfigRow {
    id: Uuid,
    name: String,
    service: String,
    description: Option<String>,
    config: serde_json::Value,
    enabled: bool,
    version: String,
    updated_by: Option<Uuid>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

/// GET /api/v1/admin/rules
///
/// List all rule configurations, optionally filtered by service/enabled.
pub async fn list_rules(
    State(state): State<AppState>,
    Query(query): Query<ListRulesQuery>,
) -> ApiResult<Json<RuleConfigListResponse>> {
    let mut conditions = vec!["1=1".to_string()];
    let mut bind_idx = 1u32;

    if query.service.is_some() {
        conditions.push(format!("service = ${}", bind_idx));
        bind_idx += 1;
    }
    if query.enabled.is_some() {
        conditions.push(format!("enabled = ${}", bind_idx));
        // bind_idx += 1;
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, name, service, description, config, enabled, version, updated_by, created_at, updated_at \
         FROM rule_configs WHERE {} ORDER BY service, name",
        where_clause
    );

    let mut q = sqlx::query_as::<_, RuleConfigRow>(&sql);

    if let Some(ref service) = query.service {
        q = q.bind(service.clone());
    }
    if let Some(enabled) = query.enabled {
        q = q.bind(enabled);
    }

    let rows = q.fetch_all(&state.db).await?;

    Ok(Json(RuleConfigListResponse {
        rules: rows.into_iter().map(row_to_response).collect(),
    }))
}

/// POST /api/v1/admin/rules
///
/// Create or update a rule configuration (upsert by name+service).
pub async fn save_rule(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Json(payload): Json<SaveRuleConfigRequest>,
) -> ApiResult<Json<SaveRuleResponse>> {
    payload
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Validate that the config JSON is a valid RuleConfig
    serde_json::from_value::<olympus_rules::config::RuleConfig>(payload.config.clone())
        .map_err(|e| {
            ApiError::Validation(format!(
                "Invalid rule config JSON: {}. Expected format: {{\"type\": \"spec\", \"name\": \"...\", \"params\": {{}}}}",
                e
            ))
        })?;

    let row = sqlx::query_as::<_, UpsertRuleRow>(
        r#"
        INSERT INTO rule_configs (name, service, description, config, enabled, version, updated_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (name, service) DO UPDATE SET
            description = EXCLUDED.description,
            config = EXCLUDED.config,
            enabled = EXCLUDED.enabled,
            version = EXCLUDED.version,
            updated_by = EXCLUDED.updated_by
        RETURNING id, name, service
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.service)
    .bind(&payload.description)
    .bind(&payload.config)
    .bind(payload.enabled)
    .bind(&payload.version)
    .bind(admin.id)
    .fetch_one(&state.db)
    .await?;

    // Notify the target service to reload config via Redis pub/sub
    let mut conn = state.redis.get().await?;
    let _ = redis::cmd("PUBLISH")
        .arg("config_reload")
        .arg(&payload.service)
        .query_async::<i64>(&mut conn)
        .await;

    tracing::info!(
        admin_id = %admin.id,
        rule_name = %payload.name,
        service = %payload.service,
        "Admin saved rule config"
    );

    Ok(Json(SaveRuleResponse {
        id: row.id,
        name: row.name,
        service: row.service,
        success: true,
        message: "Rule configuration saved and reload signal sent".to_string(),
    }))
}

#[derive(Debug, FromRow)]
struct UpsertRuleRow {
    id: Uuid,
    name: String,
    service: String,
}

/// PUT /api/v1/admin/rules/{id}
///
/// Update an existing rule configuration by ID.
pub async fn update_rule(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(rule_id): Path<Uuid>,
    Json(payload): Json<UpdateRuleConfigRequest>,
) -> ApiResult<Json<RuleConfigResponse>> {
    payload
        .validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Validate config JSON if provided
    if let Some(ref config) = payload.config {
        serde_json::from_value::<olympus_rules::config::RuleConfig>(config.clone())
            .map_err(|e| ApiError::Validation(format!("Invalid rule config JSON: {}", e)))?;
    }

    // Build dynamic update query
    let existing = sqlx::query_as::<_, RuleConfigRow>(
        "SELECT id, name, service, description, config, enabled, version, updated_by, created_at, updated_at \
         FROM rule_configs WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Rule configuration not found".to_string()))?;

    let new_description = payload
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let new_config = payload.config.as_ref().unwrap_or(&existing.config);
    let new_enabled = payload.enabled.unwrap_or(existing.enabled);
    let new_version = payload.version.as_deref().unwrap_or(&existing.version);

    let row = sqlx::query_as::<_, RuleConfigRow>(
        r#"
        UPDATE rule_configs
        SET description = $1,
            config = $2,
            enabled = $3,
            version = $4,
            updated_by = $5
        WHERE id = $6
        RETURNING id, name, service, description, config, enabled, version, updated_by, created_at, updated_at
        "#,
    )
    .bind(new_description)
    .bind(new_config)
    .bind(new_enabled)
    .bind(new_version)
    .bind(admin.id)
    .bind(rule_id)
    .fetch_one(&state.db)
    .await?;

    // Notify the target service
    let mut conn = state.redis.get().await?;
    let _ = redis::cmd("PUBLISH")
        .arg("config_reload")
        .arg(&row.service)
        .query_async::<i64>(&mut conn)
        .await;

    tracing::info!(
        admin_id = %admin.id,
        rule_id = %rule_id,
        rule_name = %row.name,
        "Admin updated rule config"
    );

    Ok(Json(row_to_response(row)))
}

/// Helper to convert a DB row to response DTO.
fn row_to_response(row: RuleConfigRow) -> RuleConfigResponse {
    RuleConfigResponse {
        id: row.id,
        name: row.name,
        service: row.service,
        description: row.description,
        config: row.config,
        enabled: row.enabled,
        version: row.version,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

// =============================================================================
// 7.2 Container Management
// =============================================================================

/// GET /api/v1/admin/containers
///
/// List running Docker containers that are part of the system (Sisyphus
/// compilation containers). Uses `docker ps` and `docker stats` under the
/// hood.
pub async fn list_containers(
    State(_state): State<AppState>,
) -> ApiResult<Json<ContainerListResponse>> {
    use std::collections::HashMap;
    use tokio::process::Command;

    // Known compilation images — any running container using one of these is
    // likely a Sisyphus compile container.
    let known_images: &[&str] = &[
        "gcc", "rust", "golang", "python", "zig", "euantorano/zig", "ubuntu",
    ];

    // 1. List running containers as JSON
    let ps_output = Command::new("docker")
        .args([
            "ps",
            "--no-trunc",
            "--format",
            "{{.ID}}\t{{.Image}}\t{{.Status}}\t{{.CreatedAt}}\t{{.State}}",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            ApiError::Internal(format!(
                "Failed to run docker ps — is the Docker socket accessible? {}",
                e
            ))
        })?;

    if !ps_output.status.success() {
        let stderr = String::from_utf8_lossy(&ps_output.stderr);
        return Err(ApiError::Internal(format!(
            "docker ps failed: {}",
            stderr.trim()
        )));
    }

    let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);

    // Parse tab-delimited lines
    let mut containers: Vec<ContainerInfo> = Vec::new();
    for line in ps_stdout.lines() {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 5 {
            continue;
        }
        let (id, image, status, created, state) =
            (parts[0], parts[1], parts[2], parts[3], parts[4]);

        // Filter: only include containers whose image matches a known
        // compilation image prefix.
        let is_sisyphus = known_images
            .iter()
            .any(|prefix| image.starts_with(prefix));
        if !is_sisyphus {
            continue;
        }

        containers.push(ContainerInfo {
            container_id: id.to_string(),
            image: image.to_string(),
            status: status.to_string(),
            created: created.to_string(),
            state: state.to_string(),
            cpu_percent: None,
            memory_usage: None,
            net_io: None,
            pids: None,
        });
    }

    // 2. Get resource usage via `docker stats --no-stream` for matched
    //    containers (skip if none are running).
    if !containers.is_empty() {
        let ids: Vec<String> = containers.iter().map(|c| c.container_id.clone()).collect();
        let mut stats_args = vec![
            "stats".to_string(),
            "--no-stream".to_string(),
            "--format".to_string(),
            "{{.ID}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.PIDs}}".to_string(),
        ];
        stats_args.extend(ids);

        let stats_output = Command::new("docker")
            .args(&stats_args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .ok();

        if let Some(output) = stats_output {
            if output.status.success() {
                let stats_stdout = String::from_utf8_lossy(&output.stdout);
                let mut stats_map: HashMap<String, (String, String, String, String)> =
                    HashMap::new();
                for line in stats_stdout.lines() {
                    let parts: Vec<&str> = line.splitn(5, '\t').collect();
                    if parts.len() >= 5 {
                        stats_map.insert(
                            parts[0].to_string(),
                            (
                                parts[1].to_string(),
                                parts[2].to_string(),
                                parts[3].to_string(),
                                parts[4].to_string(),
                            ),
                        );
                    }
                }
                for c in &mut containers {
                    // docker stats may use short IDs
                    if let Some((cpu, mem, net, pids)) = stats_map.get(&c.container_id) {
                        c.cpu_percent = Some(cpu.clone());
                        c.memory_usage = Some(mem.clone());
                        c.net_io = Some(net.clone());
                        c.pids = Some(pids.clone());
                    } else {
                        // Try matching by prefix (docker stats often uses short IDs)
                        for (sid, (cpu, mem, net, pids)) in &stats_map {
                            if c.container_id.starts_with(sid.as_str())
                                || sid.starts_with(&c.container_id)
                            {
                                c.cpu_percent = Some(cpu.clone());
                                c.memory_usage = Some(mem.clone());
                                c.net_io = Some(net.clone());
                                c.pids = Some(pids.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let total = containers.len();
    Ok(Json(ContainerListResponse { containers, total }))
}

// =============================================================================
// 7.3 Contest Rejudge
// =============================================================================

/// Database row for submissions to rejudge
#[derive(Debug, FromRow)]
struct RejudgeSubmissionRow {
    id: Uuid,
    status: String,
    file_path: Option<String>,
}

/// POST /api/v1/admin/contests/{id}/rejudge
///
/// Rejudge all submissions in a contest. Skips submissions that are currently
/// compiling or judging.
pub async fn rejudge_contest(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Path(contest_id): Path<Uuid>,
) -> ApiResult<Json<ContestRejudgeResponse>> {
    // Verify contest exists
    let contest_exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM contests WHERE id = $1")
            .bind(contest_id)
            .fetch_optional(&state.db)
            .await?;

    if contest_exists.is_none() {
        return Err(ApiError::NotFound("Contest not found".to_string()));
    }

    // Fetch all submissions for this contest
    let submissions: Vec<RejudgeSubmissionRow> = sqlx::query_as(
        "SELECT id, status, file_path FROM submissions WHERE contest_id = $1",
    )
    .bind(contest_id)
    .fetch_all(&state.db)
    .await?;

    let mut rejudged_count = 0usize;
    let mut skipped_count = 0usize;
    let mut rejudge_ids: Vec<Uuid> = Vec::new();

    for sub in &submissions {
        if sub.status == "compiling" || sub.status == "judging" {
            skipped_count += 1;
            continue;
        }
        rejudge_ids.push(sub.id);
    }

    if !rejudge_ids.is_empty() {
        // Batch reset all eligible submissions
        sqlx::query(
            r#"
            UPDATE submissions
            SET status = 'pending',
                score = NULL,
                passed_test_cases = NULL,
                max_time_ms = NULL,
                max_memory_kb = NULL,
                compilation_log = NULL,
                compiled_at = NULL,
                judged_at = NULL
            WHERE id = ANY($1)
            "#,
        )
        .bind(&rejudge_ids)
        .execute(&state.db)
        .await?;

        // Batch delete old results
        sqlx::query("DELETE FROM submission_results WHERE submission_id = ANY($1)")
            .bind(&rejudge_ids)
            .execute(&state.db)
            .await?;

        // Push each submission to compile_queue
        let mut conn = state.redis.get().await?;
        for sub in &submissions {
            if !rejudge_ids.contains(&sub.id) {
                continue;
            }
            redis::cmd("XADD")
                .arg("compile_queue")
                .arg("*")
                .arg("submission_id")
                .arg(sub.id.to_string())
                .arg("file_path")
                .arg(sub.file_path.as_deref().unwrap_or(""))
                .arg("priority")
                .arg("1")
                .query_async::<String>(&mut conn)
                .await?;
            rejudged_count += 1;
        }
    }

    tracing::info!(
        admin_id = %admin.id,
        contest_id = %contest_id,
        rejudged = rejudged_count,
        skipped = skipped_count,
        "Admin requested contest-wide rejudge"
    );

    Ok(Json(ContestRejudgeResponse {
        contest_id,
        rejudged_count,
        skipped_count,
        message: format!(
            "Rejudged {} submissions, skipped {} (in-progress)",
            rejudged_count, skipped_count
        ),
    }))
}
