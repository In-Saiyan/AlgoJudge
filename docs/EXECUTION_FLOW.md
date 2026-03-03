# Olympus Execution Flow

This document describes how files move through the system during compilation and execution.

## Overview

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   VANGUARD   │───▶│   SISYPHUS   │───▶│    MINOS     │    │    HORUS     │
│  (API Gate)  │    │  (Compiler)  │    │   (Judge)    │    │  (Cleaner)   │
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
       │                   │                   │                   │
       ▼                   ▼                   ▼                   ▼
  Upload ZIP          Compile in          Execute with        Clean stale
  Validate            Docker container    cgroups/namespaces  files
  Queue job           Save binary         Check output
```

> **Key distinction:** Sisyphus uses **Docker containers** for compilation
> sandboxing. Minos uses **cgroups v2 + Linux namespaces** directly (no Docker)
> for execution sandboxing.

---

## User Submission Format

Users must upload a ZIP file containing:

```
submission.zip
├── compile.sh      # Required: Compilation script
├── run.sh          # Required: Execution script
└── [source files]  # Your code (e.g., main.cpp, solution.rs)
```

### Example `compile.sh` (C++)
```bash
#!/bin/bash
g++ -O2 -std=c++17 -o solution main.cpp
```

### Example `run.sh`
```bash
#!/bin/bash
./solution "$1" "$2"
```

> **I/O convention:** Minos invokes the compiled binary as `./solution <input_file> <output_file>`.
> The binary must read from the input file path (argv[1]) and write to the output file path (argv[2]).
> Standard stdin/stdout piping is **not** used — this avoids broken-pipe errors with large I/O.

---

## Phase 1: Vanguard (API Gateway)

**Actions:**
1. Receives multipart upload (or source code JSON)
2. If `contest_id` is provided: validates contest is active, user is a participant/collaborator/admin, and problem is assigned to that contest
3. If `contest_id` is omitted (standalone submission): validates problem exists and user is authenticated
4. Validates ZIP structure (compile.sh, run.sh exist)
5. Security checks (no symlinks, path traversal, zip bombs)
6. Saves to persistent storage
7. Queues compilation job to Redis Stream

**File Storage:**
```
# Contest submission
/mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip

