//! Minos - Judge Service for Olympus

//! Minos - Judge Service for Olympus
//!
//! Consumes compiled submissions from Redis Stream, executes them
//! against test cases in a sandboxed environment, and records verdicts.

mod config;
mod consumer;
mod executor;
mod metrics;
mod testcase;
mod verdict;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::consumer::JudgeConsumer;
use crate::metrics::MetricsServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minos=debug,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Minos Judge Service");

    // Load configuration
    let config = Config::from_env();
    tracing::info!("Environment: {}", config.environment);

    // Create database pool
    tracing::info!("Connecting to database...");
    let db_pool = sqlx::PgPool::connect(&config.database_url).await?;
    tracing::info!("Database connected");

    // Create Redis pool
    tracing::info!("Connecting to Redis...");
    let redis_cfg = deadpool_redis::Config::from_url(&config.redis_url);
    let redis_pool = redis_cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?;
    tracing::info!("Redis connected");

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

        tracing::info!("Shutdown signal received, finishing current job...");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Start metrics server
    let metrics_port = config.metrics_port;
    tokio::spawn(async move {
        if let Err(e) = MetricsServer::run(metrics_port).await {
            tracing::error!("Metrics server error: {}", e);
        }
    });

    // Create and initialize consumer
    let mut consumer = JudgeConsumer::new(config, db_pool, redis_pool, shutdown);
    consumer.initialize().await?;

    tracing::info!("Minos ready, starting judge consumer loop");

    // Run the consumer loop
    consumer.run().await?;

    tracing::info!("Minos shutdown complete");
    Ok(())
}
