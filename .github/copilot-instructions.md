# GitHub Copilot Instructions for AlgoJudge (Olympus)

## Implementation Plan

This is an incremental implementation roadmap. Complete phases in order. Each phase builds on the previous.

---

### Phase 0: Project Setup
**Goal:** Initialize workspace structure and shared dependencies.

- [ ] Create workspace `Cargo.toml` with all crate members
- [ ] Create `crates/` directory structure
- [ ] Initialize `olympus-common` crate
  - [ ] Define `AppError` enum with error variants
  - [ ] Define common types (`UserId`, `ContestId`, `SubmissionId`, etc.)
  - [ ] Add shared utilities (UUID generation, timestamp helpers)
- [ ] Initialize `olympus-rules` crate (Specification Pattern)
  - [ ] Define `Specification<Ctx>` async trait
  - [ ] Implement `And<A, B>`, `Or<A, B>`, `Not<A>` combinators
  - [ ] Implement `BitAnd`, `BitOr`, `Not` operator overloading
  - [ ] Add `RuleConfig` serde structs for JSON serialization
  - [ ] Create `SpecRegistry` for dynamic rule building
- [ ] Setup shared `docker-compose.yml` for local development
  - [ ] PostgreSQL service
  - [ ] Redis service
  - [ ] Prometheus/Grafana services

---

### Phase 1: Vanguard (API Gateway) - Core
**Goal:** Basic API with auth, no queue integration yet.

#### 1.1 Project Scaffolding
- [ ] Initialize `vanguard` crate with Axum
- [ ] Setup `config.rs` (env vars, database pool, Redis pool)
- [ ] Create domain folder structure (`auth/`, `health/`)
- [ ] Add `AppState` struct (db pool, redis pool)

#### 1.2 Health & Database
- [ ] Implement `GET /health` endpoint
- [ ] Setup SQLx with PostgreSQL
- [ ] Create initial migrations
  - [ ] `users` table
  - [ ] `sessions` table (for refresh tokens)
- [ ] Test database connectivity

#### 1.3 Authentication
- [ ] Implement `POST /api/v1/auth/register`
  - [ ] Password hashing (argon2)
  - [ ] Email validation
  - [ ] Username uniqueness check
- [ ] Implement `POST /api/v1/auth/login`
  - [ ] JWT token generation
  - [ ] Refresh token storage in Redis
- [ ] Implement `POST /api/v1/auth/refresh`
- [ ] Implement `POST /api/v1/auth/logout`
- [ ] Implement `GET /api/v1/auth/me`
- [ ] Create `AuthMiddleware` (JWT verification)

#### 1.4 Rate Limiting Middleware
- [ ] Implement `RateLimitMiddleware`
  - [ ] Redis INCR + EXPIRE pattern
  - [ ] Extract key from IP or user_id
  - [ ] Add `X-RateLimit-*` response headers
- [ ] Configure rate limit tiers in `config.rs`
- [ ] Add `429 Too Many Requests` error response

#### 1.5 User Management
- [ ] Implement `GET /api/v1/users`
- [ ] Implement `GET /api/v1/users/{id}`
- [ ] Implement `PUT /api/v1/users/{id}` (owner only)
- [ ] Implement `GET /api/v1/users/{id}/stats`

---

### Phase 2: Vanguard - Contests & Problems
**Goal:** Contest and problem management APIs.

#### 2.1 Database Migrations
- [ ] `contests` table
- [ ] `contest_participants` table
- [ ] `contest_collaborators` table
- [ ] `problems` table
- [ ] `contest_problems` junction table
- [ ] `test_cases` table (legacy)

#### 2.2 Contest Endpoints
- [ ] Implement `GET /api/v1/contests`
- [ ] Implement `POST /api/v1/contests` (organizer/admin)
- [ ] Implement `GET /api/v1/contests/{id}`
- [ ] Implement `PUT /api/v1/contests/{id}`
- [ ] Implement `DELETE /api/v1/contests/{id}`

