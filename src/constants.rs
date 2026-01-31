//! Application-wide constants
//!
//! This module contains all constant values used throughout the application.
//! Constants are grouped by their purpose for better organization.

// =============================================================================
// SERVER DEFAULTS
// =============================================================================

/// Default server host address
pub const DEFAULT_SERVER_HOST: &str = "0.0.0.0";

/// Default server port
pub const DEFAULT_SERVER_PORT: u16 = 8080;

// =============================================================================
// DATABASE DEFAULTS
// =============================================================================

/// Default maximum database connections in the pool
pub const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 20;

// =============================================================================
// AUTHENTICATION DEFAULTS
// =============================================================================

/// Default JWT token expiry in hours
pub const DEFAULT_JWT_EXPIRY_HOURS: i64 = 24;

/// Default refresh token expiry in days
pub const DEFAULT_REFRESH_TOKEN_EXPIRY_DAYS: i64 = 7;

/// Minimum password length
pub const MIN_PASSWORD_LENGTH: u64 = 8;

/// Maximum password length
pub const MAX_PASSWORD_LENGTH: u64 = 128;

/// Username minimum length
pub const MIN_USERNAME_LENGTH: u64 = 3;

/// Username maximum length
pub const MAX_USERNAME_LENGTH: u64 = 32;

// =============================================================================
// BENCHMARK DEFAULTS
// =============================================================================

/// Default number of benchmark iterations (including warm-up)
pub const DEFAULT_BENCHMARK_ITERATIONS: u32 = 5;

/// Default time limit in seconds
pub const DEFAULT_TIME_LIMIT_SECONDS: u64 = 2;

/// Default memory limit in megabytes
pub const DEFAULT_MEMORY_LIMIT_MB: u64 = 256;

/// Maximum time limit in seconds (to prevent abuse)
pub const MAX_TIME_LIMIT_SECONDS: u64 = 30;

/// Maximum memory limit in megabytes
pub const MAX_MEMORY_LIMIT_MB: u64 = 1024;

/// CPU limit per container (number of cores)
pub const CPU_LIMIT: f64 = 1.0;

/// Disk limit in megabytes for output
pub const DISK_LIMIT_MB: u64 = 10;

// =============================================================================
// SUPPORTED LANGUAGES
// =============================================================================

/// Language identifiers
pub mod languages {
    pub const C: &str = "c";
    pub const CPP: &str = "cpp";
    pub const RUST: &str = "rust";
    pub const GO: &str = "go";
    pub const ZIG: &str = "zig";
    pub const PYTHON: &str = "python";

    /// All supported language identifiers
    pub const ALL: &[&str] = &[C, CPP, RUST, GO, ZIG, PYTHON];
}

/// Container images for each language
pub mod container_images {
    pub const C: &str = "algojudge/c:latest";
    pub const CPP: &str = "algojudge/cpp:latest";
    pub const RUST: &str = "algojudge/rust:latest";
    pub const GO: &str = "algojudge/go:latest";
    pub const ZIG: &str = "algojudge/zig:latest";
    pub const PYTHON: &str = "algojudge/python:latest";
}

/// File extensions for each language
pub mod file_extensions {
    pub const C: &str = "c";
    pub const CPP: &str = "cpp";
    pub const RUST: &str = "rs";
    pub const GO: &str = "go";
    pub const ZIG: &str = "zig";
    pub const PYTHON: &str = "py";
}

// =============================================================================
// CONTEST SETTINGS
// =============================================================================

/// Contest scoring modes
pub mod scoring_modes {
    pub const ICPC: &str = "icpc";
    pub const CODEFORCES: &str = "codeforces";
    pub const IOI: &str = "ioi";
    pub const PRACTICE: &str = "practice";

    /// All supported scoring modes
    pub const ALL: &[&str] = &[ICPC, CODEFORCES, IOI, PRACTICE];
}

/// Contest visibility options
pub mod visibility {
    pub const PUBLIC: &str = "public";
    pub const PRIVATE: &str = "private";
    pub const HIDDEN: &str = "hidden";
}

/// Registration modes
pub mod registration_modes {
    pub const OPEN: &str = "open";
    pub const CLOSED: &str = "closed";
    pub const INVITE_ONLY: &str = "invite_only";
}

/// Penalty time for wrong submission in ICPC mode (in minutes)
pub const ICPC_PENALTY_MINUTES: i64 = 20;

