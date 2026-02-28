# Olympus Execution Flow

This document describes how files move through the sandboxed containers during compilation and execution.

## Overview

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   VANGUARD   │───▶│   SISYPHUS   │───▶│    MINOS     │    │    HORUS     │
│  (API Gate)  │    │  (Compiler)  │    │   (Judge)    │    │  (Cleaner)   │
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
       │                   │                   │                   │
       ▼                   ▼                   ▼                   ▼
  Upload ZIP          Compile in          Execute in          Clean stale
  Validate            Sandbox             Sandbox             files
  Queue job           Save binary         Check output
```

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
./solution
```

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

**Redis Stream Message:**
```json
{
  "submission_id": "abc-123",
  "type": "zip",
  "file_path": "/mnt/data/submissions/.../abc-123.zip",
  "language": "cpp"          // optional — helps Sisyphus set up the right toolchain
}
```

**Queue:** `compile_queue`

> **Note:** The `language` field is optional for ZIP submissions. When present,
> Sisyphus can pre-configure the correct compiler toolchain. When absent,
> Sisyphus relies solely on the user-provided `compile.sh`.

---

## Phase 2: Sisyphus (Compiler Service)

**Actions:**
1. Consumes job from `compile_queue` (XREADGROUP)
2. Creates temporary build directory
3. Extracts ZIP to temp directory
4. Resolves a **language-specific Docker image** from the `language` hint
5. Spawns an isolated container from that image
6. Runs `compile.sh` (or the auto-generated compile command for source submissions)
7. Extracts compiled binary from the build directory
8. Queues to `run_queue` or marks `COMPILATION_ERROR`

### Language → Docker Image Mapping

Each language resolves to a dedicated image so the right toolchain is
always available.  Images are pulled lazily on first use and cached
afterwards.  Defaults can be overridden via `CONTAINER_IMAGE_*` env
vars.

| Language | Default Image | Override Env Var |
|----------|---------------|------------------|
| `cpp` / `c++` | `gcc:14` | `CONTAINER_IMAGE_CPP` |
| `c` | `gcc:14` | `CONTAINER_IMAGE_C` |
| `rust` | `rust:1.85-bookworm` | `CONTAINER_IMAGE_RUST` |
| `go` | `golang:1.23-bookworm` | `CONTAINER_IMAGE_GO` |
| `python` | `python:3.12-bookworm` | `CONTAINER_IMAGE_PYTHON` |
| `zig` | `euantorano/zig:0.13.0` | `CONTAINER_IMAGE_ZIG` |
| *(unknown/omitted)* | `ubuntu:24.04` | `CONTAINER_IMAGE_GENERIC` |

### Build Directory Structure

```
/tmp/.tmp<random>/          # tempfile::tempdir on Sisyphus host
├── compile.sh
├── run.sh
├── main.cpp
└── solution  (generated binary after compilation)
```

### Docker Container Configuration

**Sandbox Settings (applied to every compilation container):**
```
--rm                              # Remove after exit
--network=none                    # NO network access (unless NETWORK_ENABLED)
--memory=2g                       # Memory limit (MAX_MEMORY_BYTES)
--cpus=2                          # CPU cores  (MAX_CPU_CORES)
--pids-limit=256                  # Limit process spawning
--read-only                       # Read-only root filesystem
--tmpfs /tmp:rw,noexec,nosuid     # Writable /tmp for compilers
--cap-drop=ALL                    # Drop all capabilities
```

**Volume Mounts:**
| Host | Container | Mode |
|------|-----------|------|
| `<temp build directory>` | `/workspace` | rw |

**Execution (inside container):**
```bash
$ cd /workspace
$ chmod +x compile.sh
$ timeout 30s ./compile.sh

# For a cpp submission the container is gcc:14, so g++ is available:
#   compile.sh runs: g++ -O2 -std=c++17 -o solution main.cpp
#   Output: /workspace/solution (compiled binary)
```

### Output

**Binary Storage:**
```
/mnt/data/binaries/users/{submission_id}_bin
```

**Redis Stream Message:**
```json
{
  "submission_id": "abc-123",
  "problem_id": "prob-456",
  "time_limit_ms": 2000,
  "memory_limit_kb": 262144
}
```

