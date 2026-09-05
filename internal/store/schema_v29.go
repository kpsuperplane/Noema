package store

// schemaV29SQL stores the global Task policy and each claimed run's immutable snapshot.
const schemaV29SQL = `
CREATE TABLE task_execution_policy (
    policy_id TEXT PRIMARY KEY CHECK (policy_id = 'default'),
    max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
    max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
    max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
    progress_audit_interval INTEGER NOT NULL CHECK (progress_audit_interval BETWEEN 1 AND max_provider_continuations),
    max_automatic_retries INTEGER NOT NULL CHECK (max_automatic_retries BETWEEN 0 AND 20),
    max_review_rounds INTEGER NOT NULL CHECK (max_review_rounds BETWEEN 1 AND 20)
) STRICT;
INSERT INTO task_execution_policy VALUES ('default',80,400,120,20,3,3);

ALTER TABLE task_runs ADD COLUMN max_provider_continuations INTEGER NOT NULL DEFAULT 80 CHECK (max_provider_continuations BETWEEN 1 AND 1000);
ALTER TABLE task_runs ADD COLUMN max_tool_calls INTEGER NOT NULL DEFAULT 400 CHECK (max_tool_calls BETWEEN 1 AND 10000);
ALTER TABLE task_runs ADD COLUMN max_active_minutes INTEGER NOT NULL DEFAULT 120 CHECK (max_active_minutes BETWEEN 1 AND 10080);
ALTER TABLE task_runs ADD COLUMN progress_audit_interval INTEGER NOT NULL DEFAULT 20 CHECK (progress_audit_interval BETWEEN 1 AND max_provider_continuations);
ALTER TABLE task_runs ADD COLUMN max_automatic_retries INTEGER NOT NULL DEFAULT 3 CHECK (max_automatic_retries BETWEEN 0 AND 20);
ALTER TABLE task_runs ADD COLUMN max_review_rounds INTEGER NOT NULL DEFAULT 3 CHECK (max_review_rounds BETWEEN 1 AND 20);
`
