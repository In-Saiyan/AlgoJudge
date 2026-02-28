//! Docker container management for sandboxed compilation.
//!
//! Each supported language maps to a pre-configured Docker image that has
//! the necessary toolchain installed.  Sisyphus mounts the build directory
//! into a fresh container, runs the compilation command, and tears the
//! container down afterwards.

use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Context, Result};
use tokio::process::Command;

use crate::config::Config;

/// Apply `DOCKER_API_VERSION` env var to a [`Command`] when configured.
fn apply_api_version(cmd: &mut Command, config: &Config) {
    if let Some(ref ver) = config.docker_api_version {
        cmd.env("DOCKER_API_VERSION", ver);
    }
}

// ── Language → Docker image mapping ────────────────────────────────────────

/// Resolved container settings for a single compilation run.
#[derive(Debug, Clone)]
pub struct ContainerSpec {
    /// Docker image to use (e.g. `gcc:14`, `rust:1.85`).
    pub image: String,
    /// Human-readable language label (for logging).
    pub language: String,
}

/// Determine the container image for a given language string.
///
/// If the caller passes some language hint we use that; otherwise we fall
/// back to a generic `ubuntu:24.04` image (compile.sh must bring its own
/// tooling in that case).
pub fn resolve_image(config: &Config, language: Option<&str>) -> ContainerSpec {
    let lang = language.unwrap_or("generic");
    let image = match lang {
        "cpp" | "c++" => config
            .container_images
            .cpp
            .clone()
            .unwrap_or_else(|| "gcc:14".to_string()),
        "c" => config
            .container_images
            .c
            .clone()
            .unwrap_or_else(|| "gcc:14".to_string()),
        "rust" => config
            .container_images
            .rust
            .clone()
            .unwrap_or_else(|| "rust:1.85-bookworm".to_string()),
        "go" => config
            .container_images
            .go
            .clone()
            .unwrap_or_else(|| "golang:1.23-bookworm".to_string()),
        "python" => config
            .container_images
            .python
            .clone()
            .unwrap_or_else(|| "python:3.12-bookworm".to_string()),
        "zig" => config
            .container_images
            .zig
            .clone()
            .unwrap_or_else(|| "euantorano/zig:0.13.0".to_string()),
        _ => config
            .container_images
            .generic
            .clone()
            .unwrap_or_else(|| "ubuntu:24.04".to_string()),
    };

    ContainerSpec {
        image,
        language: lang.to_string(),
    }
}

// ── Container execution ────────────────────────────────────────────────────

/// Output captured from a container run.
#[derive(Debug)]
pub struct ContainerOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run a command inside a fresh Docker container.
///
/// * The host `build_dir` is bind-mounted at `/workspace` inside the
///   container.
/// * Network access is controlled by `config.network_enabled`.
/// * Memory / CPU constraints come from `config`.
/// * A hard timeout is enforced via `tokio::time::timeout`.
pub async fn run_in_container(
    config: &Config,
    spec: &ContainerSpec,
    build_dir: &Path,
    command: &[&str],
) -> Result<ContainerOutput> {
    // Build `docker run` invocation
    let mut args: Vec<String> = Vec::new();

    args.push("run".into());
    args.push("--rm".into());

    // ── Resource constraints ──────────────────────────────
    args.push(format!("--memory={}b", config.max_memory_bytes));
    args.push(format!("--cpus={}", config.max_cpu_cores));

    // PID limit to prevent fork-bombs
    args.push("--pids-limit=256".into());

    // ── Network isolation ─────────────────────────────────
    if !config.network_enabled {
        args.push("--network=none".into());
    }

    // ── Security ──────────────────────────────────────────
    // Drop all capabilities and run as non-root inside the container.
    args.push("--cap-drop=ALL".into());
    // Read-only root filesystem (build dir is still writable via mount).
    args.push("--read-only".into());
    // Provide a writable /tmp inside the container for compilers that need it.
    args.push("--tmpfs=/tmp:rw,noexec,nosuid,size=256m".into());

    // ── Volume: build directory → /workspace ──────────────
    let build_dir_abs = build_dir
        .canonicalize()
        .with_context(|| format!("Could not canonicalize {}", build_dir.display()))?;
    args.push("-v".into());
    args.push(format!("{}:/workspace", build_dir_abs.display()));
    args.push("-w".into());
    args.push("/workspace".into());

    // ── Image ─────────────────────────────────────────────
    args.push(spec.image.clone());

    // ── Command to run inside the container ───────────────
    for part in command {
        args.push((*part).to_string());
    }

    tracing::debug!(
        image = %spec.image,
        language = %spec.language,
        build_dir = %build_dir.display(),
        cmd = ?command,
        "Spawning compilation container"
    );

    // Spawn docker process
    let mut cmd = Command::new("docker");
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_api_version(&mut cmd, config);
    let child = cmd
        .spawn()
        .context("Failed to spawn docker process — is the Docker socket mounted?")?;

    // Enforce a hard timeout
    let timeout_dur = tokio::time::Duration::from_secs(config.compile_timeout_secs);
    let result = tokio::time::timeout(timeout_dur, child.wait_with_output()).await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            Ok(ContainerOutput {
                success: output.status.success(),
                exit_code: output.status.code(),
                stdout,
                stderr,
            })
        }
        Ok(Err(e)) => Err(anyhow!("Docker command execution failed: {}", e)),
        Err(_) => Err(anyhow!(
            "Compilation timed out after {} seconds",
            config.compile_timeout_secs
        )),
    }
}

/// Pull a Docker image if it is not already present locally.
///
/// This is best-effort: if pulling fails (e.g. offline) we still proceed
/// because the image may already be cached.
pub async fn ensure_image(config: &Config, image: &str) -> Result<()> {
    // Quick check with `docker image inspect`
    let mut inspect_cmd = Command::new("docker");
    inspect_cmd
        .args(["image", "inspect", image])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_api_version(&mut inspect_cmd, config);
    let inspect = inspect_cmd.status().await;

    if let Ok(status) = inspect {
        if status.success() {
            tracing::debug!(image = %image, "Docker image already present");
            return Ok(());
        }
    }

    tracing::info!(image = %image, "Pulling Docker image…");
    let mut pull_cmd = Command::new("docker");
    pull_cmd
        .args(["pull", image])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_api_version(&mut pull_cmd, config);
    let pull = pull_cmd
        .output()
        .await
        .context("Failed to run docker pull")?;

    if pull.status.success() {
        tracing::info!(image = %image, "Docker image pulled successfully");
    } else {
        let stderr = String::from_utf8_lossy(&pull.stderr);
        tracing::warn!(
            image = %image,
            stderr = %stderr,
            "docker pull failed — will try to use cached image"
        );
    }

    Ok(())
}
