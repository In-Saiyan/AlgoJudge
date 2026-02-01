//! HTTP Request Handlers
//!
//! This module contains all HTTP request handlers organized by domain.

pub mod admin;
pub mod auth;
pub mod contests;
pub mod health;
pub mod problems;
pub mod submissions;
pub mod users;

use axum::{middleware, Router};

use crate::{middleware::auth::auth_middleware, state::AppState};

/// Create all API routes with state
pub fn routes_with_state(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(health::routes())
        .nest("/auth", auth::routes())
        .nest("/users", users::routes())
        .nest("/contests", contests::routes())
        .nest(
            "/problems",
            problems::routes()
                .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware)),
        )
        .nest(
            "/submissions",
            submissions::routes()
                .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware)),
        )
        .nest(
            "/admin",
            admin::routes()
                .route_layer(middleware::from_fn_with_state(state, auth_middleware)),
        )
}

/// Create all API routes (without middleware that requires state)
/// Note: Use routes_with_state() when auth middleware is needed
pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(health::routes())
        .nest("/auth", auth::routes())
        .nest("/users", users::routes())
        .nest("/contests", contests::routes())
        .nest("/problems", problems::routes())
        .nest("/submissions", submissions::routes())
        .nest("/admin", admin::routes())
}
