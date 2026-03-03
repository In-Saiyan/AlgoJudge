//! Sandboxing via cgroups v2 and Linux namespaces.
//!
//! Provides resource isolation (memory, CPU, PIDs) for user submissions,
//! generators, and checkers. When cgroups v2 are unavailable (e.g. inside
//! unprivileged containers), falls back to reading `/proc/{pid}/status`
//! for memory metrics.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Once;

use anyhow::{anyhow, Context, Result};
use tokio::fs;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Base path for cgroup v2 hierarchy used by Minos.
const CGROUP_BASE: &str = "/sys/fs/cgroup/minos";

/// One-time cgroup v2 root delegation.
///
/// In a container with its own cgroup namespace the root
/// `cgroup.subtree_control` starts empty.  Before we can create child
/// cgroups with memory/pid controllers we must:
///   1. Create a "service" child cgroup and move all existing root
///      processes into it (the kernel forbids enabling controllers on a
///      cgroup that has processes directly in it).
///   2. Write `+memory +pids` to the root `cgroup.subtree_control`.
///
/// This is run at most once per process lifetime.
static CGROUP_INIT: Once = Once::new();

fn init_cgroup_root() {
    let root = Path::new("/sys/fs/cgroup");
    let controllers_path = root.join("cgroup.controllers");

    // Bail if cgroup v2 isn't mounted at all.
    if !controllers_path.exists() {
        tracing::debug!("cgroup v2 root not found — skipping delegation init");
        return;
    }

    // Create a service cgroup for the Minos daemon process and its
    // threads so the root cgroup has no direct members.
    let service_dir = root.join("minos.service");
    if let Err(e) = std::fs::create_dir_all(&service_dir) {
        tracing::warn!("failed to create minos.service cgroup: {e}");
        return;
    }

    // Move every PID currently in the root cgroup into minos.service.
    let procs = match std::fs::read_to_string(root.join("cgroup.procs")) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("failed to read root cgroup.procs: {e}");
            return;
        }
    };
    let service_procs = service_dir.join("cgroup.procs");
    for pid in procs.lines().filter(|l| !l.is_empty()) {
        let _ = std::fs::write(&service_procs, pid);
    }

    // Now enable memory + pids on the root subtree.
    if let Err(e) = std::fs::write(root.join("cgroup.subtree_control"), "+memory +pids") {
        tracing::warn!("failed to enable controllers at root: {e}");
    } else {
        tracing::info!("cgroup v2 controllers (memory, pids) delegated at root");
    }
}

/// Manages per-submission cgroup-based sandboxing.
pub struct Sandbox {
    /// Cgroup directory for this sandbox instance.
    cgroup_dir: PathBuf,
    /// Whether cgroups v2 were successfully initialised.
    cgroup_available: bool,
}

/// Resource usage metrics captured after execution.
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Peak memory usage in KB.
    pub memory_kb: u64,
    /// Total CPU time consumed in milliseconds (user + system).
    pub cpu_time_ms: u64,
    /// `true` when metrics came from cgroup accounting.
    pub from_cgroup: bool,
}

impl Sandbox {
    /// Create and initialise a new sandbox.
    ///
    /// Sets memory, swap and PID limits via cgroups v2.  If the cgroup
    /// filesystem is not writable the sandbox degrades gracefully and only
    /// `/proc`-based monitoring is available.
    pub async fn create(sandbox_id: &str, memory_limit_kb: u64, max_pids: i32) -> Self {
        let cgroup_dir = PathBuf::from(CGROUP_BASE).join(sandbox_id);

        let cgroup_available = Self::setup_cgroup(&cgroup_dir, memory_limit_kb, max_pids)
            .await
            .unwrap_or_else(|e| {
                tracing::debug!("cgroups v2 unavailable — falling back to /proc monitoring: {e}");
                false
            });

        Self {
            cgroup_dir,
            cgroup_available,
        }
    }

    // ------------------------------------------------------------------
    // Cgroup setup
    // ------------------------------------------------------------------

