//! Configuration for Horus Cleaner Service

use std::env;
use std::path::PathBuf;

/// Horus configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Environment (development, staging, production)
    pub environment: String,

    /// PostgreSQL connection URL
    pub database_url: String,

    /// Redis connection URL
    pub redis_url: String,

    /// Storage paths
    pub storage: StorageConfig,

    /// Cleanup schedules
    pub schedules: ScheduleConfig,
}

/// Storage path configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Base storage path
    pub base_path: PathBuf,

    /// Submissions directory
    pub submissions_path: PathBuf,

    /// User binaries directory
    pub binaries_path: PathBuf,

    /// Problem binaries directory
    pub problem_binaries_path: PathBuf,

    /// Test cases directory
    pub testcases_path: PathBuf,

    /// Temporary execution directory
    pub temp_path: PathBuf,
}

/// Cron schedule configuration
#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    /// Cron expression for testcase cleanup (default: every hour)
    pub testcase_cleanup: String,

    /// Cron expression for temp directory cleanup (default: every 15 min)
    pub temp_cleanup: String,

    /// Cron expression for orphan binary cleanup (default: daily at 3am)
    pub binary_cleanup: String,

    /// Cron expression for old submission cleanup (default: weekly)
    pub submission_cleanup: String,

    /// Hours after which testcases are considered stale
    pub testcase_stale_hours: u64,

    /// Hours after which temp directories are considered orphaned
    pub temp_orphan_hours: u64,

    /// Days after which submissions can be cleaned (0 = disabled)
    pub submission_retention_days: u64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let base_path = PathBuf::from(
            env::var("STORAGE_BASE_PATH").unwrap_or_else(|_| "/mnt/data".to_string()),
        );

        Self {
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            storage: StorageConfig {
                submissions_path: base_path.join("submissions"),
                binaries_path: base_path.join("binaries/users"),
                problem_binaries_path: base_path.join("binaries/problems"),
                testcases_path: base_path.join("testcases"),
                temp_path: base_path.join("temp"),
                base_path,
            },
            schedules: ScheduleConfig {
                testcase_cleanup: env::var("TESTCASE_CLEANUP_CRON")
                    .unwrap_or_else(|_| "0 0 * * * *".to_string()), // Every hour
                temp_cleanup: env::var("TEMP_CLEANUP_CRON")
                    .unwrap_or_else(|_| "0 */15 * * * *".to_string()), // Every 15 min
                binary_cleanup: env::var("BINARY_CLEANUP_CRON")
                    .unwrap_or_else(|_| "0 0 3 * * *".to_string()), // Daily at 3am
                submission_cleanup: env::var("SUBMISSION_CLEANUP_CRON")
                    .unwrap_or_else(|_| "0 0 4 * * 0".to_string()), // Weekly Sunday 4am
                testcase_stale_hours: env::var("TESTCASE_STALE_HOURS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(6),
                temp_orphan_hours: env::var("TEMP_ORPHAN_HOURS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1),
                submission_retention_days: env::var("SUBMISSION_RETENTION_DAYS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0), // Disabled by default
            },
        }
    }
}
