//! Database module
//!
//! This module handles database connections, migrations, and repositories.

pub mod connection;
pub mod repositories;

use sqlx::PgPool;

pub use connection::*;

/// Run database migrations
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
