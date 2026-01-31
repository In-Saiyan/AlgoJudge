//! Benchmark runner - Orchestrates the benchmarking process

use bollard::Docker;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::Config,
    constants::verdicts,
    db::repositories::{ProblemRepository, SubmissionRepository},
    error::AppResult,
    models::{BenchmarkResult, BenchmarkRun, Submission},
};

use super::{container::ContainerManager, languages::LanguageHandler, metrics::MetricsCollector};

/// Benchmark runner that processes submission queue
pub struct BenchmarkRunner {
    docker: Docker,
    pool: PgPool,
    redis: ConnectionManager,
    config: Config,
    container_manager: ContainerManager,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new(docker: Docker, pool: PgPool, redis: ConnectionManager, config: Config) -> Self {
        let container_manager = ContainerManager::new(docker.clone(), config.clone());

        Self {
            docker,
            pool,
            redis,
            config,
            container_manager,
        }
    }

    /// Start processing the submission queue
    pub async fn start(&mut self) -> AppResult<()> {
        tracing::info!("Starting benchmark runner");

        loop {
            // Pop from queue (blocking with timeout)
            let result: Option<(String, String)> = self
                .redis
                .brpop("judge_queue", 5.0)
                .await
                .unwrap_or(None);

            if let Some((_, submission_id_str)) = result {
                let submission_id = match Uuid::parse_str(&submission_id_str) {
                    Ok(id) => id,
                    Err(_) => {
                        tracing::error!("Invalid submission ID in queue: {}", submission_id_str);
                        continue;
                    }
                };

                if let Err(e) = self.process_submission(&submission_id).await {
                    tracing::error!("Failed to process submission {}: {}", submission_id, e);

                    // Update submission status to internal error
                    let _ = SubmissionRepository::update_verdict(
                        &self.pool,
                        &submission_id,
                        verdicts::INTERNAL_ERROR,
                        None,
                        None,
                        None,
                        Some(&format!("Internal error: {}", e)),
                    )
                    .await;
                }
            }
        }
    }

