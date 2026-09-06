package store

// schemaV33SQL removes Apple model accounts and their current selections.
const schemaV33SQL = `
ALTER TABLE hosted_model_assignments RENAME TO hosted_model_assignments_v32;
CREATE TABLE hosted_model_assignments (
    role TEXT PRIMARY KEY CHECK (role IN (
        'noema', 'simple_tasks', 'medium_tasks', 'difficult_tasks', 'task_reviewer',
        'web_fetch_summarizer', 'tool_progress_audit', 'action_reviewer',
        'memory_consolidation'
    )),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex','openai','openrouter','local_models')),
    provider_account_id TEXT NOT NULL REFERENCES provider_accounts(provider_account_id) ON DELETE RESTRICT,
    selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended','explicit_profile')),
    model_profile TEXT,
    reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none','minimal','low','medium','high','xhigh')),
    fast_mode INTEGER NOT NULL CHECK (fast_mode IN (0,1)),
    CHECK ((selection_mode='noema_recommended' AND model_profile IS NULL AND reasoning_effort IS NULL)
        OR (selection_mode='explicit_profile' AND model_profile IS NOT NULL AND trim(model_profile)<>''))
) STRICT;
INSERT INTO hosted_model_assignments
SELECT * FROM hosted_model_assignments_v32
WHERE provider_kind <> 'foundation_local'
  AND provider_account_id NOT IN (SELECT provider_account_id FROM provider_accounts WHERE provider_kind='foundation_local');
DROP TABLE hosted_model_assignments_v32;
DELETE FROM default_model_preference
WHERE provider_kind='foundation_local'
   OR provider_account_id IN (SELECT provider_account_id FROM provider_accounts WHERE provider_kind='foundation_local');
DELETE FROM provider_accounts WHERE provider_kind='foundation_local';
`
