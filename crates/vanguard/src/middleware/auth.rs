//! Authentication middleware.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::domain::auth::JwtManager;
use crate::error::ApiError;
use crate::state::AppState;

/// Authenticated user information extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub role: String,
}

/// Authentication middleware.
/// 
/// Extracts and validates JWT token from Authorization header.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    // Extract bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::Unauthorized)?;

    // Verify token
    let jwt_manager = JwtManager::new(
        &state.config.jwt_secret,
        state.config.jwt_access_expiration,
        state.config.jwt_refresh_expiration,
    );

    let claims = jwt_manager.verify_access_token(token)?;

    // Add user info to request extensions
    let auth_user = AuthUser {
        id: claims.sub,
        username: claims.username,
        role: claims.role,
    };
    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}

/// Optional authentication middleware.
/// 
/// Extracts JWT if present but doesn't fail if missing.
#[allow(dead_code)]
pub async fn optional_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let jwt_manager = JwtManager::new(
                &state.config.jwt_secret,
                state.config.jwt_access_expiration,
                state.config.jwt_refresh_expiration,
            );

            if let Ok(claims) = jwt_manager.verify_access_token(token) {
                let auth_user = AuthUser {
                    id: claims.sub,
                    username: claims.username,
                    role: claims.role,
                };
                request.extensions_mut().insert(auth_user);
            }
        }
    }

    next.run(request).await
}

/// Admin-only middleware.
/// 
/// Requires the user to have admin role.
#[allow(dead_code)]
pub async fn admin_middleware(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or(ApiError::Unauthorized)?;

    if auth_user.role != "admin" {
        return Err(ApiError::Forbidden);
    }

    Ok(next.run(request).await)
}

/// Organizer or Admin middleware.
/// 
/// Requires the user to have admin or organizer role.
#[allow(dead_code)]
pub async fn organizer_middleware(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or(ApiError::Unauthorized)?;

    if auth_user.role != "admin" && auth_user.role != "organizer" {
        return Err(ApiError::Forbidden);
    }

    Ok(next.run(request).await)
}
