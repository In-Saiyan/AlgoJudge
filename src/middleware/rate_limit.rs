//! Rate limiting middleware

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::net::SocketAddr;

use crate::{constants, error::AppError, state::AppState};

/// Rate limit middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let ip = addr.ip().to_string();
    let path = request.uri().path().to_string();

    // Determine rate limit based on path
    let (limit, window) = get_rate_limit(&path);

    // Check rate limit
    let key = format!("rate_limit:{}:{}", ip, path_bucket(&path));
    let mut redis = state.redis();

    let count: i64 = redis.incr(&key, 1).await.unwrap_or(0);

    if count == 1 {
        // Set expiry on first request
        let _: () = redis.expire(&key, window).await.unwrap_or(());
    }

    if count > limit {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!("Rate limit exceeded. Try again in {} seconds.", window),
        ));
    }

    Ok(next.run(request).await)
}

/// Get rate limit for a path
fn get_rate_limit(path: &str) -> (i64, i64) {
    if path.starts_with("/api/v1/auth") {
        (
            constants::rate_limits::AUTH_MAX_REQUESTS,
            constants::rate_limits::AUTH_WINDOW_SECS,
        )
    } else if path.starts_with("/api/v1/submissions") {
        (
            constants::rate_limits::SUBMISSION_MAX_REQUESTS,
            constants::rate_limits::SUBMISSION_WINDOW_SECS,
        )
    } else {
        (
            constants::rate_limits::GENERAL_MAX_REQUESTS,
            constants::rate_limits::GENERAL_WINDOW_SECS,
        )
    }
}

/// Get bucket for path (for grouping similar endpoints)
fn path_bucket(path: &str) -> &str {
    if path.starts_with("/api/v1/auth") {
        "auth"
    } else if path.starts_with("/api/v1/submissions") {
        "submissions"
    } else if path.starts_with("/api/v1/contests") {
        "contests"
    } else if path.starts_with("/api/v1/problems") {
        "problems"
    } else {
        "general"
    }
}