#### 2.3 Contest Registration
- [ ] Implement `POST /api/v1/contests/{id}/register`
- [ ] Implement `POST /api/v1/contests/{id}/unregister`
- [ ] Implement `GET /api/v1/contests/{id}/participants`

#### 2.4 Contest Collaborators
- [ ] Implement `GET /api/v1/contests/{id}/collaborators`
- [ ] Implement `POST /api/v1/contests/{id}/collaborators`
- [ ] Implement `DELETE /api/v1/contests/{id}/collaborators/{user_id}`

#### 2.5 Problems
- [ ] Implement `GET /api/v1/problems`
- [ ] Implement `POST /api/v1/problems` (with generator/verifier upload)
- [ ] Implement `GET /api/v1/problems/{id}`
- [ ] Implement `PUT /api/v1/problems/{id}`
- [ ] Implement `DELETE /api/v1/problems/{id}`
- [ ] Implement `GET /api/v1/contests/{id}/problems`
- [ ] Implement `POST /api/v1/contests/{id}/problems`
- [ ] Implement `DELETE /api/v1/contests/{id}/problems/{problem_id}`

#### 2.6 Authorization Rules (olympus-rules)
- [ ] Create `IsParticipant(contest_id)` spec
- [ ] Create `IsCollaborator(contest_id)` spec
- [ ] Create `IsContestOwner(contest_id)` spec
- [ ] Create `IsProblemOwner(problem_id)` spec
- [ ] Integrate rules into contest/problem handlers

---

### Phase 3: Vanguard - Submissions & Queue
**Goal:** Submission handling with Redis Stream integration.

#### 3.1 Database & Storage
- [ ] `submissions` table migration
- [ ] `submission_results` table migration
- [ ] Create `/mnt/data/submissions/` directory structure
- [ ] Implement file storage utilities

#### 3.2 Submission Endpoints
- [ ] Implement `POST /api/v1/submissions` (legacy source)
- [ ] Implement `POST /api/v1/submissions/zip` (algorithmic benchmark)
  - [ ] Validate ZIP structure (compile.sh, run.sh)
  - [ ] Save to `/mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip`
  - [ ] Push to `compile_queue` Redis Stream
- [ ] Implement `GET /api/v1/submissions`
- [ ] Implement `GET /api/v1/submissions/{id}`
- [ ] Implement `GET /api/v1/submissions/{id}/results`
- [ ] Implement `GET /api/v1/submissions/{id}/source`
- [ ] Implement `GET /api/v1/users/{id}/submissions`

#### 3.3 Submission Authorization Rules
- [ ] Create `IsSubmissionOwner(submission_id)` spec
- [ ] Create `CanSubmitToContest(contest_id)` composite rule
  - [ ] `IsValidUser & ((!IsRateLimited & IsParticipant) | IsAdmin)`
- [ ] Integrate into submission handlers

#### 3.4 Leaderboard
- [ ] Implement `GET /api/v1/contests/{id}/leaderboard`
- [ ] Implement scoring logic (ICPC style or IOI style)
- [ ] Add caching layer with Redis

---

### Phase 4: Sisyphus (Compiler Service)
**Goal:** Compilation worker consuming from Redis Stream.

#### 4.1 Project Setup
- [ ] Initialize `sisyphus` crate
- [ ] Setup Redis Stream consumer (`XREADGROUP`)
- [ ] Create consumer group `sisyphus_group` on startup

#### 4.2 Compilation Pipeline
- [ ] Implement job consumer loop
- [ ] Create temporary build directory
- [ ] Unzip submission from storage
- [ ] Execute `compile.sh` in isolated container
  - [ ] Network disabled
  - [ ] 30-second timeout