    async fn setup_cgroup(dir: &Path, memory_limit_kb: u64, max_pids: i32) -> Result<bool> {
        // Bail early if the cgroup v2 root is not mounted.
        if !PathBuf::from("/sys/fs/cgroup/cgroup.controllers").exists() {
            anyhow::bail!("cgroup v2 not mounted");
        }

        // One-time: migrate the daemon out of the root cgroup and enable
        // memory + pids controllers so child cgroups can use them.
        CGROUP_INIT.call_once(init_cgroup_root);

        // Ensure the parent directory (/sys/fs/cgroup/minos) exists and
        // has the required controllers delegated to its subtree.
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)
                .await
                .context("create cgroup parent dir")?;
            // Enable memory + pids controllers for child cgroups.
            // This is idempotent and harmless if already set.
            let _ = fs::write(
                parent.join("cgroup.subtree_control"),
                "+memory +pids",
            )
            .await;
        }

        fs::create_dir_all(dir).await.context("create cgroup dir")?;

        // memory.max (bytes)
        let mem_bytes = memory_limit_kb.saturating_mul(1024);
        fs::write(dir.join("memory.max"), mem_bytes.to_string())
            .await
            .context("set memory.max")?;

        // Disable swap so OOM kill triggers at the real limit.
        let _ = fs::write(dir.join("memory.swap.max"), "0").await;

        // pids.max — add a buffer for runtime threads (Go needs ~10 for
        // GC/scheduler, Rust/C++ need fewer but still a handful for
        // signal handlers and I/O threads).  +16 is generous enough
        // for any runtime while still preventing fork bombs.
        if max_pids > 0 {
            let limit = (max_pids as u64).saturating_add(16);
            fs::write(dir.join("pids.max"), limit.to_string())
                .await
                .context("set pids.max")?;
        }

        Ok(true)
    }

    // ------------------------------------------------------------------
    // Process management
    // ------------------------------------------------------------------

    /// Return the path to `cgroup.procs` for use in `pre_exec` hooks.
    ///
    /// Returns `None` if cgroups are unavailable.
    pub fn cgroup_procs_path(&self) -> Option<PathBuf> {
        if self.cgroup_available {
            Some(self.cgroup_dir.join("cgroup.procs"))
        } else {
            None
        }
    }

    /// Returns `true` if the cgroup recorded an OOM-kill event.
    pub async fn was_oom_killed(&self) -> bool {
        if !self.cgroup_available {
            return false;
        }
        let events_path = self.cgroup_dir.join("memory.events");
        let content = match fs::read_to_string(&events_path).await {
            Ok(c) => c,
            Err(_) => return false,
        };
        // Format: `oom_kill <count>`
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("oom_kill ") {
                if let Ok(n) = rest.trim().parse::<u64>() {
                    return n > 0;
                }
            }
        }
        false
    }

    // ------------------------------------------------------------------
    // Metrics collection
    // ------------------------------------------------------------------

    /// Read resource usage.  Prefers cgroup stats; falls back to `/proc`.
    pub async fn read_usage(&self, pid: Option<u32>) -> ResourceUsage {
        if self.cgroup_available {
            return self.read_cgroup_usage().await;
        }
        if let Some(p) = pid {
            return Self::read_proc_usage(p).await;
        }
        ResourceUsage::default()
    }

    async fn read_cgroup_usage(&self) -> ResourceUsage {
        let memory_kb = self.read_memory_peak().await.unwrap_or(0);
        let cpu_time_ms = self.read_cpu_time_ms().await.unwrap_or(0);
        ResourceUsage {
            memory_kb,
            cpu_time_ms,
            from_cgroup: true,
        }
    }

    /// Read peak memory from `memory.peak` (bytes → KB).
    /// Falls back to `memory.current` when peak is unavailable.
    async fn read_memory_peak(&self) -> Result<u64> {
        let peak = self.cgroup_dir.join("memory.peak");
        if peak.exists() {
            let val = fs::read_to_string(&peak).await?;
            let bytes: u64 = val.trim().parse().unwrap_or(0);
            return Ok(bytes / 1024);
        }
        let current = self.cgroup_dir.join("memory.current");
        let val = fs::read_to_string(&current).await?;
        let bytes: u64 = val.trim().parse().unwrap_or(0);
        Ok(bytes / 1024)
    }

    /// Read total CPU time (user + system) from `cpu.stat` → ms.
    async fn read_cpu_time_ms(&self) -> Result<u64> {
        let stat = self.cgroup_dir.join("cpu.stat");
        if !stat.exists() {
            return Ok(0);
        }
        let content = fs::read_to_string(&stat).await?;
        // `usage_usec <n>`
        for line in content.lines() {
            if let Some(usec_str) = line.strip_prefix("usage_usec ") {
                let usec: u64 = usec_str.trim().parse().unwrap_or(0);
                return Ok(usec / 1000);
            }
        }
        Ok(0)
    }

    /// Fallback: read `VmPeak` from `/proc/{pid}/status`.
    async fn read_proc_usage(pid: u32) -> ResourceUsage {
        let path = format!("/proc/{pid}/status");
        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => return ResourceUsage::default(),
        };
        let mut memory_kb = 0u64;
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("VmPeak:") {
                let rest = rest.trim();
                if let Some(kb) = rest.strip_suffix(" kB") {
                    memory_kb = kb.trim().parse().unwrap_or(0);
                }
                break;
            }
        }
        ResourceUsage {
            memory_kb,
            cpu_time_ms: 0,
            from_cgroup: false,
        }
    }

    // ------------------------------------------------------------------
    // Sandboxed binary execution
    // ------------------------------------------------------------------

    /// Execute a binary inside this sandbox with full cgroup + namespace
    /// isolation, timeout enforcement, and OOM detection.
    ///
    /// Returns `(stdout, stderr, exit_code, memory_kb)` on success.
    /// Returns an error on timeout, spawn failure, or OOM kill.
    ///
    /// ## Isolation applied
    ///
    /// * **cgroups v2** – process is joined to this sandbox's cgroup
    ///   before exec so memory/PID limits are enforced immediately.
    /// * **Network namespace** – `unshare(CLONE_NEWNET)` isolates from
    ///   all network interfaces when `network_allowed` is `false`.
    /// * **Process** – stdin is `/dev/null`, `kill_on_drop` ensures
    ///   cleanup if the future is cancelled.
    pub async fn run_sandboxed(
        &self,
        binary_path: &Path,
        args: &[&str],
        time_limit_ms: u64,
        network_allowed: bool,
        capture_stdout: bool,
    ) -> Result<SandboxedOutput> {
        let mut cmd = Command::new(binary_path);
        for arg in args {
            cmd.arg(arg);
        }

        cmd.stdin(Stdio::null())
            .stdout(if capture_stdout {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Pre-exec: join cgroup
        if let Some(procs_path) = self.cgroup_procs_path() {
            unsafe {
                cmd.pre_exec(move || {
                    std::fs::write(&procs_path, std::process::id().to_string())?;
                    Ok(())
                });
            }
        }

        // Pre-exec: network namespace isolation
        if !network_allowed {
            unsafe {
                cmd.pre_exec(|| {
                    match nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET) {
                        Ok(()) => {}
                        Err(_e) => {
                            eprintln!(
                                "[minos] WARNING: unshare(CLONE_NEWNET) failed — \
                                 process will run without network isolation"
                            );
                        }
                    }
                    Ok(())
                });
            }
        }

        // Spawn & wait with timeout (+ 100ms buffer)
        let child = cmd.spawn().context("failed to spawn sandboxed process")?;
        let child_pid = child.id();
        let hard_limit = Duration::from_millis(time_limit_ms.saturating_add(100));
        let result = timeout(hard_limit, child.wait_with_output()).await;

        // Collect metrics
        let usage = self.read_usage(child_pid).await;
        let oom_killed = self.was_oom_killed().await;

        match result {
            Ok(Ok(output)) => {
                // Check OOM / signal kills
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    if let Some(signal) = output.status.signal() {
                        if signal == 9 && (oom_killed || usage.memory_kb > 0) {
                            return Err(anyhow!(
                                "Process killed by OOM (signal {}, peak memory {}KB)",
                                signal,
                                usage.memory_kb
                            ));
                        }
                        return Err(anyhow!(
                            "Process killed by signal {} (peak memory {}KB)",
                            signal,
                            usage.memory_kb
                        ));
                    }
                }

                Ok(SandboxedOutput {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    exit_code: output.status.code().unwrap_or(-1),
                    memory_kb: usage.memory_kb,
                    oom_killed,
                })
            }
            Ok(Err(e)) => Err(anyhow!("Failed to execute sandboxed process: {}", e)),
            Err(_) => Err(anyhow!(
                "Process exceeded time limit ({}ms)",
                time_limit_ms
            )),
        }
    }

    // ------------------------------------------------------------------
    // Cleanup
    // ------------------------------------------------------------------

    /// Kill remaining processes and remove the cgroup directory.
    pub async fn cleanup(&self) {
        if !self.cgroup_available {
            return;
        }
        // Signal cgroup kill (kernel ≥ 5.14).
        let _ = fs::write(self.cgroup_dir.join("cgroup.kill"), "1").await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if let Err(e) = fs::remove_dir(&self.cgroup_dir).await {
            tracing::warn!(
                cgroup = %self.cgroup_dir.display(),
                error = %e,
                "failed to remove cgroup directory"
            );
        }
    }
}

/// Output captured from a sandboxed process execution.
#[derive(Debug)]
pub struct SandboxedOutput {
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
    /// Process exit code (`-1` if unavailable).
    pub exit_code: i32,
    /// Peak memory usage in KB (from cgroup or `/proc`).
    pub memory_kb: u64,
    /// Whether the process was killed by the cgroup OOM killer.
    pub oom_killed: bool,
}