/// Initial points for a problem in Codeforces mode
pub const CODEFORCES_INITIAL_POINTS: i32 = 500;

/// Points decay rate per minute in Codeforces mode
pub const CODEFORCES_DECAY_PER_MINUTE: i32 = 2;

/// Minimum points in Codeforces mode
pub const CODEFORCES_MIN_POINTS: i32 = 100;

// =============================================================================
// USER ROLES
// =============================================================================

/// User role identifiers
pub mod roles {
    pub const ADMIN: &str = "admin";
    pub const ORGANIZER: &str = "organizer";
    pub const PARTICIPANT: &str = "participant";
    pub const SPECTATOR: &str = "spectator";

    /// All user roles
    pub const ALL: &[&str] = &[ADMIN, ORGANIZER, PARTICIPANT, SPECTATOR];
}

// =============================================================================
// SUBMISSION STATUSES
// =============================================================================

/// Submission verdict statuses
pub mod verdicts {
    pub const PENDING: &str = "pending";
    pub const COMPILING: &str = "compiling";
    pub const RUNNING: &str = "running";
    pub const ACCEPTED: &str = "accepted";
    pub const WRONG_ANSWER: &str = "wrong_answer";
    pub const TIME_LIMIT_EXCEEDED: &str = "time_limit_exceeded";
    pub const MEMORY_LIMIT_EXCEEDED: &str = "memory_limit_exceeded";
    pub const RUNTIME_ERROR: &str = "runtime_error";
    pub const COMPILATION_ERROR: &str = "compilation_error";
    pub const INTERNAL_ERROR: &str = "internal_error";
}

// =============================================================================
// API VERSIONING
// =============================================================================

/// Current API version
pub const API_VERSION: &str = "v1";

/// API base path
pub const API_BASE_PATH: &str = "/api/v1";

// =============================================================================
// RATE LIMITING
// =============================================================================

/// Rate limiting configuration
pub mod rate_limits {
    /// Auth endpoint - max requests
    pub const AUTH_MAX_REQUESTS: i64 = 5;
    /// Auth endpoint - window in seconds
    pub const AUTH_WINDOW_SECS: i64 = 60;
    
    /// Submission endpoint - max requests
    pub const SUBMISSION_MAX_REQUESTS: i64 = 10;
    /// Submission endpoint - window in seconds
    pub const SUBMISSION_WINDOW_SECS: i64 = 60;
    
    /// General API - max requests
    pub const GENERAL_MAX_REQUESTS: i64 = 100;
    /// General API - window in seconds
    pub const GENERAL_WINDOW_SECS: i64 = 60;
}

/// Maximum requests per minute for authenticated users
pub const RATE_LIMIT_AUTHENTICATED: u32 = 100;

/// Maximum requests per minute for unauthenticated users
pub const RATE_LIMIT_UNAUTHENTICATED: u32 = 20;

/// Maximum login attempts per minute
pub const RATE_LIMIT_LOGIN_ATTEMPTS: u32 = 5;

/// Maximum submission attempts per minute
pub const RATE_LIMIT_SUBMISSIONS: u32 = 10;

// =============================================================================
// PAGINATION
// =============================================================================

/// Default page size for paginated results
pub const DEFAULT_PAGE_SIZE: u32 = 20;

/// Maximum page size for paginated results
pub const MAX_PAGE_SIZE: u32 = 100;

// =============================================================================
// VALIDATION
// =============================================================================

/// Maximum problem title length
pub const MAX_PROBLEM_TITLE_LENGTH: u64 = 256;

/// Maximum problem description length
pub const MAX_PROBLEM_DESCRIPTION_LENGTH: u64 = 65535;

/// Maximum contest title length
pub const MAX_CONTEST_TITLE_LENGTH: u64 = 256;

/// Maximum contest description length
pub const MAX_CONTEST_DESCRIPTION_LENGTH: u64 = 65535;

/// Maximum source code size in bytes (1 MB)
pub const MAX_SOURCE_CODE_SIZE: usize = 1024 * 1024;

/// Maximum test case input size in bytes (10 MB)
pub const MAX_TEST_CASE_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// Maximum test case output size in bytes (10 MB)
pub const MAX_TEST_CASE_OUTPUT_SIZE: usize = 10 * 1024 * 1024;