- [ ] Handle compilation success
  - [ ] Move binary to `/mnt/data/binaries/users/{submission_id}_bin`
  - [ ] Update DB status: `COMPILED`
  - [ ] Push to `run_queue` Redis Stream
  - [ ] `XACK` the message
- [ ] Handle compilation failure
  - [ ] Capture stderr logs
  - [ ] Update DB status: `COMPILATION_ERROR`
  - [ ] Store logs for user feedback
  - [ ] `XACK` the message

#### 4.3 Error Handling & Resilience
- [ ] Implement dead letter handling for failed jobs
- [ ] Add retry logic with exponential backoff
- [ ] Graceful shutdown (finish current job)

---

### Phase 5: Minos (Judge Service)
**Goal:** Execution and verification with sandboxing.

#### 5.1 Project Setup
- [ ] Initialize `minos` crate
- [ ] Setup Redis Stream consumer (`XREADGROUP`)
- [ ] Create consumer group `minos_group` on startup
- [ ] Setup Prometheus metrics exporter

#### 5.2 Test Case Management
- [ ] Implement lazy test case generation
  - [ ] Check `/mnt/data/testcases/{problem_id}/`
  - [ ] Run generator if cache miss
  - [ ] Update "last accessed" timestamp on hit

#### 5.3 Sandbox Execution
- [ ] Create `/mnt/data/temp/{submission_id}/` scratch directory
- [ ] Setup cgroups for memory/CPU limits
- [ ] Setup namespaces for network isolation
- [ ] Execute user binary: `./binary < input.txt > output.txt`
- [ ] Capture runtime and memory metrics

#### 5.4 Verdict Specification Rules (olympus-rules)
- [ ] Create `WithinTimeLimit(ms)` spec
- [ ] Create `WithinMemoryLimit(kb)` spec
- [ ] Create `ExitCodeZero` spec
- [ ] Create composite verdict rule
- [ ] Implement checker execution
- [ ] Determine final verdict (AC, WA, TLE, MLE, RE)

#### 5.5 Result Handling
- [ ] Update `submissions` table with verdict
- [ ] Store per-testcase results in `submission_results`
- [ ] `XACK` the message
- [ ] Cleanup temp directory

#### 5.6 Metrics
- [ ] Export `judge_execution_duration_seconds` histogram
- [ ] Export `judge_memory_usage_bytes` gauge
- [ ] Export `judge_verdict_total` counter by verdict type

---

### Phase 6: Horus (Cleaner Service)
**Goal:** Scheduled cleanup with configurable policies.

#### 6.1 Project Setup
- [ ] Initialize `horus` crate
- [ ] Setup cron scheduler (tokio-cron-scheduler)

#### 6.2 Cleanup Specifications (olympus-rules)
- [ ] Create `LastAccessOlderThan(duration)` spec
- [ ] Create `CreatedOlderThan(duration)` spec
- [ ] Create `IsFile` / `IsDirectory` specs
- [ ] Create `HasActiveSubmission` spec (DB lookup)

#### 6.3 Policy Implementation
- [ ] Implement directory scanner (walkdir)
- [ ] Load cleanup policies from database/config
- [ ] Stale testcase cleanup (>6 hours)
- [ ] Orphan temp directory cleanup (>1 hour)
- [ ] Log cleanup actions

#### 6.4 Admin Configuration
- [ ] Create `rule_configs` table migration
- [ ] Implement policy reload via Redis pub/sub
- [ ] Add admin endpoint to save/update policies

---

### Phase 7: Admin Dashboard APIs
**Goal:** Admin-only management endpoints.

#### 7.1 User Management
- [ ] Implement `GET /api/v1/admin/users`
- [ ] Implement `PUT /api/v1/admin/users/{id}/role`
- [ ] Implement `POST /api/v1/admin/users/{id}/ban`
- [ ] Implement `POST /api/v1/admin/users/{id}/unban`

