# AlgoJudge - Copilot Instructions

## Project Overview

**AlgoJudge** is a competitive programming judge system that benchmarks algorithmic solution submissions. It provides accurate performance metrics (memory usage, execution time) by running solutions in isolated Docker containers.

---

## Architecture Principles

### 1. Backend Structure
```
/src
├── main.rs                 # Application entry point
├── lib.rs                  # Library exports
├── config.rs               # Configuration management (env vars, settings)
├── constants.rs            # Application-wide constants
├── error.rs                # Custom error types and handling
├── state.rs                # Application state management
│
├── handlers/               # HTTP request handlers (controllers)
│   ├── mod.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── handler.rs
│   │   ├── request.rs
│   │   └── response.rs
│   ├── contests/
│   ├── problems/
│   ├── submissions/
│   ├── users/
│   └── admin/
│
├── services/               # Business logic layer
│   ├── mod.rs
│   ├── auth_service.rs
│   ├── contest_service.rs
│   ├── problem_service.rs
│   ├── submission_service.rs
│   ├── benchmark_service.rs
│   └── user_service.rs
│
├── db/                     # Database layer
│   ├── mod.rs
│   ├── connection.rs
│   ├── migrations/
│   └── repositories/
│       ├── mod.rs
│       ├── user_repo.rs
│       ├── contest_repo.rs
│       ├── problem_repo.rs
│       └── submission_repo.rs
│
├── models/                 # Domain models and database schemas
│   ├── mod.rs
│   ├── user.rs
│   ├── contest.rs
│   ├── problem.rs
│   ├── submission.rs
│   ├── benchmark.rs
│   └── test_case.rs
│
├── middleware/             # HTTP middleware
│   ├── mod.rs
│   ├── auth.rs
│   ├── rate_limit.rs
│   └── logging.rs
│
├── benchmark/              # Benchmark execution engine
│   ├── mod.rs
│   ├── runner.rs
│   ├── container.rs
│   ├── languages/
│   │   ├── mod.rs
│   │   ├── c.rs
│   │   ├── cpp.rs
│   │   ├── rust.rs
│   │   ├── go.rs
│   │   ├── zig.rs
│   │   └── python.rs
│   └── metrics.rs
│
└── utils/                  # Utility functions
    ├── mod.rs
    ├── crypto.rs
    ├── validation.rs
    └── time.rs
```

---

## Coding Standards

### Rust Conventions
- Use `snake_case` for functions, variables, and modules
- Use `PascalCase` for types, traits, and enums
- Use `SCREAMING_SNAKE_CASE` for constants
- Prefer `Result<T, E>` over panics for error handling
- Use `thiserror` for custom error types
- Use `anyhow` for error propagation in application code
- Document public APIs with `///` doc comments

### Handler Pattern
Each handler module follows this structure:
```rust
// handler.rs - HTTP handlers
// request.rs - Request DTOs (deserialize)
// response.rs - Response DTOs (serialize)
// mod.rs - Module exports and router configuration
```

### Database
- Use `sqlx` with PostgreSQL
- All queries should use parameterized statements
- Use migrations for schema changes
- Repository pattern for data access

### Error Handling
- Custom `AppError` type that implements `IntoResponse`
- Map all errors to appropriate HTTP status codes
- Never expose internal errors to clients
- Log all errors with context

---

## Supported Languages

| Language | Compiler/Runtime | Container Image |
|----------|------------------|-----------------|
| C        | GCC 13           | algojudge/c     |
| C++      | G++ 13 (C++20)   | algojudge/cpp   |
| Rust     | rustc 1.75+      | algojudge/rust  |
| Go       | go 1.21+         | algojudge/go    |
| Zig      | zig 0.11+        | algojudge/zig   |
| Python   | Python 3.11+     | algojudge/python|

---

## Benchmarking Rules

### Execution
1. Each solution runs in an isolated Docker container
2. Solutions are compiled (if applicable) before timing begins
3. Each test case runs **multiple iterations** (configurable, default: 5)
4. First run is discarded (warm-up)
5. Metrics collected: wall time, CPU time, peak memory usage
6. Results report: average, median, min, max, and outlier detection

### Resource Limits
- **Time Limit**: Configurable per problem (default: 2s)
- **Memory Limit**: Configurable per problem (default: 256MB)
- **CPU Limit**: 1 core per container
- **Disk Limit**: 10MB for output
- **Network**: Disabled

### Integrity
- Containers are destroyed after each submission
- Input files are mounted read-only
- Output is captured and compared byte-by-byte
- Solutions cannot access other submissions

