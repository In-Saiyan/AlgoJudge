//! Problem management handlers

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

/// Problem routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handler::list_problems))
        .route("/", post(handler::create_problem))
        .route("/{id}", get(handler::get_problem))
        .route("/{id}", put(handler::update_problem))
        .route("/{id}", delete(handler::delete_problem))
        // Test cases
        .route("/{id}/test-cases", get(handler::list_test_cases))
        .route("/{id}/test-cases", post(handler::add_test_case))
        .route("/{id}/test-cases/{tc_id}", put(handler::update_test_case))
        .route("/{id}/test-cases/{tc_id}", delete(handler::delete_test_case))
}
