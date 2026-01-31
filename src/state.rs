//! Application state management
//!
//! This module contains the shared application state that is passed
//! to all request handlers via Axum's State extractor.

use std::sync::Arc;

use bollard::Docker;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::config::Config;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

/// Inner state (wrapped in Arc for cheap cloning)
struct AppStateInner {
    /// Database connection pool
    pub db: PgPool,

    /// Redis connection manager
    pub redis: ConnectionManager,

    /// Docker client for benchmark containers
    pub docker: Docker,

    /// Application configuration
    pub config: Config,
}

impl AppState {
    /// Create a new application state
    pub fn new(db: PgPool, redis: ConnectionManager, docker: Docker, config: Config) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                db,
                redis,
                docker,
                config,
            }),
        }
    }

    /// Get a reference to the database pool
    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }

    /// Get a clone of the Redis connection manager
    pub fn redis(&self) -> ConnectionManager {
        self.inner.redis.clone()
    }

    /// Get a reference to the Docker client
    pub fn docker(&self) -> &Docker {
        &self.inner.docker
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &Config {
        &self.inner.config
    }
}
