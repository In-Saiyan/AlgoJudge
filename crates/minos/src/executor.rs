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
use crate::sandbox::Sandbox;
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
    /// Maximum number of threads the submission may spawn (1 = single-threaded)
    pub max_threads: i32,
    /// Whether the submission is allowed network access during execution
    pub network_allowed: bool,
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
        // Clamp max_threads to the system-wide limit (defense in depth)
        let effective_max_threads = ctx.max_threads.min(self.execution.max_threads_limit).max(1);
        if ctx.max_threads > self.execution.max_threads_limit {
            tracing::warn!(
                submission_id = %ctx.submission_id,
                requested = ctx.max_threads,
                limit = self.execution.max_threads_limit,
                "max_threads from DB exceeds system limit — clamping to {}",
                effective_max_threads,
            );
        }

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
        let temp_dir = self.storage.temp_path.join(ctx.submission_id.to_string());
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
                .run_testcase(
                    ctx,
                    effective_max_threads,
                    &binary_path,
                    testcase,
                    &temp_dir,
                )
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
                    tracing::error!("Judge error on testcase {}: {}", testcase.number, e);
                    results.push(TestCaseResult::judge_error(testcase.number, e.to_string()));
                    break;
                }
            }
        }

        // Cleanup temp directory
        if let Err(e) = fs::remove_dir_all(&temp_dir).await {
            tracing::warn!("Failed to cleanup temp dir: {}", e);
        }

        Ok(SubmissionResult::from_testcases(
            results,
            testcases.len() as i32,
        ))
    }

    /// Run a single test case
    async fn run_testcase(
        &self,
        ctx: &ExecutionContext,
        effective_max_threads: i32,
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
                effective_max_threads,
                ctx.network_allowed,
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
                    CheckerResult::Accepted(_) => Ok(TestCaseResult::accepted(
                        testcase.number,
                        elapsed_ms,
                        memory_kb,
                    )),
                    CheckerResult::WrongAnswer(comment) => Ok(TestCaseResult::wrong_answer(
                        testcase.number,
                        elapsed_ms,
                        memory_kb,
                        Some(comment),
                    )),
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
            ExecutionResult::TimeLimitExceeded => Ok(TestCaseResult::time_limit_exceeded(
                testcase.number,
                ctx.time_limit_ms,
                0,
            )),
            ExecutionResult::MemoryLimitExceeded { memory_kb } => Ok(
                TestCaseResult::memory_limit_exceeded(testcase.number, elapsed_ms, memory_kb),
            ),
            ExecutionResult::RuntimeError {
                exit_code,
                message,
                memory_kb,
            } => {
                tracing::debug!(
                    testcase = testcase.number,
                    exit_code,
                    %message,
                    "Runtime error on testcase"
                );
                Ok(TestCaseResult::runtime_error(
                    testcase.number,
                    elapsed_ms,
                    memory_kb,
                    exit_code,
                    message,
                ))
            }
        }
    }

    /// Execute binary in a sandboxed environment.
    ///
    /// The binary is invoked as: `./binary <input_file> <output_file>`
    /// instead of using stdin/stdout piping, which avoids broken-pipe
    /// errors with large I/O.
    ///
    /// For interpreted languages the "binary" is a directory containing
    /// `run.sh` and the source files.  In that case we invoke
    /// `bash run.sh <input_file> <output_file>` with the directory as cwd.
    ///
    /// ## Sandboxing
    ///
    /// * **cgroups v2** – memory limit (`memory.max`), swap disabled
    ///   (`memory.swap.max 0`), PID/thread limit (`pids.max`).
    /// * **Network namespace** – `unshare(CLONE_NEWNET)` via `pre_exec`
    ///   when `network_allowed` is `false`.
    /// * **Resource metrics** – peak memory from `memory.peak` (cgroup) or
    ///   `VmPeak` (`/proc`); CPU time from `cpu.stat`.
    ///
    /// When cgroups are unavailable the executor degrades gracefully.
    async fn execute_sandboxed(
        &self,
        binary_path: &Path,
        input_path: &Path,
        output_path: &Path,
        time_limit_ms: u64,
        memory_limit_kb: u64,
        max_threads: i32,
        network_allowed: bool,
    ) -> Result<ExecutionResult> {
        tracing::debug!(
            binary = %binary_path.display(),
            time_limit_ms,
            memory_limit_kb,
            max_threads,
            network_allowed,
            "Executing submission binary"
        );

        // ── 1. Create sandbox (cgroups v2 resource limits) ──────────
        let sandbox_id = Uuid::new_v4().to_string();
        let sandbox = Sandbox::create(&sandbox_id, memory_limit_kb, max_threads).await;

        // ── 2. Build the command (without spawning) ─────────────────
        let mut cmd = if binary_path.is_dir() {
            // Interpreted language: run.sh inside the directory.
            let run_sh = binary_path.join("run.sh");
            let run_sh_content = fs::read_to_string(&run_sh).await.unwrap_or_default();

            // Check whether run.sh already forwards args to the inner command.
            let forwards_args = run_sh_content.contains("$1")
                || run_sh_content.contains("$2")
                || run_sh_content.contains("$@")
                || run_sh_content.contains("${1")
                || run_sh_content.contains("${2")
                || run_sh_content.contains("INPUT_FILE")
                || run_sh_content.contains("OUTPUT_FILE");

            if forwards_args {
                let mut c = Command::new("bash");
                c.arg(&run_sh)
                    .arg(input_path)
                    .arg(output_path)
                    .current_dir(binary_path);
                c
            } else {
                let cmd_line = run_sh_content
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .last()
                    .unwrap_or("bash run.sh");

                tracing::debug!(
                    original_cmd = %cmd_line,
                    "run.sh does not forward args — auto-appending file paths"
                );

                let mut c = Command::new("bash");
                c.arg("-c")
                    .arg(format!("{} \"$1\" \"$2\"", cmd_line))
                    .arg("_") // $0 placeholder
                    .arg(input_path)
                    .arg(output_path)
                    .current_dir(binary_path);
                c
            }
        } else {
            let mut c = Command::new(binary_path);
            c.arg(input_path).arg(output_path);
            c
        };

        // ── 3. Common I/O and environment setup ─────────────────────
        cmd.env("INPUT_FILE", input_path)
            .env("OUTPUT_FILE", output_path)
            .env("MAX_THREADS", max_threads.to_string())
            .env("NETWORK_ALLOWED", if network_allowed { "1" } else { "0" })
            .env("TIME_LIMIT_MS", time_limit_ms.to_string())
            .env("MEMORY_LIMIT_KB", memory_limit_kb.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // ── 4. Pre-exec hooks (run in child AFTER fork, BEFORE exec) ──
        //
        // a) Join the cgroup so resource accounting starts immediately.
        if let Some(procs_path) = sandbox.cgroup_procs_path() {
            unsafe {
                cmd.pre_exec(move || {
                    std::fs::write(&procs_path, std::process::id().to_string())?;
                    Ok(())
                });
            }
        }
        // b) Network namespace isolation — completely disables networking.
        if !network_allowed {
            unsafe {
                cmd.pre_exec(|| {
                    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET)
                        .map_err(std::io::Error::from)?;
                    Ok(())
                });
            }
        }

        // ── 5. Spawn and wait ───────────────────────────────────────
        let child = cmd.spawn()?;
        let child_pid = child.id();

        let time_limit = Duration::from_millis(time_limit_ms + 100); // small buffer
        let result = timeout(time_limit, child.wait_with_output()).await;

        // ── 6. Collect resource metrics and clean up sandbox ────────
        let usage = sandbox.read_usage(child_pid).await;
        let oom_killed = sandbox.was_oom_killed().await;
        sandbox.cleanup().await;

        // ── 7. Determine execution result ───────────────────────────
        match result {
            Ok(Ok(output)) => {
                let memory_kb = usage.memory_kb;

                if output.status.success() {
                    Ok(ExecutionResult::Success { memory_kb })
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                    // Check for signal-based terminations
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(signal) = output.status.signal() {
                            // SIGKILL (9) from cgroup OOM killer
                            if signal == 9 && (oom_killed || memory_kb >= memory_limit_kb) {
                                return Ok(ExecutionResult::MemoryLimitExceeded { memory_kb });
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
                // Timeout — child is killed via kill_on_drop
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
