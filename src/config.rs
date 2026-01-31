//! Application configuration management
//!
//! This module handles loading and validating configuration from environment variables.
//! All configuration is loaded at startup and validated before the application runs.

use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::constants::{
    DEFAULT_BENCHMARK_ITERATIONS, DEFAULT_DATABASE_MAX_CONNECTIONS, DEFAULT_JWT_EXPIRY_HOURS,
    DEFAULT_MEMORY_LIMIT_MB, DEFAULT_REFRESH_TOKEN_EXPIRY_DAYS, DEFAULT_SERVER_HOST,
    DEFAULT_SERVER_PORT, DEFAULT_TIME_LIMIT_SECONDS,
};

/// Global application configuration (lazily initialized)
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    Config::from_env().expect("Failed to load configuration from environment")
});

/// Main application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub docker: DockerConfig,
    pub storage: StorageConfig,
    pub benchmark: BenchmarkConfig,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub rust_log: String,
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

/// Redis configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
}

/// JWT authentication configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry_hours: i64,
    pub refresh_token_expiry_days: i64,
}

/// Docker configuration for benchmark containers
#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub socket_path: String,
    pub network_name: String,
}

/// File storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub submissions_path: PathBuf,
    pub test_cases_path: PathBuf,
}

/// Benchmark execution configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations per test case (first is warm-up)
    pub iterations: u32,
    /// Default time limit in seconds
    pub default_time_limit_seconds: u64,
    /// Default memory limit in megabytes
    pub default_memory_limit_mb: u64,
    /// CPU limit (number of cores)
    pub cpu_limit: f64,
    /// Disk limit in megabytes
    pub disk_limit_mb: u64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Ok(Self {
            server: ServerConfig::from_env()?,
            database: DatabaseConfig::from_env()?,
            redis: RedisConfig::from_env()?,
            jwt: JwtConfig::from_env()?,
            docker: DockerConfig::from_env()?,
            storage: StorageConfig::from_env()?,
            benchmark: BenchmarkConfig::from_env()?,
        })
    }
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            host: env::var("SERVER_HOST").unwrap_or_else(|_| DEFAULT_SERVER_HOST.to_string()),
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| DEFAULT_SERVER_PORT.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("SERVER_PORT".to_string()))?,
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            url: env::var("DATABASE_URL").map_err(|_| ConfigError::Missing("DATABASE_URL".to_string()))?,
            max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| DEFAULT_DATABASE_MAX_CONNECTIONS.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("DATABASE_MAX_CONNECTIONS".to_string()))?,
        })
    }
}

impl RedisConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        })
    }
}

impl JwtConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            secret: env::var("JWT_SECRET").map_err(|_| ConfigError::Missing("JWT_SECRET".to_string()))?,
            expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| DEFAULT_JWT_EXPIRY_HOURS.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("JWT_EXPIRY_HOURS".to_string()))?,
            refresh_token_expiry_days: env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                .unwrap_or_else(|_| DEFAULT_REFRESH_TOKEN_EXPIRY_DAYS.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("REFRESH_TOKEN_EXPIRY_DAYS".to_string()))?,
        })
    }
}

impl DockerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            socket_path: env::var("DOCKER_SOCKET")
                .unwrap_or_else(|_| "/var/run/docker.sock".to_string()),
            network_name: env::var("BENCHMARK_NETWORK")
                .unwrap_or_else(|_| "algojudge-benchmark".to_string()),
        })
    }
}

impl StorageConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            submissions_path: PathBuf::from(
                env::var("SUBMISSIONS_PATH").unwrap_or_else(|_| "/data/submissions".to_string()),
            ),
            test_cases_path: PathBuf::from(
                env::var("TEST_CASES_PATH").unwrap_or_else(|_| "/data/test_cases".to_string()),
            ),
        })
    }
}

impl BenchmarkConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            iterations: env::var("BENCHMARK_ITERATIONS")
                .unwrap_or_else(|_| DEFAULT_BENCHMARK_ITERATIONS.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("BENCHMARK_ITERATIONS".to_string()))?,
            default_time_limit_seconds: env::var("DEFAULT_TIME_LIMIT_SECONDS")
                .unwrap_or_else(|_| DEFAULT_TIME_LIMIT_SECONDS.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("DEFAULT_TIME_LIMIT_SECONDS".to_string()))?,
            default_memory_limit_mb: env::var("DEFAULT_MEMORY_LIMIT_MB")
                .unwrap_or_else(|_| DEFAULT_MEMORY_LIMIT_MB.to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("DEFAULT_MEMORY_LIMIT_MB".to_string()))?,
            cpu_limit: 1.0,
            disk_limit_mb: 10,
        })
    }
}

/// Configuration loading errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    Missing(String),

    #[error("Invalid value for environment variable: {0}")]
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        // Test that defaults are applied when env vars are not set
        let server = ServerConfig {
            host: DEFAULT_SERVER_HOST.to_string(),
            port: DEFAULT_SERVER_PORT,
            rust_log: "info".to_string(),
        };
        assert_eq!(server.host, "0.0.0.0");
        assert_eq!(server.port, 8080);
    }
}
