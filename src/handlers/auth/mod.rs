//! Authentication handlers

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

/// Authentication routes
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(handler::register))
        .route("/login", post(handler::login))
        .route("/refresh", post(handler::refresh_token))
        .route("/logout", post(handler::logout))
        .route("/me", get(handler::get_current_user))
}
