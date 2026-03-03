-- Migration: Create rule_configs table for admin-configurable policies
-- Phase 6.4: Rule configuration storage for dynamic policy management

-- Rule configurations table
-- Stores JSON-serialized specification rules that can be loaded at runtime
-- by different services (vanguard, minos, horus).
CREATE TABLE IF NOT EXISTS rule_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Unique identifier for this rule within a service
    name VARCHAR(128) NOT NULL,

    -- Which service uses this rule: 'vanguard', 'minos', 'horus'
    service VARCHAR(32) NOT NULL CHECK (service IN ('vanguard', 'minos', 'horus')),

    -- Human-readable description
    description TEXT,

    -- The JSON rule tree (RuleConfig from olympus-rules)
    config JSONB NOT NULL,

    -- Whether this rule is currently active
    enabled BOOLEAN NOT NULL DEFAULT TRUE,

    -- Semantic version for tracking changes
    version VARCHAR(32) NOT NULL DEFAULT '1.0.0',

    -- Audit: who last modified this rule
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Each (name, service) pair is unique
    CONSTRAINT uq_rule_configs_name_service UNIQUE (name, service)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_rule_configs_service ON rule_configs(service);
CREATE INDEX IF NOT EXISTS idx_rule_configs_enabled ON rule_configs(enabled);
CREATE INDEX IF NOT EXISTS idx_rule_configs_name_service ON rule_configs(name, service);

-- Trigger for updated_at
DROP TRIGGER IF EXISTS update_rule_configs_updated_at ON rule_configs;
CREATE TRIGGER update_rule_configs_updated_at
    BEFORE UPDATE ON rule_configs
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Seed default cleanup policies for Horus
INSERT INTO rule_configs (name, service, description, config, version) VALUES
(
    'stale_testcases',
    'horus',
    'Remove test case directories not accessed in 6 hours and whose problem has been deleted',
    '{
        "type": "and",
        "rules": [
            { "type": "spec", "name": "IsDirectory", "params": {} },
            { "type": "spec", "name": "LastAccessOlderThan", "params": { "hours": 6 } },
            { "type": "not", "rule": { "type": "spec", "name": "HasProblemRecord", "params": {} } }
        ]
    }',
    '1.0.0'
),
(
    'orphan_temp_dirs',
    'horus',
    'Remove temp execution directories older than 1 hour with no active submission',
    '{
        "type": "and",
        "rules": [
            { "type": "spec", "name": "IsDirectory", "params": {} },
            { "type": "spec", "name": "CreatedOlderThan", "params": { "hours": 1 } },
            { "type": "not", "rule": { "type": "spec", "name": "HasActiveSubmission", "params": {} } }
        ]
    }',
    '1.0.0'
),
(
    'orphan_binaries',
    'horus',
    'Remove user binary files older than 1 day with no submission record',
    '{
        "type": "and",
        "rules": [
            { "type": "spec", "name": "IsFile", "params": {} },
            { "type": "spec", "name": "CreatedOlderThan", "params": { "hours": 24 } },
            { "type": "not", "rule": { "type": "spec", "name": "HasSubmissionRecord", "params": {} } }
        ]
    }',
    '1.0.0'
)
ON CONFLICT (name, service) DO NOTHING;
