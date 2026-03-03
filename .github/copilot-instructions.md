# GitHub Copilot Instructions for AlgoJudge (Olympus)

## Implementation Plan

This is an incremental implementation roadmap. Complete phases in order. Each phase builds on the previous.

---

### Phase 0: Project Setup ✅
**Goal:** Initialize workspace structure and shared dependencies.

- [x] Create workspace `Cargo.toml` with all crate members
- [x] Create `crates/` directory structure
- [x] Initialize `olympus-common` crate
  - [x] Define `AppError` enum with error variants (14 variants)
  - [x] Define common types (`UserId`, `ContestId`, `SubmissionId`, etc.)
  - [x] Add shared utilities (UUID generation, timestamp helpers)
  - [x] Define `Pagination` and `PaginatedResponse<T>` types
  - [x] Define enums: `UserRole`, `SubmissionStatus`, `Verdict`, `Runtime`, `ContestStatus`
- [x] Initialize `olympus-rules` crate (Specification Pattern)
  - [x] Define `Specification<Ctx>` async trait
  - [x] Implement `And<A, B>`, `Or<A, B>`, `Not<A>` combinators
  - [x] Implement `BitAnd`, `BitOr`, `Not` operator overloading via `Spec<S>` wrapper
  - [x] Add `RuleConfig` serde structs for JSON serialization (tagged enum)
  - [x] Create `SpecRegistry` for dynamic rule building
  - [x] Define context types: `EvalContext`, `FileContext`, `ExecutionContext`, `AuthContext`
  - [x] Implement file specs: `LastAccessOlderThan`, `CreatedOlderThan`, `IsFile`, `IsDirectory`, `SizeLargerThan`
  - [x] Implement execution specs: `WithinTimeLimit`, `WithinMemoryLimit`, `ExitCodeZero`, `OutputMatches`, `AcceptedVerdict`
  - [x] Implement auth specs: `IsValidUser`, `IsAdmin`, `IsOrganizer`, `IsParticipant`, `IsCollaborator`, `IsContestOwner`, `CanAddProblems`, `IsProblemOwner`, `CanAccessProblemBinaries`, `IsSubmissionOwner`, `NotRateLimited`
  - [x] Create pre-built registries: `file_context_registry()`, `execution_context_registry()`, `auth_context_registry()`
- [x] Setup shared `docker-compose.yml` for local development
  - [x] PostgreSQL service
  - [x] Redis service
  - [x] Prometheus/Grafana services

---

### Phase 1: Vanguard (API Gateway) - Core ✅
**Goal:** Basic API with auth, no queue integration yet.

#### 1.1 Project Scaffolding
- [x] Initialize `vanguard` crate with Axum
- [x] Setup `config.rs` (env vars, database pool, Redis pool)
- [x] Create domain folder structure (`auth/`, `health/`, `users/`, `contests/`, `problems/`, `submissions/`, `admin/`)
- [x] Add `AppState` struct (db pool, redis pool, config, rate_limit_config)

#### 1.2 Health & Database
- [x] Implement `GET /health` endpoint
- [x] Implement `GET /health/live` liveness probe
- [x] Implement `GET /health/ready` readiness probe
- [x] Setup SQLx with PostgreSQL (pool: max 20, min 5)
- [x] Create initial migrations
  - [x] `users` table (with role, ban fields)
  - [x] `sessions` table (for refresh tokens)

#### 1.3 Authentication
- [x] Implement `POST /api/v1/auth/register`
  - [x] Password hashing (argon2)
  - [x] Email validation
  - [x] Username uniqueness check
- [x] Implement `POST /api/v1/auth/login`
  - [x] JWT token generation (15min access, 7d refresh)
  - [x] Refresh token storage in Redis
- [x] Implement `POST /api/v1/auth/refresh`
- [x] Implement `POST /api/v1/auth/logout`
- [x] Implement `GET /api/v1/auth/me`
- [x] Create `auth_middleware` (JWT verification)
- [x] Create `optional_auth_middleware`
- [x] Create `admin_middleware` (role check)
- [x] Create `organizer_middleware` (admin or organizer)

#### 1.4 Rate Limiting Middleware
- [x] Implement rate limit middleware functions
  - [x] `api_rate_limit_middleware` (general API)
  - [x] `login_rate_limit_middleware` (per-IP)
  - [x] `register_rate_limit_middleware` (per-IP)
  - [x] `submission_rate_limit_middleware` (per-user)
  - [x] Redis INCR + EXPIRE pattern (fail-open on Redis error)
  - [x] Add `X-RateLimit-*` response headers
- [x] Configure rate limit tiers in `RateLimitConfig` (defaults: login 40/15min, register 30/15min, submit 5/1min, api_auth 600/1min, api_anon 100/1min)
- [x] Add `429 Too Many Requests` error response with `Retry-After` header

#### 1.5 User Management
- [x] Implement `GET /api/v1/users` (public)
- [x] Implement `GET /api/v1/users/{id}` (public)
- [x] Implement `PUT /api/v1/users/{id}` (owner only)
- [x] Implement `GET /api/v1/users/{id}/stats` (public)
- [x] Implement `GET /api/v1/users/{id}/submissions` (auth required)

---

### Phase 2: Vanguard - Contests & Problems ✅
**Goal:** Contest and problem management APIs.

#### 2.1 Database Migrations
- [x] `contests` table (with scoring_type, freeze_time, registration fields)
- [x] `contest_participants` table (with status, score tracking)
- [x] `contest_collaborators` table (with granular permissions: can_edit_contest, can_add_problems, can_view_submissions)
- [x] `problems` table (with max_threads, network_allowed fields)
- [x] `contest_problems` junction table (with per-contest overrides for limits, threads, network)
- [x] `test_cases` table (legacy, for manual test cases)

