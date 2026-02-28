//! Application configuration loaded from environment variables.

use std::env;
use std::time::Duration;

use deadpool_redis::{Config as RedisConfig, Pool as RedisPool, Runtime};
use sqlx::postgres::{PgPool, PgPoolOptions};

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Database URL
    pub database_url: String,
    /// Redis URL
    pub redis_url: String,
    /// JWT secret key
    pub jwt_secret: String,
    /// JWT access token expiration in seconds
    pub jwt_access_expiration: i64,
    /// JWT refresh token expiration in seconds
    pub jwt_refresh_expiration: i64,
    /// Environment (development, staging, production)
    pub environment: String,
    /// Maximum threads/cores a problem setter can allocate per problem.
    /// Controlled by the `MAX_THREADS_LIMIT` env var (default: 64).
    pub max_threads_limit: i32,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        // Load .env file if it exists (ignore errors if not found)
        dotenvy::dotenv().ok();

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8081".to_string())
                .parse()
                .expect("PORT must be a number"),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://olympus:olympus_dev@localhost:5432/olympus".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
            jwt_access_expiration: env::var("JWT_ACCESS_EXPIRATION")
                .unwrap_or_else(|_| "900".to_string()) // 15 minutes
                .parse()
                .expect("JWT_ACCESS_EXPIRATION must be a number"),
            jwt_refresh_expiration: env::var("JWT_REFRESH_EXPIRATION")
                .unwrap_or_else(|_| "604800".to_string()) // 7 days
                .parse()
                .expect("JWT_REFRESH_EXPIRATION must be a number"),
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            max_threads_limit: env::var("MAX_THREADS_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(64),
        }
    }

    /// Check if running in production
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Login attempts limit
    pub login_limit: u64,
    /// Login window in seconds
    pub login_window: u64,
    /// Registration limit
    pub register_limit: u64,
    /// Registration window in seconds
    pub register_window: u64,
    /// Submission limit per minute
    pub submission_limit: u64,
    /// Submission window in seconds
    pub submission_window: u64,
    /// General API limit (authenticated)
    pub api_auth_limit: u64,
    /// General API window in seconds
    pub api_auth_window: u64,
    /// General API limit (anonymous)
    pub api_anon_limit: u64,
    /// General API window in seconds
    pub api_anon_window: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            login_limit: 30,
            login_window: 900,       // 15 minutes
            register_limit: 30,
            register_window: 3600,   // 1 hour
            submission_limit: 5,
            submission_window: 60,   // 1 minute
            api_auth_limit: 100,
            api_auth_window: 60,     // 1 minute
            api_anon_limit: 20,
            api_anon_window: 60,     // 1 minute
        }
    }
}

/// Create a PostgreSQL connection pool
pub async fn create_db_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(database_url)
        .await
}

/// Create a Redis connection pool
pub fn create_redis_pool(redis_url: &str) -> Result<RedisPool, deadpool_redis::CreatePoolError> {
    let cfg = RedisConfig::from_url(redis_url);
    cfg.create_pool(Some(Runtime::Tokio1))
}
