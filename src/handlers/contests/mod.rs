//! Contest management handlers

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

/// Contest routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // Contest CRUD
        .route("/", get(handler::list_contests))
        .route("/", post(handler::create_contest))
        .route("/:id", get(handler::get_contest))
        .route("/:id", put(handler::update_contest))
        .route("/:id", delete(handler::delete_contest))
        // Contest participation
        .route("/:id/register", post(handler::register_for_contest))
        .route("/:id/unregister", post(handler::unregister_from_contest))
        .route("/:id/participants", get(handler::list_participants))
        // Contest problems
        .route("/:id/problems", get(handler::list_contest_problems))
        .route("/:id/problems", post(handler::add_problem_to_contest))
        .route("/:id/problems/:problem_id", delete(handler::remove_problem_from_contest))
        // Leaderboard
        .route("/:id/leaderboard", get(handler::get_leaderboard))
        // Virtual participation
        .route("/:id/virtual", post(handler::start_virtual_participation))
}