# Standalone (practice) submission — no contest_id
/mnt/data/submissions/standalone/{user_id}/{submission_id}.zip
```

**Redis Stream Message (compile_queue):**
```json
{
  "submission_id": "abc-123",
  "type": "zip",
  "file_path": "/mnt/data/submissions/.../abc-123.zip",
  "language": "cpp"
}
```

Fields sent via `XADD`:
| Field | Always Present | Description |
|-------|----------------|-------------|
| `submission_id` | Yes | UUID of the submission |
| `type` | Yes | `"zip"` or `"source"` |
| `file_path` | Only for ZIP | Path to the stored ZIP file |
| `language` | Only if provided | Language hint for Docker image selection |

**Queue:** `compile_queue`

> **Note:** The `language` field is optional for ZIP submissions. When present,
> Sisyphus selects a language-specific Docker image. When absent,
> Sisyphus uses `ubuntu:24.04` as a generic fallback.
>
> For source submissions, `file_path` is omitted — source code is stored in the
> DB `source_code` column. **Source code compilation is currently unimplemented
> in Sisyphus; only ZIP submissions are fully supported.**

---

## Phase 2: Sisyphus (Compiler Service)

**Actions:**
1. Consumes job from `compile_queue` (XREADGROUP, consumer group `sisyphus_group`)
2. Creates temporary build directory under `BUILD_DIR_BASE` (default `/mnt/data/temp/builds`)
3. Extracts ZIP to temp directory
4. Strips CRLF line endings from `compile.sh` and `run.sh`
5. Resolves a **language-specific Docker image** from the `language` hint
6. Ensures the image is available (lazy pull if missing)
7. Spawns an isolated Docker container from that image
8. Runs `compile.sh` inside the container
9. Detects compiled binary (searches for `main`, `a.out`, `solution`, `run`)
10. Saves binary to persistent storage
11. Queues to `run_queue` or marks `COMPILATION_ERROR`

### Language → Docker Image Mapping

Each language resolves to a dedicated image so the right toolchain is
always available. Images are pulled lazily on first use and cached
afterwards. Defaults can be overridden via `CONTAINER_IMAGE_*` env vars.

| Language | Default Image | Override Env Var |
|----------|---------------|------------------|
| `cpp` / `c++` | `gcc:latest` | `CONTAINER_IMAGE_CPP` |
| `c` | `gcc:latest` | `CONTAINER_IMAGE_C` |
| `rust` | `rust:1.85-bookworm` | `CONTAINER_IMAGE_RUST` |
| `go` | `golang:1.23-bookworm` | `CONTAINER_IMAGE_GO` |
| `python` | `python:3.12-bookworm` | `CONTAINER_IMAGE_PYTHON` |
| `zig` | `euantorano/zig:0.13.0` | `CONTAINER_IMAGE_ZIG` |
| *(unknown/omitted)* | `ubuntu:24.04` | `CONTAINER_IMAGE_GENERIC` |

### Build Directory Structure

```
{BUILD_DIR_BASE}/<random>/     # tempfile::tempdir
├── compile.sh
├── run.sh
├── main.cpp
└── solution  (generated binary after compilation)
```

### Docker Container Configuration

**Sandbox Settings (applied to every compilation container):**
```
--rm                              # Remove after exit
--network=none                    # NO network access (unless NETWORK_ENABLED=true)
--memory={MAX_MEMORY_BYTES}b      # Memory limit (default 2GB)
--cpus={MAX_CPU_CORES}            # CPU cores (default 2)
--pids-limit=256                  # Limit process spawning
--read-only                       # Read-only root filesystem
--tmpfs /tmp:rw,noexec,nosuid,size=256m
--tmpfs /root/.cache:rw,noexec,nosuid,size=256m
--cap-drop=ALL                    # Drop all capabilities
```

**Volume Mounts:**

The build directory is mounted into the container at `/workspace`. Three
mount strategies are supported (checked in order):

| Strategy | Config | Use Case |
|----------|--------|----------|
| Docker named volume | `DOCKER_VOLUME_NAME` | Docker-in-Docker setups |
| Host path translation | `DOCKER_HOST_DATA_PATH` | Host path differs from container path |
| Direct bind mount | (default) | Standard deployment |

**Execution (inside container):**
```bash
$ cd /workspace
$ chmod +x compile.sh
$ sh -c ./compile.sh

# For a cpp submission the container is gcc:latest, so g++ is available:
#   compile.sh runs: g++ -O2 -std=c++17 -o solution main.cpp
#   Output: /workspace/solution (compiled binary)
```

**Timeout:** Controlled by `COMPILE_TIMEOUT_SECS` (default 30 seconds), enforced via
`tokio::time::timeout` on the Docker container execution.

### Binary Detection

After compilation, Sisyphus searches the build directory for:
1. Files named `main`, `a.out`, `solution`, or `run`
2. If no binary found but `run.sh` exists (interpreted languages like Python),
   the **entire build directory** is recursively copied as the binary artifact

### Output

**Binary Storage:**
```
/mnt/data/binaries/users/{submission_id}_bin          # compiled binary (file)
/mnt/data/binaries/users/{submission_id}_bin/         # or directory for interpreted langs
```

**DB Updates:**
- On job start: `status = 'compiling'`
- On success: `status = 'compiled'`, `compiled_at = NOW()`
- On failure: `status = 'compilation_error'`, `compilation_log = <stderr output>`

**Redis Stream Message (run_queue):**
```json
{
  "submission_id": "abc-123",
  "binary_path": "/mnt/data/binaries/users/abc-123_bin"
}
```

> [!NOTE]
> Sisyphus only sends `submission_id` and `binary_path` to the run queue.
> Minos looks up `problem_id`, `contest_id`, `time_limit_ms`, `memory_limit_kb`,
> `num_test_cases`, `max_threads`, and `network_allowed` directly from the database
> by joining `submissions`, `problems`, and `contest_problems` tables. This ensures
> Minos always uses the latest problem configuration, including contest-level overrides.

**Cleanup:** Build directory is a `tempfile::tempdir` and is dropped automatically when Sisyphus finishes processing the job.

### Retry & Dead Letter

- **Retryable errors:** `"timed out"`, `"connection refused"`, `"no space left"`, `"resource temporarily unavailable"`, `"cannot allocate memory"`, `"too many open files"` (case-insensitive substring match)
- **Max retries:** 3 (hardcoded)
- **Backoff:** Exponential — delay = `1000ms * 2^(retry_count - 1)`
- **Dead letter stream:** `compile_queue_dead_letter` — stores `submission_id`, `type`, `retry_count`, `error`, `failed_at`
- Non-retryable errors immediately mark the submission as `compilation_error`

### Graceful Shutdown

Uses `AtomicBool` flag. Ctrl+C and SIGTERM handlers set the flag. The consumer loop
finishes the **current job** before exiting — no mid-job abort.

### Consumer Group Resilience

On startup, creates consumer groups for both `compile_queue` and `compile_queue_dead_letter` via `XGROUP CREATE ... $ MKSTREAM`. If a `NOGROUP` error is detected during processing, the consumer group is automatically re-created before retrying.

---

## Phase 3: Minos (Judge Service)

**Actions:**
1. Consumes job from `run_queue` (XREADGROUP, consumer group `minos_group`)
2. On startup, claims pending messages idle > 60s via `XPENDING` + `XCLAIM`
3. **Checks if generator and checker binaries exist** for the problem
   - If either is missing → sets submission status to `queue_pending`, ACKs the message, and moves on
   - The submission will be automatically re-queued when the missing binary is uploaded via `POST /api/v1/problems/{id}/generator` or `/checker`
4. Loads compiled binary from storage
5. Gets/generates test cases (lazy generation)
6. For each test case: run binary in cgroup sandbox, check output
7. **Stops on first failure** (remaining test cases are skipped)
8. Updates verdict in database
9. Records Prometheus metrics

### Test Case Generation (if not cached)

```
/mnt/data/binaries/problems/{problem_id}/generator
              │
              ▼
