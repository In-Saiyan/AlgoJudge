//! Authentication handler implementations

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use validator::Validate;

use crate::{
    error::{AppError, AppResult},
    middleware::auth::AuthenticatedUser,
    services::AuthService,
    state::AppState,
};

use super::{
    request::{LoginRequest, LogoutRequest, RefreshTokenRequest, RegisterRequest},
    response::{AuthResponse, CurrentUserResponse, LogoutResponse, RefreshResponse, RegisterResponse, UserResponse},
};

/// Register a new user
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<RegisterResponse>)> {
    // Validate request
    payload.validate()?;

    // Register user through service
    let user = AuthService::register(
        state.db(),
        &payload.username,
        &payload.email,
        &payload.password,
        payload.display_name.as_deref(),
    )
    .await?;

    let response = RegisterResponse {
        message: "User registered successfully".to_string(),
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            created_at: user.created_at,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Login with username/email and password
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    // Validate request
    payload.validate()?;

    // Authenticate user
    let (user, access_token, refresh_token, expires_in) = AuthService::login(
        state.db(),
        state.redis(),
        state.config(),
        &payload.identifier,
        &payload.password,
    )
    .await?;

    let response = AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            created_at: user.created_at,
        },
    };

    Ok(Json(response))
}

/// Refresh access token
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> AppResult<Json<RefreshResponse>> {
    let (access_token, refresh_token, expires_in) = AuthService::refresh_token(
        state.db(),
        state.redis(),
        state.config(),
        &payload.refresh_token,
    )
    .await?;

    let response = RefreshResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in,
    };

    Ok(Json(response))
}

/// Logout (invalidate tokens)
pub async fn logout(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<Option<LogoutRequest>>,
) -> AppResult<Json<LogoutResponse>> {
    let all_sessions = payload.and_then(|p| p.all_sessions).unwrap_or(false);

    AuthService::logout(state.redis(), &auth_user.id, all_sessions).await?;

    Ok(Json(LogoutResponse {
        message: "Logged out successfully".to_string(),
    }))
}

/// Get current authenticated user
pub async fn get_current_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<CurrentUserResponse>> {
    let user = AuthService::get_user_by_id(state.db(), &auth_user.id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(CurrentUserResponse {
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            created_at: user.created_at,
        },
    }))
}
