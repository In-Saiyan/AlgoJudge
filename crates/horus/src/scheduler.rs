//! Cron scheduler for cleanup jobs

use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::cleaner::CleanupRunner;
use crate::config::Config;

/// Scheduler that runs cleanup jobs on cron schedules
pub struct CleanupScheduler {
    config: Arc<Config>,
    db_pool: PgPool,
    scheduler: JobScheduler,
}

impl CleanupScheduler {
    /// Create a new cleanup scheduler
    pub async fn new(config: Arc<Config>, db_pool: PgPool) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            config,
            db_pool,
            scheduler,
        })
    }

    /// Add all cleanup jobs to the scheduler
    pub async fn setup_jobs(&mut self) -> Result<()> {
        // Testcase cleanup job
        self.add_testcase_cleanup_job().await?;

        // Temp directory cleanup job
        self.add_temp_cleanup_job().await?;

        // Binary cleanup job
        self.add_binary_cleanup_job().await?;

        // Submission cleanup job (if enabled)
        if self.config.schedules.submission_retention_days > 0 {
            self.add_submission_cleanup_job().await?;
        }

        Ok(())
    }

    /// Start the scheduler
    pub async fn start(&self) -> Result<()> {
        self.scheduler.start().await?;
        Ok(())
    }

    /// Shutdown the scheduler gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        self.scheduler.shutdown().await?;
        Ok(())
    }

    /// Add testcase cleanup job
    async fn add_testcase_cleanup_job(&self) -> Result<()> {
        let config = self.config.clone();
        let db_pool = self.db_pool.clone();
        let cron_expr = self.config.schedules.testcase_cleanup.clone();

        tracing::info!("Adding testcase cleanup job: {}", cron_expr);

        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let config = config.clone();
            let db_pool = db_pool.clone();

            Box::pin(async move {
                tracing::info!("Running testcase cleanup job");
                let runner = CleanupRunner::new(config, db_pool);

                match runner.cleanup_stale_testcases().await {
                    Ok(stats) => {
                        tracing::info!(
                            "Testcase cleanup: scanned={}, deleted={}, bytes_freed={}, errors={}",
                            stats.files_scanned,
                            stats.dirs_deleted,
                            stats.bytes_freed,
                            stats.errors
                        );
                    }
                    Err(e) => {
                        tracing::error!("Testcase cleanup failed: {}", e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add temp directory cleanup job
    async fn add_temp_cleanup_job(&self) -> Result<()> {
        let config = self.config.clone();
        let db_pool = self.db_pool.clone();
        let cron_expr = self.config.schedules.temp_cleanup.clone();

        tracing::info!("Adding temp cleanup job: {}", cron_expr);

        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let config = config.clone();
            let db_pool = db_pool.clone();

            Box::pin(async move {
                tracing::info!("Running temp cleanup job");
                let runner = CleanupRunner::new(config, db_pool);

                match runner.cleanup_orphan_temp().await {
                    Ok(stats) => {
                        tracing::info!(
                            "Temp cleanup: scanned={}, deleted={}, bytes_freed={}, errors={}",
                            stats.files_scanned,
                            stats.dirs_deleted,
                            stats.bytes_freed,
                            stats.errors
                        );
                    }
                    Err(e) => {
                        tracing::error!("Temp cleanup failed: {}", e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add binary cleanup job
    async fn add_binary_cleanup_job(&self) -> Result<()> {
        let config = self.config.clone();
        let db_pool = self.db_pool.clone();
        let cron_expr = self.config.schedules.binary_cleanup.clone();

        tracing::info!("Adding binary cleanup job: {}", cron_expr);

        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let config = config.clone();
            let db_pool = db_pool.clone();

            Box::pin(async move {
                tracing::info!("Running binary cleanup job");
                let runner = CleanupRunner::new(config, db_pool);

                match runner.cleanup_orphan_binaries().await {
                    Ok(stats) => {
                        tracing::info!(
                            "Binary cleanup: scanned={}, deleted={}, bytes_freed={}, errors={}",
                            stats.files_scanned,
                            stats.files_deleted,
                            stats.bytes_freed,
                            stats.errors
                        );
                    }
                    Err(e) => {
                        tracing::error!("Binary cleanup failed: {}", e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add submission cleanup job
    async fn add_submission_cleanup_job(&self) -> Result<()> {
        let config = self.config.clone();
        let db_pool = self.db_pool.clone();
        let cron_expr = self.config.schedules.submission_cleanup.clone();

        tracing::info!("Adding submission cleanup job: {}", cron_expr);

        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let config = config.clone();
            let db_pool = db_pool.clone();

            Box::pin(async move {
                tracing::info!("Running submission cleanup job");
                let runner = CleanupRunner::new(config, db_pool);

                match runner.cleanup_old_submissions().await {
                    Ok(stats) => {
                        tracing::info!(
                            "Submission cleanup: scanned={}, deleted={}, bytes_freed={}, errors={}",
                            stats.files_scanned,
                            stats.files_deleted,
                            stats.bytes_freed,
                            stats.errors
                        );
                    }
                    Err(e) => {
                        tracing::error!("Submission cleanup failed: {}", e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }
}