./generator {test_number} > input.txt
(60s timeout, direct process — no cgroup isolation)
              │
              ▼
/mnt/data/testcases/{problem_id}/
├── input_001.txt
├── input_002.txt
├── ...
└── .last_access   (RFC3339 timestamp for cache invalidation)
```

> **Note:** Generator and checker binaries are run as direct child processes
> without cgroup or namespace isolation. This is a known gap — the design spec
> calls for sandboxing untrusted problem-setter code but it is not yet implemented.

### Execution Sandbox (cgroups v2 + namespaces — Per Test Case)

Minos does **not** use Docker for execution. Instead, it uses Linux cgroups v2
and namespaces directly for more lightweight and precise resource control.

**cgroups v2 sandbox** (at `/sys/fs/cgroup/minos/{sandbox_id}`):
```
memory.max    = {memory_limit_kb} * 1024     # Per-problem memory limit
memory.swap.max = 0                          # Swap disabled
pids.max      = {max_threads} + 4            # Thread limit + buffer for shell wrappers
```

**Namespace isolation:**
- When `network_allowed=false` (per-problem default): `unshare(CLONE_NEWNET)` in child process
- Falls back gracefully if `EPERM` (e.g., unprivileged user)

**Process execution:**
```bash
# For compiled binaries (files):
$ ./solution <input_file> <output_file>

