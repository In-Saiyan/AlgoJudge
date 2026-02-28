//! Redis Stream consumer for judge jobs

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use deadpool_redis::redis;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::executor::{ExecutionContext, Executor};
use crate::metrics::{self, ACTIVE_JOBS, JOBS_FAILED, JOBS_PROCESSED};
use crate::verdict::SubmissionResult;

/// Job payload – built from stream message + database lookup.
#[derive(Debug, Serialize, Deserialize)]
pub struct JudgeJob {
    pub submission_id: Uuid,
    pub problem_id: Uuid,
    pub contest_id: Option<Uuid>,
    pub time_limit_ms: u64,
    pub memory_limit_kb: u64,
    pub num_testcases: i32,
    #[serde(default)]
    pub retry_count: u32,
}

/// Row returned by the submission+problem DB lookup.
#[derive(Debug, sqlx::FromRow)]
struct SubmissionProblemRow {
    problem_id: Uuid,
    contest_id: Option<Uuid>,
    time_limit_ms: i32,
    memory_limit_kb: i32,
    num_test_cases: i32,
}

/// Judge consumer that processes jobs from Redis Stream
pub struct JudgeConsumer {
    config: Config,
    db_pool: PgPool,
    redis_pool: deadpool_redis::Pool,
    shutdown: Arc<AtomicBool>,
    executor: Executor,
}

impl JudgeConsumer {
    /// Create a new judge consumer
    pub fn new(
        config: Config,
        db_pool: PgPool,
        redis_pool: deadpool_redis::Pool,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        let executor = Executor::new(
            config.storage.clone(),
            config.execution.clone(),
        );

        Self {
            config,
            db_pool,
            redis_pool,
            shutdown,
            executor,
        }
    }

