-- Migration: Support standalone submissions and queue_pending status
-- 
-- 1. Make contest_id nullable (allow submissions outside a contest)
-- 2. Add 'queue_pending' status (waiting for problem generator/checker binaries)

-- =============================================================================
-- 1. Make contest_id nullable in submissions table
-- =============================================================================

-- Drop the existing NOT NULL constraint by recreating the column constraint
-- We need to drop the FK first, alter, then re-add FK
ALTER TABLE submissions ALTER COLUMN contest_id DROP NOT NULL;

-- Update the foreign key to allow NULL (CASCADE behavior preserved)
-- The existing FK already handles this since NULL FK columns are simply ignored
-- No change needed for the FK itself.

-- =============================================================================
-- 2. Add 'queue_pending' status to the CHECK constraint
-- =============================================================================

-- Drop the existing CHECK constraint on status and recreate with new value
ALTER TABLE submissions DROP CONSTRAINT IF EXISTS submissions_status_check;

ALTER TABLE submissions ADD CONSTRAINT submissions_status_check CHECK (status IN (
    'pending',           -- Waiting in queue
    'compiling',         -- Being compiled by Sisyphus
    'compiled',          -- Compilation successful, waiting for judge
    'queue_pending',     -- Compiled, but waiting for problem binaries (generator/checker)
    'judging',           -- Being judged by Minos
    'accepted',          -- All test cases passed
    'wrong_answer',      -- Output mismatch
    'time_limit',        -- Exceeded time limit
    'memory_limit',      -- Exceeded memory limit
    'runtime_error',     -- Runtime crash/error
    'compilation_error', -- Failed to compile
    'system_error'       -- Internal system error
));

-- =============================================================================
-- 3. Index for efficiently finding queue_pending submissions by problem
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_submissions_queue_pending 
ON submissions(problem_id, status) 
WHERE status = 'queue_pending';

-- =============================================================================
-- 4. Update storage path comment (standalone submissions use NULL contest_id)
-- =============================================================================

COMMENT ON COLUMN submissions.contest_id IS 'Contest this submission belongs to. NULL for standalone (practice) submissions.';