#### 2.2 Contest Endpoints
- [x] Implement `GET /api/v1/contests` (public)
- [x] Implement `POST /api/v1/contests` (auth required)
- [x] Implement `GET /api/v1/contests/{id}` (public)
- [x] Implement `PUT /api/v1/contests/{id}` (owner/collaborator/admin)
- [x] Implement `DELETE /api/v1/contests/{id}` (owner/admin)

#### 2.3 Contest Registration
- [x] Implement `POST /api/v1/contests/{id}/register`
- [x] Implement `POST /api/v1/contests/{id}/unregister`
- [x] Implement `GET /api/v1/contests/{id}/participants` (public)

#### 2.4 Contest Collaborators
- [x] Implement `GET /api/v1/contests/{id}/collaborators`
- [x] Implement `POST /api/v1/contests/{id}/collaborators`
- [x] Implement `DELETE /api/v1/contests/{id}/collaborators/{user_id}`

#### 2.5 Problems
- [x] Implement `GET /api/v1/problems` (public)
- [x] Implement `POST /api/v1/problems` (auth required, metadata only)
- [x] Implement `GET /api/v1/problems/{id}` (public)
- [x] Implement `PUT /api/v1/problems/{id}` (owner/admin)
- [x] Implement `DELETE /api/v1/problems/{id}` (owner/admin)
- [x] Implement `POST /api/v1/problems/{id}/generator` (multipart upload)
- [x] Implement `GET /api/v1/problems/{id}/generator` (download)
- [x] Implement `POST /api/v1/problems/{id}/checker` (multipart upload)
- [x] Implement `GET /api/v1/problems/{id}/checker` (download)
- [x] Implement `GET /api/v1/contests/{id}/problems` (public)
- [x] Implement `POST /api/v1/contests/{id}/problems`
- [x] Implement `DELETE /api/v1/contests/{id}/problems/{problem_id}`

#### 2.6 Authorization Rules (olympus-rules)
- [x] Create `IsParticipant` spec (DB lookup)
- [x] Create `IsCollaborator` spec (DB lookup)
- [x] Create `IsContestOwner` spec (DB lookup)
- [x] Create `IsProblemOwner` spec (DB lookup)
- [x] Create `IsSubmissionOwner` spec (DB lookup)
- [x] Create `CanAccessProblemBinaries` spec (composite: admin OR owner OR collaborator of containing contest)
- [x] Create `CanAddProblems` spec (DB lookup for collaborator permission)
- [x] Create `NotRateLimited` spec with Redis check (fail-open)
- [x] Create `AuthContext` builder with db/redis pools and optional contest/problem/submission IDs
- [x] Create authorization.rs with `require_*` convenience functions
- [x] Integrate rules into all handlers

---

### Phase 3: Vanguard - Submissions & Queue ✅
**Goal:** Submission handling with Redis Stream integration.

#### 3.1 Database & Storage
- [x] `submissions` table migration (with nullable contest_id for standalone submissions)
- [x] `submission_results` table migration (with UPSERT constraint)
- [x] Implement file storage utilities
- [x] Add configurable upload limits per contest (1-100MB, default 10MB)
- [x] Add `queue_pending` status for submissions awaiting problem binaries

#### 3.2 Submission Endpoints
- [x] Implement `POST /api/v1/submissions` (source code, contest_id optional)
- [x] Implement `POST /api/v1/submissions/upload` (ZIP multipart, contest_id optional)
  - [x] Validate ZIP structure (compile.sh, run.sh)
  - [x] Security: path traversal (`..`), symlinks, absolute paths, zip bomb protection
  - [x] Save to `/mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip` or `standalone/...`
  - [x] Push to `compile_queue` Redis Stream (fields: submission_id, type, file_path, language)
- [x] Implement `GET /api/v1/submissions` (auth required)
- [x] Implement `GET /api/v1/submissions/{id}` (owner/collaborator/admin)
- [x] Implement `GET /api/v1/submissions/{id}/results` (owner/collaborator/admin)
- [x] Implement `GET /api/v1/submissions/{id}/source` (owner/collaborator/admin)

#### 3.3 Submission Authorization Rules
- [x] Create `require_can_submit` composite rule: `IsValidUser AND (IsAdmin OR IsCollaborator OR (IsParticipant AND NotRateLimited::submission()))`
- [x] Create `require_can_submit_standalone` rule: `IsValidUser AND NotRateLimited::submission()`
- [x] Create `require_submission_view_access`: `IsAdmin OR IsSubmissionOwner OR IsCollaborator`
- [x] Integrate rules into submission handlers

#### 3.4 Leaderboard
- [x] Implement `GET /api/v1/contests/{contest_id}/leaderboard` (public, ICPC-style scoring)
- [x] Implement per-problem breakdowns, pagination, frozen leaderboard support
- [ ] ⭐ ICPC penalty calculation (currently hardcoded to 0)
- [ ] ⭐ Add caching layer with Redis

---

### Phase 4: Sisyphus (Compiler Service) ✅
**Goal:** Compilation worker consuming from Redis Stream.

#### 4.1 Project Setup
- [x] Initialize `sisyphus` crate
- [x] Setup Redis Stream consumer (`XREADGROUP`)
- [x] Create consumer group `sisyphus_group` on startup (also for dead letter stream)
- [x] Auto-re-create consumer group on `NOGROUP` errors

