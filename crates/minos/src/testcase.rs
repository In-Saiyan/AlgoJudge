//! Test case management - generation and caching
//!
//! Generators and checkers are executed inside the same cgroup v2 /
//! namespace sandbox used for user submissions, enforcing memory limits,
//! PID limits, network isolation, and hard timeouts.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::{ExecutionConfig, StorageConfig};
use crate::sandbox::Sandbox;

/// Test case input/output pair
#[derive(Debug, Clone)]
pub struct TestCase {
    /// Test case number (1-indexed)
    pub number: i32,
    /// Path to input file
    pub input_path: PathBuf,
    /// Path to expected output file (if pre-generated)
    pub output_path: Option<PathBuf>,
}

/// Test case manager handles generation and caching
pub struct TestCaseManager {
    storage: StorageConfig,
    execution: ExecutionConfig,
}

impl TestCaseManager {
    /// Create a new test case manager
    pub fn new(storage: StorageConfig, execution: ExecutionConfig) -> Self {
        Self { storage, execution }
    }

    /// Get or generate test cases for a problem
    pub async fn get_testcases(
        &self,
        problem_id: Uuid,
        num_testcases: i32,
    ) -> Result<Vec<TestCase>> {
        let testcase_dir = self.storage.testcases_path.join(problem_id.to_string());

        // Check if test cases already exist
        if self.testcases_exist(&testcase_dir, num_testcases).await {
            tracing::debug!("Using cached test cases for problem {}", problem_id);
            self.touch_testcase_dir(&testcase_dir).await?;
            return self.load_testcases(&testcase_dir, num_testcases).await;
        }

        // Generate test cases
        tracing::info!("Generating test cases for problem {}", problem_id);
        self.generate_testcases(problem_id, num_testcases).await
    }

    /// Check if all test cases exist in cache
    async fn testcases_exist(&self, dir: &Path, count: i32) -> bool {
        for i in 1..=count {
            let input_path = dir.join(format!("input_{:03}.txt", i));
            if !input_path.exists() {
                return false;
            }
        }
        true
    }

    /// Touch directory to update last access time
    async fn touch_testcase_dir(&self, dir: &Path) -> Result<()> {
        let marker = dir.join(".last_access");
        let mut file = fs::File::create(&marker).await?;
        file.write_all(chrono::Utc::now().to_rfc3339().as_bytes())
            .await?;
        Ok(())
    }

    /// Load existing test cases from cache
    async fn load_testcases(&self, dir: &Path, count: i32) -> Result<Vec<TestCase>> {
        let mut testcases = Vec::with_capacity(count as usize);

        for i in 1..=count {
            let input_path = dir.join(format!("input_{:03}.txt", i));
            let output_path = dir.join(format!("output_{:03}.txt", i));

            testcases.push(TestCase {
                number: i,
                input_path,
                output_path: if output_path.exists() {
                    Some(output_path)
                } else {
                    None
                },
            });
        }

        Ok(testcases)
    }

    /// Generate test cases using the problem's generator.
    ///
    /// Each invocation runs the generator binary inside a cgroup v2 sandbox
    /// with the configured memory limit (`generator_memory_limit_kb`), PID
    /// limit, and network isolation (always disabled for generators).
    async fn generate_testcases(
        &self,
        problem_id: Uuid,
        num_testcases: i32,
    ) -> Result<Vec<TestCase>> {
        let generator_path = self
            .storage
            .problem_binaries_path
            .join(problem_id.to_string())
            .join("generator");

        if !generator_path.exists() {
            return Err(anyhow!("Generator not found for problem {}", problem_id));
        }

        // Ensure the generator binary is executable.
        let meta = fs::metadata(&generator_path).await?;
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&generator_path, perms).await?;

        // Create testcase directory
        let testcase_dir = self.storage.testcases_path.join(problem_id.to_string());
        fs::create_dir_all(&testcase_dir).await?;

        let mut testcases = Vec::with_capacity(num_testcases as usize);

