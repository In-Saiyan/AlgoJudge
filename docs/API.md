# AlgoJudge API Endpoints

Base URL: `/api/v1`

---

## Health Check

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/health/` | Health check (returns service status + uptime) | No |
| GET | `/health/live` | Liveness probe (always OK) | No |
| GET | `/health/ready` | Readiness probe (checks DB + Redis) | No |

---

## Authentication

| Method | Endpoint | Description | Auth | Rate Limit |
|--------|----------|-------------|------|------------|
| POST | `/api/v1/auth/register` | Register new user | No | Register tier |
| POST | `/api/v1/auth/login` | Login and get JWT token | No | Login tier |
| POST | `/api/v1/auth/refresh` | Refresh JWT token | No | — |
| POST | `/api/v1/auth/logout` | Logout (invalidate token) | Yes | — |
| GET | `/api/v1/auth/me` | Get current authenticated user | Yes | — |

---

## Users

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/users` | List all users | No |
| GET | `/api/v1/users/{id}` | Get user by ID | No |
| PUT | `/api/v1/users/{id}` | Update user profile | Yes (Owner) |
| GET | `/api/v1/users/{id}/submissions` | Get user's submissions | Yes |
| GET | `/api/v1/users/{id}/stats` | Get user statistics | No |

---

## Contests

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests` | List all contests | No |
| POST | `/api/v1/contests` | Create new contest | Yes |
| GET | `/api/v1/contests/{id}` | Get contest by ID | No |
| PUT | `/api/v1/contests/{id}` | Update contest | Yes (Owner/Collaborator/Admin) |
| DELETE | `/api/v1/contests/{id}` | Delete contest | Yes (Owner/Admin) |

### Contest Registration

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| POST | `/api/v1/contests/{id}/register` | Register for contest | Yes |
| POST | `/api/v1/contests/{id}/unregister` | Unregister from contest | Yes |
| GET | `/api/v1/contests/{id}/participants` | List contest participants | No |

### Contest Collaborators

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{id}/collaborators` | List contest collaborators | Yes |
| POST | `/api/v1/contests/{id}/collaborators` | Add collaborator to contest | Yes (Owner/Admin) |
| DELETE | `/api/v1/contests/{id}/collaborators/{user_id}` | Remove collaborator | Yes (Owner/Admin) |

### Contest Problems

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{contest_id}/problems` | List contest problems | No |
| POST | `/api/v1/contests/{contest_id}/problems` | Add problem to contest | Yes (Owner/Collaborator/Admin) |
| DELETE | `/api/v1/contests/{contest_id}/problems/{problem_id}` | Remove problem from contest | Yes (Owner/Collaborator/Admin) |

### Contest Leaderboard

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/contests/{contest_id}/leaderboard` | Get contest leaderboard (ICPC-style scoring) | No |

---

## Problems

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/problems` | List all problems | No |
| POST | `/api/v1/problems` | Create new problem (metadata only) | Yes |
| GET | `/api/v1/problems/{id}` | Get problem by ID | No |
| PUT | `/api/v1/problems/{id}` | Update problem metadata | Yes (Owner/Admin) |
| DELETE | `/api/v1/problems/{id}` | Delete problem | Yes (Owner/Admin) |
| POST | `/api/v1/problems/{id}/generator` | Upload generator binary (multipart) | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| POST | `/api/v1/problems/{id}/checker` | Upload checker/verifier binary (multipart) | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| GET | `/api/v1/problems/{id}/generator` | Download generator binary | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| GET | `/api/v1/problems/{id}/checker` | Download checker binary | Yes (Owner/Contest Owner/Collaborator†/Admin) |

> † **Collaborator access**: Users who are collaborators (with `can_add_problems` permission) of any contest that contains this problem can access the generator/checker binaries.

---

## Submissions

| Method | Endpoint | Description | Auth | Rate Limit |
|--------|----------|-------------|------|------------|
| GET | `/api/v1/submissions` | List submissions | Yes | — |
| POST | `/api/v1/submissions` | Create submission (source code; `contest_id` optional) | Yes | Submission tier |
| POST | `/api/v1/submissions/upload` | Upload ZIP submission (multipart; `contest_id` optional) | Yes | Submission tier |
| GET | `/api/v1/submissions/{id}` | Get submission by ID | Yes (Owner/Collaborator/Admin) |
| GET | `/api/v1/submissions/{id}/results` | Get submission test results | Yes (Owner/Collaborator/Admin) |
| GET | `/api/v1/submissions/{id}/source` | Download submission source/ZIP | Yes (Owner/Collaborator/Admin) |

> **Standalone submissions:** Both `POST /api/v1/submissions` and
> `POST /api/v1/submissions/upload` accept submissions without a `contest_id`.
> When omitted, the submission is a standalone practice run against the problem
> without contest rules (time window, allowed languages, participant check).
>
> **`queue_pending` status:** If the problem's generator or checker binary has
> not been uploaded yet when Minos picks up the job, the submission enters
> `queue_pending` status. It will be automatically re-queued for judging once
> both binaries are uploaded via the problem binary upload endpoints.

---

## Admin

All admin endpoints require **Admin** role (double middleware: `auth_middleware` + `admin_middleware`).

### User Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/users` | List all users (filterable by role, is_banned, search) | Yes (Admin) |
| PUT | `/api/v1/admin/users/{id}/role` | Update user role (prevents self-role-change) | Yes (Admin) |
| POST | `/api/v1/admin/users/{id}/ban` | Ban user (also deletes all sessions) | Yes (Admin) |
| POST | `/api/v1/admin/users/{id}/unban` | Unban user | Yes (Admin) |