#### 4.2 Compilation Pipeline
- [x] Implement job consumer loop
- [x] Create temporary build directory (`BUILD_DIR_BASE` / tempfile::tempdir)
- [x] Parse stream message: `submission_id`, `type`, `file_path`, `language`, `retry_count`
- [x] Unzip submission from storage
- [x] Strip CRLF line endings from compile.sh and run.sh
- [x] Resolve language-specific Docker image (cpp→gcc:latest, rust→rust:1.85, etc.)
- [x] Lazy image pull (check with `docker image inspect`, pull if missing)
- [x] Execute `compile.sh` in isolated Docker container
  - [x] `--network=none` (configurable via NETWORK_ENABLED)
  - [x] `--memory=2g`, `--cpus=2`, `--pids-limit=256`
  - [x] `--read-only`, `--cap-drop=ALL`
  - [x] `--tmpfs /tmp:rw,noexec,nosuid,size=256m` and `/root/.cache`
  - [x] 30-second timeout (configurable via COMPILE_TIMEOUT_SECS)
- [x] Three volume mount strategies (Docker volume, host path translation, direct bind)
- [x] Handle compilation success
  - [x] Detect binary: search for `main`, `a.out`, `solution`, `run`; or copy entire dir for interpreted langs
  - [x] Save binary to `/mnt/data/binaries/users/{submission_id}_bin`
  - [x] Update DB status: `compiled`, `compiled_at = NOW()`
  - [x] Push to `run_queue` Redis Stream (fields: submission_id, binary_path)
  - [x] `XACK` the message
- [x] Handle compilation failure
  - [x] Capture stderr logs
  - [x] Update DB status: `compilation_error`, `compilation_log = <error>`
  - [x] `XACK` the message

#### 4.3 Error Handling & Resilience
- [x] Implement dead letter handling — `compile_queue_dead_letter` stream after 3 retries
- [x] Retryable error detection (timed out, connection refused, no space left, etc.)
- [x] Exponential backoff: delay = `1000ms * 2^(retry_count - 1)`
- [x] Graceful shutdown (AtomicBool + signal handlers, finish current job)

#### 4.4 Known Limitations
- [ ] ⭐ Source code compilation (`type: "source"`) is unimplemented — always returns error

---

### Phase 5: Minos (Judge Service) ✅
**Goal:** Execution and verification with direct cgroup/namespace sandboxing (no Docker).

#### 5.1 Project Setup
- [x] Initialize `minos` crate
- [x] Setup Redis Stream consumer (`XREADGROUP`, group `minos_group`)
- [x] Claim pending messages on startup (idle > 60s via `XPENDING` + `XCLAIM`)
- [x] Auto-re-create consumer group on `NOGROUP` errors
- [x] Setup Prometheus metrics exporter (port 9091)

#### 5.2 Test Case Management
- [x] Implement lazy test case generation
  - [x] Check `/mnt/data/testcases/{problem_id}/input_NNN.txt`
  - [x] Run generator if cache miss: `./generator {test_number}` (stdout → input file)
  - [x] Update `.last_access` timestamp (RFC3339) on cache hit

#### 5.3 Sandbox Execution (cgroups v2 + Linux namespaces)
- [x] Create `/mnt/data/temp/{submission_id}/` scratch directory
- [x] Setup cgroups v2 at `/sys/fs/cgroup/minos/{sandbox_id}`
  - [x] `memory.max` set per-problem
  - [x] `memory.swap.max = 0`
  - [x] `pids.max = max_threads + 4`
- [x] Network isolation via `unshare(CLONE_NEWNET)` when `network_allowed=false`
- [x] Execute user binary: `./binary input.txt output.txt` (file args, no stdin/stdout piping)
- [x] For interpreted langs (directory binary): `bash run.sh <input> <output>`
- [x] Pass environment variables: `INPUT_FILE`, `OUTPUT_FILE`, `MAX_THREADS`, `NETWORK_ALLOWED`, `TIME_LIMIT_MS`, `MEMORY_LIMIT_KB`
- [x] Hard timeout: `time_limit_ms + 100ms` buffer
- [x] Capture memory metrics from cgroup `memory.peak`/`memory.current` or `/proc/{pid}/status`
- [x] OOM detection via `memory.events` → `oom_kill` counter
- [x] Cgroup cleanup: write `cgroup.kill`, wait 100ms, remove directory
- [x] Output size limit: 64MB (configurable via OUTPUT_LIMIT_BYTES)

#### 5.4 Checker & Verdict
- [x] Implement checker execution: `./checker <input> <output> <input>` (testlib convention, no expected output file)
- [x] Checker timeout: 60s (configurable)
- [x] Interpret testlib exit codes: 0=AC, 1=WA, 2=PE(→WA), 3=JE, 7=PC
- [x] Stop-on-first-failure execution strategy
- [x] Score calculation: `100.0 * (passed / total)`
- [x] Determine final verdict (AC, WA, TLE, MLE, RE, OLE→RE, JE→system_error)

#### 5.5 Result Handling
- [x] Update `submissions` table with verdict, score, max_time_ms, max_memory_kb, passed/total counts
- [x] UPSERT per-testcase results into `submission_results`
- [x] `XACK` the message
- [x] Cleanup temp directory

#### 5.6 Queue Pending
- [x] Check generator/checker binary existence before judging
- [x] Set `queue_pending` status if missing, ACK message (no retry/dead-letter)

#### 5.7 Retry & Dead Letter
- [x] Re-queue on error with incremented retry_count (up to 3, no backoff)
- [x] Dead letter to `run_queue_dlq` after max retries
- [x] Set DB status to `system_error` on exhausted retries

#### 5.8 Metrics
- [x] Export `judge_execution_duration_seconds` histogram (by problem_id)
- [x] Export `judge_memory_usage_bytes` histogram (by problem_id)
- [x] Export `judge_verdict_total` counter (by verdict)
- [x] Export `judge_jobs_processed_total` counter
- [x] Export `judge_jobs_failed_total` counter
- [x] Export `judge_active_jobs` gauge
- [x] Expose `/health` endpoint