# For interpreted languages (directory with run.sh):
$ bash run.sh <input_file> <output_file>
```

**Environment variables passed to the binary:**
| Variable | Description |
|----------|-------------|
| `INPUT_FILE` | Path to the input file |
| `OUTPUT_FILE` | Path to the output file |
| `MAX_THREADS` | Maximum thread count allowed |
| `NETWORK_ALLOWED` | Whether network access is permitted |
| `TIME_LIMIT_MS` | Time limit in milliseconds |
| `MEMORY_LIMIT_KB` | Memory limit in kilobytes |

**Process settings:**
- `stdin = /dev/null`
- `stdout = piped` (captured)
- `stderr = piped` (captured)
- `kill_on_drop = true`
- Hard timeout: `time_limit_ms + 100ms` buffer

**Metrics collection:**
- **Memory:** Prefers cgroup `memory.peak` (fallback `memory.current`, then `/proc/{pid}/status` → `VmPeak`)
- **CPU time:** From `cpu.stat` → `usage_usec`
- **OOM detection:** Reads `memory.events` for `oom_kill > 0`

**Cleanup after each test case:**
- Write `1` to `cgroup.kill` (kernel ≥5.14)
- Wait 100ms
- Remove cgroup directory

### Checker Verification

```
/mnt/data/binaries/problems/{problem_id}/checker
```

**Execution:**
```bash
$ ./checker <input_file> <user_output> <input_file>
```

> The checker follows the testlib convention. Since there are no pre-generated
> expected outputs, the input file is passed as the third argument (answer file).

**Exit Codes (Testlib convention):**

| Code | Verdict | Meaning |
|------|---------|---------|
| 0 | AC (Accepted) | Output is correct |
| 1 | WA (Wrong Answer) | Output is incorrect |
| 2 | PE (Presentation Error) | Format issue (treated as WA) |
| 3 | JE (Judge Error) | Checker crashed |
| 7 | PC (Partial Credit) | Partial credit (scoring problems) |

**Checker stderr contains verdict message:**
```
ok Correct answer: 42
wrong answer Expected 42, got 17
```

**Checker timeout:** 60 seconds (configurable via `CHECKER_TIME_LIMIT_MS`).

> **Note:** The checker runs as a plain `tokio::process::Command` without
> cgroup or namespace isolation (same caveat as generators).

### Verdict Determination

**Stop-on-first-failure:** Minos executes test cases sequentially and stops as soon
as the first non-AC verdict is encountered. Remaining test cases are skipped.

**Overall verdict:**
- If all test cases pass → `Accepted`
- Otherwise → the verdict of the **first failing** test case

**Score calculation:**
```
score = 100.0 * (passed_count / total_count)
```

**Output limit:** 64 MB (configurable via `OUTPUT_LIMIT_BYTES`). `OutputLimitExceeded` maps to `"runtime_error"` in the DB.

**Example (5 test cases):**
```
TC1: AC, TC2: AC, TC3: WA, TC4: (skipped), TC5: (skipped)
Final Verdict: WA
Score: 40.0
```

### Database Updates

**submissions table:**
| id | status | score | max_time_ms | max_memory_kb | passed_test_cases | total_test_cases |
|----|--------|-------|-------------|---------------|-------------------|------------------|
| abc-123 | wrong_answer | 40 | 52 | 12100 | 2 | 5 |

> **Note:** The `status` column holds the verdict value directly (e.g. `accepted`,
> `wrong_answer`, `time_limit`, `runtime_error`, `system_error`). There is no
> separate `verdict` column on the `submissions` table.

**submission_results table (UPSERT):**
| submission_id | test_case_number | verdict | time_ms | memory_kb | checker_output |
|---------------|------------------|---------|---------|-----------|----------------|
| abc-123 | 1 | accepted | 45 | 12000 | ok |
| abc-123 | 2 | accepted | 52 | 12100 | ok |
| abc-123 | 3 | wrong_answer | 48 | 11900 | wrong... |

**Cleanup:** `rm -rf /mnt/data/temp/{submission_id}/`

**Acknowledge:** `XACK run_queue minos_group {message_id}`

### Retry & Dead Letter

- If judging fails (not `queue_pending`): re-queued via `XADD` with incremented `retry_count` (up to 3, no exponential backoff)
- After max retries: sent to `run_queue_dlq` dead-letter stream with `submission_id`, `problem_id`, `contest_id`, `error`, `retry_count`, `failed_at`. DB status set to `system_error`.

### Prometheus Metrics

Exported on port `METRICS_PORT` (default 9091) via Axum HTTP server at `/metrics`:

| Metric | Type | Labels |
|--------|------|--------|
| `judge_execution_duration_seconds` | Histogram | `problem_id` |
| `judge_memory_usage_bytes` | Histogram | `problem_id` |
| `judge_verdict_total` | IntCounterVec | `verdict` |
| `judge_jobs_processed_total` | IntCounter | — |
| `judge_jobs_failed_total` | IntCounter | — |
| `judge_active_jobs` | IntGauge | — |

Also exposes `/health` returning `"OK"`.

---

## Phase 4: Horus (Cleaner Service)

Runs on cron schedules to clean up stale/orphaned files.

### Cleanup Policies

| Policy | Schedule | Rule | Target |
|--------|----------|------|--------|
| Stale Testcases | hourly (`0 0 * * * *`) | `IsDirectory & LastAccessOlderThan(6h) & !HasProblemRecord` | `/mnt/data/testcases/` |
| Orphan Temp Dirs | every 15 min (`0 */15 * * * *`) | `IsDirectory & CreatedOlderThan(1h) & !HasActiveSubmission` | `/mnt/data/temp/` |
| Orphan Binaries | daily @ 3am (`0 0 3 * * *`) | `IsFile & CreatedOlderThan(24h) & !HasSubmissionRecord` | `/mnt/data/binaries/users/` |
| Old Submissions | weekly Sun 4am (`0 0 4 * * 0`) | `CreatedOlderThan(retention_days)` | DB + filesystem |

> **Old Submissions** cleanup is disabled by default (`SUBMISSION_RETENTION_DAYS=0`).
> When enabled, it queries the DB for completed submissions older than the
> retention period and deletes the binary file, `submission_results` rows, and
> `submissions` row.

### Specification Pattern

Horus defines its own `CleanupSpec` trait (separate from `olympus-rules`'
`Specification` trait) with `And`, `Or`, `Not` combinators for composing
cleanup rules. DB-backed specs:

| Spec | DB Query |
|------|----------|
| `HasActiveSubmission` | `WHERE id = $1 AND status IN ('PENDING', 'COMPILING', 'JUDGING')` |
| `HasSubmissionRecord` | `WHERE id = $1` (any submission row exists) |
| `HasProblemRecord` | `WHERE id = $1` (problem exists in DB) |

### Config Reload via Redis Pub/Sub

Horus subscribes to the `config_reload` Redis pub/sub channel. When a message
with payload `"horus"` is received, it reloads policies from the `rule_configs`
table (`WHERE service = 'horus' AND enabled = true`). Auto-reconnects on error
with 5-second backoff.

> **Note:** The `PolicyStore` loads and stores policies from the DB, but the
> cleanup jobs currently use hardcoded spec compositions. Dynamic policy
> evaluation from the `PolicyStore` is not yet wired in.

---

## File Locations Summary

| Path | Purpose | Lifecycle |
|------|---------|-----------|
| `/mnt/data/submissions/{contest}/{user}/{id}.zip` | Contest ZIP upload | Permanent (until archived) |
| `/mnt/data/submissions/standalone/{user}/{id}.zip` | Standalone ZIP upload | Permanent (until archived) |
| `{BUILD_DIR_BASE}/<random>/` | Compilation workspace (temp dir) | Deleted after compile (tempfile::tempdir) |
| `/mnt/data/binaries/users/{id}_bin` | Compiled user binary (file or dir) | Deleted by Horus (24h+) |
| `/mnt/data/binaries/problems/{id}/generator` | Test generator | Permanent |
| `/mnt/data/binaries/problems/{id}/checker` | Output checker | Permanent |
| `/mnt/data/testcases/{problem_id}/` | Generated test inputs | Cached, cleaned after 6h |
| `/mnt/data/testcases/{problem_id}/.last_access` | Cache timestamp (RFC3339) | Updated on each access |
| `/mnt/data/temp/{id}/` | Execution scratch space | Deleted after judging |

---

## Queue Pending Flow

When a submission's problem does not yet have both generator and checker binaries,
the submission enters the `queue_pending` state instead of failing outright.

```
┌────────────┐   compiles ok   ┌────────────┐   binaries missing   ┌──────────────┐
│  PENDING   │ ───────────────▶│  COMPILED   │ ────────────────────▶│ QUEUE_PENDING│
└────────────┘                 └────────────┘                       └──────┬───────┘
                                                                          │
                                      ┌───────────────────────────────────┘
                                      │  binary upload triggers re-queue
                                      ▼
                               ┌────────────┐                       ┌──────────────┐
                               │  JUDGING   │ ─────────────────────▶│   JUDGED     │
                               └────────────┘                       └──────────────┘
