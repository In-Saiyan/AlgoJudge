//! Authentication handlers.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, Row};
use uuid::Uuid;
use validator::Validate;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::middleware::auth::AuthUser;
use super::{
    request::{LoginRequest, RefreshRequest, RegisterRequest},
    response::{AuthTokensResponse, LoginResponse, LogoutResponse, RegisterResponse, UserResponse},
    jwt::JwtManager,
};

/// User row from database
#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    display_name: Option<String>,
    bio: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// POST /api/v1/auth/register
/// 
/// Register a new user account.
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> ApiResult<(StatusCode, Json<RegisterResponse>)> {
    // Validate request
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Check if username exists
    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)"
    )
    .bind(&payload.username)
    .fetch_one(&state.db)
    .await?;

    if exists.0 {
        return Err(ApiError::Conflict("Username already exists".to_string()));
    }

    // Check if email exists
    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&payload.email)
    .fetch_one(&state.db)
    .await?;

    if exists.0 {
        return Err(ApiError::Conflict("Email already registered".to_string()));
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| ApiError::Internal(format!("Password hashing failed: {}", e)))?
        .to_string();

    // Create user
    let user_id = Uuid::new_v4();
    let now = Utc::now();
    let display_name = payload.display_name.clone().unwrap_or_else(|| payload.username.clone());

    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, display_name, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, 'participant', $6, $6)
        "#
    )
    .bind(user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&display_name)
    .bind(now)
    .execute(&state.db)
    .await?;

    // Generate tokens
    let jwt_manager = JwtManager::new(
        &state.config.jwt_secret,
        state.config.jwt_access_expiration,
        state.config.jwt_refresh_expiration,
    );

    let session_id = Uuid::new_v4();
    let access_token = jwt_manager.generate_access_token(user_id, &payload.username, "participant")?;
    let refresh_token = jwt_manager.generate_refresh_token(user_id, session_id)?;

    // Store refresh token session in Redis
    let mut conn = state.redis.get().await?;
    let session_key = format!("session:{}:{}", user_id, session_id);
    redis::cmd("SET")
        .arg(&session_key)
        .arg("active")
        .arg("EX")
        .arg(state.config.jwt_refresh_expiration)
        .query_async::<()>(&mut conn)
        .await?;

    let response = RegisterResponse {
        user: UserResponse {
            id: user_id,
            username: payload.username,
            email: payload.email,
            display_name: Some(display_name),
            bio: None,
            role: "participant".to_string(),
            created_at: now,
            updated_at: now,
        },
        tokens: AuthTokensResponse::new(
            access_token,
            refresh_token,
            state.config.jwt_access_expiration,
        ),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// POST /api/v1/auth/login
/// 
/// Login with username/email and password.
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    // Validate request
    payload.validate().map_err(|e| ApiError::Validation(e.to_string()))?;

    // Find user by username or email
    let user: UserRow = sqlx::query_as(
        r#"
        SELECT id, username, email, password_hash, display_name, bio, role, created_at, updated_at
        FROM users
        WHERE username = $1 OR email = $1
        "#
    )
    .bind(&payload.login)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::InvalidCredentials)?;

    // Verify password
    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|_| ApiError::Internal("Invalid password hash".to_string()))?;

    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::InvalidCredentials)?;

    // Generate tokens
    let jwt_manager = JwtManager::new(
        &state.config.jwt_secret,
        state.config.jwt_access_expiration,
        state.config.jwt_refresh_expiration,
    );

    let session_id = Uuid::new_v4();
    let access_token = jwt_manager.generate_access_token(user.id, &user.username, &user.role)?;
    let refresh_token = jwt_manager.generate_refresh_token(user.id, session_id)?;

    // Store refresh token session in Redis
    let mut conn = state.redis.get().await?;
    let session_key = format!("session:{}:{}", user.id, session_id);
    redis::cmd("SET")
        .arg(&session_key)
        .arg("active")
        .arg("EX")
        .arg(state.config.jwt_refresh_expiration)
        .query_async::<()>(&mut conn)
        .await?;

    let response = LoginResponse {
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            bio: user.bio,
            role: user.role,
            created_at: user.created_at,
            updated_at: user.updated_at,
        },
        tokens: AuthTokensResponse::new(
            access_token,
            refresh_token,
            state.config.jwt_access_expiration,
        ),
    };

    Ok(Json(response))
}

/// POST /api/v1/auth/refresh
/// 
/// Refresh access token using refresh token.
pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> ApiResult<Json<AuthTokensResponse>> {
    let jwt_manager = JwtManager::new(
        &state.config.jwt_secret,
        state.config.jwt_access_expiration,
        state.config.jwt_refresh_expiration,
    );

    // Verify refresh token
    let claims = jwt_manager.verify_refresh_token(&payload.refresh_token)?;

    // Check if session is valid in Redis
    let mut conn = state.redis.get().await?;
    let session_key = format!("session:{}:{}", claims.sub, claims.session_id);
    let exists: Option<String> = redis::cmd("GET")
        .arg(&session_key)
        .query_async(&mut conn)
        .await?;

    if exists.is_none() {
        return Err(ApiError::Token("Session has been revoked".to_string()));
    }

    // Fetch user info
    let row = sqlx::query("SELECT username, role FROM users WHERE id = $1")
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound("User not found".to_string()))?;

    let username: String = row.get("username");
    let role: String = row.get("role");

    // Generate new tokens
    let new_session_id = Uuid::new_v4();
    let access_token = jwt_manager.generate_access_token(claims.sub, &username, &role)?;
    let refresh_token = jwt_manager.generate_refresh_token(claims.sub, new_session_id)?;

    // Invalidate old session and create new one
    redis::cmd("DEL")
        .arg(&session_key)
        .query_async::<()>(&mut conn)
        .await?;

    let new_session_key = format!("session:{}:{}", claims.sub, new_session_id);
    redis::cmd("SET")
        .arg(&new_session_key)
        .arg("active")
        .arg("EX")
        .arg(state.config.jwt_refresh_expiration)
        .query_async::<()>(&mut conn)
        .await?;

    Ok(Json(AuthTokensResponse::new(
        access_token,
        refresh_token,
        state.config.jwt_access_expiration,
    )))
}

/// POST /api/v1/auth/logout
/// 
/// Logout and invalidate the current session.
pub async fn logout(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> ApiResult<Json<LogoutResponse>> {
    // Invalidate all sessions for the user
    let mut conn = state.redis.get().await?;

    // Delete all sessions for this user (simplified - in production you'd track individual sessions)
    let pattern = format!("session:{}:*", user.id);
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(&pattern)
        .query_async(&mut conn)
        .await?;

    if !keys.is_empty() {
        redis::cmd("DEL")
            .arg(&keys)
            .query_async::<()>(&mut conn)
            .await?;
    }

    Ok(Json(LogoutResponse::default()))
}

/// GET /api/v1/auth/me
/// 
/// Get the current authenticated user's profile.
pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> ApiResult<Json<UserResponse>> {
    let user_data: UserRow = sqlx::query_as(
        r#"
        SELECT id, username, email, password_hash, display_name, bio, role, created_at, updated_at
        FROM users
        WHERE id = $1
        "#
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound("User not found".to_string()))?;

    Ok(Json(UserResponse {
        id: user_data.id,
        username: user_data.username,
        email: user_data.email,
        display_name: user_data.display_name,
        bio: user_data.bio,
        role: user_data.role,
        created_at: user_data.created_at,
        updated_at: user_data.updated_at,
    }))
}