    /// Process a single submission
    async fn process_submission(&mut self, submission_id: &Uuid) -> AppResult<()> {
        tracing::info!("Processing submission: {}", submission_id);

        // Get submission
        let submission = SubmissionRepository::find_by_id(&self.pool, submission_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Submission not found"))?;

        // Update status to compiling
        SubmissionRepository::update_verdict(
            &self.pool,
            submission_id,
            verdicts::COMPILING,
            None,
            None,
            None,
            None,
        )
        .await?;

        // Get language handler
        let language_handler = LanguageHandler::for_language(&submission.language)?;

        // Create container for compilation
        let container_id = self
            .container_manager
            .create_container(submission_id, &submission.language)
            .await?;

        // Compile the solution
        let compile_result = self
            .container_manager
            .compile(&container_id, &submission.source_code, &language_handler)
            .await;

        match compile_result {
            Ok(executable_path) => {
                // Update status to running
                SubmissionRepository::update_verdict(
                    &self.pool,
                    submission_id,
                    verdicts::RUNNING,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;

                // Get test cases
                let test_cases =
                    ProblemRepository::get_test_cases(&self.pool, &submission.problem_id).await?;

                // Run against all test cases
                let (verdict, max_time, max_memory, score) = self
                    .run_test_cases(&container_id, &executable_path, &submission, &test_cases)
                    .await?;

                // Clean up container
                self.container_manager.remove_container(&container_id).await?;

                // Update final verdict
                SubmissionRepository::update_verdict(
                    &self.pool,
                    submission_id,
                    &verdict,
                    Some(max_time),
                    Some(max_memory),
                    Some(score),
                    None,
                )
                .await?;
            }
            Err(e) => {
                // Compilation error
                self.container_manager.remove_container(&container_id).await?;

                SubmissionRepository::update_verdict(
                    &self.pool,
                    submission_id,
                    verdicts::COMPILATION_ERROR,
                    None,
                    None,
                    None,
                    Some(&e.to_string()),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Run solution against test cases
    async fn run_test_cases(
        &mut self,
        container_id: &str,
        executable_path: &str,
        submission: &Submission,
        test_cases: &[crate::models::TestCase],
    ) -> AppResult<(String, i32, i32, i32)> {
        let mut max_time = 0i32;
        let mut max_memory = 0i32;
        let mut total_points = 0i32;
        let mut max_points = 0i32;
        let mut all_passed = true;
        let mut final_verdict = verdicts::ACCEPTED.to_string();

        // Get time/memory limits from problem
        let problem =
            ProblemRepository::find_by_id(&self.pool, &submission.problem_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Problem not found"))?;

        let time_limit = problem.time_limit_ms as i32;
        let memory_limit = problem.memory_limit_kb as i32;

        for test_case in test_cases {
            max_points += test_case.points.unwrap_or(100);

            // Run multiple iterations for benchmarking
            let mut runs: Vec<BenchmarkRun> = Vec::new();

            for iteration in 0..self.config.benchmark.iterations {
                // Skip first iteration (warm-up) in final results
                let is_warmup = iteration == 0;

                let run_result = self
                    .container_manager
                    .run_with_input(
                        container_id,
                        executable_path,
                        &test_case.input,
                        time_limit,
                        memory_limit,
                    )
                    .await?;

                if !is_warmup {
                    runs.push(BenchmarkRun {
                        iteration: iteration as u32,
                        wall_time_ms: run_result.wall_time_ms,
                        cpu_time_ms: run_result.cpu_time_ms,
                        memory_kb: run_result.memory_kb,
                        is_outlier: false, // Will be determined later
                    });
                }

                // Check verdict (only need to check once, not for benchmarking)
                if iteration == 0 {
                    let tc_verdict = self.evaluate_output(
                        &run_result.stdout,
                        &test_case.expected_output,
                        run_result.exit_code,
                        run_result.wall_time_ms,
                        run_result.memory_kb,
                        time_limit,
                        memory_limit,
                    );

                    // Store test case result
                    self.store_test_case_result(
                        &submission.id,
                        &test_case.id,
                        &tc_verdict,
                        run_result.wall_time_ms as i32,
                        run_result.memory_kb as i32,
                        Some(&run_result.stdout),
                        run_result.stderr.as_deref(),
                    )
                    .await?;

                    if tc_verdict != verdicts::ACCEPTED {
                        all_passed = false;
                        if final_verdict == verdicts::ACCEPTED {
                            final_verdict = tc_verdict.clone();
                        }
                    } else {
                        total_points += test_case.points.unwrap_or(100);
                    }
                }
            }

            // Calculate benchmark results
            if !runs.is_empty() {
                let benchmark_result = BenchmarkResult::from_runs(runs.clone());

                max_time = max_time.max(benchmark_result.time_avg_ms as i32);
                max_memory = max_memory.max(benchmark_result.memory_avg_kb as i32);

                // Store benchmark results
                self.store_benchmark_result(&submission.id, &test_case.id, &benchmark_result)
                    .await?;
            }
        }

        // Calculate final score (percentage of points earned)
        let score = if max_points > 0 {
            (total_points * 100) / max_points
        } else {
            if all_passed { 100 } else { 0 }
        };

        Ok((final_verdict, max_time, max_memory, score))
    }

    /// Evaluate output and determine verdict
    fn evaluate_output(
        &self,
        actual: &str,
        expected: &str,
        exit_code: i32,
        time_ms: f64,
        memory_kb: i64,
        time_limit: i32,
        memory_limit: i32,
    ) -> String {
        // Check for runtime error
        if exit_code != 0 {
            return verdicts::RUNTIME_ERROR.to_string();
        }

        // Check time limit
        if time_ms > time_limit as f64 {
            return verdicts::TIME_LIMIT_EXCEEDED.to_string();
        }

        // Check memory limit
        if memory_kb > memory_limit as i64 {
            return verdicts::MEMORY_LIMIT_EXCEEDED.to_string();
        }

        // Compare output (normalize whitespace)
        let actual_normalized = actual.trim().replace("\r\n", "\n");
        let expected_normalized = expected.trim().replace("\r\n", "\n");

        if actual_normalized == expected_normalized {
            verdicts::ACCEPTED.to_string()
        } else {
            verdicts::WRONG_ANSWER.to_string()
        }
    }

    /// Store test case result in database
    async fn store_test_case_result(
        &self,
        submission_id: &Uuid,
        test_case_id: &Uuid,
        verdict: &str,
        execution_time_ms: i32,
        memory_usage_kb: i32,
        actual_output: Option<&str>,
        error_message: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO test_case_results 
                (submission_id, test_case_id, verdict, execution_time_ms, memory_usage_kb, actual_output, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(submission_id)
        .bind(test_case_id)
        .bind(verdict)
        .bind(execution_time_ms)
        .bind(memory_usage_kb)
        .bind(actual_output)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Store benchmark result in database
    async fn store_benchmark_result(
        &self,
        submission_id: &Uuid,
        test_case_id: &Uuid,
        result: &BenchmarkResult,
    ) -> AppResult<()> {
        let outliers_json = serde_json::to_value(&result.outliers).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO benchmark_results 
                (submission_id, test_case_id, iterations, time_avg_ms, time_median_ms, 
                 time_min_ms, time_max_ms, time_stddev_ms, memory_avg_kb, memory_peak_kb, time_outliers)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(submission_id)
        .bind(test_case_id)
        .bind(result.iterations as i32)
        .bind(result.time_avg_ms)
        .bind(result.time_median_ms)
        .bind(result.time_min_ms)
        .bind(result.time_max_ms)
        .bind(result.time_stddev_ms)
        .bind(result.memory_avg_kb)
        .bind(result.memory_peak_kb)
        .bind(outliers_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Result from running a program
#[derive(Debug)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: Option<String>,
    pub exit_code: i32,
    pub wall_time_ms: f64,
    pub cpu_time_ms: f64,
    pub memory_kb: i64,
}