```

1. **Vanguard** accepts the submission (contest or standalone) and queues it on `compile_queue`.
2. **Sisyphus** compiles it and pushes to `run_queue`.
3. **Minos** picks up the job. Before running test cases it checks whether
   `/mnt/data/binaries/problems/{problem_id}/generator` and `checker` exist.
   - **Both exist** → proceeds normally (JUDGING → verdict).
   - **Either missing** → sets status to `queue_pending`, ACKs the message, no
     retry/dead-letter.
4. When a problem setter later uploads the missing binary via
   `POST /api/v1/problems/{id}/generator` or `/checker`, Vanguard checks whether
   both binaries now exist. If so it:
   - Queries all submissions with `status = 'queue_pending'` for that problem.
   - Re-queues each on `run_queue` and resets its status to `compiled`.

This ensures no submission is silently lost when binaries are uploaded out of order.

---

## Submission Status Lifecycle

```
pending → compiling → compiled → judging → {verdict}
                  ↘                  ↘
           compilation_error    queue_pending → (re-queued) → judging → {verdict}
                                                                   ↘
                                                              system_error
```

**All possible statuses:**
| Status | Set By | Meaning |
|--------|--------|---------|
| `pending` | Vanguard | Queued for compilation |
| `compiling` | Sisyphus | Currently being compiled |
| `compiled` | Sisyphus | Compilation succeeded, queued for judging |
| `compilation_error` | Sisyphus | Compilation failed |
| `queue_pending` | Minos | Waiting for problem binaries |
| `judging` | Minos | Currently being judged |
| `accepted` | Minos | All test cases passed |
| `wrong_answer` | Minos | At least one test case failed |
| `time_limit` | Minos | Time limit exceeded |
| `memory_limit` | Minos | Memory limit exceeded |
| `runtime_error` | Minos | Runtime error (or output limit exceeded) |
| `system_error` | Minos | Internal error during judging |

---

## Security Summary

| Sandbox | Technology | Network | Memory | CPU | Fork/PID | Syscalls |
|---------|-----------|---------|--------|-----|----------|----------|
| **Compiler** | Docker container | Disabled (--network=none) | 2GB | 2 cores | Limited (256 PIDs) | Filtered (cap-drop ALL) |
| **Runner** | cgroups v2 + namespaces | Disabled (CLONE_NEWNET) | Per-problem | Per-problem threads | `max_threads + 4` | Not restricted (no seccomp) |
| **Generator** | cgroups v2 + namespaces | Disabled (CLONE_NEWNET) | 4GB default (configurable) | Not limited | 5 PIDs | Not restricted (no seccomp) |
| **Checker** | cgroups v2 + namespaces | Disabled (CLONE_NEWNET) | 4GB default (configurable) | Not limited | 5 PIDs | Not restricted (no seccomp) |

> **Remaining gap:** No seccomp profile is applied to user binaries, generators,
> or checkers. All other resource limits are enforced via cgroups v2 and
> network namespaces.

---

## Full Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              USER SUBMISSION                                 │
│   submission.zip { compile.sh, run.sh, main.cpp }                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  VANGUARD: Validate → Save to /mnt/data/submissions/ → Queue compile_queue  │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  SISYPHUS: Extract ZIP → Spawn Docker (lang-specific image, --network=none) │
│            Run compile.sh → Save binary → Queue run_queue                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  MINOS: Load binary → Check generator/checker exist (or queue_pending)      │
│         Generate/cache test cases                                           │
│         FOR EACH TEST (stop on first failure):                              │
│           Run: ./solution input.txt output.txt (cgroups v2 sandbox)         │
│           Check: ./checker input.txt output.txt input.txt                   │
│         Aggregate verdicts → Update DB → Record metrics                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  DATABASE: submissions.status = verdict, submission_results per TC           │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  HORUS (cron): Clean stale testcases, orphan temps, old binaries            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Consumer Group Resilience

Both Sisyphus and Minos create their Redis consumer groups at startup via
`XGROUP CREATE ... $ MKSTREAM`. If the group is lost after startup (e.g. Redis
restart without persistence, manual `XGROUP DESTROY`, etc.), the consumers
detect the `NOGROUP` error inside their processing loops and automatically
re-create the consumer group before retrying — no manual intervention or
service restart is required.

Minos additionally calls `claim_pending_messages()` on startup, using `XPENDING`
to find messages idle > 60s and `XCLAIM`-ing them to prevent stuck messages after
a restart.
