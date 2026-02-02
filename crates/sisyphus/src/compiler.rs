//! Compilation logic for Sisyphus.
//!
//! Handles extracting submissions, running compile scripts,
//! and saving compiled binaries.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;

use crate::config::Config;
use crate::consumer::CompileJob;

/// Compiler handles the compilation of submissions.
pub struct Compiler {
    config: Config,
}

impl Compiler {
    /// Create a new compiler with the given configuration.
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Compile a submission and return the path to the compiled binary.
    pub async fn compile(&self, job: &CompileJob) -> Result<String> {
        match job.job_type.as_str() {
            "zip" => self.compile_zip(job).await,
            "source" => self.compile_source(job).await,
            other => Err(anyhow!("Unknown job type: {}", other)),
        }
    }

    /// Compile a ZIP submission.
    async fn compile_zip(&self, job: &CompileJob) -> Result<String> {
        let file_path = job.file_path.as_ref()
            .ok_or_else(|| anyhow!("ZIP submission missing file_path"))?;

        // Create temp directory for build
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temp directory")?;
        let build_dir = temp_dir.path();

        tracing::debug!(
            submission_id = %job.submission_id,
            build_dir = %build_dir.display(),
            "Created build directory"
        );

        // Extract ZIP to build directory
        self.extract_zip(file_path, build_dir).await?;

        // Run compile.sh
        let compile_output = self.run_compile_script(build_dir).await?;

        if !compile_output.success {
            return Err(anyhow!(
                "Compilation failed:\n{}",
                compile_output.stderr
            ));
        }

        // Find and copy the compiled binary
        let binary_path = self.save_binary(job, build_dir).await?;

        Ok(binary_path)
    }

    /// Compile a source code submission (legacy single-file).
    async fn compile_source(&self, job: &CompileJob) -> Result<String> {
        let language = job.language.as_ref()
            .ok_or_else(|| anyhow!("Source submission missing language"))?;

        // Fetch source code from database
        let source_code = self.fetch_source_code(&job.submission_id).await?;

        // Create temp directory for build
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temp directory")?;
        let build_dir = temp_dir.path();

        // Write source file
        let (source_file, compile_cmd) = self.get_compile_command(language, build_dir)?;
        fs::write(build_dir.join(&source_file), &source_code).await?;

        // Run compilation command
        let output = self.run_command(&compile_cmd, build_dir).await?;

        if !output.success {
            return Err(anyhow!(
                "Compilation failed:\n{}",
                output.stderr
            ));
        }

        // Save the binary
        let binary_path = self.save_binary(job, build_dir).await?;

        Ok(binary_path)
    }

