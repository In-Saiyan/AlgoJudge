//! AlgoJudge - Algorithmic Solution Benchmarking System
//!
//! This library provides the core functionality for the AlgoJudge platform,
//! a competitive programming judge system that benchmarks algorithmic solutions.
//!
//! # Features
//!
//! - Multi-language support (C, C++, Rust, Go, Zig, Python)
//! - Isolated Docker container execution
//! - Accurate performance metrics (time, memory)
//! - Contest management with multiple scoring modes
//! - Role-based access control
//!
//! # Architecture
//!
//! The application follows a layered architecture:
//! - **Handlers**: HTTP request handlers (thin layer)
//! - **Services**: Business logic
//! - **Repositories**: Database access
//! - **Models**: Domain models and DTOs

pub mod benchmark;
pub mod config;
pub mod constants;
pub mod db;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod services;
pub mod state;
pub mod utils;

// Re-export commonly used types
pub use config::Config;
pub use error::{AppError, AppResult};
pub use state::AppState;
