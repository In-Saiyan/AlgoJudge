//! Vanguard - API Gateway for Olympus
//!
//! The main entry point for the Olympus API Gateway service.

mod config;
mod domain;
mod error;
mod middleware;
mod state;

use std::net::SocketAddr;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{create_db_pool, create_redis_pool, Config, RateLimitConfig};
use crate::domain::{auth, health};
use crate::middleware::{auth::auth_middleware, rate_limit::*};
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vanguard=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env();
    let rate_limit_config = RateLimitConfig::default();

    tracing::info!("Starting Vanguard API Gateway");
    tracing::info!("Environment: {}", config.environment);

    // Create database pool
    tracing::info!("Connecting to database...");
    let db_pool = create_db_pool(&config.database_url).await?;
    tracing::info!("Database connected");

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&db_pool).await?;
    tracing::info!("Migrations complete");

    // Create Redis pool
    tracing::info!("Connecting to Redis...");
    let redis_pool = create_redis_pool(&config.redis_url)?;
    tracing::info!("Redis connected");

    // Create app state
    let state = AppState::new(db_pool, redis_pool, config.clone(), rate_limit_config);

    // Build router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

/// Create the application router with all routes and middleware.
fn create_router(state: AppState) -> Router {
    // Health routes (no auth required)
    let health_routes = Router::new()
        .route("/", get(health::health_check))
        .route("/live", get(health::liveness))
        .route("/ready", get(health::readiness));

    // Public auth routes
    let public_auth_routes = Router::new()
        .route(
            "/register",
            post(auth::register).layer(axum_middleware::from_fn_with_state(
                state.clone(),
                register_rate_limit_middleware,
            )),
        )
        .route(
            "/login",
            post(auth::login).layer(axum_middleware::from_fn_with_state(
                state.clone(),
                login_rate_limit_middleware,
            )),
        )
        .route("/refresh", post(auth::refresh));

    // Protected auth routes
    let protected_auth_routes = Router::new()
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine auth routes
    let auth_routes = Router::new()
        .merge(public_auth_routes)
        .merge(protected_auth_routes);

    // API v1 routes
    let api_v1 = Router::new()
        .nest("/auth", auth_routes)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            api_rate_limit_middleware,
        ));

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Main router
    Router::new()
        .nest("/health", health_routes)
        .nest("/api/v1", api_v1)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
