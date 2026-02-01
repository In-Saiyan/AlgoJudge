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

/// Create all API routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(health::routes())
        .nest("/auth", auth::routes())
        .nest("/users", users::routes())
        .nest("/contests", contests::routes())
        .nest("/problems", problems::routes())
        .nest("/submissions", submissions::routes())
        .nest(
            "/admin",
            admin::routes().route_layer(middleware::from_fn(auth_middleware)),
        )
}
