//! Algorithmic benchmark runner - Handles ZIP-based submissions
//!
//! Workflow:
//! 1. Extract ZIP (must contain compile.sh and run.sh)
//! 2. Run compile.sh in the runtime container
//! 3. Verify target binary exists (named after problem code: A, B, etc.)
//! 4. Generate test cases using problem's generator
//! 5. Run solution binary on each test case
//! 6. Verify output using problem's verifier
//! 7. Collect metrics and update submission

use std::io::{Cursor, Read};
use std::path::Path;

use bollard::Docker;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    config::Config,
    db::repositories::{ProblemRepository, SubmissionRepository},
    error::{AppError, AppResult},
    models::{Problem, Runtime, Submission, Verdict},
};

use super::container::ContainerManager;

/// Required files in submission ZIP
const COMPILE_SCRIPT: &str = "compile.sh";
const RUN_SCRIPT: &str = "run.sh";

/// Result of running a single test case
#[derive(Debug)]
pub struct TestCaseRunResult {
    pub test_case_number: i32,
    pub verdict: Verdict,
    pub execution_time_ms: f64,
    pub memory_usage_kb: i64,
    pub match_percentage: Option<f64>,
    pub verifier_output: Option<String>,
    pub error_message: Option<String>,
}

/// Algorithmic benchmark runner
pub struct AlgoBenchmarkRunner {
    docker: Docker,
    pool: PgPool,
    redis: ConnectionManager,
    config: Config,
    container_manager: ContainerManager,
}

