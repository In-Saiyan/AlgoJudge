//! Application state shared across all handlers.

use std::sync::Arc;

use deadpool_redis::Pool as RedisPool;
use sqlx::PgPool;

use crate::config::{Config, RateLimitConfig};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool
    pub db: PgPool,
    /// Redis connection pool
    pub redis: RedisPool,
    /// Application configuration
    pub config: Arc<Config>,
    /// Rate limit configuration
    pub rate_limit_config: Arc<RateLimitConfig>,
}

impl AppState {
    /// Create a new AppState
    pub fn new(
        db: PgPool,
        redis: RedisPool,
        config: Config,
        rate_limit_config: RateLimitConfig,
    ) -> Self {
        Self {
            db,
            redis,
            config: Arc::new(config),
            rate_limit_config: Arc::new(rate_limit_config),
        }
    }
}
