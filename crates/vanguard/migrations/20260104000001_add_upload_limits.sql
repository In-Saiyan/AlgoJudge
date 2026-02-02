-- Migration: Add configurable upload limits and file size tracking
-- Adds max_submission_size_mb to contests and file_size_bytes to submissions

-- Add configurable submission size limit per contest (default 10MB, max 100MB)
ALTER TABLE contests 
ADD COLUMN IF NOT EXISTS max_submission_size_mb INTEGER DEFAULT 10 
CHECK (max_submission_size_mb >= 1 AND max_submission_size_mb <= 100);

-- Add file size tracking to submissions (BIGINT for files up to 100MB+)
-- First drop the old column if it exists and add new one with correct type
DO $$ 
BEGIN
    -- Check if file_size exists and is INTEGER, then migrate to BIGINT
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'submissions' 
        AND column_name = 'file_size'
        AND data_type = 'integer'
    ) THEN
        -- Rename old column
        ALTER TABLE submissions RENAME COLUMN file_size TO file_size_old;
        -- Add new BIGINT column
        ALTER TABLE submissions ADD COLUMN file_size_bytes BIGINT;
        -- Copy data
        UPDATE submissions SET file_size_bytes = file_size_old;
        -- Drop old column
        ALTER TABLE submissions DROP COLUMN file_size_old;
    ELSIF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'submissions' 
        AND column_name = 'file_size_bytes'
    ) THEN
        -- Column doesn't exist at all, add it
        ALTER TABLE submissions ADD COLUMN file_size_bytes BIGINT;
    END IF;
END $$;

-- Add index for faster queries on large submissions
CREATE INDEX IF NOT EXISTS idx_submissions_file_size ON submissions(file_size_bytes) 
WHERE file_size_bytes IS NOT NULL;

COMMENT ON COLUMN contests.max_submission_size_mb IS 'Maximum submission ZIP file size in MB (1-100, default 10)';
COMMENT ON COLUMN submissions.file_size_bytes IS 'Size of uploaded submission file in bytes';
