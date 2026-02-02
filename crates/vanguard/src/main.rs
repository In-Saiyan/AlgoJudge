//! Vanguard - API Gateway for Olympus
//!
//! The main entry point for the Olympus API Gateway service.

mod config;
mod domain;
mod error;
mod middleware;
mod state;

#[cfg(test)]
mod test_utils;

use std::net::SocketAddr;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
};
use axum::http::{header, Method};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{create_db_pool, create_redis_pool, Config, RateLimitConfig};
use crate::domain::{auth, contests, health, problems, submissions, users};
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

    // Public user routes
    let public_user_routes = Router::new()
        .route("/", get(users::list_users))
        .route("/{id}", get(users::get_user))
        .route("/{id}/stats", get(users::get_user_stats));

    // Protected user routes
    let protected_user_routes = Router::new()
        .route("/{id}", axum::routing::put(users::update_user))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine user routes
    let user_routes = Router::new()
        .merge(public_user_routes)
        .merge(protected_user_routes);

    // Public contest routes
    let public_contest_routes = contests::contest_routes();

    // Protected contest routes
    let protected_contest_routes = contests::protected_contest_routes()
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine contest routes
    let contest_routes = Router::new()
        .merge(public_contest_routes)
        .merge(protected_contest_routes);

    // Public problem routes
    let public_problem_routes = problems::problem_routes();

    // Protected problem routes
    let protected_problem_routes = problems::protected_problem_routes()
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine problem routes
    let problem_routes = Router::new()
        .merge(public_problem_routes)
        .merge(protected_problem_routes);

    // Contest problems routes (nested under contests)
    let contest_problems_routes = Router::new()
        .route("/{contest_id}/problems", axum::routing::get(problems::list_contest_problems))
        .route(
            "/{contest_id}/problems",
            axum::routing::post(problems::add_problem_to_contest)
                .layer(axum_middleware::from_fn_with_state(state.clone(), auth_middleware)),
        )
        .route(
            "/{contest_id}/problems/{problem_id}",
            axum::routing::delete(problems::remove_problem_from_contest)
                .layer(axum_middleware::from_fn_with_state(state.clone(), auth_middleware)),
        );

    // Contest leaderboard routes
    let contest_leaderboard_routes = Router::new()
        .route("/{contest_id}/leaderboard", get(submissions::get_contest_leaderboard));

    // Submission routes (all protected)
    // Create routes with additional submission rate limit
    let submission_create_routes = Router::new()
        .route("/", post(submissions::create_submission))
        .route("/zip", post(submissions::create_zip_submission))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            submission_rate_limit_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Read-only submission routes (no submission rate limit)
    let submission_read_routes = Router::new()
        .route("/", get(submissions::list_submissions))
        .route("/{id}", get(submissions::get_submission))
        .route("/{id}/results", get(submissions::get_submission_results))
        .route("/{id}/source", get(submissions::get_submission_source))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine submission routes
    let submission_routes = Router::new()
        .merge(submission_create_routes)
        .merge(submission_read_routes);

    // User submissions route
    let user_submissions_routes = Router::new()
        .route(
            "/{id}/submissions",
            get(submissions::get_user_submissions)
                .layer(axum_middleware::from_fn_with_state(state.clone(), auth_middleware)),
        );

    // API v1 routes
    let api_v1 = Router::new()
        .nest("/auth", auth_routes)
        .nest("/users", user_routes)
        .merge(Router::new().nest("/users", user_submissions_routes))
        .nest("/contests", contest_routes)
        .merge(Router::new().nest("/contests", contest_problems_routes))
        .merge(Router::new().nest("/contests", contest_leaderboard_routes))
        .nest("/problems", problem_routes)
        .nest("/submissions", submission_routes)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            api_rate_limit_middleware,
        ));

    // CORS configuration - permissive for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .expose_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    // Main router
    // Note: Layers are applied bottom-up, so CORS must be last to wrap everything
    Router::new()
        .nest("/health", health_routes)
        .nest("/api/v1", api_v1)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
