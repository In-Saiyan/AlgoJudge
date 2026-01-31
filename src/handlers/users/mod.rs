//! User management handlers

mod handler;
pub mod request;
pub mod response;

pub use handler::*;
pub use request::*;
pub use response::*;

use axum::{
    routing::{get, put},
    Router,
};

use crate::state::AppState;

/// User routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handler::list_users))
        .route("/{id}", get(handler::get_user))
        .route("/{id}", put(handler::update_user))
        .route("/{id}/submissions", get(handler::get_user_submissions))
        .route("/{id}/stats", get(handler::get_user_stats))
}
