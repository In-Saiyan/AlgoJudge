//! Configuration for Sisyphus compiler service.

use std::env;

/// Per-language Docker image overrides.
///
/// Each field can be set via the corresponding env var
/// (e.g. `CONTAINER_IMAGE_CPP=gcc:14`).  When unset the
/// container module falls back to sensible defaults.
#[derive(Debug, Clone, Default)]
pub struct ContainerImages {
    pub cpp: Option<String>,
    pub c: Option<String>,
    pub rust: Option<String>,
    pub go: Option<String>,
    pub python: Option<String>,
    pub zig: Option<String>,
    /// Fallback image when the language is unknown or not specified.
    pub generic: Option<String>,
}

/// Sisyphus configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Environment (development, production)
    pub environment: String,
    /// Database connection URL
    pub database_url: String,
    /// Redis connection URL
    pub redis_url: String,
    /// Consumer group name
    pub consumer_group: String,
    /// Consumer name (unique per instance)
    pub consumer_name: String,
    /// Stream name for compile jobs
    pub compile_stream: String,
    /// Stream name for run jobs (output)
    pub run_stream: String,
    /// Compilation timeout in seconds
    pub compile_timeout_secs: u64,
    /// Base path for submission storage
    pub submissions_path: String,
    /// Base path for compiled binaries
    pub binaries_path: String,
    /// Enable network during compilation (for package downloads)
    pub network_enabled: bool,
    /// Maximum memory for compilation (in bytes)
    pub max_memory_bytes: u64,
    /// Maximum CPU cores for compilation
    pub max_cpu_cores: u32,
    /// Per-language Docker image overrides
    pub container_images: ContainerImages,
    /// Docker API version to negotiate with the daemon.
    /// Set this when the client binary is older than the daemon's
    /// minimum supported API version (e.g. "1.44").
    /// When `None` the client uses its built-in default.
    pub docker_api_version: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://olympus:olympus@localhost:5432/olympus".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            consumer_group: env::var("CONSUMER_GROUP")
                .unwrap_or_else(|_| "sisyphus_group".to_string()),
            consumer_name: env::var("CONSUMER_NAME")
                .unwrap_or_else(|_| format!("sisyphus_{}", uuid::Uuid::new_v4())),
            compile_stream: env::var("COMPILE_STREAM")
                .unwrap_or_else(|_| "compile_queue".to_string()),
            run_stream: env::var("RUN_STREAM")
                .unwrap_or_else(|_| "run_queue".to_string()),
            compile_timeout_secs: env::var("COMPILE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            submissions_path: env::var("SUBMISSIONS_PATH")
                .unwrap_or_else(|_| "/mnt/data/submissions".to_string()),
            binaries_path: env::var("BINARIES_PATH")
                .unwrap_or_else(|_| "/mnt/data/binaries/users".to_string()),
            network_enabled: env::var("NETWORK_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            max_memory_bytes: env::var("MAX_MEMORY_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2 * 1024 * 1024 * 1024), // 2GB
            max_cpu_cores: env::var("MAX_CPU_CORES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            container_images: ContainerImages {
                cpp: env::var("CONTAINER_IMAGE_CPP").ok(),
                c: env::var("CONTAINER_IMAGE_C").ok(),
                rust: env::var("CONTAINER_IMAGE_RUST").ok(),
                go: env::var("CONTAINER_IMAGE_GO").ok(),
                python: env::var("CONTAINER_IMAGE_PYTHON").ok(),
                zig: env::var("CONTAINER_IMAGE_ZIG").ok(),
                generic: env::var("CONTAINER_IMAGE_GENERIC").ok(),
            },
            docker_api_version: env::var("DOCKER_API_VERSION").ok(),
        }
    }
}
