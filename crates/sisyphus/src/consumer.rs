//! Redis Stream consumer for compilation jobs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use deadpool_redis::redis;
use deadpool_redis::Pool as RedisPool;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::compiler::Compiler;
use crate::config::Config;

/// Maximum retry attempts for a job before moving to dead letter.
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (in milliseconds).
const BASE_RETRY_DELAY_MS: u64 = 1000;

/// A compilation job from the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileJob {
    pub submission_id: Uuid,
    pub job_type: String, // "source" or "zip"
    pub file_path: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub retry_count: u32,
}

/// Redis Stream consumer for compilation jobs.
pub struct JobConsumer {
    config: Config,
    db: PgPool,
    redis: RedisPool,
    compiler: Compiler,
    shutdown: Arc<AtomicBool>,
}

impl JobConsumer {
    /// Create a new job consumer.
    pub fn new(
        config: Config,
        db: PgPool,
        redis: RedisPool,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        let compiler = Compiler::new(config.clone());
        Self {
            config,
            db,
            redis,
            compiler,
            shutdown,
        }
    }

    /// Initialize the consumer group (create if not exists).
    pub async fn initialize(&mut self) -> Result<()> {
        let mut conn = self.redis.get().await?;

        // Create consumer group for compile_queue
        self.create_consumer_group(
            &mut conn,
            &self.config.compile_stream,
            &self.config.consumer_group,
        )
        .await?;

        // Create dead letter stream consumer group
        let dead_letter_stream = format!("{}_dead_letter", self.config.compile_stream);
        self.create_consumer_group(
            &mut conn,
            &dead_letter_stream,
            &self.config.consumer_group,
        )
        .await?;

        Ok(())
    }

    /// Create a consumer group on a stream.
    async fn create_consumer_group(
        &self,
        conn: &mut deadpool_redis::Connection,
        stream: &str,
        group: &str,
    ) -> Result<()> {
        let result: Result<(), _> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut **conn)
            .await;

        match result {
            Ok(_) => {
                tracing::info!(
                    "Created consumer group '{}' on stream '{}'",
                    group,
                    stream
                );
            }
            Err(e) => {
                if !e.to_string().contains("BUSYGROUP") {
                    return Err(e.into());
                }
                tracing::debug!("Consumer group '{}' already exists on '{}'", group, stream);
            }
        }

