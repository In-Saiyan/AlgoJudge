//! Example rule implementations.
//!
//! These demonstrate how to create specifications for various contexts.

use crate::context::{ExecutionContext, FileContext};
use crate::specification::Specification;
use async_trait::async_trait;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// File-based rules for Horus (Cleaner)
// =============================================================================

/// Check if the file/directory was last accessed more than N hours ago.
pub struct LastAccessOlderThan {
    pub hours: u64,
}

impl LastAccessOlderThan {
    pub fn new(hours: u64) -> Self {
        Self { hours }
    }
}

#[async_trait]
impl Specification<FileContext> for LastAccessOlderThan {
    async fn is_satisfied_by(&self, ctx: &FileContext) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let age_hours = (now - ctx.accessed_at) / 3600;
        age_hours > self.hours as i64
    }
}

/// Check if the file/directory was created more than N hours ago.
pub struct CreatedOlderThan {
    pub hours: u64,
}

impl CreatedOlderThan {
    pub fn new(hours: u64) -> Self {
        Self { hours }
    }
}

#[async_trait]
impl Specification<FileContext> for CreatedOlderThan {
    async fn is_satisfied_by(&self, ctx: &FileContext) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let age_hours = (now - ctx.created_at) / 3600;
        age_hours > self.hours as i64
    }
}

/// Check if the path is a file.
pub struct IsFile;

#[async_trait]
impl Specification<FileContext> for IsFile {
    async fn is_satisfied_by(&self, ctx: &FileContext) -> bool {
        ctx.is_file
    }
}

/// Check if the path is a directory.
pub struct IsDirectory;

#[async_trait]
impl Specification<FileContext> for IsDirectory {
    async fn is_satisfied_by(&self, ctx: &FileContext) -> bool {
        ctx.is_directory
    }
}

/// Check if file size is larger than N bytes.
pub struct SizeLargerThan {
    pub bytes: u64,
}

impl SizeLargerThan {
    pub fn new(bytes: u64) -> Self {
        Self { bytes }
    }
}

#[async_trait]
impl Specification<FileContext> for SizeLargerThan {
    async fn is_satisfied_by(&self, ctx: &FileContext) -> bool {
        ctx.size_bytes > self.bytes
    }
}

// =============================================================================
// Execution rules for Minos (Judge)
// =============================================================================

/// Check if execution completed within the time limit.
pub struct WithinTimeLimit;

#[async_trait]
impl Specification<ExecutionContext> for WithinTimeLimit {
    async fn is_satisfied_by(&self, ctx: &ExecutionContext) -> bool {
        ctx.time_ms <= ctx.time_limit_ms
    }
}

/// Check if execution stayed within the memory limit.
pub struct WithinMemoryLimit;

#[async_trait]
impl Specification<ExecutionContext> for WithinMemoryLimit {
    async fn is_satisfied_by(&self, ctx: &ExecutionContext) -> bool {
        ctx.memory_kb <= ctx.memory_limit_kb
    }
}

/// Check if the program exited with code 0.
pub struct ExitCodeZero;

#[async_trait]
impl Specification<ExecutionContext> for ExitCodeZero {
    async fn is_satisfied_by(&self, ctx: &ExecutionContext) -> bool {
        ctx.exit_code == 0
    }
}

/// Check if the output matches expected.
pub struct OutputMatches;

#[async_trait]
impl Specification<ExecutionContext> for OutputMatches {
    async fn is_satisfied_by(&self, ctx: &ExecutionContext) -> bool {
        ctx.output_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operators::Spec;

    fn sample_file_context(accessed_hours_ago: i64) -> FileContext {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        FileContext {
            path: "/mnt/data/testcases/problem1/test1.txt".to_string(),
            is_file: true,
            is_directory: false,
            size_bytes: 1024,
            created_at: now - (accessed_hours_ago * 3600),
            modified_at: now - (accessed_hours_ago * 3600),
            accessed_at: now - (accessed_hours_ago * 3600),
        }
    }

    fn sample_execution_context(time_ms: u64, memory_kb: u64, exit_code: i32) -> ExecutionContext {
        ExecutionContext {
            submission_id: "sub-123".to_string(),
            problem_id: "prob-456".to_string(),
            test_case_id: "tc-789".to_string(),
            exit_code,
            time_ms,
            memory_kb,
            time_limit_ms: 1000,
            memory_limit_kb: 262144, // 256MB
            output_matches: true,
        }
    }

    #[tokio::test]
    async fn test_last_access_older_than() {
        let ctx = sample_file_context(8); // 8 hours ago
        let rule = LastAccessOlderThan::new(6);
        assert!(rule.is_satisfied_by(&ctx).await);

        let ctx = sample_file_context(4); // 4 hours ago
        assert!(!rule.is_satisfied_by(&ctx).await);
    }

    #[tokio::test]
    async fn test_cleanup_rule_composition() {
        let ctx = sample_file_context(8);
        
        // Stale file rule: (accessed > 6 hours ago) AND (is file)
        let rule = Spec(LastAccessOlderThan::new(6)) & Spec(IsFile);
        assert!(rule.is_satisfied_by(&ctx).await);
    }

    #[tokio::test]
    async fn test_execution_rules() {
        let ctx = sample_execution_context(500, 100000, 0);
        
        // Accepted rule: within time AND within memory AND exit 0 AND output matches
        let rule = Spec(WithinTimeLimit)
            & Spec(WithinMemoryLimit)
            & Spec(ExitCodeZero)
            & Spec(OutputMatches);
        assert!(rule.is_satisfied_by(&ctx).await);
    }

    #[tokio::test]
    async fn test_tle_detection() {
        let ctx = sample_execution_context(1500, 100000, 0); // 1500ms > 1000ms limit
        
        let rule = Spec(WithinTimeLimit);
        assert!(!rule.is_satisfied_by(&ctx).await);
    }
}
