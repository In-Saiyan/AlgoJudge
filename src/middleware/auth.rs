//! Authentication middleware

use axum::{
    body::Body,
    extract::{FromRequestParts, Request, State},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::AppError, services::AuthService, state::AppState};

/// Authenticated user extracted from JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub username: String,
    pub role: String,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}

/// Optional authenticated user wrapper (never fails)
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuth(parts.extensions.get::<AuthenticatedUser>().cloned()))
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(AppError::Unauthorized);
    }

    let token = &auth_header[7..];

    let claims = AuthService::verify_token(token, &state.config().jwt.secret)?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::InvalidToken)?;

    let user = AuthenticatedUser {
        id: user_id,
        username: claims.username,
        role: claims.role,
    };

    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

/// Optional authentication middleware (doesn't fail if no token)
pub async fn optional_auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    if let Some(auth_header) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];

            if let Ok(claims) = AuthService::verify_token(token, &state.config().jwt.secret) {
                if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                    let user = AuthenticatedUser {
                        id: user_id,
                        username: claims.username,
                        role: claims.role,
                    };
                    request.extensions_mut().insert(user);
                }
            }
        }
    }

    next.run(request).await
}

/// Require specific role middleware
pub fn require_role(allowed_roles: &'static [&'static str]) -> impl Fn(Extension<AuthenticatedUser>) -> Result<(), AppError> + Clone {
    move |Extension(user): Extension<AuthenticatedUser>| {
        if allowed_roles.contains(&user.role.as_str()) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Insufficient permissions".to_string()))
        }
    }
}
