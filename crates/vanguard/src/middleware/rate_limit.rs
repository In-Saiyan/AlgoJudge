//! Rate limiting middleware using Redis.

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::net::SocketAddr;

use crate::error::{ApiError, ApiErrorBody, ApiErrorResponse};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;

/// Rate limit information
#[derive(Debug)]
pub struct RateLimitInfo {
    pub limit: u64,
    pub remaining: u64,
    pub reset: u64,
    pub allowed: bool,
}

/// Rate limit tier for different operations
#[derive(Debug, Clone, Copy)]
pub enum RateLimitTier {
    Login,
    Register,
    Submission,
    ApiAuth,
    ApiAnon,
}

impl RateLimitTier {
    fn key_prefix(&self) -> &'static str {
        match self {
            RateLimitTier::Login => "rl:login",
            RateLimitTier::Register => "rl:register",
            RateLimitTier::Submission => "rl:submit",
            RateLimitTier::ApiAuth => "rl:api",
            RateLimitTier::ApiAnon => "rl:api",
        }
    }
}

/// Check rate limit using Redis INCR + EXPIRE pattern
pub async fn check_rate_limit(
    state: &AppState,
    key: &str,
    limit: u64,
    window_secs: u64,
) -> Result<RateLimitInfo, ApiError> {
    let mut conn = state.redis.get().await?;

    // Increment counter
    let count: u64 = redis::cmd("INCR")
        .arg(key)
        .query_async(&mut conn)
        .await?;

    // Set expiry on first request
    if count == 1 {
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(window_secs as i64)
            .query_async::<()>(&mut conn)
            .await?;
    }

    // Get TTL for reset time
    let ttl: i64 = redis::cmd("TTL")
        .arg(key)
        .query_async(&mut conn)
        .await?;

    Ok(RateLimitInfo {
        limit,
        remaining: limit.saturating_sub(count),
        reset: ttl.max(0) as u64,
        allowed: count <= limit,
    })
}

/// Add rate limit headers to response
fn add_rate_limit_headers(response: &mut Response, info: &RateLimitInfo) {
    let headers = response.headers_mut();
    headers.insert(
        "X-RateLimit-Limit",
        info.limit.to_string().parse().unwrap(),
    );
    headers.insert(
        "X-RateLimit-Remaining",
        info.remaining.to_string().parse().unwrap(),
    );
    headers.insert(
        "X-RateLimit-Reset",
        info.reset.to_string().parse().unwrap(),
    );
}

/// Create rate limit exceeded response
fn rate_limit_response(info: &RateLimitInfo) -> Response {
    let body = ApiErrorResponse {
        error: ApiErrorBody {
            code: "RATE_LIMIT_EXCEEDED",
            message: format!(
                "Rate limit exceeded. Try again in {} seconds.",
                info.reset
            ),
            details: None,
        },
    };

    let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    add_rate_limit_headers(&mut response, info);
    response
        .headers_mut()
        .insert("Retry-After", info.reset.to_string().parse().unwrap());
    response
}

/// Extract client identifier (user_id or IP)
fn get_client_key(request: &Request, tier: RateLimitTier) -> String {
    let prefix = tier.key_prefix();

    // Try to get user ID from extensions (for authenticated requests)
    if let Some(user) = request.extensions().get::<AuthUser>() {
        return format!("{}:{}", prefix, user.id);
    }

    // Fall back to IP address
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .or_else(|| {
            request
                .headers()
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    format!("{}:{}", prefix, ip)
}

/// Rate limiting middleware for general API requests.
pub async fn api_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let is_authenticated = request.extensions().get::<AuthUser>().is_some();
    let tier = if is_authenticated {
        RateLimitTier::ApiAuth
    } else {
        RateLimitTier::ApiAnon
    };

    let (limit, window) = if is_authenticated {
        (
            state.rate_limit_config.api_auth_limit,
            state.rate_limit_config.api_auth_window,
        )
    } else {
        (
            state.rate_limit_config.api_anon_limit,
            state.rate_limit_config.api_anon_window,
        )
    };

    let key = get_client_key(&request, tier);

    match check_rate_limit(&state, &key, limit, window).await {
        Ok(info) => {
            if !info.allowed {
                return rate_limit_response(&info);
            }

            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &info);
            response
        }
        Err(e) => {
            tracing::error!("Rate limit check failed: {:?}", e);
            // On Redis error, allow the request through
            next.run(request).await
        }
    }
}

/// Rate limiting middleware specifically for login attempts.
pub async fn login_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = get_client_key(&request, RateLimitTier::Login);

    match check_rate_limit(
        &state,
        &key,
        state.rate_limit_config.login_limit,
        state.rate_limit_config.login_window,
    )
    .await
    {
        Ok(info) => {
            if !info.allowed {
                return rate_limit_response(&info);
            }

            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &info);
            response
        }
        Err(e) => {
            tracing::error!("Rate limit check failed: {:?}", e);
            next.run(request).await
        }
    }
}

/// Rate limiting middleware for registration.
pub async fn register_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = get_client_key(&request, RateLimitTier::Register);

    match check_rate_limit(
        &state,
        &key,
        state.rate_limit_config.register_limit,
        state.rate_limit_config.register_window,
    )
    .await
    {
        Ok(info) => {
            if !info.allowed {
                return rate_limit_response(&info);
            }

            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &info);
            response
        }
        Err(e) => {
            tracing::error!("Rate limit check failed: {:?}", e);
            next.run(request).await
        }
    }
}

/// Rate limiting middleware for submissions.
pub async fn submission_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let key = get_client_key(&request, RateLimitTier::Submission);

    match check_rate_limit(
        &state,
        &key,
        state.rate_limit_config.submission_limit,
        state.rate_limit_config.submission_window,
    )
    .await
    {
        Ok(info) => {
            if !info.allowed {
                return rate_limit_response(&info);
            }

            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &info);
            response
        }
        Err(e) => {
            tracing::error!("Rate limit check failed: {:?}", e);
            next.run(request).await
        }
    }
}
