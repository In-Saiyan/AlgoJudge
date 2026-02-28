//! Sandboxed executor for user submissions

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use anyhow::{anyhow, Result};
use tokio::fs;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::config::{ExecutionConfig, StorageConfig};
use crate::testcase::{CheckerResult, TestCase, TestCaseManager};
use crate::verdict::{SubmissionResult, TestCaseResult, Verdict};

/// Execution context for a submission
pub struct ExecutionContext {
    /// Submission ID
    pub submission_id: Uuid,
    /// Problem ID
    pub problem_id: Uuid,
    /// Contest ID (None for standalone submissions)
    pub contest_id: Option<Uuid>,
    /// Time limit in milliseconds
    pub time_limit_ms: u64,
    /// Memory limit in KB
    pub memory_limit_kb: u64,
    /// Number of test cases
    pub num_testcases: i32,
}

/// Sandboxed executor
pub struct Executor {
    storage: StorageConfig,
    execution: ExecutionConfig,
    testcase_manager: TestCaseManager,
}

impl Executor {
    /// Create a new executor
    pub fn new(storage: StorageConfig, execution: ExecutionConfig) -> Self {
        let testcase_manager = TestCaseManager::new(storage.clone(), execution.clone());
        Self {
            storage,
            execution,
            testcase_manager,
        }
    }

    /// Expose the storage config for binary path lookups.
    pub fn storage_config(&self) -> &StorageConfig {
        &self.storage
    }

