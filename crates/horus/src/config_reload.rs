//! Redis pub/sub based configuration reloading.
//!
//! Listens for config reload notifications on the `config_reload` channel.
//! When a message targeting the "horus" service is received, policies are
//! reloaded from the database and cached in memory.

use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use olympus_rules::config::RuleConfig;

/// A loaded cleanup policy from the database.
#[derive(Debug, Clone)]
pub struct LoadedPolicy {
    pub name: String,
    pub description: Option<String>,
    pub config: RuleConfig,
    pub enabled: bool,
    pub version: String,
}

/// Shared, reloadable policy store.
///
/// Holds the currently active cleanup policies. Protected by an `RwLock`
/// so the scheduler can read while the reload listener can write.
#[derive(Clone)]
pub struct PolicyStore {
    inner: Arc<RwLock<Vec<LoadedPolicy>>>,
}

impl PolicyStore {
    /// Create a new empty policy store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Load all enabled Horus policies from the database.
    pub async fn load_from_db(&self, db: &PgPool) -> Result<usize> {
        let rows = sqlx::query_as::<_, PolicyRow>(
            r#"
            SELECT name, description, config, enabled, version
            FROM rule_configs
            WHERE service = 'horus' AND enabled = true
            ORDER BY name
            "#,
        )
        .fetch_all(db)
        .await?;

        let mut policies = Vec::with_capacity(rows.len());
        for row in &rows {
            match serde_json::from_value::<RuleConfig>(row.config.clone()) {
                Ok(config) => {
                    policies.push(LoadedPolicy {
                        name: row.name.clone(),
                        description: row.description.clone(),
                        config,
                        enabled: row.enabled,
                        version: row.version.clone(),
                    });
                }
                Err(e) => {
                    tracing::warn!("Skipping policy '{}': invalid config JSON: {}", row.name, e);
                }
            }
        }

        let count = policies.len();
        *self.inner.write().await = policies;
        tracing::info!("Loaded {} cleanup policies from database", count);
        Ok(count)
    }

    /// Get a snapshot of current policies.
    pub async fn get_policies(&self) -> Vec<LoadedPolicy> {
        self.inner.read().await.clone()
    }

    /// Get a specific policy by name.
    #[allow(dead_code)]
    pub async fn get_policy(&self, name: &str) -> Option<LoadedPolicy> {
        self.inner
            .read()
            .await
            .iter()
            .find(|p| p.name == name)
            .cloned()
    }
}

/// Database row for policy queries.
#[derive(Debug, sqlx::FromRow)]
struct PolicyRow {
    name: String,
    description: Option<String>,
    config: serde_json::Value,
    enabled: bool,
    version: String,
}

/// Start a background task that subscribes to `config_reload` Redis channel.
///
/// When a message with payload `"horus"` is received, all policies are
/// reloaded from the database.
pub fn start_config_reload_listener(
    redis_url: String,
    db: PgPool,
    store: PolicyStore,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match run_subscriber(&redis_url, &db, &store).await {
                Ok(()) => {
                    tracing::info!("Config reload subscriber exited cleanly");
                    break;
                }
                Err(e) => {
                    tracing::error!(
                        "Config reload subscriber error: {}. Reconnecting in 5s...",
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    })
}

/// Inner subscription loop with automatic reconnect on failure.
async fn run_subscriber(redis_url: &str, db: &PgPool, store: &PolicyStore) -> Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe("config_reload").await?;
    tracing::info!("Subscribed to config_reload channel");

    let mut msg_stream = pubsub.on_message();

    loop {
        let msg = {
            use futures::StreamExt;
            msg_stream.next().await
        };

        match msg {
            Some(msg) => {
                let payload: String = msg.get_payload()?;

                if payload == "horus" {
                    tracing::info!("Received config reload signal for horus");
                    match store.load_from_db(db).await {
                        Ok(count) => {
                            tracing::info!("Successfully reloaded {} policies", count);
                        }
                        Err(e) => {
                            tracing::error!("Failed to reload policies: {}", e);
                        }
                    }
                } else {
                    tracing::debug!("Ignoring config_reload for service: {}", payload);
                }
            }
            None => {
                return Err(anyhow::anyhow!("Pub/sub stream ended unexpectedly"));
            }
        }
    }
}
