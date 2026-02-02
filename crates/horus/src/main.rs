//! Horus - Cleaner Service for Olympus
//!
//! Scheduled cleanup service that maintains storage hygiene:
//! - Removes stale test cases not accessed recently
//! - Cleans orphaned temp directories
//! - Removes binaries for deleted submissions
//! - Optional: Archives old submissions based on retention policy

mod cleaner;
mod config;
mod scheduler;
mod specs;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::scheduler::CleanupScheduler;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "horus=debug,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Horus Cleaner Service");

    // Load configuration
    let config = Arc::new(Config::from_env());
    tracing::info!("Environment: {}", config.environment);

    // Create database pool
    tracing::info!("Connecting to database...");
    let db_pool = sqlx::PgPool::connect(&config.database_url).await?;
    tracing::info!("Database connected");

    // Create shutdown signal
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    // Setup signal handlers
    tokio::spawn(async move {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        tracing::info!("Shutdown signal received");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Create and setup scheduler
    let mut scheduler = CleanupScheduler::new(config, db_pool).await?;
    scheduler.setup_jobs().await?;

    tracing::info!("Horus ready, starting scheduler");

    // Start the scheduler
    scheduler.start().await?;

    // Wait for shutdown signal
    while !shutdown.load(Ordering::SeqCst) {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    // Graceful shutdown
    tracing::info!("Shutting down scheduler...");
    scheduler.shutdown().await?;

    tracing::info!("Horus shutdown complete");
    Ok(())
}