### System Management

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/stats` | Get system statistics (users, contests, submissions, storage) | Yes (Admin) |

### Submission Queue

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/queue` | Get queue info (`XLEN`, `XINFO GROUPS`, `XPENDING` for compile_queue + run_queue) | Yes (Admin) |
| POST | `/api/v1/admin/queue/{id}/rejudge` | Rejudge a submission (resets status to pending, deletes old results, re-queues to compile_queue) | Yes (Admin) |

### Rule Configuration

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/admin/rules` | List rule configs (filterable by `service`, `enabled`) | Yes (Admin) |
| POST | `/api/v1/admin/rules` | Create or upsert a rule config (validates JSON against `SpecRegistry`) | Yes (Admin) |
| PUT | `/api/v1/admin/rules/{id}` | Update an existing rule config (partial update) | Yes (Admin) |

> Rule configs are stored in the `rule_configs` table and can target any service
> (`vanguard`, `minos`, `horus`). After saving, a Redis pub/sub notification is
> published on the `config_reload` channel so the target service hot-reloads the
> new policy without restarting.

---

## User Roles

| Role | Description |
|------|-------------|
| `admin` | Full system access |
| `organizer` | Can create/manage contests and problems |
| `participant` | Can participate in contests and submit solutions (default) |
| `spectator` | Can view public contests and leaderboards |

---

## Authentication

All authenticated endpoints require a JWT token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

JWT configuration:
- Access token expiration: 15 minutes (default, `JWT_ACCESS_EXPIRATION`)
- Refresh token expiration: 7 days (default, `JWT_REFRESH_EXPIRATION`)

---

## Rate Limiting

All API endpoints are rate limited via Redis `INCR` + `EXPIRE` fixed window counters. On Redis failure, requests pass through (fail-open). Responses include rate limit headers:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests allowed in window |
| `X-RateLimit-Remaining` | Remaining requests in current window |
| `X-RateLimit-Reset` | Unix timestamp when the window resets |
| `Retry-After` | Seconds to wait (only on 429 responses) |

### Rate Limit Tiers (Defaults)

| Action | Limit | Window | Key Pattern |
|--------|-------|--------|-------------|
| Login attempts | 40 | 15 min | `rl:login:{ip}` |
| Registration | 30 | 15 min | `rl:register:{ip}` |
| Submission | 5 | 1 min | `rl:submit:{user_id}` |
| API (authenticated) | 600 | 1 min | `rl:api:{user_id}` |
| API (anonymous) | 100 | 1 min | `rl:api:{ip}` |

---

## File Upload (Multipart)

All file uploads use `multipart/form-data` format instead of base64 encoding for efficiency and streaming support.

### ZIP Submission Upload (`POST /api/v1/submissions/upload`)

**Content-Type:** `multipart/form-data`

**Query Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `contest_id` | UUID | No | Target contest ID (omit for standalone/practice submission) |
| `problem_id` | UUID | Yes | Target problem ID |
| `language` | String | No | Language hint (`cpp`, `c`, `rust`, `go`, `python`, `zig`). Helps Sisyphus select the correct Docker image. If omitted, Sisyphus uses `ubuntu:24.04` as a generic image. |

**Form Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | File | Yes | The submission ZIP file |

**Size Limits:**
- Default: 10MB
- Per-contest configurable via `max_submission_size_mb` (1-100MB)

> **Standalone submissions:** When `contest_id` is omitted the submission is
> treated as a standalone practice run. The problem must exist and the user must
> be authenticated. No contest time-window or participant checks are enforced.
>
> If the problem's generator or checker binary has not been uploaded yet, the
> submission will enter `queue_pending` status after compilation and will be
> automatically re-queued for judging once both binaries are uploaded.

**Example (curl):**
```bash
# Contest submission with language hint
curl -X POST "https://api.algojudge.com/api/v1/submissions/upload?contest_id=...&problem_id=...&language=cpp" \
  -H "Authorization: Bearer <token>" \
  -F "file=@submission.zip"

