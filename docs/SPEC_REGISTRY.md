# SpecRegistry — Dynamic Rule Engine

The `olympus-rules` crate implements the **Specification Pattern** to provide a composable, async rule engine used across all AlgoJudge services. The `SpecRegistry` is the centerpiece that enables dynamic rule construction from JSON configuration, powering the admin dashboard's rule management.

---

## Table of Contents

- [Core Concepts](#core-concepts)
- [Specification Trait](#specification-trait)
- [Combinators](#combinators)
- [Operator Overloading](#operator-overloading)
- [Context Types](#context-types)
- [SpecRegistry](#specregistry)
- [Pre-built Registries](#pre-built-registries)
- [JSON Configuration (RuleConfig)](#json-configuration-ruleconfig)
- [Registered Specifications](#registered-specifications)
- [Usage Examples](#usage-examples)
- [Admin Dashboard Integration](#admin-dashboard-integration)
- [Crate Structure](#crate-structure)

---

## Core Concepts

The Specification Pattern decomposes complex business rules into small, reusable predicates that can be combined with logical operators. Each specification:

1. Implements the async `Specification<Ctx>` trait
2. Evaluates against a typed **context** (`FileContext`, `ExecutionContext`, `AuthContext`)
3. Returns a boolean indicating whether the rule is satisfied
4. Can be composed with `And`, `Or`, `Not` combinators

---

## Specification Trait

```rust
#[async_trait]
pub trait Specification<Ctx>: Send + Sync {
    /// Check if the specification is satisfied by the given context.
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool;

    /// Combine with another using AND logic.
    fn and<S: Specification<Ctx>>(self, other: S) -> And<Self, S>;

    /// Combine with another using OR logic.
    fn or<S: Specification<Ctx>>(self, other: S) -> Or<Self, S>;

    /// Negate this specification.
    fn not(self) -> Not<Self>;
}
```

Built-in implementations:
- `AlwaysTrue` — always returns `true`
- `AlwaysFalse` — always returns `false`
- `BoxedSpec<Ctx>` (`Arc<dyn Specification<Ctx>>`) — type-erased dynamic dispatch
- `AllOf<Ctx>` — all specs in a `Vec<BoxedSpec<Ctx>>` must be satisfied
- `AnyOf<Ctx>` — any spec in a `Vec<BoxedSpec<Ctx>>` must be satisfied

---

## Combinators

### Static Combinators (generic types)

| Combinator | Struct | Behavior |
|------------|--------|----------|
| AND | `And<A, B>` | Both A and B must be satisfied |
| OR | `Or<A, B>` | Either A or B must be satisfied |
| NOT | `Not<A>` | A must NOT be satisfied |

### Dynamic Combinators (boxed specs)

| Combinator | Struct | Behavior |
|------------|--------|----------|
| All Of | `AllOf<Ctx>` | All specs in the vec must be satisfied (short-circuits on first `false`) |
| Any Of | `AnyOf<Ctx>` | Any spec in the vec must be satisfied (short-circuits on first `true`) |

---

## Operator Overloading

The `Spec<S>` wrapper enables Rust operator syntax for combining specifications:

```rust
use olympus_rules::prelude::*;

// Using the Spec wrapper with operators
let rule = Spec(IsValidUser) & Spec(IsParticipant);               // AND
let rule = Spec(IsAdmin) | Spec(IsCollaborator);                  // OR
let rule = !Spec(AlwaysFalse);                                     // NOT

// Complex composition
let can_submit = Spec(IsValidUser)
    & ((Spec(NotRateLimited::submission()) & Spec(IsParticipant))
       | Spec(IsAdmin)
       | Spec(IsCollaborator));
```

Operators map to:
- `&` (`BitAnd`) → `And<A, B>`
- `|` (`BitOr`) → `Or<A, B>`
- `!` (`Not`) → `Not<A>`

---

## Context Types

Each specification evaluates against a specific context type that carries the data needed for evaluation.

### EvalContext

Generic key-value context for arbitrary evaluation:

```rust
pub struct EvalContext {
    pub strings: HashMap<String, String>,
    pub integers: HashMap<String, i64>,
    pub booleans: HashMap<String, bool>,
}
```

Builder pattern: `EvalContext::new().with_string("key", "val").with_int("count", 5)`

### FileContext

File metadata for cleanup rules (used by **Horus**):

```rust
pub struct FileContext {
    pub path: String,
    pub is_file: bool,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub created_at: i64,    // Unix timestamp
    pub modified_at: i64,   // Unix timestamp
    pub accessed_at: i64,   // Unix timestamp
}
```

### ExecutionContext

Execution results for judge verdict determination (used by **Minos**):

```rust
pub struct ExecutionContext {
    pub submission_id: String,
    pub problem_id: String,
    pub test_case_id: String,
    pub exit_code: i32,
    pub time_ms: u64,
    pub memory_kb: u64,
    pub time_limit_ms: u64,
    pub memory_limit_kb: u64,
    pub output_matches: bool,
}
```

### AuthContext (feature: `auth`)

Authorization context for API access control (used by **Vanguard**):

```rust
pub struct AuthContext {
    pub user_id: Uuid,
    pub role: String,           // "admin", "organizer", "participant", "spectator"
    pub is_banned: bool,
    pub db: Arc<sqlx::PgPool>,
    pub redis: Arc<deadpool_redis::Pool>,
    pub contest_id: Option<Uuid>,
    pub problem_id: Option<Uuid>,
    pub submission_id: Option<Uuid>,
}
```

Builder pattern:
```rust
AuthContext::new(user_id, role, is_banned, db, redis)
    .with_contest(contest_id)
    .with_problem(problem_id)
    .with_submission(submission_id)
```

The `db` and `redis` fields allow auth specs to perform async database lookups and Redis queries during evaluation.

---

## SpecRegistry

The `SpecRegistry<Ctx>` maps specification names to factory functions, enabling dynamic rule construction at runtime from JSON configs.

### API

```rust
pub struct SpecRegistry<Ctx> {
    factories: HashMap<String, SpecFactory<Ctx>>,
}

impl<Ctx: Send + Sync + 'static> SpecRegistry<Ctx> {
    /// Create a new empty registry.
    fn new() -> Self;

    /// Register a specification factory.
    /// Factory receives params HashMap and returns Option<BoxedSpec<Ctx>>.
    fn register<F>(&mut self, name: &str, factory: F);

    /// Check if a spec is registered.
    fn contains(&self, name: &str) -> bool;

    /// List all registered spec names.
    fn list(&self) -> Vec<&str>;

    /// Create a spec by name with params.
    fn create(&self, name: &str, params: &HashMap<String, Value>) -> Option<BoxedSpec<Ctx>>;

    /// Build a specification tree from a RuleConfig.
    fn build(&self, config: &RuleConfig) -> Option<BoxedSpec<Ctx>>;

    /// Validate a RuleConfig without building it. Returns list of errors.
    fn validate(&self, config: &RuleConfig) -> Vec<String>;
}
```

### Factory Type

```rust
pub type SpecFactory<Ctx> =
    Arc<dyn Fn(&HashMap<String, Value>) -> Option<BoxedSpec<Ctx>> + Send + Sync>;
```

Each factory receives a `HashMap<String, serde_json::Value>` of parameters and returns `Option<BoxedSpec<Ctx>>`. Returning `None` signals invalid parameters.

### Registering Custom Specs

```rust
let mut registry = SpecRegistry::<FileContext>::new();

// Parameterless spec
registry.register("IsFile", |_| Some(Arc::new(IsFile)));

// Spec with parameters
registry.register("LastAccessOlderThan", |params| {
    let hours = params.get("hours")?.as_u64()?;
    Some(Arc::new(LastAccessOlderThan::new(hours)))
});
```

### Building from Config

```rust
let config = RuleConfig::And {
    rules: vec![
        RuleConfig::Spec {
            name: "LastAccessOlderThan".to_string(),
            params: [("hours".into(), json!(6))].into(),
        },
        RuleConfig::Spec {
            name: "IsDirectory".to_string(),
            params: HashMap::new(),
        },
    ],
};

let spec = registry.build(&config).expect("valid config");
let result = spec.is_satisfied_by(&file_ctx).await;
```

### Validation

```rust
let errors = registry.validate(&config);
if !errors.is_empty() {
    // errors contains: "Unknown specification: FooBar"
    // or: "Invalid parameters for specification 'LastAccessOlderThan': {...}"
    // or: "Empty AND/OR rule list"
}
```

---

## Pre-built Registries

Three pre-configured registries are provided as convenience constructors:

### `file_context_registry()` — Horus Cleanup Rules

| Spec Name | Parameters | Description |
|-----------|-----------|-------------|
| `LastAccessOlderThan` | `hours: u64` | True if last access was more than N hours ago |
| `CreatedOlderThan` | `hours: u64` | True if created more than N hours ago |
| `IsFile` | — | True if path is a file |
| `IsDirectory` | — | True if path is a directory |
| `SizeLargerThan` | `bytes: u64` | True if file size exceeds N bytes |

### `execution_context_registry()` — Minos Verdict Rules

| Spec Name | Parameters | Description |
|-----------|-----------|-------------|
| `WithinTimeLimit` | — | True if `time_ms <= time_limit_ms` |
| `WithinMemoryLimit` | — | True if `memory_kb <= memory_limit_kb` |
| `ExitCodeZero` | — | True if `exit_code == 0` |
| `OutputMatches` | — | True if checker confirmed output correctness |
| `AcceptedVerdict` | — | Composite: time + memory + exit + output all pass |

### `auth_context_registry()` — Vanguard Authorization Rules

Requires the `auth` feature flag.

| Spec Name | Parameters | Description |
|-----------|-----------|-------------|
| `IsValidUser` | — | True if user is not banned |
| `IsAdmin` | — | True if `role == "admin"` |
| `IsOrganizer` | — | True if `role == "admin"` or `role == "organizer"` |
| `IsParticipant` | — | DB lookup: user is a contest participant (requires `contest_id`) |
| `IsCollaborator` | — | DB lookup: user is a contest collaborator (requires `contest_id`) |
| `IsContestOwner` | — | DB lookup: user owns the contest (requires `contest_id`) |
| `CanAddProblems` | — | DB lookup: collaborator with `can_add_problems` permission (requires `contest_id`) |
| `IsProblemOwner` | — | DB lookup: user owns the problem (requires `problem_id`) |
| `CanAccessProblemBinaries` | — | Composite DB lookup: admin OR problem owner OR collaborator of containing contest (requires `problem_id`) |
| `IsSubmissionOwner` | — | DB lookup: user owns the submission (requires `submission_id`) |
| `NotRateLimited` | `action: str`, `limit: u64`, `window_secs: u64` | Redis check: user hasn't exceeded rate limit (fail-open) |
| `NotRateLimited:submission` | — | Pre-configured: 10 submissions per 60s |
| `NotRateLimited:api` | — | Pre-configured: 100 API calls per 60s |

---

## JSON Configuration (RuleConfig)

Rules are serialized as a tagged enum for storage in the `rule_configs` database table and admin API management.

### Schema

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleConfig {
    Spec { name: String, params: HashMap<String, Value> },
    And { rules: Vec<RuleConfig> },
    Or { rules: Vec<RuleConfig> },
    Not { rule: Box<RuleConfig> },
}
```

### JSON Examples

**Simple spec (no params):**
```json
{
  "type": "spec",
  "name": "IsFile",
  "params": {}
}
```

**Spec with parameters:**
```json
{
  "type": "spec",
  "name": "LastAccessOlderThan",
  "params": { "hours": 6 }
}
```

**AND combination:**
```json
{
  "type": "and",
  "rules": [
    { "type": "spec", "name": "IsDirectory", "params": {} },
    { "type": "spec", "name": "LastAccessOlderThan", "params": { "hours": 6 } }
  ]
}
```

**Complex nested rule (submission authorization):**
```json
{
  "type": "and",
  "rules": [
    { "type": "spec", "name": "IsValidUser", "params": {} },
    {
      "type": "or",
      "rules": [
        {
          "type": "and",
          "rules": [
            { "type": "spec", "name": "IsParticipant", "params": {} },
            { "type": "spec", "name": "NotRateLimited", "params": { "action": "submit", "limit": "10", "window_secs": "60" } }
          ]
        },
        { "type": "spec", "name": "IsAdmin", "params": {} },
        { "type": "spec", "name": "IsCollaborator", "params": {} }
      ]
    }
  ]
}
```

### Named Rule Config

For database storage and admin management:

```rust
pub struct NamedRuleConfig {
    pub name: String,
    pub description: Option<String>,
    pub service: String,    // "vanguard", "minos", "horus"
    pub rule: RuleConfig,
    pub version: String,
    pub enabled: bool,
}
```

### Cleanup Policy

Specialized config for Horus cleanup jobs:

```rust
pub struct CleanupPolicy {
    pub name: String,
    pub description: Option<String>,
    pub target_path: String,
    pub rule: RuleConfig,
    pub action: CleanupAction,  // Delete, Archive, Move, Log
    pub enabled: bool,
}
```

---

## Registered Specifications — Detailed Reference

### File Specifications

#### `LastAccessOlderThan`
Compares `FileContext.accessed_at` against current time. Returns `true` if the file was last accessed more than `hours` hours ago.

#### `CreatedOlderThan`
Compares `FileContext.created_at` against current time. Returns `true` if the file was created more than `hours` hours ago.

#### `IsFile` / `IsDirectory`
Simple boolean checks on `FileContext.is_file` and `FileContext.is_directory`.

#### `SizeLargerThan`
Returns `true` if `FileContext.size_bytes > bytes`.

### Execution Specifications

#### `WithinTimeLimit`
Returns `true` if `ExecutionContext.time_ms <= time_limit_ms`.

#### `WithinMemoryLimit`
Returns `true` if `ExecutionContext.memory_kb <= memory_limit_kb`.

#### `ExitCodeZero`
Returns `true` if `ExecutionContext.exit_code == 0`.

#### `OutputMatches`
Returns `true` if `ExecutionContext.output_matches == true` (set by checker evaluation).

#### `AcceptedVerdict`
Composite spec that checks all four conditions: time, memory, exit code, and output. Equivalent to `WithinTimeLimit AND WithinMemoryLimit AND ExitCodeZero AND OutputMatches`.

#### `VerdictDeterminer`
Not a spec, but a utility that checks specs in priority order to determine the verdict string:
1. `!WithinTimeLimit` → `"TLE"`
2. `!WithinMemoryLimit` → `"MLE"`
3. `!ExitCodeZero` → `"RE"`
4. `!OutputMatches` → `"WA"`
5. All pass → `"AC"`

### Auth Specifications

#### `IsValidUser`
Returns `!ctx.is_banned`. No database lookup needed.

#### `IsAdmin`
Returns `ctx.role == "admin"`. No database lookup needed.

#### `IsOrganizer`
Returns `ctx.role == "admin" || ctx.role == "organizer"`. No database lookup needed.

#### `IsParticipant`
Queries `contest_participants` table. Requires `ctx.contest_id` to be set. Returns `false` with a warning log if `contest_id` is `None`.

#### `IsCollaborator`
Queries `contest_collaborators` table. Requires `ctx.contest_id` to be set.

#### `IsContestOwner`
Queries `contests.owner_id` field. Requires `ctx.contest_id` to be set.

#### `CanAddProblems`
Queries `contest_collaborators.can_add_problems` permission. Requires `ctx.contest_id` to be set.

#### `IsProblemOwner`
Queries `problems.owner_id` field. Requires `ctx.problem_id` to be set.

#### `CanAccessProblemBinaries`
Composite check (admin OR problem owner OR collaborator/owner of any contest containing the problem). Requires `ctx.problem_id` to be set.

#### `IsSubmissionOwner`
Queries `submissions.user_id` field. Requires `ctx.submission_id` to be set.

#### `NotRateLimited`
Queries Redis key `rl:{action}:{user_id}` and checks if the count is below the limit. **Fail-open behavior**: returns `true` if Redis is unavailable.

Pre-configured shortcuts:
- `NotRateLimited::submission()` — 10 per 60s
- `NotRateLimited::api_authenticated()` — 100 per 60s

---

## Usage Examples

### Horus: Stale Testcase Cleanup

```rust
use olympus_rules::prelude::*;

// Build rule: directory AND last access > 6 hours ago
let rule = Spec(IsDirectory) & Spec(LastAccessOlderThan::new(6));

let ctx = FileContext {
    path: "/mnt/data/testcases/problem-abc".into(),
    is_file: false,
    is_directory: true,
    size_bytes: 4096,
    created_at: 1709400000,
    modified_at: 1709400000,
    accessed_at: 1709400000, // 6+ hours ago
};

if rule.is_satisfied_by(&ctx).await {
    // Delete the directory
}
```

### Minos: Verdict Determination

```rust
use olympus_rules::prelude::*;

let ctx = ExecutionContext {
    submission_id: "sub-123".into(),
    problem_id: "prob-456".into(),
    test_case_id: "tc-1".into(),
    exit_code: 0,
    time_ms: 450,
    memory_kb: 131072,
    time_limit_ms: 1000,
    memory_limit_kb: 262144,
    output_matches: true,
};

// Using the composite spec
if AcceptedVerdict.is_satisfied_by(&ctx).await {
    println!("Accepted!");
}

// Or get the specific verdict
let verdict = VerdictDeterminer::determine(&ctx).await;
// verdict: "AC", "TLE", "MLE", "RE", or "WA"
```

### Vanguard: Authorization Check

```rust
use olympus_rules::prelude::*;

let auth_ctx = AuthContext::new(user_id, "participant".into(), false, db, redis)
    .with_contest(contest_id);

// Check if user can submit
let is_valid = IsValidUser.is_satisfied_by(&auth_ctx).await;
let is_admin = IsAdmin.is_satisfied_by(&auth_ctx).await;
let is_collaborator = IsCollaborator.is_satisfied_by(&auth_ctx).await;
let is_participant = IsParticipant.is_satisfied_by(&auth_ctx).await;
let not_rate_limited = NotRateLimited::submission().is_satisfied_by(&auth_ctx).await;

// Composite logic: IsValidUser AND (IsAdmin OR IsCollaborator OR (IsParticipant AND NotRateLimited))
if is_valid && (is_admin || is_collaborator || (is_participant && not_rate_limited)) {
    // Allow submission
}
```

### Dynamic Rule Building from JSON

```rust
use olympus_rules::prelude::*;

// Admin creates a rule via the API
let json = r#"{
    "type": "and",
    "rules": [
        { "type": "spec", "name": "IsDirectory", "params": {} },
        { "type": "spec", "name": "CreatedOlderThan", "params": { "hours": 24 } },
        { "type": "not", "rule": { "type": "spec", "name": "SizeLargerThan", "params": { "bytes": 1073741824 } } }
    ]
}"#;

let config: RuleConfig = serde_json::from_str(json).unwrap();
let registry = file_context_registry();

// Validate before building
let errors = registry.validate(&config);
if !errors.is_empty() {
    eprintln!("Invalid config: {:?}", errors);
    return;
}

// Build the executable specification
let spec = registry.build(&config).unwrap();
let matches = spec.is_satisfied_by(&file_ctx).await;
```

---

## Admin Dashboard Integration

The admin API endpoints in Vanguard allow managing rules stored in the `rule_configs` database table:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/admin/rules` | GET | List all rules (filterable by `service`, `enabled`) |
| `/api/v1/admin/rules` | POST | Upsert a rule (validates JSON via `SpecRegistry`) |
| `/api/v1/admin/rules/{id}` | PUT | Partial update of a rule |

After saving, the admin API publishes a notification on the Redis `config_reload` channel. Horus subscribes to this channel and reloads its policies from the database when it receives a `"horus"` payload.

### Database Schema (`rule_configs`)

```sql
CREATE TABLE rule_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    service VARCHAR(50) NOT NULL,       -- "vanguard", "minos", "horus"
    rule_json JSONB NOT NULL,           -- RuleConfig serialized
    version VARCHAR(50) NOT NULL DEFAULT '1.0',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Validation Flow

1. Admin submits a `RuleConfig` JSON via `POST /api/v1/admin/rules`
2. Vanguard deserializes the JSON into `RuleConfig`
3. The appropriate `SpecRegistry` is selected based on `service` field
4. `registry.validate(&config)` checks that all referenced specs exist and have valid parameters
5. If valid, the rule is saved to `rule_configs` and a Redis pub/sub notification is sent

---

## Crate Structure

```
crates/olympus-rules/
├── Cargo.toml              # Optional "auth" feature gates DB/Redis deps
└── src/
    ├── lib.rs              # Crate root, prelude module
    ├── specification.rs    # Core trait, And/Or/Not, AlwaysTrue/False, BoxedSpec, AllOf, AnyOf
    ├── operators.rs        # Spec<S> wrapper for BitAnd(&), BitOr(|), Not(!) overloading
    ├── context.rs          # EvalContext, FileContext, ExecutionContext, AuthContext
    ├── rules.rs            # File specs, execution specs, VerdictDeterminer
    ├── auth_rules.rs       # Auth specs (behind "auth" feature flag)
    ├── config.rs           # RuleConfig, NamedRuleConfig, CleanupPolicy, CleanupAction
    └── registry.rs         # SpecRegistry, file_context_registry, execution_context_registry, auth_context_registry
```

### Feature Flags

| Feature | Description | Dependencies Added |
|---------|-------------|-------------------|
| `auth` | Enables `AuthContext` and all auth specs | `sqlx`, `deadpool-redis`, `redis`, `uuid`, `tracing` |

Without the `auth` feature, the crate has minimal dependencies and can be used purely for file/execution rule evaluation.