**Queue:** `run_queue`

**Cleanup:** Build directory is a `tempfile::tempdir` and is dropped automatically when Sisyphus finishes processing the job.

---

## Phase 3: Minos (Judge Service)

**Actions:**
1. Consumes job from `run_queue` (XREADGROUP)
2. **Checks if generator and checker binaries exist** for the problem
   - If either is missing → sets submission status to `queue_pending`, ACKs the message, and moves on (no retry/dead-letter)
   - The submission will be automatically re-queued when the missing binary is uploaded via `POST /api/v1/problems/{id}/generator` or `/checker`
3. Loads compiled binary from storage
4. Gets/generates test cases (lazy generation)
5. For each test case: spawn sandboxed container, run binary, check output
6. Updates verdict in database

### Test Case Generation (if not cached)

```
/mnt/data/binaries/problems/{problem_id}/generator
              │
              ▼
SANDBOXED: ./generator {test_number} > input.txt
(Network disabled, 60s timeout, 4GB RAM)
              │
              ▼
/mnt/data/testcases/{problem_id}/
├── input_001.txt
├── input_002.txt
├── ...
└── .last_access   (timestamp for cache invalidation)
```

### Execution Container (Per Test Case)

```
Image: olympus-runner:latest (minimal, no compilers)
```

**Sandbox Settings (STRICTER than compilation):**
```
--rm
--network=none                    # NO network (not even loopback!)
--memory={limit}                  # Per-problem memory limit
--cpus=1                          # Single CPU core
--pids-limit=1                    # NO forking allowed
--read-only
--security-opt=no-new-privileges
--cap-drop=ALL
--security-opt seccomp=/etc/olympus/seccomp-strict.json
   # Whitelist: read, write, mmap, brk, exit_group
   # BLOCKED: fork, clone, execve, socket, ptrace, etc.
```

**Volume Mounts:**
| Host | Container | Mode |
|------|-----------|------|
| `/mnt/data/binaries/users/{id}_bin` | `/app/solution` | ro |
| `/mnt/data/temp/{id}/` | `/sandbox` | rw |

**Execution Flow:**
```
input_001.txt ──────┐
                    ▼
$ timeout {time_limit}s /app/solution < /sandbox/input.txt \
                                      > /sandbox/output.txt
                    │
                    ▼
output_001.txt ◄────┘

Metrics captured:
- Wall clock time (ms)
- Peak memory usage (KB) via cgroups
- Exit code
```

### Checker Verification

```
/mnt/data/binaries/problems/{problem_id}/checker
```

**Execution:**
```bash
$ ./checker input.txt user_output.txt [expected.txt]
```

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

### Verdict Determination

**Priority:** `JE > RE > MLE > TLE > OLE > WA > AC`

**Example (5 test cases):**
```
TC1: AC, TC2: AC, TC3: WA, TC4: (skipped), TC5: (skipped)
Final Verdict: WA
```

**Score calculation (if all passed):**
```
score = 100.0 * (passed_count / total_count)
```

### Database Updates

**submissions table:**
| id | status | verdict | total_time | score |
|----|--------|---------|------------|-------|
| abc-123 | judged | WA | 150 | 40.0 |

**submission_results table:**
| submission | tc_num | verdict | time_ms | mem_kb | comment |
|------------|--------|---------|---------|--------|---------|
| abc-123 | 1 | AC | 45 | 12000 | ok |
| abc-123 | 2 | AC | 52 | 12100 | ok |
| abc-123 | 3 | WA | 48 | 11900 | wrong... |

**Cleanup:** `rm -rf /mnt/data/temp/{submission_id}/`

**Acknowledge:** `XACK run_queue minos_group {message_id}`

---

## Phase 4: Horus (Cleaner Service)

Runs on cron schedules to clean up stale/orphaned files.

### Cleanup Policies

| Policy | Schedule | Rule | Target |
|--------|----------|------|--------|
| Stale Testcases | hourly | `LastAccessOlderThan(6h) & IsDirectory & !HasProblemRecord` | `/mnt/data/testcases/{problem_id}/` |
| Orphan Temp Dirs | every 15 min | `CreatedOlderThan(1h) & IsDirectory & !HasActiveSubmission` | `/mnt/data/temp/` |
| Orphan Binaries | daily @ 3am | `CreatedOlderThan(24h) & IsFile & !HasSubmissionRecord` | `/mnt/data/binaries/users/` |
| Old Submissions | weekly (optional) | `CreatedOlderThan(retention_days)` | `/mnt/data/submissions/` |