#### 5.9 Known Limitations
- [x] ~~Generator and checker are NOT sandboxed~~ — now run inside cgroups v2 sandbox with memory/PID limits and network isolation
- [ ] ⭐ No seccomp profile applied to user binaries, generators, or checkers

---

### Phase 6: Horus (Cleaner Service) ✅
**Goal:** Scheduled cleanup with configurable policies.

#### 6.1 Project Setup
- [x] Initialize `horus` crate
- [x] Setup cron scheduler (tokio-cron-scheduler)
- [x] Graceful shutdown (AtomicBool + signal handlers)

#### 6.2 Cleanup Specifications
- [x] Create own `CleanupSpec` trait (separate from olympus-rules `Specification`)
- [x] Implement `And`, `Or`, `Not` combinators with `CleanupSpecExt` fluent API
- [x] Create `LastAccessOlderThan` spec (checks `.last_access` file, falls back to atime)
- [x] Create `CreatedOlderThan` spec (checks created time, falls back to mtime)
- [x] Create `IsFile` / `IsDirectory` specs
- [x] Create `HasActiveSubmission` spec (DB: status IN PENDING/COMPILING/JUDGING)
- [x] Create `HasSubmissionRecord` spec (DB: any submission row exists)
- [x] Create `HasProblemRecord` spec (DB: problem exists)

#### 6.3 Policy Implementation
- [x] Stale testcase cleanup (hourly): `IsDirectory & LastAccessOlderThan(6h) & !HasProblemRecord`
- [x] Orphan temp directory cleanup (every 15 min): `IsDirectory & CreatedOlderThan(1h) & !HasActiveSubmission`
- [x] Orphan binary cleanup (daily @ 3am): `IsFile & CreatedOlderThan(24h) & !HasSubmissionRecord`
- [x] Old submission cleanup (weekly, disabled by default): DB query + file/row deletion
- [x] Log cleanup actions

#### 6.4 Config Reload via Redis Pub/Sub
- [x] Create `rule_configs` table migration (with 3 default Horus policies seeded)
- [x] Subscribe to `config_reload` channel (filter for `"horus"` payload)
- [x] Reload policies from `rule_configs` table on signal
- [x] Auto-reconnect on Redis error (5s backoff)
- [x] `PolicyStore` (Arc<RwLock<Vec<LoadedPolicy>>>) for storing loaded policies

#### 6.5 Known Limitations
- [ ] ⭐ Loaded policies from `PolicyStore` are not wired into cleanup jobs (cleanup uses hardcoded spec compositions)

---

### Phase 7: Admin Dashboard APIs ✅
**Goal:** Admin-only management endpoints.

#### 7.1 User Management
- [x] Implement `GET /api/v1/admin/users` (filterable by role, is_banned, search)
- [x] Implement `PUT /api/v1/admin/users/{id}/role` (prevents self-role-change)
- [x] Implement `POST /api/v1/admin/users/{id}/ban` (also deletes all sessions)
- [x] Implement `POST /api/v1/admin/users/{id}/unban`

#### 7.2 System Management
- [x] Implement `GET /api/v1/admin/stats` (user/contest/submission/storage counts)
- [ ] Implement `GET /api/v1/admin/containers` (list running Docker containers with resource usage which are part of the system, i.e. Sisyphus compile containers, or the containers spawned by sisyphus for compilation if we go with the host path translation strategy) 

#### 7.3 Queue Management
- [x] Implement `GET /api/v1/admin/queue` (XLEN + XINFO GROUPS + XPENDING for both streams)
- [x] Implement `POST /api/v1/admin/queue/{id}/rejudge` (reset status, delete results, re-queue)
- [ ] Implement `GET /api/v1/admin/contest/{id}/rejudge` (rejudge all submissions in a contest)

#### 7.4 Rule Configuration
- [x] Implement `GET /api/v1/admin/rules` (filterable by service, enabled)
- [x] Implement `POST /api/v1/admin/rules` (upsert, validates JSON against SpecRegistry)
- [x] Implement `PUT /api/v1/admin/rules/{id}` (partial update)
- [x] Implement Redis pub/sub notification on `config_reload` channel after save

---

### Phase 8: Testing & Documentation
**Goal:** Comprehensive test coverage and API docs.

#### 8.1 Unit Tests
- [x] `olympus-rules` specification tests (operators, rules, registry — 16+ tests)
- [ ] `olympus-common` utility tests
- [ ] Vanguard handler unit tests
- [ ] Sisyphus compilation logic tests
- [ ] Minos verdict logic tests
- [ ] Horus policy evaluation tests

#### 8.2 Integration Tests
- [ ] Auth flow integration tests
- [ ] Full submission flow (Vanguard → Sisyphus → Minos)
- [ ] Contest lifecycle tests
- [ ] Rate limiting tests

#### 8.3 Infrastructure
- [ ] Setup testcontainers for PostgreSQL
- [ ] Mock Redis for unit tests
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Docker build for all services

#### 8.4 Documentation
- [x] docs/API.md — API endpoint reference
- [x] docs/EXECUTION_FLOW.md — Detailed execution flow documentation
- [x] docs/DESIGN.md — Architecture overview
- [x] docs/SPEC_REGISTRY.md — SpecRegistry and rules engine documentation
- [x] docs/CONTESTANT_GUIDE.md
- [x] docs/QUESTION_SETTER_GUIDE.md
- [ ] OpenAPI/Swagger spec generation
- [ ] README for each crate
- [ ] Deployment guide

---

### Phase 9: Production Readiness
**Goal:** Hardening for production deployment.

