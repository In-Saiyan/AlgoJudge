-- Migration: Create submissions tables
-- Phase 3.1: Database schema for submissions and results

-- Submissions table
CREATE TABLE IF NOT EXISTS submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- References
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Submission type: 'source' (legacy single file) or 'zip' (algorithmic benchmark)
    submission_type VARCHAR(10) NOT NULL DEFAULT 'source' CHECK (submission_type IN ('source', 'zip')),
    
    -- For source submissions
    language VARCHAR(20),  -- cpp, c, rust, go, python, zig
    source_code TEXT,
    
    -- For zip submissions
    file_path VARCHAR(512),     -- /mnt/data/submissions/{contest_id}/{user_id}/{submission_id}.zip
    file_size_bytes BIGINT,     -- Size in bytes (BIGINT for files up to 100MB+)
    
    -- Status tracking
    status VARCHAR(30) NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending',           -- Waiting in queue
        'compiling',         -- Being compiled by Sisyphus
        'compiled',          -- Compilation successful, waiting for judge
        'judging',           -- Being judged by Minos
        'accepted',          -- All test cases passed
        'wrong_answer',      -- Output mismatch
        'time_limit',        -- Exceeded time limit
        'memory_limit',      -- Exceeded memory limit
        'runtime_error',     -- Runtime crash/error
        'compilation_error', -- Failed to compile
        'system_error'       -- Internal system error
    )),
    
    -- Results (populated after judging)
    score INTEGER,                    -- Points earned
    total_test_cases INTEGER,         -- Total number of test cases
    passed_test_cases INTEGER,        -- Number passed
    max_time_ms INTEGER,              -- Maximum time used across test cases
    max_memory_kb INTEGER,            -- Maximum memory used
    compilation_log TEXT,             -- Compilation output (for errors)
    
    -- Timestamps
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    compiled_at TIMESTAMPTZ,
    judged_at TIMESTAMPTZ,
    
    -- Metadata
    ip_address INET,
    user_agent VARCHAR(512)
);

-- Submission results (per test case)
CREATE TABLE IF NOT EXISTS submission_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    submission_id UUID NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    
    -- Test case info
    test_case_number INTEGER NOT NULL,
    
    -- Verdict for this test case
    verdict VARCHAR(20) NOT NULL CHECK (verdict IN (
        'accepted',
        'wrong_answer',
        'time_limit',
        'memory_limit',
        'runtime_error',
        'system_error'
    )),
    
    -- Performance metrics
    time_ms INTEGER,
    memory_kb INTEGER,
    
    -- Output info (optional, for debugging)
    expected_output_hash VARCHAR(64),  -- SHA256 of expected output
    actual_output_hash VARCHAR(64),    -- SHA256 of actual output
    
    -- Checker output (if custom checker used)
    checker_output TEXT,
    checker_score DECIMAL(5,2),  -- For partial scoring
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique constraint
    UNIQUE (submission_id, test_case_number)
);

-- Indexes for performance
CREATE INDEX idx_submissions_contest ON submissions(contest_id);
CREATE INDEX idx_submissions_problem ON submissions(problem_id);
CREATE INDEX idx_submissions_user ON submissions(user_id);
CREATE INDEX idx_submissions_status ON submissions(status);
CREATE INDEX idx_submissions_submitted_at ON submissions(submitted_at DESC);
CREATE INDEX idx_submissions_contest_user ON submissions(contest_id, user_id);
CREATE INDEX idx_submissions_contest_problem ON submissions(contest_id, problem_id);

-- For leaderboard queries
CREATE INDEX idx_submissions_leaderboard ON submissions(contest_id, user_id, problem_id, status, score DESC, submitted_at);

CREATE INDEX idx_submission_results_submission ON submission_results(submission_id);
CREATE INDEX idx_submission_results_verdict ON submission_results(verdict);