    /// Extract a ZIP file to a directory.
    async fn extract_zip(&self, zip_path: &str, dest_dir: &Path) -> Result<()> {
        let zip_path = PathBuf::from(zip_path);
        let dest_dir = dest_dir.to_path_buf();

        // Run extraction in blocking task
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&zip_path)
                .with_context(|| format!("Failed to open ZIP: {}", zip_path.display()))?;
            let mut archive = zip::ZipArchive::new(file)
                .context("Failed to read ZIP archive")?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = dest_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }

                // Set permissions on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    /// Run the compile.sh script in the build directory.
    async fn run_compile_script(&self, build_dir: &Path) -> Result<CommandOutput> {
        let compile_script = build_dir.join("compile.sh");

        if !compile_script.exists() {
            return Err(anyhow!("compile.sh not found in submission"));
        }

        // Make script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            fs::set_permissions(&compile_script, perms).await?;
        }

        self.run_command(&["./compile.sh"], build_dir).await
    }

    /// Run a command in the build directory with timeout.
    async fn run_command(&self, cmd: &[&str], cwd: &Path) -> Result<CommandOutput> {
        let (program, args) = cmd.split_first()
            .ok_or_else(|| anyhow!("Empty command"))?;

        let child = Command::new(program)
            .args(args.iter())
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn {}", program))?;

        // Apply timeout
        let timeout = tokio::time::Duration::from_secs(self.config.compile_timeout_secs);
        let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                Ok(CommandOutput {
                    success: output.status.success(),
                    exit_code: output.status.code(),
                    stdout,
                    stderr,
                })
            }
            Ok(Err(e)) => Err(anyhow!("Command execution failed: {}", e)),
            Err(_) => Err(anyhow!(
                "Compilation timed out after {} seconds",
                self.config.compile_timeout_secs
            )),
        }
    }

    /// Get the source file name and compile command for a language.
    fn get_compile_command(&self, language: &str, _build_dir: &Path) -> Result<(String, Vec<&str>)> {
        match language {
            "cpp" | "c++" => Ok((
                "main.cpp".to_string(),
                vec!["g++", "-O2", "-std=c++17", "-o", "main", "main.cpp"],
            )),
            "c" => Ok((
                "main.c".to_string(),
                vec!["gcc", "-O2", "-std=c11", "-o", "main", "main.c"],
            )),
            "rust" => Ok((
                "main.rs".to_string(),
                vec!["rustc", "-O", "-o", "main", "main.rs"],
            )),
            "go" => Ok((
                "main.go".to_string(),
                vec!["go", "build", "-o", "main", "main.go"],
            )),
            "python" => {
                // Python doesn't need compilation, just syntax check
                Ok((
                    "main.py".to_string(),
                    vec!["python3", "-m", "py_compile", "main.py"],
                ))
            }
            "zig" => Ok((
                "main.zig".to_string(),
                vec!["zig", "build-exe", "-O", "ReleaseFast", "main.zig"],
            )),
            other => Err(anyhow!("Unsupported language: {}", other)),
        }
    }

    /// Find the compiled binary and save it to the binaries directory.
    async fn save_binary(&self, job: &CompileJob, build_dir: &Path) -> Result<String> {
        // Look for common binary names
        let binary_names = ["main", "a.out", "solution", "run"];
        let mut binary_path = None;

        for name in binary_names {
            let path = build_dir.join(name);
            if path.exists() {
                binary_path = Some(path);
                break;
            }
        }

        // If run.sh exists, that's the "binary" for interpreted languages
        let run_script = build_dir.join("run.sh");
        if binary_path.is_none() && run_script.exists() {
            // For ZIP submissions, copy the entire build directory as the "binary"
            let dest_dir = format!(
                "{}/{}_bin",
                self.config.binaries_path,
                job.submission_id
            );
            
            // Create destination directory
            fs::create_dir_all(&dest_dir).await?;

            // Copy all files from build_dir to dest_dir
            self.copy_dir_recursive(build_dir, Path::new(&dest_dir)).await?;

            return Ok(dest_dir);
        }

        let binary_path = binary_path
            .ok_or_else(|| anyhow!("No compiled binary found"))?;

        // Create destination path
        let dest_path = format!(
            "{}/{}_bin",
            self.config.binaries_path,
            job.submission_id
        );

        // Ensure parent directory exists
        if let Some(parent) = Path::new(&dest_path).parent() {
            fs::create_dir_all(parent).await?;
        }

        // Copy binary
        fs::copy(&binary_path, &dest_path).await
            .with_context(|| format!(
                "Failed to copy binary from {} to {}",
                binary_path.display(),
                dest_path
            ))?;

        // Make binary executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            fs::set_permissions(&dest_path, perms).await?;
        }

        Ok(dest_path)
    }

    /// Recursively copy a directory.
    async fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst).await?;

        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if entry.file_type().await?.is_dir() {
                Box::pin(self.copy_dir_recursive(&src_path, &dst_path)).await?;
            } else {
                fs::copy(&src_path, &dst_path).await?;
            }
        }

        Ok(())
    }

    /// Fetch source code from database for legacy source submissions.
    async fn fetch_source_code(&self, submission_id: &uuid::Uuid) -> Result<String> {
        // This would require database access, which we don't have in the compiler
        // For now, return an error - source code should be passed in the job
        Err(anyhow!(
            "Source code submissions require source_code field in job (submission {})",
            submission_id
        ))
    }
}

/// Output from a command execution.
#[derive(Debug)]
struct CommandOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_compile_command() {
        let config = Config::from_env();
        let compiler = Compiler::new(config);
        let build_dir = PathBuf::from("/tmp/test");

        let (file, cmd) = compiler.get_compile_command("cpp", &build_dir).unwrap();
        assert_eq!(file, "main.cpp");
        assert!(cmd.contains(&"g++"));

        let (file, cmd) = compiler.get_compile_command("python", &build_dir).unwrap();
        assert_eq!(file, "main.py");
        assert!(cmd.contains(&"python3"));

        assert!(compiler.get_compile_command("unknown", &build_dir).is_err());
    }
}