        for i in 1..=num_testcases {
            let input_path = testcase_dir.join(format!("input_{:03}.txt", i));

            // Create a per-invocation sandbox with generator resource limits.
            let sandbox_id = format!("gen_{}_{}", problem_id, i);
            let sandbox = Sandbox::create(
                &sandbox_id,
                self.execution.generator_memory_limit_kb,
                // Generators are single-threaded; small PID buffer.
                1,
            )
            .await;

            let tc_num = i.to_string();
            let result = sandbox
                .run_sandboxed(
                    &generator_path,
                    &[&tc_num],
                    self.execution.generator_time_limit_ms,
                    false, // generators never need network
                    true,  // capture stdout → test case input
                )
                .await;

            sandbox.cleanup().await;

            let output = result.map_err(|e| {
                anyhow!(
                    "Generator failed for testcase {} (problem {}): {}",
                    i,
                    problem_id,
                    e
                )
            })?;

            if output.exit_code != 0 {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!(
                    "Generator exited with code {} for testcase {}: {}",
                    output.exit_code,
                    i,
                    stderr
                ));
            }

            // Write input to file
            fs::write(&input_path, &output.stdout).await?;

            testcases.push(TestCase {
                number: i,
                input_path,
                output_path: None, // We use checker, not pre-generated output
            });
        }

        // Touch the directory
        self.touch_testcase_dir(&testcase_dir).await?;

        tracing::info!(
            "Generated {} test cases for problem {} (sandboxed, mem_limit={}KB)",
            num_testcases,
            problem_id,
            self.execution.generator_memory_limit_kb,
        );

        Ok(testcases)
    }

    /// Run the checker to verify output.
    ///
    /// The checker binary is executed inside a cgroup v2 sandbox with the
    /// configured memory limit (`checker_memory_limit_kb`), PID limit,
    /// and network isolation (always disabled for checkers).
    ///
    /// Testlib convention: `./checker <input> <output> <answer>`
    pub async fn run_checker(
        &self,
        problem_id: Uuid,
        input_path: &Path,
        output_path: &Path,
        answer_path: &Path,
    ) -> Result<CheckerResult> {
        let checker_path = self
            .storage
            .problem_binaries_path
            .join(problem_id.to_string())
            .join("checker");

        if !checker_path.exists() {
            return Err(anyhow!("Checker not found for problem {}", problem_id));
        }

        // Ensure the checker binary is executable.
        let meta = fs::metadata(&checker_path).await?;
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&checker_path, perms).await?;

        // Create a sandbox for this checker invocation.
        let sandbox_id = format!("chk_{}", Uuid::new_v4());
        let sandbox = Sandbox::create(
            &sandbox_id,
            self.execution.checker_memory_limit_kb,
            // Checkers are single-threaded; small PID buffer.
            1,
        )
        .await;

        let input_str = input_path.to_string_lossy().to_string();
        let output_str = output_path.to_string_lossy().to_string();
        let answer_str = answer_path.to_string_lossy().to_string();

        let result = sandbox
            .run_sandboxed(
                &checker_path,
                &[&input_str, &output_str, &answer_str],
                self.execution.checker_time_limit_ms,
                false, // checkers never need network
                true,  // capture stdout for checker messages
            )
            .await;

        sandbox.cleanup().await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // Testlib exit codes:
                // 0 = AC (accepted)
                // 1 = WA (wrong answer)
                // 2 = PE (presentation error, treated as WA)
                // 3 = FAIL (judge error)
                // 7 = Points (partial credit)
                match output.exit_code {
                    0 => Ok(CheckerResult::Accepted(stdout)),
                    1 | 2 => Ok(CheckerResult::WrongAnswer(if stderr.is_empty() {
                        stdout
                    } else {
                        stderr
                    })),
                    3 => Ok(CheckerResult::JudgeError(stderr)),
                    7 => {
                        // Parse partial points from output
                        let points = stdout
                            .lines()
                            .next()
                            .and_then(|l| l.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        Ok(CheckerResult::PartialCredit(points, stdout))
                    }
                    code => Ok(CheckerResult::JudgeError(format!(
                        "Checker exited with code {}: {}",
                        code, stderr
                    ))),
                }
            }
            Err(e) => {
                // Sandbox-level failure (timeout, OOM, spawn error)
                Ok(CheckerResult::JudgeError(format!(
                    "Checker sandbox error: {}",
                    e
                )))
            }
        }
    }
}

/// Result from running the checker
#[derive(Debug)]
pub enum CheckerResult {
    /// Output is correct
    Accepted(String),
    /// Output is incorrect
    WrongAnswer(String),
    /// Partial credit (0.0 - 1.0 typically)
    PartialCredit(f64, String),
    /// Internal checker error
    JudgeError(String),
}
