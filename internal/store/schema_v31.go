package store

// schemaV31SQL stores verified local models, their events, and the system model default.
const schemaV31SQL = `
ALTER TABLE hosted_model_assignments RENAME TO hosted_model_assignments_v30;
CREATE TABLE hosted_model_assignments (
    role TEXT PRIMARY KEY CHECK (role IN (
        'noema', 'simple_tasks', 'medium_tasks', 'difficult_tasks', 'task_reviewer',
        'web_fetch_summarizer', 'tool_progress_audit', 'action_reviewer',
        'memory_consolidation'
    )),
    provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex','openai','openrouter','foundation_local','local_models')),
    provider_account_id TEXT NOT NULL REFERENCES provider_accounts(provider_account_id) ON DELETE RESTRICT,
    selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended','explicit_profile')),
    model_profile TEXT,
    reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none','minimal','low','medium','high','xhigh')),
    fast_mode INTEGER NOT NULL CHECK (fast_mode IN (0,1)),
    CHECK ((selection_mode='noema_recommended' AND model_profile IS NULL AND reasoning_effort IS NULL)
        OR (selection_mode='explicit_profile' AND model_profile IS NOT NULL AND trim(model_profile)<>''))
) STRICT;
INSERT INTO hosted_model_assignments SELECT * FROM hosted_model_assignments_v30;
DROP TABLE hosted_model_assignments_v30;

CREATE TABLE local_model_installations (
    installation_id TEXT PRIMARY KEY CHECK (length(trim(installation_id)) BETWEEN 1 AND 256),
    model_id TEXT NOT NULL CHECK (length(trim(model_id)) BETWEEN 1 AND 256),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    file TEXT NOT NULL CHECK (length(trim(file)) BETWEEN 1 AND 1024),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('catalog','public_gguf','local_file')),
    repo TEXT, revision TEXT, license TEXT,
    sha256 TEXT CHECK (sha256 IS NULL OR (length(sha256)=64 AND sha256=lower(sha256))),
    completed_bytes INTEGER NOT NULL DEFAULT 0 CHECK (completed_bytes>=0),
    total_bytes INTEGER CHECK (total_bytes IS NULL OR total_bytes>0),
    disk_bytes INTEGER NOT NULL DEFAULT 0 CHECK (disk_bytes>=0),
    blob_path TEXT,
    backend TEXT CHECK (backend IS NULL OR backend IN ('metal','cuda','vulkan','cpu')),
    status TEXT NOT NULL CHECK (status IN ('queued','downloading','verifying','installed','cancelled','failed')),
    is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0,1)),
    error_code TEXT, error_message TEXT,
    created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
    CHECK (completed_bytes<=COALESCE(total_bytes,completed_bytes)),
    CHECK (status<>'installed' OR (sha256 IS NOT NULL AND blob_path IS NOT NULL AND disk_bytes>0)),
    CHECK (source_kind<>'catalog' OR (repo IS NOT NULL AND revision IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX local_model_one_active ON local_model_installations(is_active) WHERE is_active=1;
CREATE INDEX local_model_by_model ON local_model_installations(model_id,status,updated_at_ms DESC);

CREATE TABLE local_model_events (
    cursor INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('installation_updated','transfer_progress','active_model_changed','default_preference_changed')),
    installation_id TEXT, model_id TEXT,
    created_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE default_model_preference (
    preference_id TEXT PRIMARY KEY CHECK (preference_id='default'),
    provider_kind TEXT NOT NULL,
    provider_account_id TEXT NOT NULL REFERENCES provider_accounts(provider_account_id) ON DELETE RESTRICT,
    selection_mode TEXT NOT NULL CHECK (selection_mode IN ('noema_recommended','explicit_profile')),
    model_profile TEXT, reasoning_effort TEXT,
    fast_mode INTEGER NOT NULL CHECK (fast_mode IN (0,1)),
    updated_at_ms INTEGER NOT NULL,
    CHECK ((selection_mode='noema_recommended' AND model_profile IS NULL AND reasoning_effort IS NULL)
        OR (selection_mode='explicit_profile' AND model_profile IS NOT NULL AND trim(model_profile)<>''))
) STRICT;
`
