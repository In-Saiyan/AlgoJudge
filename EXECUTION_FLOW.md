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
1. Receives multipart upload
2. Validates ZIP structure (compile.sh, run.sh exist)
3. Security checks (no symlinks, path traversal, zip bombs)
4. Saves to persistent storage
5. Queues compilation job to Redis Stream

**File Storage:**
```
/mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip
```

**Redis Stream Message:**
```json
{
  "submission_id": "abc-123",
  "file_path": "/mnt/data/submissions/.../abc-123.zip",
  "language": "cpp"
}
```

**Queue:** `compile_queue`

---

## Phase 2: Sisyphus (Compiler Service)

**Actions:**
1. Consumes job from `compile_queue` (XREADGROUP)
2. Creates temporary build directory
3. Extracts ZIP to temp directory
4. Spawns SANDBOXED Docker container
5. Runs `compile.sh` inside container
6. Extracts compiled binary
7. Queues to `run_queue` or marks `COMPILATION_ERROR`

### Build Directory Structure

```
/mnt/data/temp/build_{submission_id}/
├── compile.sh
├── run.sh
├── main.cpp
└── solution  (generated binary after compilation)
```

### Docker Container Configuration

```
Image: olympus-compiler:latest
Contains: g++, clang, rustc, go, python3, zig
```

**Sandbox Settings:**
```
--rm                              # Remove after exit
--network=none                    # NO network access
--memory=2g                       # 2GB RAM limit
--cpus=2                          # 2 CPU cores
--pids-limit=100                  # Limit process spawning
--read-only                       # Read-only root filesystem
--security-opt=no-new-privileges
--cap-drop=ALL                    # Drop all capabilities
```

**Volume Mounts:**
| Host | Container | Mode |
|------|-----------|------|
| `/mnt/data/temp/build_{id}/` | `/build` | rw |
| `/tmp/compile_{id}/` | `/tmp` | rw |