---

## Contest System

### Roles (Similar to CTFd)
| Role        | Permissions |
|-------------|-------------|
| Admin       | Full system access, manage all contests |
| Organizer   | Create/manage own contests, view all submissions |
| Participant | Join contests, submit solutions, view own results |
| Spectator   | View public contests and leaderboards |

### Contest Modes
1. **ICPC Style**: Penalty time for wrong submissions
2. **Codeforces Style**: Points decay over time
3. **IOI Style**: Partial scoring based on test cases passed
4. **Practice**: No scoring, just benchmarks

### Contest Settings
- Start/End time with timezone support
- Registration period (open/closed/invite-only)
- Visibility (public/private/hidden)
- Allowed languages per contest
- Custom time/memory limits per problem
- Freeze leaderboard before end
- Virtual participation after contest ends

---

## API Design

### Authentication
- JWT-based authentication
- Refresh token rotation
- Session management with Redis
- Rate limiting per user/IP

### Endpoints Convention
```
POST   /api/v1/auth/register
POST   /api/v1/auth/login
POST   /api/v1/auth/refresh
POST   /api/v1/auth/logout

GET    /api/v1/contests
POST   /api/v1/contests
GET    /api/v1/contests/{id}
PUT    /api/v1/contests/{id}
DELETE /api/v1/contests/{id}

GET    /api/v1/contests/{id}/problems
POST   /api/v1/contests/{id}/problems
GET    /api/v1/problems/{id}

POST   /api/v1/submissions
GET    /api/v1/submissions/{id}
GET    /api/v1/submissions/{id}/results

GET    /api/v1/leaderboard/{contest_id}
```

---

## Testing Strategy

### Unit Tests
- Test services with mocked repositories
- Test handlers with mocked services
- Test benchmark logic with sample inputs

### Integration Tests
- Database tests with test containers
- API tests with real HTTP requests
- Benchmark tests with actual containers

### Container Testing
- Run containers sequentially to maintain system integrity
- Clean up containers after each test
- Verify resource limits are enforced

---

## Environment Variables

```bash
# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
RUST_LOG=info

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/algojudge
DATABASE_MAX_CONNECTIONS=20

# Redis
REDIS_URL=redis://localhost:6379

# JWT
JWT_SECRET=your-secret-key
JWT_EXPIRY_HOURS=24
REFRESH_TOKEN_EXPIRY_DAYS=7

# Docker
DOCKER_SOCKET=/var/run/docker.sock
BENCHMARK_NETWORK=algojudge-benchmark

# Storage
SUBMISSIONS_PATH=/data/submissions
TEST_CASES_PATH=/data/test_cases
```

---

## Security Checklist

- [ ] Input validation on all endpoints
- [ ] SQL injection prevention (parameterized queries)
- [ ] XSS prevention (sanitize outputs)
- [ ] CSRF tokens for state-changing operations
- [ ] Rate limiting on authentication endpoints
- [ ] Container isolation (no network, limited resources)
- [ ] Secrets management (never commit secrets)
- [ ] Audit logging for admin actions

---

## Git Workflow

- `main` - Production-ready code
- `develop` - Integration branch
- `feature/*` - New features
- `fix/*` - Bug fixes
- `release/*` - Release preparation

Commit messages follow conventional commits:
```
feat: add contest registration endpoint
fix: correct memory calculation in benchmark
docs: update API documentation
refactor: extract benchmark runner logic
test: add integration tests for submissions
```

---

## Dependencies (Cargo.toml)

Core dependencies to use:
- `axum` - Web framework
- `tokio` - Async runtime
- `sqlx` - Database (PostgreSQL)
- `diesel` - database ORM
- `serde` / `serde_json` - Serialization
- `jsonwebtoken` - JWT handling
- `bcrypt` / `argon2` - Password hashing
- `bollard` - Docker API client
- `redis` - Session/cache
- `tracing` - Logging/observability
- `thiserror` / `anyhow` - Error handling
- `validator` - Input validation
- `chrono` - Date/time handling
- `uuid` - Unique identifiers
- `tower` / `tower-http` - Middleware

---

## Notes for Development

1. Always run `cargo fmt` and `cargo clippy` before committing
2. Keep handlers thin - business logic goes in services
3. Use transactions for multi-step database operations
4. Prefer compile-time SQL checking with `sqlx::query_as::<_, ProblemStats>`
5. Document breaking API changes
6. Write tests for edge cases in benchmarking
7. Monitor container resource usage in production