        Ok(())
    }

    /// Run the consumer loop.
    pub async fn run(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::SeqCst) {
            match self.process_next_job().await {
                Ok(processed) => {
                    if !processed {
                        // No jobs available, brief pause
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::error!("Error processing job: {}", err_msg);

                    // If Redis lost the consumer group, re-create it
                    if err_msg.contains("NOGROUP") {
                        tracing::warn!("Consumer group missing, re-initializing...");
                        if let Err(init_err) = self.initialize().await {
                            tracing::error!("Failed to re-initialize consumer group: {}", init_err);
                        }
                    }

                    // Brief pause on error to avoid tight loop
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }

        tracing::info!("Consumer loop exiting due to shutdown signal");
        Ok(())
    }

    /// Process the next job from the queue.
    /// Returns Ok(true) if a job was processed, Ok(false) if no jobs available.
    async fn process_next_job(&mut self) -> Result<bool> {
        let mut conn = self.redis.get().await?;

        // Read from stream with consumer group (blocking for 5 seconds)
        let result: redis::Value = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.config.consumer_group)
            .arg(&self.config.consumer_name)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(5000) // 5 second timeout
            .arg("STREAMS")
            .arg(&self.config.compile_stream)
            .arg(">") // Only new messages
            .query_async(&mut *conn)
            .await?;

        // Parse the result
        let messages = match result {
            redis::Value::Nil => return Ok(false),
            redis::Value::Array(streams) => {
                if streams.is_empty() {
                    return Ok(false);
                }
                streams
            }
            _ => return Ok(false),
        };

        // Extract message from nested structure
        // Format: [[stream_name, [[message_id, [field, value, ...]]]]]
        let (message_id, mut job) = match self.parse_stream_message(&messages) {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        tracing::info!(
            submission_id = %job.submission_id,
            message_id = %message_id,
            retry_count = job.retry_count,
            "Processing compilation job"
        );

        // Update status to compiling
        self.update_submission_status(&job.submission_id, "compiling")
            .await?;

        // Compile the submission
        let compile_result = self.compiler.compile(&job).await;

        match compile_result {
            Ok(binary_path) => {
                tracing::info!(
                    submission_id = %job.submission_id,
                    binary_path = %binary_path,
                    "Compilation successful"
                );

                // Update status and store binary path
                self.update_compilation_success(&job.submission_id, &binary_path)
                    .await?;

                // Queue for judging
                self.queue_for_judging(&job.submission_id, &binary_path)
                    .await?;

                // Acknowledge the message
                self.ack_message(&message_id).await?;
            }
            Err(e) => {
                let error_msg = e.to_string();

                // Check if this is a retryable error (e.g., timeout, infrastructure issue)
                let is_retryable = self.is_retryable_error(&error_msg);

                if is_retryable && job.retry_count < MAX_RETRIES {
                    // Retry with exponential backoff
                    job.retry_count += 1;
                    let delay_ms = BASE_RETRY_DELAY_MS * 2u64.pow(job.retry_count - 1);

                    tracing::warn!(
                        submission_id = %job.submission_id,
                        error = %error_msg,
                        retry_count = job.retry_count,
                        delay_ms = delay_ms,
                        "Retrying compilation after delay"
                    );

                    // Wait before retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

                    // Re-queue for retry
                    self.requeue_for_retry(&job).await?;

                    // Acknowledge original message
                    self.ack_message(&message_id).await?;
                } else if is_retryable && job.retry_count >= MAX_RETRIES {
                    // Move to dead letter queue
                    tracing::error!(
                        submission_id = %job.submission_id,
                        error = %error_msg,
                        retry_count = job.retry_count,
                        "Max retries exceeded, moving to dead letter queue"
                    );

                    self.move_to_dead_letter(&job, &error_msg).await?;
                    self.update_compilation_failure(
                        &job.submission_id,
                        &format!("Max retries exceeded: {}", error_msg),
                    )
                    .await?;

                    // Acknowledge original message
                    self.ack_message(&message_id).await?;
                } else {
                    // Non-retryable error (e.g., actual compilation error)
                    tracing::warn!(
                        submission_id = %job.submission_id,
                        error = %error_msg,
                        "Compilation failed (non-retryable)"
                    );

                    self.update_compilation_failure(&job.submission_id, &error_msg)
                        .await?;

                    // Acknowledge the message
                    self.ack_message(&message_id).await?;
                }
            }
        }

        Ok(true)
    }

    /// Check if an error is retryable.
    fn is_retryable_error(&self, error_msg: &str) -> bool {
        // Infrastructure/timeout errors are retryable
        let retryable_patterns = [
            "timed out",
            "connection refused",
            "no space left",
            "resource temporarily unavailable",
            "cannot allocate memory",
            "too many open files",
        ];

        let error_lower = error_msg.to_lowercase();
        retryable_patterns
            .iter()
            .any(|pattern| error_lower.contains(pattern))
    }

    /// Acknowledge a message in the stream.
    async fn ack_message(&self, message_id: &str) -> Result<()> {
        let mut conn = self.redis.get().await?;

        let _: i64 = redis::cmd("XACK")
            .arg(&self.config.compile_stream)
            .arg(&self.config.consumer_group)
            .arg(message_id)
            .query_async(&mut *conn)
            .await?;

        tracing::debug!(message_id = %message_id, "Message acknowledged");
        Ok(())
    }

    /// Re-queue a job for retry.
    async fn requeue_for_retry(&self, job: &CompileJob) -> Result<()> {
        let mut conn = self.redis.get().await?;

        let mut cmd = redis::cmd("XADD");
        cmd.arg(&self.config.compile_stream)
            .arg("*")
            .arg("submission_id")
            .arg(job.submission_id.to_string())
            .arg("type")
            .arg(&job.job_type)
            .arg("retry_count")
            .arg(job.retry_count.to_string());

        if let Some(ref file_path) = job.file_path {
            cmd.arg("file_path").arg(file_path);
        }

        if let Some(ref language) = job.language {
            cmd.arg("language").arg(language);
        }

        let _: String = cmd.query_async(&mut *conn).await?;

        tracing::debug!(
            submission_id = %job.submission_id,
            retry_count = job.retry_count,
            "Job re-queued for retry"
        );

        Ok(())
    }

    /// Move a job to the dead letter queue.
    async fn move_to_dead_letter(&self, job: &CompileJob, error: &str) -> Result<()> {
        let mut conn = self.redis.get().await?;
        let dead_letter_stream = format!("{}_dead_letter", self.config.compile_stream);

        let mut cmd = redis::cmd("XADD");
        cmd.arg(&dead_letter_stream)
            .arg("*")
            .arg("submission_id")
            .arg(job.submission_id.to_string())
            .arg("type")
            .arg(&job.job_type)
            .arg("retry_count")
            .arg(job.retry_count.to_string())
            .arg("error")
            .arg(error)
            .arg("failed_at")
            .arg(chrono::Utc::now().to_rfc3339());

        if let Some(ref file_path) = job.file_path {
            cmd.arg("file_path").arg(file_path);
        }

        if let Some(ref language) = job.language {
            cmd.arg("language").arg(language);
        }

        let stream_id: String = cmd.query_async(&mut *conn).await?;

        tracing::info!(
            submission_id = %job.submission_id,
            stream_id = %stream_id,
            "Moved to dead letter queue"
        );

        Ok(())
    }

    /// Parse a Redis stream message into job data.
    fn parse_stream_message(&self, messages: &[redis::Value]) -> Option<(String, CompileJob)> {
        // Structure: [[stream_name, [[message_id, [field, value, ...]]]]]
        let stream = messages.first()?;
        let stream_arr = match stream {
            redis::Value::Array(arr) => arr,
            _ => return None,
        };

        let msgs = stream_arr.get(1)?;
        let msgs_arr = match msgs {
            redis::Value::Array(arr) => arr,
            _ => return None,
        };

        let msg = msgs_arr.first()?;
        let msg_arr = match msg {
            redis::Value::Array(arr) => arr,
            _ => return None,
        };

        // Extract message ID
        let message_id = match msg_arr.first()? {
            redis::Value::BulkString(s) => String::from_utf8_lossy(s).to_string(),
            redis::Value::SimpleString(s) => s.clone(),
            _ => return None,
        };

        // Extract fields
        let fields = match msg_arr.get(1)? {
            redis::Value::Array(arr) => arr,
            _ => return None,
        };

        // Parse fields into HashMap
        let mut data: HashMap<String, String> = HashMap::new();
        let mut iter = fields.iter();
        while let (Some(key), Some(value)) = (iter.next(), iter.next()) {
            let k = match key {
                redis::Value::BulkString(s) => String::from_utf8_lossy(s).to_string(),
                redis::Value::SimpleString(s) => s.clone(),
                _ => continue,
            };
            let v = match value {
                redis::Value::BulkString(s) => String::from_utf8_lossy(s).to_string(),
                redis::Value::SimpleString(s) => s.clone(),
                _ => continue,
            };
            data.insert(k, v);
        }

        // Build CompileJob
        let submission_id = data.get("submission_id")?.parse().ok()?;
        let job_type = data.get("type").cloned().unwrap_or_else(|| "zip".to_string());
        let file_path = data.get("file_path").cloned();
        let language = data.get("language").cloned();
        let retry_count = data
            .get("retry_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Some((
            message_id,
            CompileJob {
                submission_id,
                job_type,
                file_path,
                language,
                retry_count,
            },
        ))
    }

    /// Update submission status in database.
    async fn update_submission_status(&self, submission_id: &Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE submissions SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(submission_id)
            .execute(&self.db)
            .await
            .context("Failed to update submission status")?;
        Ok(())
    }

    /// Update submission on successful compilation.
    async fn update_compilation_success(
        &self,
        submission_id: &Uuid,
        binary_path: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE submissions 
               SET status = 'compiled', 
                   compiled_at = NOW(),
                   file_path = COALESCE(file_path, $2)
               WHERE id = $1"#,
        )
        .bind(submission_id)
        .bind(binary_path)
        .execute(&self.db)
        .await
        .context("Failed to update compilation success")?;
        Ok(())
    }

    /// Update submission on compilation failure.
    async fn update_compilation_failure(
        &self,
        submission_id: &Uuid,
        error_message: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE submissions 
               SET status = 'compilation_error', 
                   compiled_at = NOW(),
                   compilation_log = $2
               WHERE id = $1"#,
        )
        .bind(submission_id)
        .bind(error_message)
        .execute(&self.db)
        .await
        .context("Failed to update compilation failure")?;
        Ok(())
    }

    /// Queue the compiled submission for judging.
    async fn queue_for_judging(&self, submission_id: &Uuid, binary_path: &str) -> Result<()> {
        let mut conn = self.redis.get().await?;

        let stream_id: String = redis::cmd("XADD")
            .arg(&self.config.run_stream)
            .arg("*")
            .arg("submission_id")
            .arg(submission_id.to_string())
            .arg("binary_path")
            .arg(binary_path)
            .query_async(&mut *conn)
            .await?;

        tracing::info!(
            submission_id = %submission_id,
            stream_id = %stream_id,
            "Queued for judging"
        );

        Ok(())
    }
}
