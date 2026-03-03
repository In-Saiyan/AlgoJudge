//! Admin management domain module.
//!
//! Provides admin-only endpoints for user management, system stats,
//! queue management, and rule configuration.

pub mod handler;
pub mod request;
pub mod response;

pub use handler::*;
pub use request::*;
pub use response::*;
