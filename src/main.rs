//! AlgoJudge - Application Entry Point
//!
//! This is the main entry point for the AlgoJudge server.

use std::net::SocketAddr;

use axum::Router;
use bollard::Docker;
use redis::Client as RedisClient;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use algojudge::{
    config::CONFIG,
    db,
    handlers,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| CONFIG.server.rust_log.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting AlgoJudge server...");

    // Initialize database connection pool
    tracing::info!("Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(CONFIG.database.max_connections)
        .connect(&CONFIG.database.url)
        .await?;

    // Run database migrations
    tracing::info!("Running database migrations...");
    db::run_migrations(&db_pool).await?;

    // Initialize Redis connection
    tracing::info!("Connecting to Redis...");
    let redis_client = RedisClient::open(CONFIG.redis.url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;

    // Initialize Docker client
    tracing::info!("Connecting to Docker...");
    let docker = Docker::connect_with_socket_defaults()?;

    // Verify Docker connection
    let docker_info = docker.version().await?;
    tracing::info!(
        "Connected to Docker version: {}",
        docker_info.version.unwrap_or_default()
    );

    // Create application state
    let state = AppState::new(db_pool, redis_conn, docker, CONFIG.clone());

    // Build the router
    let app = Router::new()
        .nest("/api/v1", handlers::routes())
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Start the server
    let addr = SocketAddr::new(
        CONFIG.server.host.parse()?,
        CONFIG.server.port,
    );
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("Server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
