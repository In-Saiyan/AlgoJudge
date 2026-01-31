-- AlgoJudge Database Schema
-- Initial migration: Create all tables

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(32) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(64),
    role VARCHAR(20) NOT NULL DEFAULT 'participant',
    is_banned BOOLEAN NOT NULL DEFAULT FALSE,
    ban_reason TEXT,
    ban_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_role ON users(role);

-- Contests table
CREATE TABLE contests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    organizer_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    scoring_mode VARCHAR(20) NOT NULL DEFAULT 'icpc',
    visibility VARCHAR(20) NOT NULL DEFAULT 'public',
    registration_mode VARCHAR(20) NOT NULL DEFAULT 'open',
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    registration_start TIMESTAMPTZ,
    registration_end TIMESTAMPTZ,
    allowed_languages VARCHAR(20)[] NOT NULL DEFAULT '{}',
    freeze_time_minutes INTEGER,
    allow_virtual BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_contest_times CHECK (end_time > start_time),
    CONSTRAINT valid_registration_times CHECK (
        registration_start IS NULL OR registration_end IS NULL 
        OR registration_end > registration_start
    )
);

CREATE INDEX idx_contests_organizer ON contests(organizer_id);
CREATE INDEX idx_contests_start_time ON contests(start_time);
CREATE INDEX idx_contests_visibility ON contests(visibility);

-- Problems table
CREATE TABLE problems (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    input_format TEXT,
    output_format TEXT,
    constraints TEXT,
    samples JSONB,
    notes TEXT,
    time_limit_ms INTEGER NOT NULL DEFAULT 2000,
    memory_limit_kb INTEGER NOT NULL DEFAULT 262144,
    difficulty VARCHAR(20),
    tags VARCHAR(50)[] NOT NULL DEFAULT '{}',
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_problems_author ON problems(author_id);
CREATE INDEX idx_problems_is_public ON problems(is_public);
CREATE INDEX idx_problems_difficulty ON problems(difficulty);
CREATE INDEX idx_problems_tags ON problems USING GIN(tags);

-- Test cases table
CREATE TABLE test_cases (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    "order" INTEGER NOT NULL DEFAULT 1,
    input TEXT NOT NULL,
    expected_output TEXT NOT NULL,
    is_sample BOOLEAN NOT NULL DEFAULT FALSE,
    points INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(problem_id, "order")
);

CREATE INDEX idx_test_cases_problem ON test_cases(problem_id);

-- Contest problems (many-to-many)
CREATE TABLE contest_problems (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE RESTRICT,
    "order" INTEGER NOT NULL DEFAULT 1,
    time_limit_ms INTEGER,
    memory_limit_kb INTEGER,
    points INTEGER,
    
    UNIQUE(contest_id, problem_id),
    UNIQUE(contest_id, "order")
);

CREATE INDEX idx_contest_problems_contest ON contest_problems(contest_id);
CREATE INDEX idx_contest_problems_problem ON contest_problems(problem_id);

-- Contest participants
CREATE TABLE contest_participants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_virtual BOOLEAN NOT NULL DEFAULT FALSE,
    virtual_start TIMESTAMPTZ,
    
    UNIQUE(contest_id, user_id)
);

CREATE INDEX idx_contest_participants_contest ON contest_participants(contest_id);
CREATE INDEX idx_contest_participants_user ON contest_participants(user_id);

-- Submissions table
CREATE TABLE submissions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    problem_id UUID NOT NULL REFERENCES problems(id) ON DELETE RESTRICT,
    contest_id UUID REFERENCES contests(id) ON DELETE SET NULL,
    language VARCHAR(20) NOT NULL,
    source_code TEXT NOT NULL,
    verdict VARCHAR(30) NOT NULL DEFAULT 'pending',
    execution_time_ms INTEGER,
    memory_usage_kb INTEGER,
    score INTEGER,
    compilation_output TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    judged_at TIMESTAMPTZ
);

CREATE INDEX idx_submissions_user ON submissions(user_id);
CREATE INDEX idx_submissions_problem ON submissions(problem_id);
CREATE INDEX idx_submissions_contest ON submissions(contest_id);
CREATE INDEX idx_submissions_verdict ON submissions(verdict);
CREATE INDEX idx_submissions_submitted_at ON submissions(submitted_at DESC);

-- Test case results
CREATE TABLE test_case_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    submission_id UUID NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    test_case_id UUID NOT NULL REFERENCES test_cases(id) ON DELETE CASCADE,
    verdict VARCHAR(30) NOT NULL,
    execution_time_ms INTEGER,
    memory_usage_kb INTEGER,
    actual_output TEXT,
    error_message TEXT,
    
    UNIQUE(submission_id, test_case_id)
);

CREATE INDEX idx_test_case_results_submission ON test_case_results(submission_id);

-- Benchmark results
CREATE TABLE benchmark_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    submission_id UUID NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    test_case_id UUID REFERENCES test_cases(id) ON DELETE CASCADE,
    iterations INTEGER NOT NULL,
    time_avg_ms DOUBLE PRECISION NOT NULL,
    time_median_ms DOUBLE PRECISION NOT NULL,
    time_min_ms DOUBLE PRECISION NOT NULL,
    time_max_ms DOUBLE PRECISION NOT NULL,
    time_stddev_ms DOUBLE PRECISION NOT NULL,
    memory_avg_kb DOUBLE PRECISION NOT NULL,
    memory_peak_kb DOUBLE PRECISION NOT NULL,
    time_outliers JSONB NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_benchmark_results_submission ON benchmark_results(submission_id);

-- Contest standings (materialized for performance)
CREATE TABLE contest_standings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    contest_id UUID NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rank INTEGER NOT NULL,
    score INTEGER NOT NULL DEFAULT 0,
    penalty_time INTEGER NOT NULL DEFAULT 0,
    problems_solved INTEGER NOT NULL DEFAULT 0,
    last_accepted_at TIMESTAMPTZ,
    problem_scores JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(contest_id, user_id)
);

CREATE INDEX idx_contest_standings_contest ON contest_standings(contest_id);
CREATE INDEX idx_contest_standings_rank ON contest_standings(contest_id, rank);

-- Audit log for admin actions
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    target_type VARCHAR(50),
    target_id UUID,
    details JSONB,
    ip_address INET,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers for updated_at
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_contests_updated_at
    BEFORE UPDATE ON contests
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_problems_updated_at
    BEFORE UPDATE ON problems
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_contest_standings_updated_at
    BEFORE UPDATE ON contest_standings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
