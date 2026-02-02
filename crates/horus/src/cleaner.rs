//! Cleanup job implementations

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use tokio::fs;
use walkdir::WalkDir;

use crate::config::Config;
use crate::specs::{
    CleanupContext, CleanupSpec, CleanupSpecExt, CreatedOlderThan, HasActiveSubmission,
    HasProblemRecord, HasSubmissionRecord, IsDirectory, IsFile, LastAccessOlderThan,
};

/// Statistics from a cleanup run
#[derive(Debug, Default)]
pub struct CleanupStats {
    pub files_scanned: u64,
    pub files_deleted: u64,
    pub dirs_deleted: u64,
    pub bytes_freed: u64,
    pub errors: u64,
}

/// Cleanup job runner
pub struct CleanupRunner {
    config: Arc<Config>,
    db_pool: PgPool,
}

impl CleanupRunner {
    pub fn new(config: Arc<Config>, db_pool: PgPool) -> Self {
        Self { config, db_pool }
    }

    /// Clean stale test cases (not accessed in X hours)
    pub async fn cleanup_stale_testcases(&self) -> Result<CleanupStats> {
        let mut stats = CleanupStats::default();
        let testcases_path = &self.config.storage.testcases_path;

        if !testcases_path.exists() {
            tracing::debug!("Testcases directory does not exist, skipping");
            return Ok(stats);
        }

        let stale_hours = self.config.schedules.testcase_stale_hours;
        tracing::info!(
            "Cleaning testcases older than {} hours in {:?}",
            stale_hours,
            testcases_path
        );

        // Build cleanup spec: directory AND last access older than threshold AND has problem record
        let spec = IsDirectory
            .and(LastAccessOlderThan::hours(stale_hours))
            .and(HasProblemRecord.not()); // Only clean if problem was deleted

        // Walk top-level directories (each is a problem_id)
        for entry in WalkDir::new(testcases_path).min_depth(1).max_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Error reading directory entry: {}", e);
                    stats.errors += 1;
                    continue;
                }
            };

            stats.files_scanned += 1;
            let ctx = CleanupContext::new(entry.path(), &self.db_pool);

            if spec.is_satisfied_by(&ctx).await {
                match self.delete_directory(entry.path()).await {
                    Ok(bytes) => {
                        stats.dirs_deleted += 1;
                        stats.bytes_freed += bytes;
                        tracing::info!("Deleted stale testcase dir: {:?}", entry.path());
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete {:?}: {}", entry.path(), e);
                        stats.errors += 1;
                    }
                }
            }
        }

        tracing::info!(
            "Testcase cleanup complete: {} dirs deleted, {} bytes freed",
            stats.dirs_deleted,
            stats.bytes_freed
        );

        Ok(stats)
    }

    /// Clean orphaned temp directories (older than X hours with no active submission)
    pub async fn cleanup_orphan_temp(&self) -> Result<CleanupStats> {
        let mut stats = CleanupStats::default();
        let temp_path = &self.config.storage.temp_path;

        if !temp_path.exists() {
            tracing::debug!("Temp directory does not exist, skipping");
            return Ok(stats);
        }

        let orphan_hours = self.config.schedules.temp_orphan_hours;
        tracing::info!(
            "Cleaning orphan temp dirs older than {} hours in {:?}",
            orphan_hours,
            temp_path
        );

        // Build cleanup spec: directory AND created older than threshold AND no active submission
        let spec = IsDirectory
            .and(CreatedOlderThan::hours(orphan_hours))
            .and(HasActiveSubmission.not());

        for entry in WalkDir::new(temp_path).min_depth(1).max_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Error reading directory entry: {}", e);
                    stats.errors += 1;
                    continue;
                }
            };

            stats.files_scanned += 1;
            let ctx = CleanupContext::new(entry.path(), &self.db_pool);

            if spec.is_satisfied_by(&ctx).await {
                match self.delete_directory(entry.path()).await {
                    Ok(bytes) => {
                        stats.dirs_deleted += 1;
                        stats.bytes_freed += bytes;
                        tracing::info!("Deleted orphan temp dir: {:?}", entry.path());
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete {:?}: {}", entry.path(), e);
                        stats.errors += 1;
                    }
                }
            }
        }

        tracing::info!(
            "Temp cleanup complete: {} dirs deleted, {} bytes freed",
            stats.dirs_deleted,
            stats.bytes_freed
        );

        Ok(stats)
    }

    /// Clean orphaned user binaries (no submission record)
    pub async fn cleanup_orphan_binaries(&self) -> Result<CleanupStats> {
        let mut stats = CleanupStats::default();
        let binaries_path = &self.config.storage.binaries_path;

        if !binaries_path.exists() {
            tracing::debug!("Binaries directory does not exist, skipping");
            return Ok(stats);
        }

        tracing::info!("Cleaning orphan binaries in {:?}", binaries_path);

        // Build cleanup spec: file AND older than 1 day AND no submission record
        let spec = IsFile
            .and(CreatedOlderThan::days(1))
            .and(HasSubmissionRecord.not());

        for entry in WalkDir::new(binaries_path).min_depth(1).max_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Error reading directory entry: {}", e);
                    stats.errors += 1;
                    continue;
                }
            };

            stats.files_scanned += 1;
            let ctx = CleanupContext::new(entry.path(), &self.db_pool);

            if spec.is_satisfied_by(&ctx).await {
                match self.delete_file(entry.path()).await {
                    Ok(bytes) => {
                        stats.files_deleted += 1;
                        stats.bytes_freed += bytes;
                        tracing::info!("Deleted orphan binary: {:?}", entry.path());
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete {:?}: {}", entry.path(), e);
                        stats.errors += 1;
                    }
                }
            }
        }

        tracing::info!(
            "Binary cleanup complete: {} files deleted, {} bytes freed",
            stats.files_deleted,
            stats.bytes_freed
        );

        Ok(stats)
    }

    /// Clean old submissions (based on retention policy)
    pub async fn cleanup_old_submissions(&self) -> Result<CleanupStats> {
        let mut stats = CleanupStats::default();
        let retention_days = self.config.schedules.submission_retention_days;

        if retention_days == 0 {
            tracing::debug!("Submission retention disabled, skipping");
            return Ok(stats);
        }

        tracing::info!(
            "Cleaning submissions older than {} days",
            retention_days
        );

        // Get old submissions from database
        let old_submissions = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            SELECT id FROM submissions 
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
            AND status NOT IN ('PENDING', 'COMPILING', 'JUDGING')
            "#,
        )
        .bind(retention_days as i32)
        .fetch_all(&self.db_pool)
        .await?;

        for submission_id in old_submissions {
            stats.files_scanned += 1;

            // Delete associated files
            let binary_path = self
                .config
                .storage
                .binaries_path
                .join(format!("{}_bin", submission_id));

            if binary_path.exists() {
                match self.delete_file(&binary_path).await {
                    Ok(bytes) => {
                        stats.files_deleted += 1;
                        stats.bytes_freed += bytes;
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete binary {:?}: {}", binary_path, e);
                        stats.errors += 1;
                    }
                }
            }

            // Delete database record
            if let Err(e) = sqlx::query("DELETE FROM submission_results WHERE submission_id = $1")
                .bind(submission_id)
                .execute(&self.db_pool)
                .await
            {
                tracing::error!("Failed to delete submission results: {}", e);
                stats.errors += 1;
            }

            if let Err(e) = sqlx::query("DELETE FROM submissions WHERE id = $1")
                .bind(submission_id)
                .execute(&self.db_pool)
                .await
            {
                tracing::error!("Failed to delete submission: {}", e);
                stats.errors += 1;
            }
        }

        tracing::info!(
            "Submission cleanup complete: {} files deleted, {} bytes freed",
            stats.files_deleted,
            stats.bytes_freed
        );

        Ok(stats)
    }

    /// Delete a directory recursively and return bytes freed
    async fn delete_directory(&self, path: &Path) -> Result<u64> {
        let bytes = self.calculate_dir_size(path).await;
        fs::remove_dir_all(path).await?;
        Ok(bytes)
    }

    /// Delete a file and return bytes freed
    async fn delete_file(&self, path: &Path) -> Result<u64> {
        let bytes = fs::metadata(path).await.map(|m| m.len()).unwrap_or(0);
        fs::remove_file(path).await?;
        Ok(bytes)
    }

    /// Calculate total size of directory
    async fn calculate_dir_size(&self, path: &Path) -> u64 {
        let mut total = 0u64;
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        total
    }
}