- [x] Structured logging (tracing crate — used in all services)
- [x] Health checks for all services (Vanguard: /health/*, Minos: /health)
- [x] Graceful shutdown handling (all services: AtomicBool + signal handlers)
- [x] Configuration via environment variables (all services)
- [ ] Secrets management (no hardcoded values)
- [ ] Rate limit tuning based on load testing
- [ ] Database connection pool sizing tuning
- [ ] Redis connection pool sizing tuning
- [ ] Prometheus alerting rules
- [ ] Grafana dashboards
- [x] Sandbox generator/checker binaries (cgroups/namespaces)
- [ ] Add seccomp profiles for runner sandbox
- [ ] Implement source code compilation in Sisyphus
- [ ] Wire PolicyStore into Horus cleanup jobs

---

## Project Overview

AlgoJudge (codename: **Olympus**) is a distributed competitive programming judge system built entirely in **Rust**. It follows a microservices architecture with four distinct services communicating via Redis Streams.

## Architecture

### Microservices

| Service | Name | Purpose |
|---------|------|---------|
| API Gateway | **Vanguard** | REST API, Authentication, Contest Management |
| Compiler | **Sisyphus** | Compilation Worker (Docker containers) |
| Judge | **Minos** | Execution, Verification (cgroups v2 + namespaces) |
| Cleaner | **Horus** | Maintenance, Cleanup, Cron Jobs |

### Technology Stack

- **Language:** Rust (all services)
- **Async Runtime:** Tokio
- **Web Framework:** Axum (Vanguard, Minos metrics server)
- **Database:** PostgreSQL (via SQLx)
- **Message Queue:** Redis Streams (for high-performance job queuing)
- **Cache & Rate Limiting:** Redis (via deadpool-redis)
- **Metrics:** Prometheus/Grafana
- **Compilation Sandboxing:** Docker containers (Sisyphus)
- **Execution Sandboxing:** cgroups v2 + Linux namespaces (Minos)
- **Storage:** Shared persistent volume at `/mnt/data`

### Workspace Crates

```
olympus/
├── Cargo.toml                 # Workspace root (6 members)
├── crates/
│   ├── olympus-common/        # Shared types, errors, utilities
│   ├── olympus-rules/         # Shared Specification Pattern crate
│   ├── vanguard/              # API Gateway
│   ├── sisyphus/              # Compiler
│   ├── minos/                 # Judge
│   └── horus/                 # Cleaner
├── docs/
│   ├── API.md
│   ├── DESIGN.md
│   ├── EXECUTION_FLOW.md
│   ├── SPEC_REGISTRY.md
│   ├── CONTESTANT_GUIDE.md
│   └── QUESTION_SETTER_GUIDE.md
├── config/
│   └── prometheus.yml
├── scripts/
│   └── start.sh
├── Dockerfile
├── docker-compose.yml
└── docker-compose.testing.yml
```

## Code Style & Patterns

### Design Patterns

1. **Domain-Driven Design (DDD):** Organize code by business domain (auth, submission, contest, etc.)
2. **Specification Pattern:** Used for authorization rules (olympus-rules), cleanup policies (Horus), and verdict determination (Minos)
3. **Clean Architecture:** Strict separation of concerns with handler/request/response structure

### Rust Conventions

- Use **Tokio** as the async runtime for all services
- Use `async/await` for all I/O operations
- Prefer `Result<T, AppError>` or `Result<T, ApiError>` for error handling
- Use `State<AppState>` for dependency injection in handlers
- Follow the module structure: `mod.rs`, `handler.rs`, `request.rs`, `response.rs`
- Use `deadpool-redis` for Redis connection pooling
- Use `tracing` for structured logging

### Directory Structure (Vanguard)

```
vanguard/
├── Cargo.toml
├── migrations/
│   ├── 20260101000001_create_users_and_sessions.sql
│   ├── 20260102000001_create_contests_and_problems.sql
│   ├── 20260103000001_create_submissions.sql
│   ├── 20260104000001_add_upload_limits.sql
│   ├── 20260105000001_standalone_submissions_and_queue_pending.sql
│   ├── 20260106000001_create_rule_configs.sql
│   └── 20260107000001_add_threads_and_network_to_problems.sql
└── src/
    ├── main.rs
    ├── config.rs              # Environment config & DB/Redis pool setup
    ├── state.rs               # AppState struct
    ├── error.rs               # ApiError enum & response formatting
    ├── middleware/
    │   ├── auth.rs            # JWT auth, admin, organizer middleware
    │   └── rate_limit.rs      # Per-tier rate limiting
    └── domain/
        ├── authorization.rs   # Specification-based auth checks (require_* functions)
        ├── auth/              # Register, login, refresh, logout, me
        ├── health/            # Health, liveness, readiness probes
        ├── users/             # User CRUD + stats + submissions
        ├── contests/          # Contest CRUD + registration + collaborators
        ├── problems/          # Problem CRUD + binary upload/download
        ├── submissions/       # Submission creation + results + leaderboard
        └── admin/             # User mgmt, stats, queue, rules
```

### Backend Service Structure (Sisyphus, Minos, Horus)

```
service/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, signal handling, consumer loop
    ├── config.rs         # Environment-based configuration
    ├── consumer.rs       # Redis Stream consumer (XREADGROUP)
    ├── ...               # Service-specific modules
```

## API Guidelines

### Base URL

All API endpoints use the base URL: `/api/v1`

### Authentication

- JWT tokens required in `Authorization: Bearer <token>` header
- Middleware: `auth_middleware` for protected routes, `admin_middleware` for admin routes
- Role-based access: `admin`, `organizer`, `participant`, `spectator`

### Response Codes

| Code | Usage |
|------|-------|
| 200 | Success |
| 201 | Created |
| 202 | Accepted (async operation queued) |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 409 | Conflict |
| 422 | Unprocessable Entity |
| 429 | Too Many Requests |
| 500 | Internal Server Error |
| 502 | External Service Error |
| 504 | Timeout Error |

### Request/Response Format

- Use JSON for all request/response bodies
- Use `snake_case` for JSON field names
- Include proper validation in request DTOs
- Return consistent error response structure: `{ "error": { "code": "...", "message": "..." } }`

## Storage Paths

```
/mnt/data/
├── submissions/
│   ├── {contest_id}/{user_id}/{submission_id}.zip   # Contest submissions
│   └── standalone/{user_id}/{submission_id}.zip     # Standalone submissions
├── binaries/
│   ├── problems/{problem_id}/
│   │   ├── generator        # Test case generator binary
│   │   └── checker          # Output checker binary
│   └── users/
│       └── {submission_id}_bin  # Compiled binary (file or directory)
├── testcases/{problem_id}/
│   ├── input_001.txt        # Generated test case inputs
│   ├── input_002.txt
│   └── .last_access         # RFC3339 timestamp for cache invalidation
└── temp/{submission_id}/    # Volatile execution scratch (Minos)
```

## Submission Flow

1. **Vanguard:** Receives ZIP/source, validates, saves to storage, queues to `compile_queue` (Redis Stream)
2. **Sisyphus:** Consumes from `compile_queue`, compiles in Docker container, saves binary, queues to `run_queue`
3. **Minos:** Consumes from `run_queue`, checks binaries exist (or `queue_pending`), runs binary in cgroup sandbox against test cases, verifies with checker, updates verdict
4. **Horus:** Cron-scheduled cleanup of stale testcases (>6h), orphan temps (>1h), orphan binaries (>24h)

## Rate Limiting (Redis-based)

### Strategy: Fixed Window Counter (Fail-Open)

Use Redis `INCR` with `EXPIRE` for distributed rate limiting. On Redis failure, requests pass through.

### Rate Limit Tiers (Defaults)

| Action | Limit | Window | Key Pattern |
|--------|-------|--------|-------------|
| Login attempts | 40 | 15 min | `rl:login:{ip}` |
| Registration | 30 | 15 min | `rl:register:{ip}` |
| Submission | 5 | 1 min | `rl:submit:{user_id}` |
| API (authenticated) | 600 | 1 min | `rl:api:{user_id}` |
| API (anonymous) | 100 | 1 min | `rl:api:{ip}` |

### Rate Limit Response Headers

```
X-RateLimit-Limit: 600
X-RateLimit-Remaining: 595
X-RateLimit-Reset: 1706832000
Retry-After: 60  # Only on 429
```

## Redis Streams for Message Queues

### Stream Names

| Stream | Producer | Consumer | Purpose |
|--------|----------|----------|---------|
| `compile_queue` | Vanguard | Sisyphus (`sisyphus_group`) | Compilation jobs |
| `compile_queue_dead_letter` | Sisyphus | — | Failed compilation jobs |
| `run_queue` | Sisyphus | Minos (`minos_group`) | Execution/judging jobs |
| `run_queue_dlq` | Minos | — | Failed judging jobs |

### Consumer Groups

```
XGROUP CREATE compile_queue sisyphus_group $ MKSTREAM
XGROUP CREATE run_queue minos_group $ MKSTREAM
```

Both services auto-recreate consumer groups on `NOGROUP` errors.

### Producer Pattern (Vanguard → compile_queue)

```
XADD compile_queue *
  submission_id  <uuid>
  type           "zip" | "source"
  file_path      <path>          # only for ZIP
  language       <lang>          # optional
```

### Producer Pattern (Sisyphus → run_queue)

```
XADD run_queue *
  submission_id  <uuid>
  binary_path    <path>
```

### Consumer Pattern (Sisyphus/Minos)

```
XREADGROUP GROUP {group} {worker_id}
  COUNT 1
  BLOCK 5000
  STREAMS {stream} >
```

## Specification Pattern Implementation

The `olympus-rules` crate provides a composable rule engine using the Specification Pattern.

### Crate Structure

```
crates/olympus-rules/
├── Cargo.toml              # Optional "auth" feature gates DB/Redis deps
└── src/
    ├── lib.rs
    ├── specification.rs    # Core trait, And/Or/Not, AlwaysTrue/False, BoxedSpec, AllOf, AnyOf
    ├── operators.rs        # Spec<S> wrapper for BitAnd, BitOr, Not overloading
    ├── context.rs          # EvalContext, FileContext, ExecutionContext, AuthContext
    ├── rules.rs            # File specs, execution specs, VerdictDeterminer
    ├── auth_rules.rs       # Auth specs (behind "auth" feature)
    ├── config.rs           # RuleConfig, NamedRuleConfig, CleanupPolicy, CleanupAction
    └── registry.rs         # SpecRegistry, file_context_registry, execution_context_registry, auth_context_registry
```

### Context Types

| Context | Fields | Used By |
|---------|--------|---------|
| `EvalContext` | HashMap-backed strings/ints/booleans | Generic evaluation |
| `FileContext` | path, is_file, is_directory, size_bytes, timestamps | Horus cleanup |
| `ExecutionContext` | submission_id, problem_id, exit_code, time_ms, memory_kb, limits | Minos verdict |
| `AuthContext` | user_id, role, is_banned, db, redis, contest_id?, problem_id?, submission_id? | Vanguard auth |

### Authorization Usage (Vanguard)

Authorization checks in Vanguard use `require_*` convenience functions in `authorization.rs`,
which build an `AuthContext` and evaluate individual spec instances:

```rust
// Example: checking if user can submit to a contest
pub async fn require_can_submit(ctx: &AuthContext) -> ApiResult<()> {
    // IsValidUser AND (IsAdmin OR IsCollaborator OR (IsParticipant AND NotRateLimited))
    if !IsValidUser.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden("..."));
    }
    if IsAdmin.is_satisfied_by(ctx).await || IsCollaborator.is_satisfied_by(ctx).await {
        return Ok(());
    }
    if !IsParticipant.is_satisfied_by(ctx).await {
        return Err(ApiError::Forbidden("..."));
    }
    if !NotRateLimited::submission().is_satisfied_by(ctx).await {
        return Err(ApiError::RateLimitExceeded);
    }
    Ok(())
}
```

### JSON Configuration for Admin Dashboard

Rules can be serialized/deserialized for admin configuration:

```json
{
  "rule_name": "submission_authorization",
  "version": "1.0",
  "rule": {
    "type": "And",
    "rules": [
      { "type": "Spec", "name": "IsValidUser", "params": {} },
      {
        "type": "Or",
        "rules": [
          {
            "type": "And",
            "rules": [
              { "type": "Not", "rule": { "type": "Spec", "name": "NotRateLimited", "params": { "action": "submission", "limit": "10", "window_secs": "60" } } },
              { "type": "Spec", "name": "IsParticipant", "params": {} }
            ]
          },
          { "type": "Spec", "name": "IsAdmin", "params": {} }
        ]
      }
    ]
  }
}
```

### Pre-built Registries

| Registry | Context | Specs |
|----------|---------|-------|
| `file_context_registry()` | `FileContext` | `LastAccessOlderThan`, `CreatedOlderThan`, `IsFile`, `IsDirectory`, `SizeLargerThan` |
| `execution_context_registry()` | `ExecutionContext` | `WithinTimeLimit`, `WithinMemoryLimit`, `ExitCodeZero`, `OutputMatches`, `AcceptedVerdict` |
| `auth_context_registry()` | `AuthContext` | `IsValidUser`, `IsAdmin`, `IsOrganizer`, `IsParticipant`, `IsCollaborator`, `IsContestOwner`, `CanAddProblems`, `IsProblemOwner`, `CanAccessProblemBinaries`, `IsSubmissionOwner`, `NotRateLimited`, `NotRateLimited:submission`, `NotRateLimited:api` |

## Metrics (Minos)

Export these Prometheus metrics on port 9091:
- `judge_execution_duration_seconds` (Histogram, by `problem_id`)
- `judge_memory_usage_bytes` (Histogram, by `problem_id`)
- `judge_verdict_total{verdict="accepted|wrong_answer|..."}` (Counter)
- `judge_jobs_processed_total` (Counter)
- `judge_jobs_failed_total` (Counter)
- `judge_active_jobs` (Gauge)

## File Uploads

All file uploads use `multipart/form-data` (NOT base64 encoding) for efficiency.

### Submission Upload

`POST /api/v1/submissions/upload?contest_id=...&problem_id=...&language=...`

- Content-Type: `multipart/form-data`
- Field: `file` (the ZIP submission)
- Size limit: Contest-specific (1-100MB, default 10MB)
- `contest_id` optional (omit for standalone/practice)
- `language` optional (helps Sisyphus select Docker image)

### Problem Binaries

Generator and checker binaries are uploaded separately after problem creation:

1. `POST /api/v1/problems` - Create problem metadata (returns draft status)
2. `POST /api/v1/problems/{id}/generator` - Upload generator binary
3. `POST /api/v1/problems/{id}/checker` - Upload checker binary
4. Problem status becomes "ready" when both are uploaded; `queue_pending` submissions are re-queued

Binary uploads:
- Content-Type: `multipart/form-data`
- Field: `file` (Linux ELF executable)
- Size limit: 50MB

## ZIP Submission Format

User submissions must contain:
```
submission.zip
├── compile.sh    # Compilation script (required)
└── run.sh        # Execution script (required)
```

**Security validation:**
- No symlinks pointing outside archive
- No absolute paths
- No path traversal (`..`)
- Total uncompressed size < 5x compressed size (zip bomb protection)
- Both compile.sh and run.sh must exist
- CRLF line endings are stripped automatically by Sisyphus

Supported runtimes: `cpp`, `c`, `rust`, `go`, `python`, `zig`

## Problem Definition

Problems require:
- Generator binary (uploaded via multipart, creates test cases via `./generator N > input.txt`)
- Checker/Verifier binary (uploaded via multipart, testlib convention exit codes)
- Problem code (A, B, C, etc.)
- Time/memory limits
- Number of test cases
- `max_threads` (default 1, controls PID cgroup limit)
- `network_allowed` (default false, controls network namespace isolation)
- Allowed runtimes
- All limits can be overridden per-contest in `contest_problems`

## Security & Isolation

### Sisyphus (Compilation) — Docker container
- Network: Disabled (`--network=none`, configurable)
- Timeout: 30 seconds (configurable)
- Memory: 2GB limit
- CPU: 2 cores
- PIDs: 256 limit
- Filesystem: Read-only root + tmpfs for /tmp and /root/.cache
- Capabilities: ALL dropped

### Minos (Execution) — cgroups v2 + namespaces
- Network: Disabled via `unshare(CLONE_NEWNET)` when `network_allowed=false`
- Timeout: Per-problem time limit + 100ms buffer
- Memory: Per-problem limit via cgroups v2 (`memory.max`, swap disabled)
- PIDs: `max_threads + 16` via cgroups v2 (`pids.max`) — buffer for runtime threads (Go GC, etc.)
- Process: `stdin=null`, `kill_on_drop=true`
- **No seccomp profile** (not yet implemented)

### Generator/Checker — cgroups v2 + namespaces
- Network: Disabled via `unshare(CLONE_NEWNET)` (always)
- Timeout: 60s each (configurable via `GENERATOR_TIME_LIMIT_MS`, `CHECKER_TIME_LIMIT_MS`)
- Memory: Configurable via cgroups v2 (`GENERATOR_MEMORY_LIMIT_KB` default 4GB, `CHECKER_MEMORY_LIMIT_KB` default 4GB)
- PIDs: Limited (`pids.max = max_pids + 16`) via cgroups v2
- Process: `stdin=null`, `kill_on_drop=true`
- OOM detection via `memory.events` → `oom_kill` counter
- **No seccomp profile** (not yet implemented)

## Testing Guidelines

- Unit test all Specification implementations
- Integration test the full submission flow
- Mock Redis and storage for handler tests
- Use testcontainers for PostgreSQL tests

## Configuration Reference

### Vanguard
| Env Var | Default | Description |
|---------|---------|-------------|
| `HOST` | `0.0.0.0` | Listen address |
| `PORT` | `8081` | Listen port |
| `DATABASE_URL` | `postgres://olympus:olympus@localhost:5432/olympus` | PostgreSQL connection |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection |
| `JWT_SECRET` | `dev-secret-...` | JWT signing secret |
| `JWT_ACCESS_EXPIRATION` | `900` | Access token TTL (seconds) |
| `JWT_REFRESH_EXPIRATION` | `604800` | Refresh token TTL (seconds) |
| `MAX_THREADS_LIMIT` | `64` | Max allowed threads per problem |

### Sisyphus
| Env Var | Default | Description |
|---------|---------|-------------|
| `DATABASE_URL` | `postgres://olympus:olympus@localhost:5432/olympus` | PostgreSQL connection |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection |
| `CONSUMER_GROUP` | `sisyphus_group` | Redis consumer group |
| `COMPILE_STREAM` | `compile_queue` | Input stream |
| `RUN_STREAM` | `run_queue` | Output stream |
| `COMPILE_TIMEOUT_SECS` | `30` | Compilation timeout |
| `NETWORK_ENABLED` | `false` | Allow network in Docker containers |
| `MAX_MEMORY_BYTES` | `2147483648` | Docker memory limit (2GB) |
| `MAX_CPU_CORES` | `2` | Docker CPU limit |
| `BUILD_DIR_BASE` | `/mnt/data/temp/builds` | Build directory base |
| `STORAGE_BASE_PATH` | `/mnt/data` | Storage root |
| `DOCKER_HOST_DATA_PATH` | — | Host-side path for Docker volume mounts |
| `DOCKER_VOLUME_NAME` | — | Docker named volume for builds |
| `CONTAINER_IMAGE_CPP` | `gcc:latest` | Docker image override |
| `CONTAINER_IMAGE_RUST` | `rust:1.85-bookworm` | Docker image override |
| `CONTAINER_IMAGE_GO` | `golang:1.23-bookworm` | Docker image override |
| `CONTAINER_IMAGE_PYTHON` | `python:3.12-bookworm` | Docker image override |
| `CONTAINER_IMAGE_ZIG` | `euantorano/zig:0.13.0` | Docker image override |
| `CONTAINER_IMAGE_GENERIC` | `ubuntu:24.04` | Docker image override |

### Minos
| Env Var | Default | Description |
|---------|---------|-------------|
| `DATABASE_URL` | — (required) | PostgreSQL connection |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection |
| `CONSUMER_GROUP` | `minos_group` | Redis consumer group |
| `STREAM_NAME` | `run_queue` | Input stream |
| `BLOCK_TIMEOUT_MS` | `5000` | XREADGROUP block time |
| `MAX_RETRIES` | `3` | Max retry count |
| `METRICS_PORT` | `9091` | Prometheus metrics port |
| `GENERATOR_TIME_LIMIT_MS` | `60000` | Generator timeout |
| `CHECKER_TIME_LIMIT_MS` | `60000` | Checker timeout |
| `OUTPUT_LIMIT_BYTES` | `67108864` | Output file size limit (64MB) |
| `MAX_THREADS_LIMIT` | `64` | Max threads clamp |
| `STORAGE_BASE_PATH` | `/mnt/data` | Storage root |

### Horus
| Env Var | Default | Description |
|---------|---------|-------------|
| `DATABASE_URL` | — (required) | PostgreSQL connection |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection |
| `TESTCASE_STALE_HOURS` | `6` | Testcase cache TTL |
| `TEMP_ORPHAN_HOURS` | `1` | Temp dir cleanup threshold |
| `SUBMISSION_RETENTION_DAYS` | `0` | Submission retention (0=disabled) |
| `STORAGE_BASE_PATH` | `/mnt/data` | Storage root |
| `TESTCASE_CLEANUP_CRON` | `0 0 * * * *` | Testcase cleanup schedule |
| `TEMP_CLEANUP_CRON` | `0 */15 * * * *` | Temp cleanup schedule |
| `BINARY_CLEANUP_CRON` | `0 0 3 * * *` | Binary cleanup schedule |
| `SUBMISSION_CLEANUP_CRON` | `0 0 4 * * 0` | Submission cleanup schedule |

## Do Not

- Use base64 encoding for file uploads (use multipart/form-data)
- Expose internal error details to API responses
- Skip authentication middleware on protected endpoints
- Store secrets in code (use environment variables)
- Allow network access in sandboxed execution (unless explicitly configured per-problem)
- Trust problem setter code without the sandbox layer (generators/checkers are sandboxed via cgroups v2 + namespaces)
- Forget to clean up temp directories after execution
- Assume source code compilation works (only ZIP submissions are fully supported)
