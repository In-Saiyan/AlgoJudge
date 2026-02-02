//! Health check handlers.

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::state::AppState;

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub services: ServiceHealth,
}

/// Individual service health status
#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub database: ServiceStatus,
    pub redis: ServiceStatus,
}

/// Service status
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ServiceStatus {
    fn healthy(latency_ms: u64) -> Self {
        Self {
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
        }
    }

    fn unhealthy(error: String) -> Self {
        Self {
            status: "unhealthy",
            latency_ms: None,
            error: Some(error),
        }
    }
}

/// GET /health
/// 
/// Returns the health status of the API and its dependencies.
pub async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, Json<HealthResponse>)> {
    let mut all_healthy = true;

    // Check database health
    let db_status = {
        let start = std::time::Instant::now();
        match sqlx::query("SELECT 1").fetch_one(&state.db).await {
            Ok(_) => ServiceStatus::healthy(start.elapsed().as_millis() as u64),
            Err(e) => {
                all_healthy = false;
                ServiceStatus::unhealthy(e.to_string())
            }
        }
    };

    // Check Redis health
    let redis_status = {
        let start = std::time::Instant::now();
        match state.redis.get().await {
            Ok(mut conn) => {
                match redis::cmd("PING")
                    .query_async::<String>(&mut conn)
                    .await
                {
                    Ok(_) => ServiceStatus::healthy(start.elapsed().as_millis() as u64),
                    Err(e) => {
                        all_healthy = false;
                        ServiceStatus::unhealthy(e.to_string())
                    }
                }
            }
            Err(e) => {
                all_healthy = false;
                ServiceStatus::unhealthy(e.to_string())
            }
        }
    };

    let response = HealthResponse {
        status: if all_healthy { "healthy" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        services: ServiceHealth {
            database: db_status,
            redis: redis_status,
        },
    };

    if all_healthy {
        Ok(Json(response))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(response)))
    }
}

/// GET /health/live
/// 
/// Simple liveness probe - returns 200 if the service is running.
pub async fn liveness() -> StatusCode {
    StatusCode::OK
}

/// GET /health/ready
/// 
/// Readiness probe - returns 200 if the service is ready to accept traffic.
pub async fn readiness(State(state): State<AppState>) -> StatusCode {
    // Check if we can connect to the database
    if sqlx::query("SELECT 1").fetch_one(&state.db).await.is_err() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    // Check if we can connect to Redis
    if let Ok(mut conn) = state.redis.get().await {
        if redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .is_err()
        {
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    } else {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    StatusCode::OK
}