#### 7.2 System Management
- [ ] Implement `GET /api/v1/admin/stats`
- [ ] Implement `GET /api/v1/admin/containers`
- [ ] Implement `DELETE /api/v1/admin/containers/{id}`

#### 7.3 Queue Management
- [ ] Implement `GET /api/v1/admin/queue`
- [ ] Implement `POST /api/v1/admin/queue/{id}/rejudge`

#### 7.4 Rule Configuration
- [ ] Implement `GET /api/v1/admin/rules`
- [ ] Implement `POST /api/v1/admin/rules`
- [ ] Implement `PUT /api/v1/admin/rules/{id}`
- [ ] Implement Redis pub/sub for hot reload

---

### Phase 8: Testing & Documentation
**Goal:** Comprehensive test coverage and API docs.

#### 8.1 Unit Tests
- [ ] `olympus-rules` specification tests
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
- [ ] OpenAPI/Swagger spec generation
- [ ] README for each crate
- [ ] Deployment guide

---

### Phase 9: Production Readiness
**Goal:** Hardening for production deployment.

- [ ] Structured logging (tracing crate)
- [ ] Health checks for all services
- [ ] Graceful shutdown handling
- [ ] Configuration via environment variables
- [ ] Secrets management (no hardcoded values)
- [ ] Rate limit tuning based on load testing
- [ ] Database connection pool sizing
- [ ] Redis connection pool sizing
- [ ] Prometheus alerting rules
- [ ] Grafana dashboards

---

## Project Overview

AlgoJudge (codename: **Olympus**) is a distributed competitive programming judge system built entirely in **Rust**. It follows a microservices architecture with four distinct services communicating via Redis Streams.

## Architecture

### Microservices

| Service | Name | Purpose |
|---------|------|---------|
| API Gateway | **Vanguard** | REST API, Authentication, Contest Management |
| Compiler | **Sisyphus** | Compilation Worker, Queue Consumer |
| Judge | **Minos** | Execution, Verification, Sandboxing |
| Cleaner | **Horus** | Maintenance, Cleanup, Cron Jobs |

### Technology Stack

- **Language:** Rust (all services)
- **Async Runtime:** Tokio
- **Database:** PostgreSQL
- **Message Queue:** Redis Streams (for high-performance job queuing)
- **Cache & Rate Limiting:** Redis
- **Metrics:** Prometheus/Grafana
- **Containerization:** Docker with cgroups/namespaces for sandboxing
- **Storage:** Shared persistent volume at `/mnt/data`

### Workspace Crates

```
olympus/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── olympus-rules/         # Shared Specification Pattern crate
│   ├── olympus-common/        # Shared types, errors, utilities
│   ├── vanguard/              # API Gateway
│   ├── sisyphus/              # Compiler
│   ├── minos/                 # Judge
│   └── horus/                 # Cleaner
```

## Code Style & Patterns

### Design Patterns

1. **Domain-Driven Design (DDD):** Organize code by business domain (auth, submission, contest, etc.)
2. **Specification Pattern:** Used in Minos and Horus for decoupling validation/policy logic
3. **Clean Architecture:** Strict separation of concerns with handler/request/response structure

### Rust Conventions

- Use **Tokio** as the async runtime for all services
- Use `async/await` for all I/O operations
- Prefer `Result<T, AppError>` for error handling
- Use `State<AppState>` for dependency injection in handlers
- Follow the module structure: `mod.rs`, `handler.rs`, `request.rs`, `response.rs`
- Use `deadpool-redis` for Redis connection pooling

### Directory Structure (Vanguard Example)

```
service_name/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── config.rs              # Environment & DB pool setup
    ├── middleware/            # Auth, RateLimiting, CORS, RequestID
    └── domain/
        └── {feature}/         # e.g., auth, submission, contest
            └── handler/
                ├── mod.rs
                ├── handler.rs # Implementation
                ├── request.rs # Request DTOs
                └── response.rs# Response DTOs
```

## API Guidelines

### Base URL