# Standalone submission (no contest, no language hint — uses generic image)
curl -X POST "https://api.algojudge.com/api/v1/submissions/upload?problem_id=..." \
  -H "Authorization: Bearer <token>" \
  -F "file=@submission.zip"
```

---

### Source Code Submission (`POST /api/v1/submissions`)

**Content-Type:** `application/json`

**Body:**
```json
{
  "contest_id": "...",
  "problem_id": "...",
  "language": "cpp",
  "source_code": "#include <iostream>\n..."
}
```

> `contest_id` is optional. Source code is stored in the DB (`source_code` column).
> Sisyphus auto-generates a compile command based on the language.
>
> **Note:** Source code compilation is currently unimplemented in Sisyphus — only ZIP submissions are fully supported.

---

### Generator Upload (`POST /api/v1/problems/{id}/generator`)

**Content-Type:** `multipart/form-data`

**Form Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | File | Yes | The generator binary (Linux ELF executable) |
| `filename` | String | No | Custom filename (default: `generator`) |

**Size Limits:** 50MB max

**Example (curl):**
```bash
curl -X POST "https://api.algojudge.com/api/v1/problems/{id}/generator" \
  -H "Authorization: Bearer <token>" \
  -F "file=@generator"
```

---

### Checker Upload (`POST /api/v1/problems/{id}/checker`)

**Content-Type:** `multipart/form-data`

**Form Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | File | Yes | The checker/verifier binary (Linux ELF executable) |
| `filename` | String | No | Custom filename (default: `checker`) |

**Size Limits:** 50MB max

**Example (curl):**
```bash
curl -X POST "https://api.algojudge.com/api/v1/problems/{id}/checker" \
  -H "Authorization: Bearer <token>" \
  -F "file=@checker"
```

---

### ZIP Submission Contents (Required Structure)

```
submission.zip
├── compile.sh    # Compilation script (required, executable)
└── run.sh        # Execution script (required, executable)
```

**I/O Convention:** The compiled binary is invoked as `./solution <input_file> <output_file>`.
It must read from the file path given as `argv[1]` and write output to the file path given as `argv[2]`.
Standard stdin/stdout piping is **not** used.

**Example `run.sh`:**
```bash
#!/bin/bash
./solution "$1" "$2"
```

**Validation Rules:**
- Both `compile.sh` and `run.sh` must exist
- No symlinks pointing outside the archive
- No absolute paths
- No path traversal (`..`)
- Total uncompressed size must be < 5x compressed size (zip bomb protection)

Supported runtimes: `cpp`, `c`, `rust`, `go`, `python`, `zig`

---

## Problem Creation (Metadata Only)

**POST `/api/v1/problems`** - Create problem metadata first, then upload binaries separately.

```json
{
  "title": "Sort 4GB File",
  "description": "...",
  "problem_code": "A",
  "time_limit_ms": 60000,
  "memory_limit_kb": 524288,
  "num_test_cases": 5,
  "max_threads": 1,
  "network_allowed": false,
  "allowed_runtimes": ["cpp", "rust", "go"]
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Sort 4GB File",
  "status": "draft",
  "generator_uploaded": false,
  "checker_uploaded": false,
  "message": "Problem created. Upload generator and checker binaries to activate."
}
```

**Workflow:**
1. `POST /api/v1/problems` - Create metadata
2. `POST /api/v1/problems/{id}/generator` - Upload generator binary
3. `POST /api/v1/problems/{id}/checker` - Upload checker binary
4. Problem status changes to `ready` when both binaries are uploaded

> **Per-problem settings:** `max_threads` (default 1, max 64) controls the PID limit
> in the execution sandbox via cgroups (`pids.max = max_threads + 4`).
> `network_allowed` (default false) controls whether network namespace isolation
> is applied via `unshare(CLONE_NEWNET)`. Both can be overridden per-contest
> in `contest_problems`.

---

## Contest Upload Limits

Contest organizers can configure per-contest upload limits and per-problem overrides:

**PUT `/api/v1/contests/{id}`**

```json
{
  "max_submission_size_mb": 25,
  "allowed_languages": ["cpp", "rust", "go"]
}
```

| Setting | Type | Default | Range | Description |
|---------|------|---------|-------|-------------|
| `max_submission_size_mb` | Integer | 10 | 1-100 | Max ZIP file size in MB |

**Contest-problem overrides** (set when adding a problem to a contest):

Per-contest overrides for `time_limit_ms`, `memory_limit_kb`, `max_threads`, and
`network_allowed` can be specified in `contest_problems`. When set, these take
precedence over the problem-level defaults during judging.

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
| 502 | External Service Error |
| 504 | Timeout Error |