    /// Initialize consumer group
    pub async fn initialize(&self) -> Result<()> {
        let mut conn = self.redis_pool.get().await?;

        // Create consumer group (ignore error if already exists)
        let result: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.config.stream_name)
            .arg(&self.config.consumer_group)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut *conn)
            .await;

        match result {
            Ok(_) => {
                tracing::info!(
                    "Created consumer group '{}' on stream '{}'",
                    self.config.consumer_group,
                    self.config.stream_name
                );
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                tracing::debug!("Consumer group already exists");
            }
            Err(e) => {
                return Err(anyhow!("Failed to create consumer group: {}", e));
            }
        }

        Ok(())
    }

    /// Run the consumer loop
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!(
            "Starting judge consumer '{}' in group '{}'",
            self.config.worker_id,
            self.config.consumer_group
        );

        // First, claim any pending messages that may have been abandoned.
        // Tolerate NOGROUP here — the group will be re-created on first NOGROUP
        // inside the loop.
        if let Err(e) = self.claim_pending_messages().await {
            let msg = e.to_string();
            if msg.contains("NOGROUP") {
                tracing::warn!("Consumer group not found during pending-claim, re-initializing...");
                self.initialize().await?;
            } else {
                return Err(e);
            }
        }

        while !self.shutdown.load(Ordering::SeqCst) {
            match self.process_next_job().await {
                Ok(true) => {
                    // Job processed successfully
                }
                Ok(false) => {
                    // No job available, will retry after block timeout
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

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        tracing::info!("Judge consumer shutting down");
        Ok(())
    }

    /// Claim and reprocess any pending messages from dead consumers
    async fn claim_pending_messages(&self) -> Result<()> {
        let mut conn = self.redis_pool.get().await?;

        // Get pending messages older than 60 seconds
        let pending: Vec<(String, String, u64, u64)> = redis::cmd("XPENDING")
            .arg(&self.config.stream_name)
            .arg(&self.config.consumer_group)
            .arg("-")
            .arg("+")
            .arg(10)
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        for (message_id, _consumer, idle_time, _delivery_count) in pending {
            // Claim messages idle for more than 60 seconds
            if idle_time > 60000 {
                tracing::info!("Claiming abandoned message: {}", message_id);

                let _: Result<(), _> = redis::cmd("XCLAIM")
                    .arg(&self.config.stream_name)
                    .arg(&self.config.consumer_group)
                    .arg(&self.config.worker_id)
                    .arg(60000) // Min idle time
                    .arg(&message_id)
                    .query_async(&mut *conn)
                    .await;
            }
        }

        Ok(())
    }

    /// Process the next job from the stream
    async fn process_next_job(&self) -> Result<bool> {
        let mut conn = self.redis_pool.get().await?;

        // Read from stream with consumer group
        let result: Vec<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.config.consumer_group)
            .arg(&self.config.worker_id)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(self.config.block_timeout_ms)
            .arg("STREAMS")
            .arg(&self.config.stream_name)
            .arg(">")
            .query_async(&mut *conn)
            .await?;

        if result.is_empty() {
            return Ok(false);
        }

        // Parse minimal fields from the stream message
        let (message_id, submission_id, retry_count) = self.parse_stream_message(&result)?;

        // Look up problem_id and limits from the database
        let job = self.load_job_from_db(submission_id, retry_count).await?;

        tracing::info!(
            "Processing job for submission {} (problem: {}, message: {})",
            job.submission_id,
            job.problem_id,
            message_id
        );

        ACTIVE_JOBS.inc();

        // Process the job
        let result = self.judge_submission(&job).await;

        ACTIVE_JOBS.dec();

        match result {
            Ok(submission_result) => {
                // Update database with results
                self.save_results(&job, &submission_result).await?;

                // Record metrics
                JOBS_PROCESSED.inc();
                metrics::record_verdict(submission_result.verdict.code());

                // Acknowledge the message
                self.ack_message(&message_id).await?;

                tracing::info!(
                    "Submission {} judged: {} ({}/{} passed)",
                    job.submission_id,
                    submission_result.verdict.code(),
                    submission_result.passed_count,
                    submission_result.total_count
                );
            }
            Err(e) if e.to_string() == "QUEUE_PENDING" => {
                // Binaries not ready — submission marked as queue_pending.
                // ACK the message without retry or dead-letter.
                self.ack_message(&message_id).await?;

                tracing::info!(
                    "Submission {} deferred (queue_pending) — binaries not ready",
                    job.submission_id,
                );
            }
            Err(e) => {
                tracing::error!("Failed to judge submission {}: {}", job.submission_id, e);
                JOBS_FAILED.inc();

                // Handle retry or dead letter
                if job.retry_count < self.config.max_retries {
                    self.retry_job(&job, &e.to_string()).await?;
                } else {
                    self.send_to_dead_letter(&job, &e.to_string()).await?;
                }

                // Acknowledge original message
                self.ack_message(&message_id).await?;
            }
        }

        Ok(true)
    }

    /// Parse Redis stream message into minimal fields.
    /// Only `submission_id` is required from the stream; all other job
    /// metadata is looked up from the database via `load_job_from_db`.
    fn parse_stream_message(&self, result: &[redis::Value]) -> Result<(String, Uuid, u32)> {
        // XREADGROUP returns: [[stream_name, [[message_id, [field, value, ...]]]]]
        let stream_data = match result.first() {
            Some(redis::Value::Array(data)) => data,
            _ => return Err(anyhow!("Invalid stream response format")),
        };

        let messages = match stream_data.get(1) {
            Some(redis::Value::Array(msgs)) => msgs,
            _ => return Err(anyhow!("No messages in response")),
        };

        let message = match messages.first() {
            Some(redis::Value::Array(msg)) => msg,
            _ => return Err(anyhow!("No message data")),
        };

        let message_id = match message.first() {
            Some(redis::Value::BulkString(id)) => String::from_utf8_lossy(id).to_string(),
            _ => return Err(anyhow!("Invalid message ID")),
        };

        let fields = match message.get(1) {
            Some(redis::Value::Array(f)) => f,
            _ => return Err(anyhow!("No message fields")),
        };

        // Parse fields into a map
        let mut field_map = std::collections::HashMap::new();
        for chunk in fields.chunks(2) {
            if let [redis::Value::BulkString(key), redis::Value::BulkString(value)] = chunk {
                field_map.insert(
                    String::from_utf8_lossy(key).to_string(),
                    String::from_utf8_lossy(value).to_string(),
                );
            }
        }

        let submission_id: Uuid = field_map
            .get("submission_id")
            .ok_or_else(|| anyhow!("Missing submission_id"))?
            .parse()?;

        let retry_count: u32 = field_map
            .get("retry_count")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        Ok((message_id, submission_id, retry_count))
    }

    /// Look up submission and problem metadata from the database.
    async fn load_job_from_db(&self, submission_id: Uuid, retry_count: u32) -> Result<JudgeJob> {
        let row = sqlx::query_as::<_, SubmissionProblemRow>(
            r#"
            SELECT s.problem_id, s.contest_id,
                   p.time_limit_ms, p.memory_limit_kb, p.num_test_cases
            FROM submissions s
            JOIN problems p ON p.id = s.problem_id
            WHERE s.id = $1
            "#,
        )
        .bind(submission_id)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| anyhow!("Submission {} not found in database", submission_id))?;

        Ok(JudgeJob {
            submission_id,
            problem_id: row.problem_id,
            contest_id: row.contest_id,
            time_limit_ms: row.time_limit_ms as u64,
            memory_limit_kb: row.memory_limit_kb as u64,
            num_testcases: row.num_test_cases,
            retry_count,
        })
    }

    /// Judge a submission
    async fn judge_submission(&self, job: &JudgeJob) -> Result<SubmissionResult> {
        // Check if generator and checker binaries exist for this problem.
        // If either is missing, the problem is not yet ready for judging.
        // Mark submission as "queue_pending" and return without error.
        if !self.problem_binaries_ready(job.problem_id).await {
            tracing::info!(
                submission_id = %job.submission_id,
                problem_id = %job.problem_id,
                "Problem binaries (generator/checker) not ready — setting queue_pending"
            );

            sqlx::query(
                "UPDATE submissions SET status = 'queue_pending' WHERE id = $1",
            )
            .bind(job.submission_id)
            .execute(&self.db_pool)
            .await?;

            // Return a sentinel result so the caller knows to ACK without recording
            // test-case results.
            return Err(anyhow!("QUEUE_PENDING"));
        }

        // Update status to judging
        sqlx::query(
            "UPDATE submissions SET status = 'judging', judged_at = NOW() WHERE id = $1",
        )
        .bind(job.submission_id)
        .execute(&self.db_pool)
        .await?;

        let ctx = ExecutionContext {
            submission_id: job.submission_id,
            problem_id: job.problem_id,
            contest_id: job.contest_id,
            time_limit_ms: job.time_limit_ms,
            memory_limit_kb: job.memory_limit_kb,
            num_testcases: job.num_testcases,
        };

        // Execute and judge
        let result = self.executor.execute(&ctx).await?;

        // Record execution metrics
        metrics::record_execution(
            &job.problem_id.to_string(),
            result.max_time_ms as f64 / 1000.0,
            result.max_memory_kb * 1024,
        );

        Ok(result)
    }

    /// Save judging results to database
    async fn save_results(&self, job: &JudgeJob, result: &SubmissionResult) -> Result<()> {
        // Update submission status
        sqlx::query(
            r#"
            UPDATE submissions 
            SET status = $1, 
                score = $2,
                max_time_ms = $3,
                max_memory_kb = $4,
                passed_test_cases = $5,
                total_test_cases = $6,
                judged_at = NOW()
            WHERE id = $7
            "#,
        )
        .bind(result.verdict.to_db_string())
        .bind(result.score)
        .bind(result.max_time_ms as i64)
        .bind(result.max_memory_kb as i64)
        .bind(result.passed_count)
        .bind(result.total_count)
        .bind(job.submission_id)
        .execute(&self.db_pool)
        .await?;

        // Insert individual test case results
        for tc in &result.testcase_results {
            sqlx::query(
                r#"
                INSERT INTO submission_results 
                (submission_id, test_case_number, verdict, time_ms, memory_kb, checker_output)
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (submission_id, test_case_number) 
                DO UPDATE SET 
                    verdict = EXCLUDED.verdict,
                    time_ms = EXCLUDED.time_ms,
                    memory_kb = EXCLUDED.memory_kb,
                    checker_output = EXCLUDED.checker_output
                "#,
            )
            .bind(job.submission_id)
            .bind(tc.testcase_number)
            .bind(tc.verdict.to_db_string())
            .bind(tc.time_ms as i64)
            .bind(tc.memory_kb as i64)
            .bind(tc.error_message.clone())
            .execute(&self.db_pool)
            .await?;
        }

        Ok(())
    }

    /// Acknowledge a message
    async fn ack_message(&self, message_id: &str) -> Result<()> {
        let mut conn = self.redis_pool.get().await?;

        redis::cmd("XACK")
            .arg(&self.config.stream_name)
            .arg(&self.config.consumer_group)
            .arg(message_id)
            .query_async::<i64>(&mut *conn)
            .await?;

        Ok(())
    }

    /// Retry a failed job
    async fn retry_job(&self, job: &JudgeJob, error: &str) -> Result<()> {
        let mut conn = self.redis_pool.get().await?;

        tracing::warn!(
            "Retrying job for submission {} (attempt {}/{}): {}",
            job.submission_id,
            job.retry_count + 1,
            self.config.max_retries,
            error
        );

        // Add back to stream with incremented retry count
        redis::cmd("XADD")
            .arg(&self.config.stream_name)
            .arg("*")
            .arg("submission_id")
            .arg(job.submission_id.to_string())
            .arg("problem_id")
            .arg(job.problem_id.to_string())
            .arg("contest_id")
            .arg(job.contest_id.map(|id| id.to_string()).unwrap_or_default())
            .arg("time_limit_ms")
            .arg(job.time_limit_ms.to_string())
            .arg("memory_limit_kb")
            .arg(job.memory_limit_kb.to_string())
            .arg("num_testcases")
            .arg(job.num_testcases.to_string())
            .arg("retry_count")
            .arg((job.retry_count + 1).to_string())
            .query_async::<String>(&mut *conn)
            .await?;

        Ok(())
    }

    /// Send job to dead letter queue
    async fn send_to_dead_letter(&self, job: &JudgeJob, error: &str) -> Result<()> {
        let mut conn = self.redis_pool.get().await?;

        tracing::error!(
            "Sending job to dead letter queue: submission {} - {}",
            job.submission_id,
            error
        );

        // Add to dead letter stream
        redis::cmd("XADD")
            .arg("run_queue_dlq")
            .arg("*")
            .arg("submission_id")
            .arg(job.submission_id.to_string())
            .arg("problem_id")
            .arg(job.problem_id.to_string())
            .arg("contest_id")
            .arg(job.contest_id.map(|id| id.to_string()).unwrap_or_default())
            .arg("error")
            .arg(error)
            .arg("retry_count")
            .arg(job.retry_count.to_string())
            .arg("failed_at")
            .arg(chrono::Utc::now().to_rfc3339())
            .query_async::<String>(&mut *conn)
            .await?;

        // Update submission status to system_error
        sqlx::query(
            r#"
            UPDATE submissions 
            SET status = 'system_error',
                compilation_log = $1,
                judged_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(error)
        .bind(job.submission_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Check whether both generator and checker binaries exist for a problem.
    async fn problem_binaries_ready(&self, problem_id: Uuid) -> bool {
        let base = self
            .executor
            .storage_config()
            .problem_binaries_path
            .join(problem_id.to_string());

        let generator_exists = base.join("generator").exists();
        let checker_exists = base.join("checker").exists();

        generator_exists && checker_exists
    }
}
