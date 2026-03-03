//! Sandboxing via cgroups v2 and Linux namespaces.
//!
//! Provides resource isolation (memory, CPU, PIDs) for user submissions.
//! When cgroups v2 are unavailable (e.g. inside unprivileged containers),
//! falls back to reading `/proc/{pid}/status` for memory metrics.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::fs;

/// Base path for cgroup v2 hierarchy used by Minos.
const CGROUP_BASE: &str = "/sys/fs/cgroup/minos";

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

        fs::create_dir_all(dir).await.context("create cgroup dir")?;

        // memory.max (bytes)
        let mem_bytes = memory_limit_kb.saturating_mul(1024);
        fs::write(dir.join("memory.max"), mem_bytes.to_string())
            .await
            .context("set memory.max")?;

        // Disable swap so OOM kill triggers at the real limit.
        let _ = fs::write(dir.join("memory.swap.max"), "0").await;

        // pids.max — add a small buffer for shell wrapper processes.
        if max_pids > 0 {
            let limit = (max_pids as u64).saturating_add(4);
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