All API endpoints use the base URL: `/api/v1`

### Authentication

- JWT tokens required in `Authorization: Bearer <token>` header
- Endpoints marked with "Auth: Yes" require authentication
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
| 422 | Unprocessable Entity |
| 429 | Too Many Requests |

### Request/Response Format

- Use JSON for all request/response bodies
- Use `snake_case` for JSON field names
- Include proper validation in request DTOs
- Return consistent error response structure

## Storage Paths

```
/mnt/data/
├── submissions/{contest_id}/{user_id}/{submission_id}.zip
├── binaries/
│   ├── problems/{problem_id}/generator, checker
│   └── users/{submission_id}_bin
├── testcases/{problem_id}/testcase{n}.txt
└── temp/{submission_id}/  # Volatile execution scratch
```

## Submission Flow

1. **Vanguard:** Receives ZIP, saves to storage, queues to `compile_queue` (Redis Stream)
2. **Sisyphus:** Compiles submission, saves binary, queues to `run_queue` (Redis Stream)
3. **Minos:** Runs binary against test cases, verifies output, updates verdict
4. **Horus:** Cleans stale testcases (>6h) and orphan temp dirs (>1h)

## Rate Limiting (Redis-based)

### Strategy: Fixed Window Counter

Use Redis `INCR` with `EXPIRE` for simple, efficient distributed rate limiting.

### Rate Limit Tiers

| Action | Limit | Window | Key Pattern |
|--------|-------|--------|-------------|
| Login attempts | 5 | 15 min | `rl:login:{ip}` |
| Registration | 3 | 1 hour | `rl:register:{ip}` |
| Submission | 10 | 1 min | `rl:submit:{user_id}` |
| API (authenticated) | 100 | 1 min | `rl:api:{user_id}` |
| API (anonymous) | 20 | 1 min | `rl:api:{ip}` |

### Implementation (Simple INCR + EXPIRE)

```rust
// middleware/rate_limit.rs
pub async fn check_rate_limit(
    redis: &deadpool_redis::Pool,
    key: &str,
    limit: u64,
    window_secs: u64,
) -> Result<RateLimitInfo, AppError> {
    let mut conn = redis.get().await?;
    
    let count: u64 = redis::cmd("INCR")
        .arg(key)
        .query_async(&mut conn)
        .await?;
    
    if count == 1 {
        // First request - set expiry
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(window_secs)
            .query_async(&mut conn)
            .await?;
    }
    
    let ttl: i64 = redis::cmd("TTL")
        .arg(key)
        .query_async(&mut conn)
        .await?;
    
    Ok(RateLimitInfo {
        limit,
        remaining: limit.saturating_sub(count),
        reset: ttl.max(0) as u64,
        allowed: count <= limit,
    })
}
```

### Rate Limit Response Headers

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1706832000
Retry-After: 60  # Only on 429
```

## Redis Streams for Message Queues

### Stream Names

- `compile_queue` - Compilation jobs
- `run_queue` - Execution/judging jobs
- `notification_queue` - User notifications (optional)

### Consumer Groups

```rust
// Create consumer group (run once at startup)
XGROUP CREATE compile_queue sisyphus_group $ MKSTREAM
XGROUP CREATE run_queue minos_group $ MKSTREAM
```

### Producer Pattern (Vanguard)

```rust
// Adding job to stream
redis.xadd(
    "compile_queue",
    "*",  // Auto-generate ID
    &[
        ("submission_id", submission_id.to_string()),
        ("file_path", file_path),
        ("priority", priority.to_string()),
    ],
).await?;
```

### Consumer Pattern (Sisyphus/Minos)

```rust
// Read with consumer group (blocking)
let jobs = redis.xreadgroup(
    "sisyphus_group",
    "worker_1",
    &["compile_queue"],
    &[">"],  // Only new messages
    Some(1), // Count
    Some(5000), // Block 5s
).await?;

