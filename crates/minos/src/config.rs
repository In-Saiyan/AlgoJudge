//! Configuration for Minos Judge Service

use std::env;
use std::path::PathBuf;

/// Minos configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Environment (development, staging, production)
    pub environment: String,

    /// PostgreSQL connection URL
    pub database_url: String,

    /// Redis connection URL
    pub redis_url: String,

    /// Worker ID for consumer group
    pub worker_id: String,

    /// Consumer group name
    pub consumer_group: String,

    /// Stream name for judge jobs
    pub stream_name: String,

    /// Block timeout for XREADGROUP (milliseconds)
    pub block_timeout_ms: usize,

    /// Maximum retries before sending to dead letter queue
    pub max_retries: u32,

    /// Prometheus metrics port
    pub metrics_port: u16,

    /// Storage paths
    pub storage: StorageConfig,

    /// Execution limits
    pub execution: ExecutionConfig,
}

/// Storage path configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Base storage path
    pub base_path: PathBuf,

    /// User binaries directory
    pub binaries_path: PathBuf,

    /// Problem binaries directory (generators, checkers)
    pub problem_binaries_path: PathBuf,

    /// Test cases directory
    pub testcases_path: PathBuf,

    /// Temporary execution directory
    pub temp_path: PathBuf,
}

/// Execution limits configuration
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Default time limit in milliseconds
    pub default_time_limit_ms: u64,

    /// Maximum time limit in milliseconds
    pub max_time_limit_ms: u64,

    /// Default memory limit in KB
    pub default_memory_limit_kb: u64,

    /// Maximum memory limit in KB
    pub max_memory_limit_kb: u64,

    /// Output size limit in bytes
    pub output_limit_bytes: u64,

    /// Generator time limit in milliseconds
    pub generator_time_limit_ms: u64,

    /// Generator memory limit in KB
    pub generator_memory_limit_kb: u64,

    /// Checker time limit in milliseconds
    pub checker_time_limit_ms: u64,

    /// Checker memory limit in KB
    pub checker_memory_limit_kb: u64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let base_path = PathBuf::from(
            env::var("STORAGE_BASE_PATH").unwrap_or_else(|_| "/mnt/data".to_string()),
        );

        Self {
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            worker_id: env::var("WORKER_ID").unwrap_or_else(|_| {
                format!("minos_worker_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())
            }),
            consumer_group: env::var("CONSUMER_GROUP")
                .unwrap_or_else(|_| "minos_group".to_string()),
            stream_name: env::var("STREAM_NAME")
                .unwrap_or_else(|_| "run_queue".to_string()),
            block_timeout_ms: env::var("BLOCK_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            max_retries: env::var("MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            metrics_port: env::var("METRICS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9091),
            storage: StorageConfig {
                binaries_path: base_path.join("binaries/users"),
                problem_binaries_path: base_path.join("binaries/problems"),
                testcases_path: base_path.join("testcases"),
                temp_path: base_path.join("temp"),
                base_path,
            },
            execution: ExecutionConfig {
                default_time_limit_ms: env::var("DEFAULT_TIME_LIMIT_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2000),
                max_time_limit_ms: env::var("MAX_TIME_LIMIT_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30000),
                default_memory_limit_kb: env::var("DEFAULT_MEMORY_LIMIT_KB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(256 * 1024), // 256 MB
                max_memory_limit_kb: env::var("MAX_MEMORY_LIMIT_KB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1024 * 1024), // 1 GB
                output_limit_bytes: env::var("OUTPUT_LIMIT_BYTES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(64 * 1024 * 1024), // 64 MB
                generator_time_limit_ms: env::var("GENERATOR_TIME_LIMIT_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60000), // 60 seconds
                generator_memory_limit_kb: env::var("GENERATOR_MEMORY_LIMIT_KB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(4 * 1024 * 1024), // 4 GB
                checker_time_limit_ms: env::var("CHECKER_TIME_LIMIT_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60000), // 60 seconds
                checker_memory_limit_kb: env::var("CHECKER_MEMORY_LIMIT_KB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(4 * 1024 * 1024), // 4 GB
            },
        }
    }
}
