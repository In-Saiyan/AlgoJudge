//! Docker container management for benchmarking

use bollard::{
    container::LogOutput,
    exec::{CreateExecOptions, StartExecResults},
    models::ContainerCreateBody,
    query_parameters::{CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder},
    Docker,
};
use futures::StreamExt;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{config::Config, constants, error::AppResult};

use super::{languages::LanguageHandler, runner::RunResult};

/// Docker container manager for benchmark execution
pub struct ContainerManager {
    docker: Docker,
    config: Config,
}

impl ContainerManager {
    /// Create a new container manager
    pub fn new(docker: Docker, config: Config) -> Self {
        Self { docker, config }
    }

    /// Create a container for a submission
    pub async fn create_container(
        &self,
        submission_id: &Uuid,
        language: &str,
    ) -> AppResult<String> {
        let image = self.get_image_for_language(language)?;

        let container_name = format!("algojudge-{}", submission_id);

        let options = CreateContainerOptionsBuilder::default()
            .name(&container_name)
            .build();

        let host_config = bollard::models::HostConfig {
            memory: Some((self.config.benchmark.default_memory_limit_mb * 1024 * 1024) as i64),
            memory_swap: Some((self.config.benchmark.default_memory_limit_mb * 1024 * 1024) as i64),
            cpu_period: Some(100000),
            cpu_quota: Some(100000), // 1 CPU
            network_mode: Some("none".to_string()), // No network access
            pids_limit: Some(64), // Limit number of processes
            readonly_rootfs: Some(false),
            ..Default::default()
        };

        let config = ContainerCreateBody {
            image: Some(image),
            tty: Some(true),
            open_stdin: Some(true),
            host_config: Some(host_config),
            working_dir: Some("/workspace".to_string()),
            env: Some(vec!["LANG=C.UTF-8".to_string()]),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("algojudge.submission".to_string(), submission_id.to_string());
                labels
            }),
            ..Default::default()
        };

        let container = self.docker.create_container(Some(options), config).await?;

        // Start the container
        self.docker
            .start_container(&container.id, None::<bollard::query_parameters::StartContainerOptions>)
            .await?;

        Ok(container.id)
    }

    /// Compile source code in the container
    pub async fn compile(
        &self,
        container_id: &str,
        source_code: &str,
        language: &LanguageHandler,
    ) -> AppResult<String> {
        // Write source code to container
        let source_file = language.source_file();
        self.write_file(container_id, &format!("/workspace/{}", source_file), source_code)
            .await?;

        // Get compile command
        if let Some(compile_cmd) = language.compile_command() {
            let result = self.exec_command(container_id, &compile_cmd).await?;

            if result.exit_code != 0 {
                return Err(anyhow::anyhow!(
                    "Compilation failed:\n{}{}",
                    result.stdout,
                    result.stderr.unwrap_or_default()
                )
                .into());
            }
        }

        Ok(language.executable())
    }

    /// Run executable with input
    pub async fn run_with_input(
        &self,
        container_id: &str,
        executable: &str,
        input: &str,
        time_limit_ms: i32,
        _memory_limit_kb: i32,
    ) -> AppResult<RunResult> {
        // Write input to file
        self.write_file(container_id, "/workspace/input.txt", input)
            .await?;

        // Build run command with resource limits and timing
        let run_cmd = format!(
            "timeout {}s /usr/bin/time -v {} < /workspace/input.txt 2>&1",
            (time_limit_ms as f64 / 1000.0) + 0.5,
            executable
        );

        let start = std::time::Instant::now();
        let result = self.exec_command(container_id, &run_cmd).await?;
        let wall_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Parse /usr/bin/time output for memory usage
        let (stdout, time_output) = self.split_time_output(&result.stdout);
        let memory_kb = self.parse_memory_usage(&time_output);
        let cpu_time_ms = self.parse_cpu_time(&time_output);

        // Check if timed out
        let exit_code = if wall_time_ms > time_limit_ms as f64 + 500.0 {
            124 // timeout exit code
        } else {
            result.exit_code
        };

        Ok(RunResult {
            stdout,
            stderr: result.stderr,
            exit_code,
            wall_time_ms,
            cpu_time_ms,
            memory_kb,
        })
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str) -> AppResult<()> {
        let options = RemoveContainerOptionsBuilder::default()
            .force(true)
            .build();

        self.docker.remove_container(container_id, Some(options)).await?;

        Ok(())
    }

    /// Write a file to the container
    async fn write_file(&self, container_id: &str, path: &str, content: &str) -> AppResult<()> {
        // Use echo with base64 to handle special characters
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, content);
        let cmd = format!("echo '{}' | base64 -d > {}", encoded, path);

        self.exec_command(container_id, &cmd).await?;

        Ok(())
    }

    /// Execute a command in the container
    async fn exec_command(&self, container_id: &str, cmd: &str) -> AppResult<RunResult> {
        let exec = self
            .docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    cmd: Some(vec!["/bin/sh", "-c", cmd]),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        let output = self.docker.start_exec(&exec.id, None).await?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        if let StartExecResults::Attached { mut output, .. } = output {
            while let Some(msg) = output.next().await {
                match msg? {
                    LogOutput::StdOut { message } => {
                        stdout.push_str(&String::from_utf8_lossy(&message));
                    }
                    LogOutput::StdErr { message } => {
                        stderr.push_str(&String::from_utf8_lossy(&message));
                    }
                    _ => {}
                }
            }
        }

        // Get exit code
        let inspect = self.docker.inspect_exec(&exec.id).await?;
        let exit_code = inspect.exit_code.unwrap_or(-1) as i32;

        Ok(RunResult {
            stdout,
            stderr: if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
            exit_code,
            wall_time_ms: 0.0, // Calculated by caller
            cpu_time_ms: 0.0,
            memory_kb: 0,
        })
    }

    /// Get Docker image for a language
    fn get_image_for_language(&self, language: &str) -> AppResult<String> {
        let image = match language {
            constants::languages::C => constants::container_images::C,
            constants::languages::CPP => constants::container_images::CPP,
            constants::languages::RUST => constants::container_images::RUST,
            constants::languages::GO => constants::container_images::GO,
            constants::languages::ZIG => constants::container_images::ZIG,
            constants::languages::PYTHON => constants::container_images::PYTHON,
            _ => return Err(anyhow::anyhow!("Unsupported language: {}", language).into()),
        };

        Ok(image.to_string())
    }

    /// Split stdout from /usr/bin/time output
    fn split_time_output(&self, combined: &str) -> (String, String) {
        // /usr/bin/time output starts with "Command being timed:" or similar
        if let Some(idx) = combined.find("\tCommand being timed:") {
            let (stdout, time_part) = combined.split_at(idx);
            (stdout.to_string(), time_part.to_string())
        } else if let Some(idx) = combined.find("Command exited with non-zero status") {
            let (stdout, time_part) = combined.split_at(idx);
            (stdout.to_string(), time_part.to_string())
        } else {
            (combined.to_string(), String::new())
        }
    }

    /// Parse memory usage from /usr/bin/time -v output
    fn parse_memory_usage(&self, time_output: &str) -> i64 {
        for line in time_output.lines() {
            if line.contains("Maximum resident set size") {
                if let Some(kb_str) = line.split(':').nth(1) {
                    if let Ok(kb) = kb_str.trim().parse::<i64>() {
                        return kb;
                    }
                }
            }
        }
        0
    }

    /// Parse CPU time from /usr/bin/time -v output
    fn parse_cpu_time(&self, time_output: &str) -> f64 {
        let mut user_time = 0.0f64;
        let mut sys_time = 0.0f64;

        for line in time_output.lines() {
            if line.contains("User time (seconds)") {
                if let Some(time_str) = line.split(':').nth(1) {
                    user_time = time_str.trim().parse().unwrap_or(0.0);
                }
            } else if line.contains("System time (seconds)") {
                if let Some(time_str) = line.split(':').nth(1) {
                    sys_time = time_str.trim().parse().unwrap_or(0.0);
                }
            }
        }

        (user_time + sys_time) * 1000.0 // Convert to ms
    }

    // =========================================================================
    // Methods for ZIP-based algorithmic benchmarking
    // =========================================================================

    /// Copy and extract a ZIP file to the container workspace
    pub async fn copy_zip_to_container(&self, container_id: &str, zip_data: &[u8]) -> AppResult<()> {
        // Encode ZIP as base64 and decode in container
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, zip_data);
        
        // Write base64 data to container and decode
        let cmd = format!(
            "echo '{}' | base64 -d > /workspace/submission.zip && cd /workspace && unzip -o submission.zip && rm submission.zip && chmod +x *.sh 2>/dev/null || true",
            encoded
        );

        let result = self.exec_command(container_id, &cmd).await?;
        if result.exit_code != 0 {
            return Err(anyhow::anyhow!(
                "Failed to extract ZIP: {}",
                result.stderr.unwrap_or_default()
            ).into());
        }

        Ok(())
    }

    /// Copy a binary file to the container
    pub async fn copy_binary_to_container(
        &self,
        container_id: &str,
        binary_data: &[u8],
        filename: &str,
    ) -> AppResult<()> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, binary_data);
        
        let cmd = format!(
            "echo '{}' | base64 -d > /workspace/{} && chmod +x /workspace/{}",
            encoded, filename, filename
        );

        let result = self.exec_command(container_id, &cmd).await?;
        if result.exit_code != 0 {
            return Err(anyhow::anyhow!(
                "Failed to copy binary '{}': {}",
                filename,
                result.stderr.unwrap_or_default()
            ).into());
        }

        Ok(())
    }

    /// Run a shell script in the container
    pub async fn run_script(
        &self,
        container_id: &str,
        script_name: &str,
        timeout_ms: u64,
    ) -> AppResult<String> {
        let timeout_secs = (timeout_ms as f64 / 1000.0).max(1.0);
        let cmd = format!(
            "cd /workspace && timeout {}s ./{} 2>&1",
            timeout_secs, script_name
        );

        let result = self.exec_command(container_id, &cmd).await?;
        
        if result.exit_code == 124 {
            return Err(anyhow::anyhow!("Script '{}' timed out", script_name).into());
        }

        if result.exit_code != 0 {
            return Err(anyhow::anyhow!(
                "Script '{}' failed with exit code {}: {}{}",
                script_name,
                result.exit_code,
                result.stdout,
                result.stderr.unwrap_or_default()
            ).into());
        }

        Ok(result.stdout)
    }

    /// Check if a file exists in the container
    pub async fn file_exists(&self, container_id: &str, filename: &str) -> AppResult<bool> {
        let cmd = format!("test -f /workspace/{} && echo 'EXISTS'", filename);
        let result = self.exec_command(container_id, &cmd).await?;
        
        Ok(result.stdout.contains("EXISTS"))
    }

    /// Run a command in the container and return output
    pub async fn run_command(
        &self,
        container_id: &str,
        cmd: &str,
        timeout_ms: u64,
    ) -> AppResult<String> {
        let timeout_secs = (timeout_ms as f64 / 1000.0).max(1.0);
        let full_cmd = format!("cd /workspace && timeout {}s {}", timeout_secs, cmd);

        let result = self.exec_command(container_id, &full_cmd).await?;
        
        if result.exit_code == 124 {
            return Err(anyhow::anyhow!("Command timed out").into());
        }

        Ok(result.stdout)
    }

    /// Run a command with metrics collection
    pub async fn run_with_metrics(
        &self,
        container_id: &str,
        cmd: &str,
        timeout_ms: u64,
        memory_limit_kb: u64,
    ) -> AppResult<MetricsResult> {
        let timeout_secs = (timeout_ms as f64 / 1000.0).max(1.0);
        
        // Use /usr/bin/time -v to get memory and CPU stats
        let full_cmd = format!(
            "cd /workspace && timeout {}s /usr/bin/time -v sh -c '{}' 2>&1",
            timeout_secs, cmd.replace("'", "'\\''")
        );

        let start = std::time::Instant::now();
        let result = self.exec_command(container_id, &full_cmd).await?;
        let wall_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Check timeout
        if result.exit_code == 124 || wall_time_ms > timeout_ms as f64 + 500.0 {
            return Err(anyhow::anyhow!("time limit exceeded").into());
        }

        // Parse time output
        let (stdout, time_output) = self.split_time_output(&result.stdout);
        let memory_kb = self.parse_memory_usage(&time_output);

        // Check memory limit
        if memory_kb > memory_limit_kb as i64 {
            return Err(anyhow::anyhow!("memory limit exceeded").into());
        }

        // Check for runtime errors
        if result.exit_code != 0 {
            return Err(anyhow::anyhow!(
                "Runtime error (exit code {}): {}",
                result.exit_code,
                result.stderr.unwrap_or_default()
            ).into());
        }

        Ok(MetricsResult {
            time_ms: wall_time_ms,
            memory_kb,
            output: stdout,
        })
    }
}

/// Result with metrics from execution
#[derive(Debug)]
pub struct MetricsResult {
    pub time_ms: f64,
    pub memory_kb: i64,
    pub output: String,
}