---

## File Locations Summary

| Path | Purpose | Lifecycle |
|------|---------|-----------|
| `/mnt/data/submissions/{contest}/{user}/{id}.zip` | Contest ZIP upload | Permanent (until archived) |
| `/mnt/data/submissions/standalone/{user}/{id}.zip` | Standalone ZIP upload | Permanent (until archived) |
| `/tmp/.tmp<random>/` | Compilation workspace (temp dir) | Deleted after compile |
| `/mnt/data/binaries/users/{id}_bin` | Compiled user binary | Deleted by Horus (24h+) |
| `/mnt/data/binaries/problems/{id}/generator` | Test generator | Permanent |
| `/mnt/data/binaries/problems/{id}/checker` | Output checker | Permanent |
| `/mnt/data/testcases/{problem_id}/` | Generated test inputs | Cached, cleaned after 6h |
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

## Security Summary

| Container | Network | Memory | CPU | Fork | Syscalls |
|-----------|---------|--------|-----|------|----------|
| **Compiler** | None | 2GB | 2 | Limited | Filtered |
| **Runner** | None | Per-problem | 1 | **Blocked** | Strict whitelist |
| **Generator** | None | 4GB | 2 | Limited | Filtered |
| **Checker** | None | 4GB | 2 | Limited | Filtered |

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
│  SISYPHUS: Extract ZIP → Spawn Docker (--network=none, --memory=2g)         │
│            Run compile.sh → Save binary → Queue run_queue                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  MINOS: Load binary → Generate/cache test cases                             │
│         FOR EACH TEST:                                                      │
│           Spawn Docker (--pids-limit=1, seccomp whitelist)                  │
│           Run: ./solution < input.txt > output.txt                          │
│           Check: ./checker input.txt output.txt                             │
│         Aggregate verdicts → Update DB                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  DATABASE: submissions.verdict = WA, submission_results = [AC, AC, WA, ...] │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  HORUS (cron): Clean stale testcases, orphan temps, old binaries            │
└─────────────────────────────────────────────────────────────────────────────┘
```


```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                    USER SUBMISSION                                       │
│                                                                                          │
│   submission.zip                                                                         │
│   ├── compile.sh      #!/bin/bash                                                        │
│   │                   g++ -O2 -std=c++17 -o solution main.cpp                           │
│   ├── run.sh          #!/bin/bash                                                        │
│   │                   ./solution                                                         │
│   └── main.cpp        #include <iostream> ...                                           │
│                                                                                          │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              PHASE 1: VANGUARD (API Gateway)                             │
│                                                                                          │
│   1. Receives multipart upload                                                           │
│   2. Validates ZIP structure (compile.sh, run.sh exist)                                  │
│   3. Security checks (no symlinks, path traversal, zip bombs)                            │
│   4. Saves to persistent storage                                                         │
│   5. Queues compilation job to Redis Stream                                              │
│                                                                                          │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  /mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip       │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                                           │                                              │
│                                           ▼                                              │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Redis Stream: compile_queue                                            │           │
│   │  {                                                                      │           │
│   │    "submission_id": "abc-123",                                          │           │
│   │    "file_path": "/mnt/data/submissions/.../abc-123.zip",                │           │
│   │    "language": "cpp"                                                    │           │
│   │  }                                                                      │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           PHASE 2: SISYPHUS (Compiler Service)                           │
│                                                                                          │
│   1. Consumes job from compile_queue (XREADGROUP)                                        │
│   2. Creates temporary build directory                                                   │
│   3. Extracts ZIP to temp directory                                                      │
│   4. Spawns SANDBOXED Docker container                                                   │
│   5. Runs compile.sh inside container                                                    │
│   6. Extracts compiled binary                                                            │
│   7. Queues to run_queue or marks COMPILATION_ERROR                                      │
│                                                                                          │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Host: /tmp/.tmp<random>/  (tempfile build directory)                   │           │
│   │  ├── compile.sh                                                         │           │
│   │  ├── run.sh                                                             │           │
│   │  ├── main.cpp                                                           │           │
│   │  └── solution  (generated binary after compilation)                     │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│   │              DOCKER CONTAINER (Language-Specific, Sandboxed)                     │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Image resolved from language hint:                                     │   │   │
│   │  │    cpp → gcc:14 │ rust → rust:1.85 │ go → golang:1.23 │ ...            │   │   │
│   │  │  Pulled lazily and cached locally                                       │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Sandbox Settings:                                                              │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  --rm                    # Remove after exit                            │   │   │
│   │  │  --network=none          # NO network access                            │   │   │
│   │  │  --memory=2g             # 2GB RAM limit                                │   │   │
│   │  │  --cpus=2                # 2 CPU cores                                  │   │   │
│   │  │  --pids-limit=256        # Limit process spawning                       │   │   │
│   │  │  --read-only             # Read-only root filesystem                    │   │   │
│   │  │  --tmpfs /tmp            # Writable /tmp for compilers                  │   │   │
│   │  │  --cap-drop=ALL          # Drop all capabilities                        │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Volume Mounts:                                                                 │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Host                              Container                            │   │   │
│   │  │  <temp build dir>             →   /workspace:rw (writable)              │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Execution (inside container):                                                  │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  $ cd /workspace                                                        │   │   │
│   │  │  $ chmod +x compile.sh                                                  │   │   │
│   │  │  $ timeout 30s ./compile.sh                                             │   │   │
│   │  │                                                                         │   │   │
│   │  │  # e.g. compile.sh runs: g++ -O2 -std=c++17 -o solution main.cpp       │   │   │
│   │  │  # Output: /workspace/solution (compiled binary)                        │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   └─────────────────────────────────────────────────────────────────────────────────┘   │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Binary copied to persistent storage:                                   │           │
│   │  /mnt/data/binaries/users/{submission_id}_bin                           │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Redis Stream: run_queue                                                │           │
│   │  {                                                                      │           │
│   │    "submission_id": "abc-123",                                          │           │
│   │    "problem_id": "prob-456",                                            │           │
│   │    "time_limit_ms": 2000,                                               │           │
│   │    "memory_limit_kb": 262144                                            │           │
│   │  }                                                                      │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                                                                                          │
│   Cleanup: temp build directory removed automatically (tempfile::tempdir)                │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                            PHASE 3: MINOS (Judge Service)                              │
│                                                                                        │
│   1. Consumes job from run_queue (XREADGROUP)                                          │
│   2. Loads compiled binary from storage                                                │
│   3. Gets/generates test cases (lazy generation)                                       │
│   4. For each test case: spawn sandboxed container, run binary, check output           │
│   5. Updates verdict in database                                                       │
│                                                                                        │
│   ┌─────────────────────────────────────────────────────────────────────────┐          │
│   │  Test Case Generation (if not cached):                                  │          │
│   │                                                                         │          │
│   │  /mnt/data/binaries/problems/{problem_id}/generator                     │          │
│   │                    │                                                    │          │
│   │                    ▼                                                    │          │
│   │  ┌─────────────────────────────────────────────────────────────┐        │          │
│   │  │  SANDBOXED: ./generator {test_number} > input.txt           │        │          │
│   │  │  (Network disabled, 60s timeout, 4GB RAM)                   │        │          │
│   │  └─────────────────────────────────────────────────────────────┘        │          │
│   │                    │                                                    │          │
│   │                    ▼                                                    │          │
│   │  ├── ...                                                                │          │
│   │  /mnt/data/testcases/{problem_id}/                                      │          │
│   │  ├── input_001.txt                                                      │          │
│   │  ├── input_002.txt                                                      │          │
│   │  └── .last_access   (timestamp for cache invalidation)                  │          │
│   └─────────────────────────────────────────────────────────────────────────┘          │
│                                                                                        │
│   FOR EACH TEST CASE:                                                                  │
│   ┌────────────────────────────────────────────────────────────────────────────────┐   │
│   │                     DOCKER CONTAINER (Sandboxed - STRICT)                      │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Image: olympus-runner:latest (minimal, no compilers)                   │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                │   │
│   │  Sandbox Settings (STRICTER than compilation):                                 │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  --rm                                                                   │   │   │
│   │  │  --network=none          # NO network (not even loopback!)              │   │   │
│   │  │  --memory={limit}        # Per-problem memory limit                     │   │   │
│   │  │  --cpus=1                # Single CPU core                              │   │   │
│   │  │  --pids-limit=1          # NO forking allowed                           │   │   │
│   │  │  --read-only                                                            │   │   │
│   │  │  --security-opt=no-new-privileges                                       │   │   │
│   │  │  --cap-drop=ALL                                                         │   │   │
│   │  │  --security-opt seccomp=/etc/olympus/seccomp-strict.json                │   │   │
│   │  │     # Whitelist: read, write, mmap, brk, exit_group                     │   │   │
│   │  │     # BLOCKED: fork, clone, execve, socket, ptrace, etc.                │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                │   │
│   │  Volume Mounts:                                                                │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Host                                    Container                      │   │   │
│   │  │  /mnt/data/binaries/users/{id}_bin  →   /app/solution:ro (read-only)    │   │   │
│   │  │  /mnt/data/temp/{id}/               →   /sandbox:rw     (scratch)       │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                │   │
│   │  Execution Flow:                                                               │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │                                                                         │   │   │
│   │  │   input_001.txt ──────┐                                                 │   │   │
│   │  │                       ▼                                                 │   │   │
│   │  │   $ timeout {time_limit}s /app/solution < /sandbox/input.txt \          │   │   │
│   │  │                                         > /sandbox/output.txt           │   │   │
│   │  │                       │                                                 │   │   │
│   │  │                       ▼                                                 │   │   │
│   │  │   output_001.txt ◄────┘                                                 │   │   │
│   │  │                                                                         │   │   │
│   │  │   Metrics captured:                                                     │   │   │
│   │  │   - Wall clock time (ms)                                                │   │   │
│   │  │   - Peak memory usage (KB) via cgroups                                  │   │   │
│   │  │   - Exit code                                                           │   │   │
│   │  │                                                                         │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   └────────────────────────────────────────────────────────────────────────────────┘   │
│                         │                                                              │
│                         ▼                                                              │
│   ┌────────────────────────────────────────────────────────────────────────────────┐   │
│   │                     CHECKER CONTAINER (Sandboxed)                              │   │
│   │                                                                                │   │
│   │  /mnt/data/binaries/problems/{problem_id}/checker                              │   │
│   │                                                                                │   │
│   │  Execution:                                                                    │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  $ ./checker input.txt user_output.txt [expected.txt]                   │   │   │
│   │  │                                                                         │   │   │
│   │  │  Exit Codes (Testlib convention):                                       │   │   │
│   │  │  ┌────────┬─────────────────┬────────────────────────────────────────┐  │   │   │
│   │  │  │ Code   │ Verdict         │ Meaning                                │  │   │   │
│   │  │  ├────────┼─────────────────┼────────────────────────────────────────┤  │   │   │
│   │  │  │ 0      │ AC (Accepted)   │ Output is correct                      │  │   │   │
│   │  │  │ 1      │ WA (Wrong)      │ Output is incorrect                    │  │   │   │
│   │  │  │ 2      │ PE              │ Presentation error (treated as WA)     │  │   │   │
│   │  │  │ 3      │ JE (Judge Err)  │ Checker crashed                        │  │   │   │
│   │  │  │ 7      │ PC (Partial)    │ Partial credit (scoring problems)      │  │   │   │
│   │  │  └────────┴─────────────────┴────────────────────────────────────────┘  │   │   │
│   │  │                                                                         │   │   │
│   │  │  Checker stderr contains verdict message:                               │   │   │
│   │  │  "ok Correct answer: 42"                                                │   │   │
│   │  │  "wrong answer Expected 42, got 17"                                     │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   └────────────────────────────────────────────────────────────────────────────────┘   │
│                         │                                                              │
│                         ▼                                                              │
│   ┌────────────────────────────────────────────────────────────────────────┐           │
│   │  Verdict Determination:                                                │           │
│   │                                                                        │           │
│   │  ┌─────────────────────────────────────────────────────────────────┐   │           │
│   │  │  Priority: JE > RE > MLE > TLE > OLE > WA > AC                  │   │           │
│   │  │                                                                 │   │           │
│   │  │  Example (5 test cases):                                        │   │           │
│   │  │  TC1: AC, TC2: AC, TC3: WA, TC4: (skipped), TC5: (skipped)      │   │           │
│   │  │  Final Verdict: WA                                              │   │           │
│   │  │                                                                 │   │           │
│   │  │  Score calculation (if all passed):                             │   │           │
│   │  │  score = 100.0 * (passed_count / total_count)                   │   │           │
│   │  └─────────────────────────────────────────────────────────────────┘   │           │
│   └────────────────────────────────────────────────────────────────────────┘           │
│                         │                                                              │
│                         ▼                                                              │
│   ┌─────────────────────────────────────────────────────────────────────────┐          │
│   │  Database Updates:                                                      │          │
│   │                                                                         │          │
│   │  submissions table:                                                     │          │
│   │  ┌──────────────┬─────────┬──────────┬─────────────┬─────────┐          │          │
│   │  │ id           │ status  │ verdict  │ total_time  │ score   │          │          │
│   │  ├──────────────┼─────────┼──────────┼─────────────┼─────────┤          │          │
│   │  │ abc-123      │ judged  │ WA       │ 150         │ 40.0    │          │          │
│   │  └──────────────┴─────────┴──────────┴─────────────┴─────────┘          │          │
│   │                                                                         │          │
│   │  submission_results table:                                              │          │
│   │  ┌──────────────┬─────────┬─────────┬─────────┬──────────┬──────────┐   │          │
│   │  │ submission   │ tc_num  │ verdict │ time_ms │ mem_kb   │ comment  │   │          │
│   │  ├──────────────┼─────────┼─────────┼─────────┼──────────┼──────────┤   │          │
│   │  │ abc-123      │ 1       │ AC      │ 45      │ 12000    │ ok       │   │          │
│   │  │ abc-123      │ 2       │ AC      │ 52      │ 12100    │ ok       │   │          │
│   │  │ abc-123      │ 3       │ WA      │ 48      │ 11900    │ wrong... │   │          │
│   │  └──────────────┴─────────┴─────────┴─────────┴──────────┴──────────┘   │          │
│   └─────────────────────────────────────────────────────────────────────────┘          │
│                                                                                        │
│   Cleanup: rm -rf /mnt/data/temp/{submission_id}/                                      │
│   XACK run_queue minos_group {message_id}                                              │
└────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                            PHASE 4: HORUS (Cleaner Service)                             │
│                                                                                         │
│   Runs on cron schedules to clean up stale/orphaned files                               │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Cleanup Policies:                                                      │           │
│   │                                                                         │           │
│   │  1. Stale Testcases (hourly)                                            │           │
│   │     Rule: LastAccessOlderThan(6h) & IsDirectory & !HasProblemRecord     │           │
│   │     Target: /mnt/data/testcases/{problem_id}/                           │           │
│   │                                                                         │           │
│   │  2. Orphan Temp Dirs (every 15 min)                                     │           │
│   │     Rule: CreatedOlderThan(1h) & IsDirectory & !HasActiveSubmission     │           │
│   │     Target: /mnt/data/temp/                                             │           │
│   │                                                                         │           │
│   │  3. Orphan Binaries (daily @ 3am)                                       │           │
│   │     Rule: CreatedOlderThan(24h) & IsFile & !HasSubmissionRecord         │           │
│   │     Target: /mnt/data/binaries/users/                                   │           │
│   │                                                                         │           │
│   │  4. Old Submissions (weekly, optional)                                  │           │
│   │     Rule: CreatedOlderThan(retention_days)                              │           │
│   │     Target: /mnt/data/submissions/                                      │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Consumer Group Resilience

Both Sisyphus and Minos create their Redis consumer groups at startup via
`XGROUP CREATE ... MKSTREAM`. If the group is lost after startup (e.g. Redis
restart without persistence, manual `XGROUP DESTROY`, etc.), the consumers
detect the `NOGROUP` error inside their processing loops and automatically
re-create the consumer group before retrying — no manual intervention or
service restart is required.