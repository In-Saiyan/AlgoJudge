//! Admin management handlers

mod handler;
pub mod request;
pub mod response;

pub use handler::*;
pub use request::*;
pub use response::*;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::state::AppState;

/// Admin routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // User management
        .route("/users", get(handler::list_all_users))
        .route("/users/:id/role", put(handler::update_user_role))
        .route("/users/:id/ban", post(handler::ban_user))
        .route("/users/:id/unban", post(handler::unban_user))
        // System management
        .route("/stats", get(handler::get_system_stats))
        .route("/containers", get(handler::list_benchmark_containers))
        .route("/containers/:id", delete(handler::stop_container))
        // Queue management
        .route("/queue", get(handler::get_submission_queue))
        .route("/queue/:id/rejudge", post(handler::rejudge_submission))
}
