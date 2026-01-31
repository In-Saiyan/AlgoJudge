//! Logging middleware

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn};

/// Request logging middleware
pub async fn logging_middleware(request: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    let duration_ms = duration.as_secs_f64() * 1000.0;

    if status.is_server_error() {
        warn!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %format!("{:.2}", duration_ms),
            "Request completed with server error"
        );
    } else if status.is_client_error() && status != StatusCode::NOT_FOUND {
        warn!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %format!("{:.2}", duration_ms),
            "Request completed with client error"
        );
    } else {
        info!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            duration_ms = %format!("{:.2}", duration_ms),
            "Request completed"
        );
    }

    response
}
