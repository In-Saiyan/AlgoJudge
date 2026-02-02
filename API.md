# AlgoJudge API Endpoints

Base URL: `/api/v1`

---

## Health Check

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/health` | Health check | No |

---

## Authentication

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/v1/auth/register` | Register new user | No |
| POST | `/api/v1/auth/login` | Login and get JWT token | No |
| POST | `/api/v1/auth/refresh` | Refresh JWT token | No |
| POST | `/api/v1/auth/logout` | Logout (invalidate token) | Yes |
| GET | `/api/v1/auth/me` | Get current authenticated user | Yes |

---

## Users

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/users` | List all users | No |
| GET | `/api/v1/users/{id}` | Get user by ID | No |
| PUT | `/api/v1/users/{id}` | Update user profile | Yes (Owner) |
| GET | `/api/v1/users/{id}/submissions` | Get user's submissions | No |
| GET | `/api/v1/users/{id}/stats` | Get user statistics | No |

---

## Contests

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests` | List all contests | Yes |
| POST | `/api/v1/contests` | Create new contest | Yes (Organizer/Admin) |
| GET | `/api/v1/contests/{id}` | Get contest by ID | Yes |
| PUT | `/api/v1/contests/{id}` | Update contest | Yes (Owner/Collaborator/Admin) |
| DELETE | `/api/v1/contests/{id}` | Delete contest | Yes (Owner/Admin) |

### Contest Registration

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/v1/contests/{id}/register` | Register for contest | Yes |
| POST | `/api/v1/contests/{id}/unregister` | Unregister from contest | Yes |
| GET | `/api/v1/contests/{id}/participants` | List contest participants | Yes |

### Contest Collaborators

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{id}/collaborators` | List contest collaborators | Yes (Owner/Collaborator/Admin) |
| POST | `/api/v1/contests/{id}/collaborators` | Add collaborator to contest | Yes (Owner/Admin) |
| DELETE | `/api/v1/contests/{id}/collaborators/{user_id}` | Remove collaborator | Yes (Owner/Admin) |

### Contest Problems

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{id}/problems` | List contest problems | Yes |
| POST | `/api/v1/contests/{id}/problems` | Add problem to contest | Yes (Owner/Collaborator/Admin) |
| DELETE | `/api/v1/contests/{id}/problems/{problem_id}` | Remove problem from contest | Yes (Owner/Collaborator/Admin) |

### Contest Leaderboard & Virtual Participation

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{id}/leaderboard` | Get contest leaderboard | Yes |
| POST | `/api/v1/contests/{id}/virtual` | Start virtual participation | Yes |

---

## Problems

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/problems` | List all problems | Yes |
| POST | `/api/v1/problems` | Create new problem | Yes (Organizer/Admin) |
| GET | `/api/v1/problems/{id}` | Get problem by ID | Yes |
| PUT | `/api/v1/problems/{id}` | Update problem | Yes (Owner/Admin) |
| DELETE | `/api/v1/problems/{id}` | Delete problem | Yes (Owner/Admin) |

### Test Cases (Legacy - for traditional judge)

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/problems/{id}/test-cases` | List test cases | Yes |
| POST | `/api/v1/problems/{id}/test-cases` | Add test case | Yes (Owner/Admin) |
| PUT | `/api/v1/problems/{id}/test-cases/{tc_id}` | Update test case | Yes (Owner/Admin) |
| DELETE | `/api/v1/problems/{id}/test-cases/{tc_id}` | Delete test case | Yes (Owner/Admin) |

---

## Submissions

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/submissions` | List submissions | Yes |
| POST | `/api/v1/submissions` | Create submission (legacy source code) | Yes |
| POST | `/api/v1/submissions/zip` | Create ZIP submission (algorithmic benchmark) | Yes |
| GET | `/api/v1/submissions/{id}` | Get submission by ID | Yes (Owner/Admin) |
| GET | `/api/v1/submissions/{id}/results` | Get submission test results | Yes (Owner/Admin) |
| GET | `/api/v1/submissions/{id}/source` | Get submission source code | Yes (Owner/Admin) |

---

## Admin

All admin endpoints require **Admin** role.

### User Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/users` | List all users (admin view) | Yes (Admin) |
| PUT | `/api/v1/admin/users/{id}/role` | Update user role | Yes (Admin) |
| POST | `/api/v1/admin/users/{id}/ban` | Ban user | Yes (Admin) |
| POST | `/api/v1/admin/users/{id}/unban` | Unban user | Yes (Admin) |

### System Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/stats` | Get system statistics | Yes (Admin) |
| GET | `/api/v1/admin/containers` | List running benchmark containers | Yes (Admin) |
| DELETE | `/api/v1/admin/containers/{id}` | Stop/remove container | Yes (Admin) |

### Submission Queue

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/queue` | Get pending submission queue | Yes (Admin) |
| POST | `/api/v1/admin/queue/{id}/rejudge` | Rejudge a submission | Yes (Admin) |

---

## User Roles

| Role | Description |
|------|-------------|
| `admin` | Full system access |
| `organizer` | Can create/manage contests and problems |
| `participant` | Can participate in contests and submit solutions |
| `spectator` | Can view public contests and leaderboards |

---

## Authentication

All authenticated endpoints require a JWT token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

---

## Rate Limiting

All API endpoints are rate limited. Responses include rate limit headers:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests allowed in window |
| `X-RateLimit-Remaining` | Remaining requests in current window |
| `X-RateLimit-Reset` | Unix timestamp when the window resets |
| `Retry-After` | Seconds to wait (only on 429 responses) |

### Rate Limit Tiers

| Action | Limit | Window |
|--------|-------|--------|
| Login attempts | 5 | 15 minutes |
| Registration | 3 | 1 hour |
| Submission | 10 | 1 minute |
| API (authenticated) | 100 | 1 minute |
| API (anonymous) | 20 | 1 minute |

---

## ZIP Submission Format

For algorithmic benchmarking submissions (`POST /api/v1/submissions/zip`):

```json
{
  "problem_id": "uuid",
  "contest_id": "uuid (optional)",
  "runtime": "cpp|c|rust|go|python|zig",
  "submission_zip_base64": "base64_encoded_zip",
  "custom_generator_base64": "base64_encoded_binary (optional)",
  "custom_generator_filename": "string (optional)"
}
```

### ZIP Contents (Required)

```
submission.zip
├── compile.sh    # Compilation script
└── run.sh        # Execution script
```

The compiled binary must be named after the problem code (e.g., `A`, `B`, `C`).

---

## Problem Creation (Algorithmic Benchmarking)

```json
{
  "title": "Sort 4GB File",
  "description": "...",
  "problem_code": "A",
  "time_limit_ms": 60000,
  "memory_limit_kb": 524288,
  "num_test_cases": 5,
  "allowed_runtimes": ["cpp", "rust", "go"],
  "generator_base64": "base64_encoded_generator_binary",
  "generator_filename": "generator",
  "verifier_base64": "base64_encoded_verifier_binary", 
  "verifier_filename": "checker"
}
```

---

## Response Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 202 | Accepted (submission queued) |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 409 | Conflict |
| 422 | Unprocessable Entity |
| 429 | Too Many Requests |
| 500 | Internal Server Error |

