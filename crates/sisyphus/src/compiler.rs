//! Compilation logic for Sisyphus.
//!
//! Both ZIP and source-code submissions are compiled inside ephemeral
//! Docker containers.  The language (when known) selects the image so
//! the right toolchain is available.  For ZIP submissions the user's
//! `compile.sh` is executed; for legacy source submissions we generate
//! the appropriate compiler invocation automatically.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::config::Config;
use crate::consumer::CompileJob;
use crate::container::{ensure_image, resolve_image, run_in_container};

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

    /// Compile a ZIP submission inside a language-specific Docker container.
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

        // Log extracted directory contents for debugging
        match tokio::fs::read_dir(build_dir).await {
            Ok(mut entries) => {
                let mut names = Vec::new();
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let ft = entry.file_type().await.ok();
                    let kind = match ft {
                        Some(t) if t.is_dir() => "dir",
                        Some(t) if t.is_symlink() => "symlink",
                        _ => "file",
                    };
                    names.push(format!("{}({})", entry.file_name().to_string_lossy(), kind));
                }
                names.sort();
                tracing::debug!(
                    submission_id = %job.submission_id,
                    build_dir = %build_dir.display(),
                    contents = %names.join(", "),
                    "Build directory contents after ZIP extraction"
                );
            }
            Err(e) => {
                tracing::warn!(
                    submission_id = %job.submission_id,
                    error = %e,
                    "Failed to list build directory"
                );
            }
        }

        // Resolve the container image from the language hint
        let spec = resolve_image(&self.config, job.language.as_deref());

        // Ensure the image exists locally (pull if needed)
        ensure_image(&self.config, &spec.image).await?;

        // Strip Windows CRLF line-endings from shell scripts so shebangs work
        // inside Linux containers (e.g. #!/bin/bash\r → not found).
        for name in &["compile.sh", "run.sh"] {
            let path = build_dir.join(name);
            if path.exists() {
                let content = fs::read(&path).await?;
                if content.windows(2).any(|w| w == b"\r\n") {
                    let cleaned: Vec<u8> = content.into_iter().filter(|&b| b != b'\r').collect();
                    fs::write(&path, &cleaned).await?;
                    tracing::debug!(file = %name, "Stripped CRLF line endings");
                }
            }
        }

        // Make compile.sh executable before mounting
        let compile_script = build_dir.join("compile.sh");
        if !compile_script.exists() {
            return Err(anyhow!("compile.sh not found in submission"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&compile_script, std::fs::Permissions::from_mode(0o755)).await?;
        }

        // Run compile.sh inside the container
        let output = run_in_container(
            &self.config,
            &spec,
            build_dir,
            &["sh", "-c", "./compile.sh"],
        )
        .await?;

        if !output.success {
            return Err(anyhow!(
                "Compilation failed:\n{}",
                output.stderr
            ));
        }

        // Find and copy the compiled binary
        let binary_path = self.save_binary(job, build_dir).await?;

        Ok(binary_path)
    }

    /// Compile a source code submission inside a language-specific Docker container.
    async fn compile_source(&self, job: &CompileJob) -> Result<String> {
        let language = job.language.as_ref()
            .ok_or_else(|| anyhow!("Source submission missing language"))?;

        // Fetch source code from database
        let source_code = self.fetch_source_code(&job.submission_id).await?;

        // Create temp directory for build
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temp directory")?;
        let build_dir = temp_dir.path();

        // Write source file and determine compile command
        let (source_file, compile_cmd) = self.get_compile_command(language, build_dir)?;
        fs::write(build_dir.join(&source_file), &source_code).await?;

        // Resolve the container image
        let spec = resolve_image(&self.config, Some(language));
        ensure_image(&self.config, &spec.image).await?;

        // Build a single shell string so we can run it as `sh -c "..."`
        let shell_cmd = compile_cmd.join(" ");

        let output = run_in_container(
            &self.config,
            &spec,
            build_dir,
            &["sh", "-c", &shell_cmd],
        )
        .await?;

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