// Acknowledge after processing
redis.xack("compile_queue", "sisyphus_group", &[message_id]).await?;
```

## Specification Pattern Implementation

The `olympus-rules` crate provides a composable rule engine using the Specification Pattern with operator overloading for intuitive business logic.

### Crate Structure

```
crates/olympus-rules/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── specification.rs    # Core trait & combinators
    ├── operators.rs        # BitAnd, BitOr, Not implementations
    ├── context.rs          # Evaluation context
    ├── rules/
    │   ├── mod.rs
    │   ├── user.rs         # IsValidUser, IsAdmin, IsBanned
    │   ├── rate_limit.rs   # IsRateLimited
    │   ├── contest.rs      # IsParticipant, IsCollaborator, IsOwner
    │   ├── submission.rs   # HasValidFormat, WithinSizeLimit
    │   └── cleanup.rs      # IsStale, IsOrphan (for Horus)
    └── config/
        ├── mod.rs
        ├── loader.rs       # JSON config loader
        └── schema.rs       # Serde structs for JSON rules
```

### Core Trait Definition

```rust
// specification.rs
use async_trait::async_trait;

#[async_trait]
pub trait Specification<Ctx>: Send + Sync {
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool;
    
    fn and<S: Specification<Ctx>>(self, other: S) -> And<Self, S>
    where Self: Sized {
        And(self, other)
    }
    
    fn or<S: Specification<Ctx>>(self, other: S) -> Or<Self, S>
    where Self: Sized {
        Or(self, other)
    }
    
    fn not(self) -> Not<Self>
    where Self: Sized {
        Not(self)
    }
}
```

### Operator Overloading

```rust
// operators.rs
use std::ops::{BitAnd, BitOr, Not as StdNot};

impl<Ctx, A, B> BitAnd<B> for A
where
    A: Specification<Ctx>,
    B: Specification<Ctx>,
{
    type Output = And<A, B>;
    fn bitand(self, rhs: B) -> Self::Output {
        And(self, rhs)
    }
}

impl<Ctx, A, B> BitOr<B> for A
where
    A: Specification<Ctx>,
    B: Specification<Ctx>,
{
    type Output = Or<A, B>;
    fn bitor(self, rhs: B) -> Self::Output {
        Or(self, rhs)
    }
}

impl<Ctx, A> StdNot for A
where
    A: Specification<Ctx>,
{
    type Output = Not<A>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}
```

### Composable Rule Example

```rust
use olympus_rules::prelude::*;

// Define submission authorization rule
let can_submit = IsValidUser
    & ((!IsRateLimited & IsParticipant(contest_id)) | IsAdmin);

// Evaluate against context
let ctx = SubmissionContext {
    user_id,
    contest_id,
    redis: redis_pool.clone(),
    db: db_pool.clone(),
};

if can_submit.is_satisfied_by(&ctx).await {
    // Allow submission
} else {
    return Err(AppError::Forbidden);
}
```

### JSON Configuration for Admin Dashboard

Rules can be serialized/deserialized for admin configuration, an example rule in JSON:

```json
{
  "rule_name": "submission_authorization",
  "version": "1.0",
  "rule": {
    "and": [
      { "spec": "IsValidUser" },
      {
        "or": [
          {
            "and": [
              { "not": { "spec": "IsRateLimited" } },
              { "spec": "IsParticipant", "params": { "contest_id": "$context.contest_id" } }
            ]
          },
          { "spec": "IsAdmin" }
        ]
      }
    ]
  }
}
```

### Loading Rules from JSON

```rust
// config/loader.rs
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum RuleConfig {
    Spec { name: String, params: Option<serde_json::Value> },
    And { rules: Vec<RuleConfig> },
    Or { rules: Vec<RuleConfig> },
    Not { rule: Box<RuleConfig> },
}

