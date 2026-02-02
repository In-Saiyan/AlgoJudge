//! Test utilities with lazy testcontainers support
//!
//! Containers are started lazily on first use and shared across tests.

#[cfg(test)]
pub mod containers {
    use std::sync::OnceLock;
    use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
    use testcontainers_modules::{postgres::Postgres, redis::Redis};

    static POSTGRES: OnceLock<ContainerAsync<Postgres>> = OnceLock::new();
    static REDIS: OnceLock<ContainerAsync<Redis>> = OnceLock::new();

    /// Get or start a PostgreSQL container (lazy initialization)
    pub async fn get_postgres() -> &'static ContainerAsync<Postgres> {
        if POSTGRES.get().is_none() {
            let container = Postgres::default()
                .with_user("olympus")
                .with_password("olympus_test")
                .with_db_name("olympus_test")
                .start()
                .await
                .expect("Failed to start PostgreSQL container");

            let _ = POSTGRES.set(container);
        }
        POSTGRES.get().unwrap()
    }

    /// Get or start a Redis container (lazy initialization)
    pub async fn get_redis() -> &'static ContainerAsync<Redis> {
        if REDIS.get().is_none() {
            let container = Redis::default()
                .start()
                .await
                .expect("Failed to start Redis container");

            let _ = REDIS.set(container);
        }
        REDIS.get().unwrap()
    }

    /// Get PostgreSQL connection URL from the container
    pub async fn postgres_url() -> String {
        let container = get_postgres().await;
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        format!("postgres://olympus:olympus_test@{}:{}/olympus_test", host, port)
    }

    /// Get Redis connection URL from the container
    pub async fn redis_url() -> String {
        let container = get_redis().await;
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        format!("redis://{}:{}", host, port)
    }
}

#[cfg(test)]
pub mod test_app {
    use super::containers;
    use crate::config::Config;
    use crate::state::AppState;
    use axum::Router;
    use sqlx::PgPool;
    use std::sync::Arc;

    /// Create a test application with real database and redis containers
    pub async fn create_test_app() -> (Router, Arc<AppState>) {
        // Get container URLs (lazy start)
        let database_url = containers::postgres_url().await;
        let redis_url = containers::redis_url().await;

        // Create database pool
        let db_pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");

        // Create Redis pool
        let redis_cfg = deadpool_redis::Config::from_url(&redis_url);
        let redis_pool = redis_cfg
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("Failed to create Redis pool");

        // Create app state
        let config = Config {
            environment: "test".to_string(),
            port: 0,
            database_url,
            redis_url,
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_access_expiration: 900,
            jwt_refresh_expiration: 604800,
        };

        let state = Arc::new(AppState {
            db: db_pool,
            redis: redis_pool,
            config,
        });

        // Build the router
        let app = crate::create_router(state.clone());

        (app, state)
    }

    /// Clean up test data between tests
    pub async fn cleanup_test_data(pool: &PgPool) {
        sqlx::query("TRUNCATE users, sessions, contests, problems, submissions CASCADE")
            .execute(pool)
            .await
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_containers_start_lazily() {
        // First call starts the container
        let url1 = containers::postgres_url().await;
        assert!(url1.contains("postgres://"));

        // Second call returns same container
        let url2 = containers::postgres_url().await;
        assert_eq!(url1, url2);
    }
}