impl AlgoBenchmarkRunner {
    /// Create a new algorithmic benchmark runner
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
        info!("Starting algorithmic benchmark runner");

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
                        error!("Invalid submission ID in queue: {}", submission_id_str);
                        continue;
                    }
                };

                if let Err(e) = self.process_submission(&submission_id).await {
                    error!("Failed to process submission {}: {}", submission_id, e);
                    self.update_verdict(&submission_id, Verdict::SystemError, Some(&format!("{}", e))).await;
                }
            }
        }
    }

    /// Process a single ZIP-based submission
    async fn process_submission(&mut self, submission_id: &Uuid) -> AppResult<()> {
        info!(submission_id = %submission_id, "Processing submission");

        // Get submission
        let submission = SubmissionRepository::find_by_id(&self.pool, submission_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Submission not found".to_string()))?;

        // Get problem
        let problem = ProblemRepository::find_by_id(&self.pool, &submission.problem_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Problem not found".to_string()))?;

        // Validate ZIP format
        let zip_data = submission.submission_zip.as_ref()
            .ok_or_else(|| AppError::Validation("No submission ZIP provided".to_string()))?;

        debug!(submission_id = %submission_id, "Validating ZIP format");
        if let Err(e) = self.validate_zip_format(zip_data) {
            self.update_verdict(submission_id, Verdict::InvalidFormat, Some(&e.to_string())).await;
            return Ok(());
        }

        // Get runtime
        let runtime = self.get_runtime(&submission).await?;

        // Create container
        let container_id = self
            .container_manager
            .create_container(submission_id, &runtime.name)
            .await?;

        // Extract and copy ZIP contents to container
        debug!(submission_id = %submission_id, "Extracting ZIP to container");
        self.container_manager
            .copy_zip_to_container(&container_id, zip_data)
            .await?;

        // Copy generator and verifier binaries
        if let Some(ref generator) = problem.generator_binary {
            debug!(submission_id = %submission_id, "Copying generator binary");
            self.container_manager
                .copy_binary_to_container(
                    &container_id,
                    generator,
                    problem.generator_filename.as_deref().unwrap_or("generator"),
                )
                .await?;
        }

        if let Some(ref verifier) = problem.verifier_binary {
            debug!(submission_id = %submission_id, "Copying verifier binary");
            self.container_manager
                .copy_binary_to_container(
                    &container_id,
                    verifier,
                    problem.verifier_filename.as_deref().unwrap_or("verifier"),
                )
                .await?;
        }

        // Update status to compiling
        self.update_verdict(submission_id, Verdict::Compiling, None).await;

        // Run compile.sh
        debug!(submission_id = %submission_id, "Running compile.sh");
        let compile_result = self
            .container_manager
            .run_script(&container_id, COMPILE_SCRIPT, problem.time_limit_ms as u64 * 2)
            .await;

        match compile_result {
            Ok(output) => {
                debug!(submission_id = %submission_id, "Compilation output: {}", output);
            }
            Err(e) => {
                warn!(submission_id = %submission_id, "Compilation failed: {}", e);
                self.container_manager.remove_container(&container_id).await?;
                self.update_verdict_with_output(
                    submission_id,
                    Verdict::CompilationError,
                    Some(&e.to_string()),
                ).await;
                return Ok(());
            }
        }

        // Check if target binary exists (named after problem code)
        let binary_name = problem.problem_code.as_deref().unwrap_or("solution");
        debug!(submission_id = %submission_id, binary = %binary_name, "Checking for target binary");
        
        if !self.container_manager.file_exists(&container_id, binary_name).await? {
            warn!(submission_id = %submission_id, "Target binary not found: {}", binary_name);
            self.container_manager.remove_container(&container_id).await?;
            self.update_verdict_with_output(
                submission_id,
                Verdict::CompilationError,
                Some(&format!(
                    "Target binary '{}' not found after compilation. \
                    Make sure compile.sh produces an executable named '{}'.",
                    binary_name, binary_name
                )),
            ).await;
            return Ok(());
        }

        // Update status to running
        self.update_verdict(submission_id, Verdict::Running, None).await;

        // Generate test cases and run solution
        let results = self
            .run_all_test_cases(&container_id, &submission, &problem, binary_name)
            .await?;

        // Clean up container
        self.container_manager.remove_container(&container_id).await?;

        // Calculate final verdict and stats
        let (final_verdict, max_time, max_memory, score, match_pct) = 
            self.calculate_final_results(&results, &problem);

        // Save test case results
        for result in &results {
            SubmissionRepository::save_test_case_result(
                &self.pool,
                submission_id,
                result.test_case_number,
                result.verdict.as_str(),
                Some(result.execution_time_ms),
                Some(result.memory_usage_kb),
                result.match_percentage,
                result.verifier_output.as_deref(),
                result.error_message.as_deref(),
            ).await?;
        }

        // Update final submission verdict
        SubmissionRepository::update_verdict(
            &self.pool,
            submission_id,
            final_verdict.as_str(),
            Some(max_time as i32),
            Some(max_memory as i32),
            Some(score),
            None,
        ).await?;

        info!(
            submission_id = %submission_id,
            verdict = %final_verdict,
            time_ms = max_time,
            memory_kb = max_memory,
            score = score,
            "Submission judged"
        );

        Ok(())
    }

    /// Validate ZIP contains required files
    fn validate_zip_format(&self, zip_data: &[u8]) -> AppResult<()> {
        let cursor = Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| AppError::Validation(format!("Invalid ZIP file: {}", e)))?;

        let mut has_compile = false;
        let mut has_run = false;

        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .map_err(|e| AppError::Validation(format!("Cannot read ZIP entry: {}", e)))?;
            
            let name = file.name();
            if name == COMPILE_SCRIPT || name.ends_with(&format!("/{}", COMPILE_SCRIPT)) {
                has_compile = true;
            }
            if name == RUN_SCRIPT || name.ends_with(&format!("/{}", RUN_SCRIPT)) {
                has_run = true;
            }
        }

        if !has_compile {
            return Err(AppError::Validation(
                format!("ZIP must contain '{}' script", COMPILE_SCRIPT)
            ));
        }

        if !has_run {
            return Err(AppError::Validation(
                format!("ZIP must contain '{}' script", RUN_SCRIPT)
            ));
        }

        Ok(())
    }

    /// Get runtime for submission
    async fn get_runtime(&self, submission: &Submission) -> AppResult<Runtime> {
        // TODO: Fetch from database using runtime_id
        // For now, create a default based on language
        Ok(Runtime {
            id: Uuid::new_v4(),
            name: submission.language.clone(),
            display_name: submission.language.clone(),
            docker_image: format!("algojudge/{}", submission.language),
            default_compile_cmd: None,
            default_run_cmd: None,
            is_active: true,
            created_at: chrono::Utc::now(),
        })
    }

    /// Run solution on all generated test cases
    async fn run_all_test_cases(
        &self,
        container_id: &str,
        submission: &Submission,
        problem: &Problem,
        binary_name: &str,
    ) -> AppResult<Vec<TestCaseRunResult>> {
        let mut results = Vec::new();
        let num_test_cases = problem.num_test_cases;

        for tc_num in 1..=num_test_cases {
            debug!(test_case = tc_num, "Generating test case");
            
            // Generate test case using generator
            let generator_name = problem.generator_filename.as_deref().unwrap_or("generator");
            let generate_cmd = format!("./{} {} > testcase{}.txt", generator_name, tc_num, tc_num);
            
            if let Err(e) = self.container_manager
                .run_command(container_id, &generate_cmd, problem.time_limit_ms as u64)
                .await
            {
                results.push(TestCaseRunResult {
                    test_case_number: tc_num,
                    verdict: Verdict::SystemError,
                    execution_time_ms: 0.0,
                    memory_usage_kb: 0,
                    match_percentage: None,
                    verifier_output: None,
                    error_message: Some(format!("Test case generation failed: {}", e)),
                });
                continue;
            }

            // Run solution using run.sh
            debug!(test_case = tc_num, "Running solution");
            let run_result = self
                .container_manager
                .run_with_metrics(
                    container_id,
                    &format!("./run.sh < testcase{}.txt > output{}.txt", tc_num, tc_num),
                    problem.time_limit_ms as u64,
                    problem.memory_limit_kb as u64,
                )
                .await;

            let (exec_time, memory, run_verdict, run_error) = match run_result {
                Ok(metrics) => (metrics.time_ms, metrics.memory_kb, None, None),
                Err(e) => {
                    let verdict = if e.to_string().contains("time limit") {
                        Verdict::TimeLimitExceeded
                    } else if e.to_string().contains("memory limit") {
                        Verdict::MemoryLimitExceeded
                    } else {
                        Verdict::RuntimeError
                    };
                    (0.0, 0, Some(verdict), Some(e.to_string()))
                }
            };

            // If runtime error, record and continue
            if let Some(verdict) = run_verdict {
                results.push(TestCaseRunResult {
                    test_case_number: tc_num,
                    verdict,
                    execution_time_ms: exec_time,
                    memory_usage_kb: memory,
                    match_percentage: None,
                    verifier_output: None,
                    error_message: run_error,
                });
                continue;
            }

            // Verify output using verifier
            debug!(test_case = tc_num, "Verifying output");
            let verifier_name = problem.verifier_filename.as_deref().unwrap_or("verifier");
            let verify_cmd = format!(
                "./{} testcase{}.txt output{}.txt",
                verifier_name, tc_num, tc_num
            );

            let verify_result = self
                .container_manager
                .run_command(container_id, &verify_cmd, problem.time_limit_ms as u64)
                .await;

            let (verdict, match_pct, verifier_output) = match verify_result {
                Ok(output) => self.parse_verifier_output(&output),
                Err(e) => (Verdict::WrongAnswer, Some(0.0), Some(e.to_string())),
            };

            results.push(TestCaseRunResult {
                test_case_number: tc_num,
                verdict,
                execution_time_ms: exec_time,
                memory_usage_kb: memory,
                match_percentage: match_pct,
                verifier_output,
                error_message: None,
            });
        }

        Ok(results)
    }

    /// Parse verifier output to extract verdict and match percentage
    /// Expected format: "ACCEPTED" or "ACCEPTED 100.0" or "WRONG_ANSWER 45.5"
    fn parse_verifier_output(&self, output: &str) -> (Verdict, Option<f64>, Option<String>) {
        let output = output.trim();
        let parts: Vec<&str> = output.split_whitespace().collect();
        
        let verdict = match parts.first().map(|s| s.to_uppercase()).as_deref() {
            Some("ACCEPTED") | Some("AC") => Verdict::Accepted,
            Some("PARTIAL") => Verdict::Partial,
            _ => Verdict::WrongAnswer,
        };

        let match_pct = parts.get(1)
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| if verdict == Verdict::Accepted { Some(100.0) } else { None });

        (verdict, match_pct, Some(output.to_string()))
    }

    /// Calculate final results from all test case results
    fn calculate_final_results(
        &self,
        results: &[TestCaseRunResult],
        problem: &Problem,
    ) -> (Verdict, f64, i64, i32, Option<f64>) {
        if results.is_empty() {
            return (Verdict::SystemError, 0.0, 0, 0, None);
        }

        let mut max_time = 0.0f64;
        let mut max_memory = 0i64;
        let mut total_match = 0.0f64;
        let mut accepted_count = 0;

        for result in results {
            max_time = max_time.max(result.execution_time_ms);
            max_memory = max_memory.max(result.memory_usage_kb);
            
            if result.verdict == Verdict::Accepted {
                accepted_count += 1;
                total_match += result.match_percentage.unwrap_or(100.0);
            } else if result.verdict == Verdict::Partial {
                total_match += result.match_percentage.unwrap_or(0.0);
            }
        }

        let avg_match = if !results.is_empty() {
            total_match / results.len() as f64
        } else {
            0.0
        };

        // Calculate score (0-100)
        let score = ((accepted_count as f64 / results.len() as f64) * 100.0) as i32;

        // Determine final verdict
        let final_verdict = if accepted_count == results.len() {
            Verdict::Accepted
        } else if accepted_count > 0 || avg_match > 0.0 {
            Verdict::Partial
        } else {
            // Return the first non-accepted verdict
            results.iter()
                .find(|r| r.verdict != Verdict::Accepted)
                .map(|r| r.verdict)
                .unwrap_or(Verdict::WrongAnswer)
        };

        (final_verdict, max_time, max_memory, score, Some(avg_match))
    }

    async fn update_verdict(&self, submission_id: &Uuid, verdict: Verdict, error: Option<&str>) {
        let _ = SubmissionRepository::update_verdict(
            &self.pool,
            submission_id,
            verdict.as_str(),
            None,
            None,
            None,
            error,
        ).await;
    }

    async fn update_verdict_with_output(&self, submission_id: &Uuid, verdict: Verdict, output: Option<&str>) {
        let _ = SubmissionRepository::update_verdict(
            &self.pool,
            submission_id,
            verdict.as_str(),
            None,
            None,
            None,
            output,
        ).await;
    }
}
