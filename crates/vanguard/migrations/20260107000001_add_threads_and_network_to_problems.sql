-- Migration: Add max_threads and network_allowed to problems and contest_problems
-- These allow per-problem configuration of thread limits and network access during execution.

-- Add columns to problems table
ALTER TABLE problems
    ADD COLUMN max_threads INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN network_allowed BOOLEAN NOT NULL DEFAULT false;

-- Add override columns to contest_problems junction table
ALTER TABLE contest_problems
    ADD COLUMN max_threads INTEGER,
    ADD COLUMN network_allowed BOOLEAN;
