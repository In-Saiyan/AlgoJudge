-- AlgoJudge Schema Redesign for Algorithmic Benchmarking
-- Migration: Add collaborators, generators, verifiers, and ZIP-based submissions

-- ============================================================================
-- CONTEST COLLABORATORS
-- Organizers can add users to help manage their contests
-- ============================================================================

CREATE TABLE contest_collaborators (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- 'editor' can add/modify problems, 'viewer' can only view submissions
    role VARCHAR(20) NOT NULL DEFAULT 'editor',
    added_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(contest_id, user_id),
    CONSTRAINT valid_collaborator_role CHECK (role IN ('editor', 'viewer'))
);

CREATE INDEX idx_contest_collaborators_contest ON contest_collaborators(contest_id);
CREATE INDEX idx_contest_collaborators_user ON contest_collaborators(user_id);

-- ============================================================================
-- PROBLEM REDESIGN
-- Problems now use generators and verifiers instead of static test cases
-- ============================================================================

-- Add new columns to problems for generator/verifier approach
ALTER TABLE problems ADD COLUMN IF NOT EXISTS problem_code VARCHAR(10);
ALTER TABLE problems ADD COLUMN IF NOT EXISTS generator_binary BYTEA;
ALTER TABLE problems ADD COLUMN IF NOT EXISTS generator_filename VARCHAR(255);
ALTER TABLE problems ADD COLUMN IF NOT EXISTS verifier_binary BYTEA;
ALTER TABLE problems ADD COLUMN IF NOT EXISTS verifier_filename VARCHAR(255);
ALTER TABLE problems ADD COLUMN IF NOT EXISTS num_test_cases INTEGER NOT NULL DEFAULT 5;
ALTER TABLE problems ADD COLUMN IF NOT EXISTS allowed_runtimes VARCHAR(50)[] NOT NULL DEFAULT ARRAY['cpp', 'c', 'rust', 'go', 'python', 'zig'];

-- Problem code is like A, B, C for contest problems
CREATE UNIQUE INDEX IF NOT EXISTS idx_problems_code ON problems(problem_code) WHERE problem_code IS NOT NULL;

-- ============================================================================
-- RUNTIME ENVIRONMENTS
-- Define available container runtimes for submissions
-- ============================================================================

CREATE TABLE IF NOT EXISTS runtimes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(50) NOT NULL UNIQUE,
    display_name VARCHAR(100) NOT NULL,
    docker_image VARCHAR(255) NOT NULL,
    -- Default compile command template (can use {source} and {output} placeholders)
    default_compile_cmd TEXT,
    -- Default run command template
    default_run_cmd TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default runtimes
INSERT INTO runtimes (name, display_name, docker_image, default_compile_cmd, default_run_cmd) VALUES
    ('cpp', 'C++ (G++ 13, C++20)', 'algojudge/cpp', 'g++ -std=c++20 -O2 -o {output} {source}', './{output}'),
    ('c', 'C (GCC 13)', 'algojudge/c', 'gcc -std=c17 -O2 -o {output} {source}', './{output}'),
    ('rust', 'Rust (1.75+)', 'algojudge/rust', 'rustc -O -o {output} {source}', './{output}'),
    ('go', 'Go (1.21+)', 'algojudge/go', 'go build -o {output} {source}', './{output}'),
    ('python', 'Python (3.11+)', 'algojudge/python', NULL, 'python3 {source}'),
    ('zig', 'Zig (0.11+)', 'algojudge/zig', 'zig build-exe -O ReleaseFast -o {output} {source}', './{output}')
ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- SUBMISSIONS REDESIGN
-- Users submit ZIP files with compile.sh and run.sh
-- ============================================================================

-- Modify submissions table for ZIP-based submissions
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS submission_zip BYTEA;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS runtime_id UUID REFERENCES runtimes(id);
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS custom_generator_binary BYTEA;
ALTER TABLE submissions ADD COLUMN IF NOT EXISTS custom_generator_filename VARCHAR(255);

-- Update verdict enum to include new statuses
-- Valid verdicts: pending, compiling, running, accepted, wrong_answer, 
--                 time_limit_exceeded, memory_limit_exceeded, runtime_error,
--                 compilation_error, invalid_format, system_error, partial

-- ============================================================================
-- TEST CASE RESULTS REDESIGN
-- Results now include match percentage from verifier
-- ============================================================================

ALTER TABLE test_case_results ADD COLUMN IF NOT EXISTS test_case_number INTEGER;
ALTER TABLE test_case_results ADD COLUMN IF NOT EXISTS match_percentage DOUBLE PRECISION;
ALTER TABLE test_case_results ADD COLUMN IF NOT EXISTS verifier_output TEXT;

-- Make test_case_id nullable since we use generated test cases
ALTER TABLE test_case_results ALTER COLUMN test_case_id DROP NOT NULL;

-- Drop the unique constraint that required test_case_id
ALTER TABLE test_case_results DROP CONSTRAINT IF EXISTS test_case_results_submission_id_test_case_id_key;

-- Add new unique constraint on submission_id and test_case_number
ALTER TABLE test_case_results ADD CONSTRAINT test_case_results_submission_test_number_key 
    UNIQUE(submission_id, test_case_number);

-- ============================================================================
-- BENCHMARK RESULTS UPDATE
-- Update for new test case approach
-- ============================================================================

ALTER TABLE benchmark_results ADD COLUMN IF NOT EXISTS test_case_number INTEGER;
ALTER TABLE benchmark_results ALTER COLUMN test_case_id DROP NOT NULL;

-- ============================================================================
-- VIEWS FOR COMMON QUERIES
-- ============================================================================

-- View for checking contest permissions (owner or collaborator)
CREATE OR REPLACE VIEW contest_permissions AS
SELECT 
    c.id as contest_id,
    c.organizer_id as user_id,
    'owner' as permission_type
FROM contests c
UNION ALL
SELECT 
    cc.contest_id,
    cc.user_id,
    cc.role as permission_type
FROM contest_collaborators cc;

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

-- Function to check if user can modify a contest
CREATE OR REPLACE FUNCTION can_modify_contest(p_user_id UUID, p_contest_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM contests WHERE id = p_contest_id AND organizer_id = p_user_id
    ) OR EXISTS (
        SELECT 1 FROM contest_collaborators 
        WHERE contest_id = p_contest_id AND user_id = p_user_id AND role = 'editor'
    ) OR EXISTS (
        SELECT 1 FROM users WHERE id = p_user_id AND role = 'admin'
    );
END;
$$ LANGUAGE plpgsql;

-- Function to check if user can view contest submissions
CREATE OR REPLACE FUNCTION can_view_contest_submissions(p_user_id UUID, p_contest_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM contests WHERE id = p_contest_id AND organizer_id = p_user_id
    ) OR EXISTS (
        SELECT 1 FROM contest_collaborators 
        WHERE contest_id = p_contest_id AND user_id = p_user_id
    ) OR EXISTS (
        SELECT 1 FROM users WHERE id = p_user_id AND role = 'admin'
    );
END;
$$ LANGUAGE plpgsql;
