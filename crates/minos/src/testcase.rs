//! Test case management - generation and caching

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, Result};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::config::{ExecutionConfig, StorageConfig};

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
        file.write_all(
            chrono::Utc::now()
                .to_rfc3339()
                .as_bytes(),
        )
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

    /// Generate test cases using the problem's generator
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
            return Err(anyhow!(
                "Generator not found for problem {}",
                problem_id
            ));
        }

        // Create testcase directory
        let testcase_dir = self.storage.testcases_path.join(problem_id.to_string());
        fs::create_dir_all(&testcase_dir).await?;

        let mut testcases = Vec::with_capacity(num_testcases as usize);

        for i in 1..=num_testcases {
            let input_path = testcase_dir.join(format!("input_{:03}.txt", i));
            
            // Run generator with test case number as argument
            let output = timeout(
                Duration::from_millis(self.execution.generator_time_limit_ms),
                Command::new(&generator_path)
                    .arg(i.to_string())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output(),
            )
            .await
            .map_err(|_| anyhow!("Generator timeout for testcase {}", i))?
            .map_err(|e| anyhow!("Failed to run generator: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!(
                    "Generator failed for testcase {}: {}",
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
            "Generated {} test cases for problem {}",
            num_testcases,
            problem_id
        );

        Ok(testcases)
    }

    /// Run the checker to verify output
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

        // Run checker: checker <input> <output> <answer>
        // Testlib-style checkers use this convention
        let result = timeout(
            Duration::from_millis(self.execution.checker_time_limit_ms),
            Command::new(&checker_path)
                .arg(input_path)
                .arg(output_path)
                .arg(answer_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| anyhow!("Checker timeout"))?
        .map_err(|e| anyhow!("Failed to run checker: {}", e))?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();

        // Testlib exit codes:
        // 0 = AC (accepted)
        // 1 = WA (wrong answer)
        // 2 = PE (presentation error, treated as WA)
        // 3 = FAIL (judge error)
        // 7 = Points (partial credit)
        match result.status.code() {
            Some(0) => Ok(CheckerResult::Accepted(stdout)),
            Some(1) | Some(2) => Ok(CheckerResult::WrongAnswer(
                if stderr.is_empty() { stdout } else { stderr },
            )),
            Some(3) => Ok(CheckerResult::JudgeError(stderr)),
            Some(7) => {
                // Parse partial points from output
                let points = stdout
                    .lines()
                    .next()
                    .and_then(|l| l.parse::<f64>().ok())
                    .unwrap_or(0.0);
                Ok(CheckerResult::PartialCredit(points, stdout))
            }
            Some(code) => Ok(CheckerResult::JudgeError(format!(
                "Checker exited with code {}: {}",
                code, stderr
            ))),
            None => Ok(CheckerResult::JudgeError(
                "Checker terminated by signal".to_string(),
            )),
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
