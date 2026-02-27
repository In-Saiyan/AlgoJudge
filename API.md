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
| POST | `/api/v1/problems` | Create new problem (metadata only) | Yes (Organizer/Admin) |
| GET | `/api/v1/problems/{id}` | Get problem by ID | Yes |
| PUT | `/api/v1/problems/{id}` | Update problem metadata | Yes (Owner/Admin) |
| DELETE | `/api/v1/problems/{id}` | Delete problem | Yes (Owner/Admin) |
| POST | `/api/v1/problems/{id}/generator` | Upload generator binary (multipart) | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| POST | `/api/v1/problems/{id}/checker` | Upload checker/verifier binary (multipart) | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| GET | `/api/v1/problems/{id}/generator` | Download generator binary | Yes (Owner/Contest Owner/Collaborator†/Admin) |
| GET | `/api/v1/problems/{id}/checker` | Download checker binary | Yes (Owner/Contest Owner/Collaborator†/Admin) |

> † **Collaborator access**: Users who are collaborators (with `can_add_problems` permission) of any contest that contains this problem can access the generator/checker binaries.

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
| POST | `/api/v1/submissions` | Create submission (source code; `contest_id` optional) | Yes |
| POST | `/api/v1/submissions/upload` | Upload ZIP submission (multipart; `contest_id` optional) | Yes |
| GET | `/api/v1/submissions/{id}` | Get submission by ID | Yes (Owner/Admin) |
| GET | `/api/v1/submissions/{id}/results` | Get submission test results | Yes (Owner/Admin) |
| GET | `/api/v1/submissions/{id}/source` | Download submission source/ZIP | Yes (Owner/Admin) |
| GET | `/api/v1/submissions/{id}/logs` | Get compilation/runtime logs | Yes (Owner/Admin) |

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

## File Upload (Multipart)

All file uploads use `multipart/form-data` format instead of base64 encoding for efficiency and streaming support.

### Submission Upload (`POST /api/v1/submissions/upload`)

**Content-Type:** `multipart/form-data`

**Query Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `contest_id` | UUID | No | Target contest ID (omit for standalone/practice submission) |
| `problem_id` | UUID | Yes | Target problem ID |

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
# Contest submission
curl -X POST "https://api.algojudge.com/api/v1/submissions/upload?contest_id=...&problem_id=..." \
  -H "Authorization: Bearer <token>" \
  -F "file=@submission.zip"

# Standalone submission (no contest)
curl -X POST "https://api.algojudge.com/api/v1/submissions/upload?problem_id=..." \
  -H "Authorization: Bearer <token>" \
  -F "file=@submission.zip"
```

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

**Validation Rules:**
- Both `compile.sh` and `run.sh` must exist
- No symlinks pointing outside the archive
- No absolute paths
- Total uncompressed size must be < 5x compressed size (zip bomb protection)
- The compiled binary must be named after the problem code (e.g., `A`, `B`, `C`)

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

---

## Contest Upload Limits

Contest organizers can configure per-contest upload limits:

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