    /// Execute a submission against all test cases
    pub async fn execute(&self, ctx: &ExecutionContext) -> Result<SubmissionResult> {
        // Get path to compiled binary
        let binary_path = self
            .storage
            .binaries_path
            .join(format!("{}_bin", ctx.submission_id));

        if !binary_path.exists() {
            return Err(anyhow!(
                "Binary not found for submission {}",
                ctx.submission_id
            ));
        }

        // Create temp directory for this execution
        let temp_dir = self
            .storage
            .temp_path
            .join(ctx.submission_id.to_string());
        fs::create_dir_all(&temp_dir).await?;

        // Ensure binary (or run.sh for interpreted languages) is executable.
        // For interpreted languages Sisyphus stores a *directory* containing
        // run.sh and the source files.
        let meta = fs::metadata(&binary_path).await?;
        if meta.is_file() {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).await?;
        } else if meta.is_dir() {
            let run_sh = binary_path.join("run.sh");
            if !run_sh.exists() {
                return Err(anyhow!(
                    "Interpreted submission directory missing run.sh: {}",
                    binary_path.display()
                ));
            }
            let mut perms = fs::metadata(&run_sh).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&run_sh, perms).await?;
        }

        // Get or generate test cases
        let testcases = self
            .testcase_manager
            .get_testcases(ctx.problem_id, ctx.num_testcases)
            .await?;

        let mut results = Vec::with_capacity(testcases.len());

        for testcase in &testcases {
            let result = self
                .run_testcase(ctx, &binary_path, testcase, &temp_dir)
                .await;

            match result {
                Ok(tc_result) => {
                    let failed = tc_result.verdict.is_failure();
                    results.push(tc_result);

                    // Stop on first failure (can be made configurable)
                    if failed {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Judge error on testcase {}: {}",
                        testcase.number,
                        e
                    );
                    results.push(TestCaseResult::judge_error(
                        testcase.number,
                        e.to_string(),
                    ));
                    break;
                }
            }
        }

        // Cleanup temp directory
        if let Err(e) = fs::remove_dir_all(&temp_dir).await {
            tracing::warn!("Failed to cleanup temp dir: {}", e);
        }

        Ok(SubmissionResult::from_testcases(results, testcases.len() as i32))
    }

    /// Run a single test case
    async fn run_testcase(
        &self,
        ctx: &ExecutionContext,
        binary_path: &Path,
        testcase: &TestCase,
        temp_dir: &Path,
    ) -> Result<TestCaseResult> {
        let output_path = temp_dir.join(format!("output_{:03}.txt", testcase.number));

        // Execute the binary with file arguments: ./binary <input_file> <output_file>
        let start = Instant::now();

        let result = self
            .execute_sandboxed(
                binary_path,
                &testcase.input_path,
                &output_path,
                ctx.time_limit_ms,
                ctx.memory_limit_kb,
            )
            .await?;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Check execution result
        match result {
            ExecutionResult::Success { memory_kb } => {
                // Check output size
                let output_size = fs::metadata(&output_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);

                if output_size > self.execution.output_limit_bytes {
                    return Ok(TestCaseResult::output_limit_exceeded(
                        testcase.number,
                        elapsed_ms,
                        memory_kb,
                    ));
                }

                // Run checker
                let checker_result = self
                    .testcase_manager
                    .run_checker(
                        ctx.problem_id,
                        &testcase.input_path,
                        &output_path,
                        // For custom checker, we don't have expected output
                        // The checker generates/knows the expected result
                        &testcase.input_path, // Pass input as "answer" for interoperability
                    )
                    .await?;

                match checker_result {
                    CheckerResult::Accepted(_) => {
                        Ok(TestCaseResult::accepted(testcase.number, elapsed_ms, memory_kb))
                    }
                    CheckerResult::WrongAnswer(comment) => {
                        Ok(TestCaseResult::wrong_answer(
                            testcase.number,
                            elapsed_ms,
                            memory_kb,
                            Some(comment),
                        ))
                    }
                    CheckerResult::PartialCredit(_points, comment) => {
                        // For now, treat partial as wrong answer
                        // TODO: Implement partial scoring
                        Ok(TestCaseResult::wrong_answer(
                            testcase.number,
                            elapsed_ms,
                            memory_kb,
                            Some(comment),
                        ))
                    }
                    CheckerResult::JudgeError(msg) => {
                        Ok(TestCaseResult::judge_error(testcase.number, msg))
                    }
                }
            }
            ExecutionResult::TimeLimitExceeded => {
                Ok(TestCaseResult::time_limit_exceeded(
                    testcase.number,
                    ctx.time_limit_ms,
                    0,
                ))
            }
            ExecutionResult::MemoryLimitExceeded { memory_kb } => {
                Ok(TestCaseResult::memory_limit_exceeded(
                    testcase.number,
                    elapsed_ms,
                    memory_kb,
                ))
            }
            ExecutionResult::RuntimeError {
                exit_code,
                message,
                memory_kb,
            } => Ok(TestCaseResult::runtime_error(
                testcase.number,
                elapsed_ms,
                memory_kb,
                exit_code,
                message,
            )),
        }
    }

    /// Execute binary in sandboxed environment.
    ///
    /// The binary is invoked as: `./binary <input_file> <output_file>`
    /// instead of using stdin/stdout piping, which avoids broken-pipe
    /// errors with large I/O.
    ///
    /// For interpreted languages the "binary" is a directory containing
    /// `run.sh` and the source files.  In that case we invoke
    /// `bash run.sh <input_file> <output_file>` with the directory as cwd.
    async fn execute_sandboxed(
        &self,
        binary_path: &Path,
        input_path: &Path,
        output_path: &Path,
        time_limit_ms: u64,
        memory_limit_kb: u64,
    ) -> Result<ExecutionResult> {
        // For production, this should use proper sandboxing (nsjail, seccomp, cgroups)
        // This is a simplified version that uses basic process isolation

        let child = if binary_path.is_dir() {
            // Interpreted language: run.sh inside the directory
            let run_sh = binary_path.join("run.sh");
            Command::new("bash")
                .arg(&run_sh)
                .arg(input_path)
                .arg(output_path)
                .current_dir(binary_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?
        } else {
            Command::new(binary_path)
                .arg(input_path)
                .arg(output_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?
        };

        // Wait with timeout
        let time_limit = Duration::from_millis(time_limit_ms + 100); // Add buffer
        let result = timeout(time_limit, child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                // Get memory usage (simplified - would need /proc parsing for accurate values)
                let memory_kb = 0; // TODO: Implement actual memory tracking

                if output.status.success() {
                    Ok(ExecutionResult::Success { memory_kb })
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                    // Check for common signal-based terminations
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(signal) = output.status.signal() {
                            // SIGKILL (9) or SIGXCPU (24) often indicate resource limits
                            if signal == 9 {
                                // Could be memory limit - check memory_kb
                                if memory_kb > memory_limit_kb {
                                    return Ok(ExecutionResult::MemoryLimitExceeded { memory_kb });
                                }
                            }
                            return Ok(ExecutionResult::RuntimeError {
                                exit_code: -signal,
                                message: format!("Killed by signal {}", signal),
                                memory_kb,
                            });
                        }
                    }

                    Ok(ExecutionResult::RuntimeError {
                        exit_code,
                        message: if stderr.is_empty() {
                            format!("Process exited with code {}", exit_code)
                        } else {
                            stderr.chars().take(500).collect()
                        },
                        memory_kb,
                    })
                }
            }
            Ok(Err(e)) => Err(anyhow!("Failed to execute process: {}", e)),
            Err(_) => {
                // Timeout - kill the process if still running
                Ok(ExecutionResult::TimeLimitExceeded)
            }
        }
    }
}

/// Result of executing a binary
#[derive(Debug)]
enum ExecutionResult {
    /// Successful execution
    Success { memory_kb: u64 },
    /// Time limit exceeded
    TimeLimitExceeded,
    /// Memory limit exceeded
    MemoryLimitExceeded { memory_kb: u64 },
    /// Runtime error (crash, non-zero exit)
    RuntimeError {
        exit_code: i32,
        message: String,
        memory_kb: u64,
    },
}