impl RuleConfig {
    pub fn build<Ctx>(&self, registry: &SpecRegistry<Ctx>) -> Box<dyn Specification<Ctx>> {
        match self {
            RuleConfig::Spec { name, params } => registry.create(name, params),
            RuleConfig::And { rules } => {
                let specs: Vec<_> = rules.iter().map(|r| r.build(registry)).collect();
                Box::new(AllOf(specs))
            }
            // ... other variants
        }
    }
}
```

### Minos Cleanup Rules (Horus)

```json
{
  "cleanup_policies": [
    {
      "name": "stale_testcases",
      "target": "/mnt/data/testcases",
      "rule": {
        "and": [
          { "spec": "LastAccessOlderThan", "params": { "hours": 6 } },
          { "spec": "IsFile" }
        ]
      },
      "action": "delete"
    },
    {
      "name": "orphan_temp_dirs",
      "target": "/mnt/data/temp",
      "rule": {
        "and": [
          { "spec": "CreatedOlderThan", "params": { "hours": 1 } },
          { "spec": "IsDirectory" },
          { "not": { "spec": "HasActiveSubmission" } }
        ]
      },
      "action": "delete"
    }
  ]
}
```

### Admin Dashboard Integration

```rust
// Endpoint to save rule configuration
pub async fn save_rule_config(
    State(app_state): State<AppState>,
    Json(payload): Json<RuleConfigRequest>,
) -> Result<Json<RuleConfigResponse>, AppError> {
    // Validate JSON structure
    let config: RuleConfig = serde_json::from_value(payload.rule.clone())?;
    
    // Store in database
    sqlx::query!(
        "INSERT INTO rule_configs (name, service, config, updated_by) 
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (name, service) DO UPDATE SET config = $3, updated_by = $4",
        payload.name,
        payload.service, // "minos" | "horus" | "vanguard"
        payload.rule,
        ctx.user_id
    )
    .execute(&app_state.db)
    .await?;
    
    // Notify service to reload (via Redis pub/sub)
    app_state.redis.publish("config_reload", payload.service).await?;
    
    Ok(Json(RuleConfigResponse { success: true }))
}
```

## Metrics (Minos)

Export these Prometheus metrics:
- `judge_execution_duration_seconds` (Histogram)
- `judge_memory_usage_bytes` (Gauge)
- `judge_verdict_total{type="AC|WA|TLE|RE"}` (Counter)

## ZIP Submission Format

User submissions must contain:
```
submission.zip
├── compile.sh    # Compilation script
└── run.sh        # Execution script
```

Supported runtimes: `cpp`, `c`, `rust`, `go`, `python`, `zig`

## Problem Definition

Problems require:
- Generator binary (creates test cases)
- Checker/Verifier binary (validates output)
- Problem code (A, B, C, etc.)
- Time/memory limits
- Number of test cases
- Allowed runtimes

## Security Considerations

- Network disabled during compilation (Sisyphus)
- cgroups for RAM/CPU limits (Minos)
- namespaces for network isolation (Minos)
- 30-second compilation timeout
- File size limit: 10MB for submissions

## Testing Guidelines

- Unit test all Specification implementations
- Integration test the full submission flow
- Mock RabbitMQ and storage for handler tests
- Use testcontainers for PostgreSQL tests

## Common Patterns

### Handler Implementation

```rust
pub async fn handler_name(
    State(app_state): State<AppState>,
    Json(payload): Json<RequestDto>,
) -> Result<Json<ResponseDto>, AppError> {
    // Implementation
    Ok(Json(ResponseDto { ... }))
}
```

### Queue Message Payload

```rust
#[derive(Serialize, Deserialize)]
pub struct CompileJob {
    pub submission_id: Uuid,
    pub file_path: String,
}
```

## Do Not

- Expose internal error details to API responses
- Skip authentication middleware on protected endpoints
- Store secrets in code (use environment variables)
- Allow network access in sandboxed execution
- Forget to clean up temp directories after execution
