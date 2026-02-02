-- Migration: Create contests and problems tables
-- Phase 2.1: Database schema for contests, problems, and related tables

-- Contests table
CREATE TABLE IF NOT EXISTS contests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    short_description VARCHAR(500),
    
    -- Contest timing
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    freeze_time TIMESTAMPTZ,  -- When to freeze the leaderboard (optional)
    
    -- Contest settings
    scoring_type VARCHAR(20) NOT NULL DEFAULT 'icpc' CHECK (scoring_type IN ('icpc', 'ioi', 'custom')),
    is_public BOOLEAN NOT NULL DEFAULT true,
    is_rated BOOLEAN NOT NULL DEFAULT false,
    registration_required BOOLEAN NOT NULL DEFAULT true,
    max_participants INTEGER,  -- NULL means unlimited
    
    -- Upload limits (configurable per contest)
    max_submission_size_mb INTEGER DEFAULT 10 CHECK (max_submission_size_mb >= 1 AND max_submission_size_mb <= 100),
    
    -- Allowed programming languages (NULL means all)
    allowed_languages TEXT[],
    
    -- Owner and timestamps
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_contest_times CHECK (end_time > start_time),
    CONSTRAINT valid_freeze_time CHECK (freeze_time IS NULL OR (freeze_time >= start_time AND freeze_time <= end_time))
);

-- Contest participants (registered users)
CREATE TABLE IF NOT EXISTS contest_participants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Registration info
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Participation status
    status VARCHAR(20) NOT NULL DEFAULT 'registered' CHECK (status IN ('registered', 'participating', 'finished', 'disqualified')),
    
    -- Score tracking (denormalized for performance)
    total_score INTEGER NOT NULL DEFAULT 0,
    total_penalty INTEGER NOT NULL DEFAULT 0,  -- For ICPC-style scoring
    problems_solved INTEGER NOT NULL DEFAULT 0,
    last_submission_at TIMESTAMPTZ,
    
    -- Unique constraint: one registration per user per contest
    UNIQUE (contest_id, user_id)
);

-- Contest collaborators (problem setters, testers)
CREATE TABLE IF NOT EXISTS contest_collaborators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Collaborator role
    role VARCHAR(20) NOT NULL DEFAULT 'tester' CHECK (role IN ('co-owner', 'problem-setter', 'tester')),
    
    -- Permissions
    can_edit_contest BOOLEAN NOT NULL DEFAULT false,
    can_add_problems BOOLEAN NOT NULL DEFAULT false,
    can_view_submissions BOOLEAN NOT NULL DEFAULT true,
    
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    added_by UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Unique constraint: one role per user per contest
    UNIQUE (contest_id, user_id)
);

-- Problems table
CREATE TABLE IF NOT EXISTS problems (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Problem metadata
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    input_format TEXT,
    output_format TEXT,
    constraints TEXT,
    
    -- Sample test cases (shown to users)
    sample_input TEXT,
    sample_output TEXT,
    sample_explanation TEXT,
    
    -- Difficulty and tags
    difficulty VARCHAR(20) CHECK (difficulty IN ('easy', 'medium', 'hard', 'expert')),
    tags TEXT[],
    
    -- Execution limits
    time_limit_ms INTEGER NOT NULL DEFAULT 1000,
    memory_limit_kb INTEGER NOT NULL DEFAULT 262144,  -- 256 MB
    
    -- Test generation
    num_test_cases INTEGER NOT NULL DEFAULT 10,
    generator_path VARCHAR(512),  -- Path to generator binary
    checker_path VARCHAR(512),    -- Path to checker/verifier binary
    
    -- Scoring
    max_score INTEGER NOT NULL DEFAULT 100,
    partial_scoring BOOLEAN NOT NULL DEFAULT false,  -- IOI-style partial points
    
    -- Visibility
    is_public BOOLEAN NOT NULL DEFAULT false,
    
    -- Allowed languages (NULL means inherit from contest or allow all)
    allowed_languages TEXT[],
    
    -- Owner and timestamps
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Junction table: Contest-Problem relationship
CREATE TABLE IF NOT EXISTS contest_problems (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    
    -- Problem ordering within contest
    problem_code VARCHAR(10) NOT NULL,  -- e.g., 'A', 'B', 'C' or '1', '2', '3'
    sort_order INTEGER NOT NULL DEFAULT 0,
    
    -- Contest-specific overrides
    max_score INTEGER,  -- Override problem's default score
    time_limit_ms INTEGER,  -- Override problem's time limit
    memory_limit_kb INTEGER,  -- Override problem's memory limit
    
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    added_by UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Unique constraints
    UNIQUE (contest_id, problem_id),
    UNIQUE (contest_id, problem_code)
);

-- Test cases table (legacy/manual test cases)
CREATE TABLE IF NOT EXISTS test_cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    
    -- Test case number (1-indexed)
    case_number INTEGER NOT NULL,
    
    -- Input/output (for manually created test cases)
    input TEXT,
    expected_output TEXT,
    
    -- Or file paths (for large test cases)
    input_path VARCHAR(512),
    output_path VARCHAR(512),
    
    -- Scoring weight (for partial scoring)
    score_weight INTEGER NOT NULL DEFAULT 1,
    
    -- Metadata
    is_sample BOOLEAN NOT NULL DEFAULT false,
    description VARCHAR(255),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique constraint: one case number per problem
    UNIQUE (problem_id, case_number)
);

-- Indexes for performance
CREATE INDEX idx_contests_owner ON contests(owner_id);
CREATE INDEX idx_contests_start_time ON contests(start_time);
CREATE INDEX idx_contests_end_time ON contests(end_time);
CREATE INDEX idx_contests_public ON contests(is_public) WHERE is_public = true;

CREATE INDEX idx_contest_participants_contest ON contest_participants(contest_id);
CREATE INDEX idx_contest_participants_user ON contest_participants(user_id);
CREATE INDEX idx_contest_participants_score ON contest_participants(contest_id, total_score DESC, total_penalty ASC);

CREATE INDEX idx_contest_collaborators_contest ON contest_collaborators(contest_id);
CREATE INDEX idx_contest_collaborators_user ON contest_collaborators(user_id);

CREATE INDEX idx_problems_owner ON problems(owner_id);
CREATE INDEX idx_problems_public ON problems(is_public) WHERE is_public = true;
CREATE INDEX idx_problems_difficulty ON problems(difficulty);
CREATE INDEX idx_problems_tags ON problems USING GIN(tags);

CREATE INDEX idx_contest_problems_contest ON contest_problems(contest_id);
CREATE INDEX idx_contest_problems_problem ON contest_problems(problem_id);
CREATE INDEX idx_contest_problems_order ON contest_problems(contest_id, sort_order);

CREATE INDEX idx_test_cases_problem ON test_cases(problem_id);
CREATE INDEX idx_test_cases_sample ON test_cases(problem_id, is_sample) WHERE is_sample = true;

-- Triggers for updated_at
CREATE TRIGGER update_contests_updated_at
    BEFORE UPDATE ON contests
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_problems_updated_at
    BEFORE UPDATE ON problems
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