**Execution (inside container):**
```bash
$ cd /build
$ chmod +x compile.sh
$ timeout 30s ./compile.sh

# compile.sh runs: g++ -O2 -std=c++17 -o solution main.cpp
# Output: /build/solution (compiled binary)
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

**Cleanup:** `rm -rf /mnt/data/temp/build_{submission_id}/`

---

## Phase 3: Minos (Judge Service)

**Actions:**
1. Consumes job from `run_queue` (XREADGROUP)
2. Loads compiled binary from storage
3. Gets/generates test cases (lazy generation)
4. For each test case: spawn sandboxed container, run binary, check output
5. Updates verdict in database

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
| `/mnt/data/submissions/{contest}/{user}/{id}.zip` | Original ZIP upload | Permanent (until archived) |
| `/mnt/data/temp/build_{id}/` | Compilation workspace | Deleted after compile |
| `/mnt/data/binaries/users/{id}_bin` | Compiled user binary | Deleted by Horus (24h+) |
| `/mnt/data/binaries/problems/{id}/generator` | Test generator | Permanent |
| `/mnt/data/binaries/problems/{id}/checker` | Output checker | Permanent |
| `/mnt/data/testcases/{problem_id}/` | Generated test inputs | Cached, cleaned after 6h |
| `/mnt/data/temp/{id}/` | Execution scratch space | Deleted after judging |

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
│   │  Host: /mnt/data/temp/build_{submission_id}/                            │           │
│   │  ├── compile.sh                                                         │           │
│   │  ├── run.sh                                                             │           │
│   │  ├── main.cpp                                                           │           │
│   │  └── solution  (generated binary after compilation)                     │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│   │                     DOCKER CONTAINER (Sandboxed)                                │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Image: olympus-compiler:latest                                         │   │   │
│   │  │  Contains: g++, clang, rustc, go, python3, zig                          │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Sandbox Settings:                                                              │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  --rm                    # Remove after exit                            │   │   │
│   │  │  --network=none          # NO network access                            │   │   │
│   │  │  --memory=2g             # 2GB RAM limit                                │   │   │
│   │  │  --cpus=2                # 2 CPU cores                                  │   │   │
│   │  │  --pids-limit=100        # Limit process spawning                       │   │   │
│   │  │  --read-only             # Read-only root filesystem                    │   │   │
│   │  │  --security-opt=no-new-privileges                                       │   │   │
│   │  │  --cap-drop=ALL          # Drop all capabilities                        │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Volume Mounts:                                                                 │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Host                              Container                            │   │   │
│   │  │  /mnt/data/temp/build_{id}/   →   /build:rw    (writable)              │   │   │
│   │  │  /tmp/compile_{id}/           →   /tmp:rw      (temp space)            │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Execution (inside container):                                                  │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  $ cd /build                                                            │   │   │
│   │  │  $ chmod +x compile.sh                                                  │   │   │
│   │  │  $ timeout 30s ./compile.sh                                             │   │   │
│   │  │                                                                         │   │   │
│   │  │  # compile.sh runs: g++ -O2 -std=c++17 -o solution main.cpp            │   │   │
│   │  │  # Output: /build/solution (compiled binary)                            │   │   │
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
│   Cleanup: rm -rf /mnt/data/temp/build_{submission_id}/                                 │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                            PHASE 3: MINOS (Judge Service)                                │
│                                                                                          │
│   1. Consumes job from run_queue (XREADGROUP)                                            │
│   2. Loads compiled binary from storage                                                  │
│   3. Gets/generates test cases (lazy generation)                                         │
│   4. For each test case: spawn sandboxed container, run binary, check output             │
│   5. Updates verdict in database                                                         │
│                                                                                          │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Test Case Generation (if not cached):                                  │           │
│   │                                                                         │           │
│   │  /mnt/data/binaries/problems/{problem_id}/generator                     │           │
│   │                    │                                                    │           │
│   │                    ▼                                                    │           │
│   │  ┌─────────────────────────────────────────────────────────────┐       │           │
│   │  │  SANDBOXED: ./generator {test_number} > input.txt           │       │           │
│   │  │  (Network disabled, 60s timeout, 4GB RAM)                   │       │           │
│   │  └─────────────────────────────────────────────────────────────┘       │           │
│   │                    │                                                    │           │
│   │                    ▼                                                    │           │
│   │  /mnt/data/testcases/{problem_id}/                                      │           │
│   │  ├── input_001.txt                                                      │           │
│   │  ├── input_002.txt                                                      │           │
│   │  ├── ...                                                                │           │
│   │  └── .last_access   (timestamp for cache invalidation)                  │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                                                                                          │
│   FOR EACH TEST CASE:                                                                    │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│   │                     DOCKER CONTAINER (Sandboxed - STRICT)                       │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Image: olympus-runner:latest (minimal, no compilers)                   │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Sandbox Settings (STRICTER than compilation):                                  │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  --rm                                                                   │   │   │
│   │  │  --network=none          # NO network (not even loopback!)             │   │   │
│   │  │  --memory={limit}        # Per-problem memory limit                    │   │   │
│   │  │  --cpus=1                # Single CPU core                             │   │   │
│   │  │  --pids-limit=1          # NO forking allowed                          │   │   │
│   │  │  --read-only                                                            │   │   │
│   │  │  --security-opt=no-new-privileges                                       │   │   │
│   │  │  --cap-drop=ALL                                                         │   │   │
│   │  │  --security-opt seccomp=/etc/olympus/seccomp-strict.json               │   │   │
│   │  │     # Whitelist: read, write, mmap, brk, exit_group                    │   │   │
│   │  │     # BLOCKED: fork, clone, execve, socket, ptrace, etc.               │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Volume Mounts:                                                                 │   │
│   │  ┌─────────────────────────────────────────────────────────────────────────┐   │   │
│   │  │  Host                                    Container                      │   │   │
│   │  │  /mnt/data/binaries/users/{id}_bin  →   /app/solution:ro (read-only)   │   │   │
│   │  │  /mnt/data/temp/{id}/               →   /sandbox:rw     (scratch)      │   │   │
│   │  └─────────────────────────────────────────────────────────────────────────┘   │   │
│   │                                                                                 │   │
│   │  Execution Flow:                                                                │   │
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
│   └─────────────────────────────────────────────────────────────────────────────────┘   │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│   │                     CHECKER CONTAINER (Sandboxed)                               │   │
│   │                                                                                 │   │
│   │  /mnt/data/binaries/problems/{problem_id}/checker                               │   │
│   │                                                                                 │   │
│   │  Execution:                                                                     │   │
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
│   └─────────────────────────────────────────────────────────────────────────────────┘   │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Verdict Determination:                                                 │           │
│   │                                                                         │           │
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
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                         │                                                                │
│                         ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────┐           │
│   │  Database Updates:                                                      │           │
│   │                                                                         │           │
│   │  submissions table:                                                     │           │
│   │  ┌──────────────┬─────────┬──────────┬─────────────┬─────────┐         │           │
│   │  │ id           │ status  │ verdict  │ total_time  │ score   │         │           │
│   │  ├──────────────┼─────────┼──────────┼─────────────┼─────────┤         │           │
│   │  │ abc-123      │ judged  │ WA       │ 150         │ 40.0    │         │           │
│   │  └──────────────┴─────────┴──────────┴─────────────┴─────────┘         │           │
│   │                                                                         │           │
│   │  submission_results table:                                              │           │
│   │  ┌──────────────┬─────────┬─────────┬─────────┬──────────┬──────────┐  │           │
│   │  │ submission   │ tc_num  │ verdict │ time_ms │ mem_kb   │ comment  │  │           │
│   │  ├──────────────┼─────────┼─────────┼─────────┼──────────┼──────────┤  │           │
│   │  │ abc-123      │ 1       │ AC      │ 45      │ 12000    │ ok       │  │           │
│   │  │ abc-123      │ 2       │ AC      │ 52      │ 12100    │ ok       │  │           │
│   │  │ abc-123      │ 3       │ WA      │ 48      │ 11900    │ wrong... │  │           │
│   │  └──────────────┴─────────┴─────────┴─────────┴──────────┴──────────┘  │           │
│   └─────────────────────────────────────────────────────────────────────────┘           │
│                                                                                          │
│   Cleanup: rm -rf /mnt/data/temp/{submission_id}/                                       │
│   XACK run_queue minos_group {message_id}                                               │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                            PHASE 4: HORUS (Cleaner Service)                              │
│                                                                                          │
│   Runs on cron schedules to clean up stale/orphaned files                                │
│                                                                                          │
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

