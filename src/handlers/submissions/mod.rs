//! Submission handlers

mod handler;
pub mod request;
pub mod response;

pub use handler::*;
pub use request::*;
pub use response::*;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

/// Submission routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(handler::create_submission))
        .route("/", get(handler::list_submissions))
        .route("/{id}", get(handler::get_submission))
        .route("/{id}/results", get(handler::get_submission_results))
        .route("/{id}/source", get(handler::get_submission_source))
}
