package store

// schemaV12SQL adds Task placement and recurring schedule authority.
const schemaV12SQL = `
ALTER TABLE tasks ADD COLUMN project_id TEXT REFERENCES projects(project_id) ON DELETE RESTRICT;
ALTER TABLE tasks ADD COLUMN executor_agent_id TEXT NOT NULL DEFAULT 'agent:task-executor'
    CHECK (trim(executor_agent_id) <> '');
ALTER TABLE tasks ADD COLUMN executor_acp_connection_revision INTEGER
    CHECK (executor_acp_connection_revision IS NULL OR executor_acp_connection_revision >= 1);
ALTER TABLE tasks ADD COLUMN cwd_override TEXT
    CHECK (cwd_override IS NULL OR trim(cwd_override) <> '');
ALTER TABLE tasks ADD COLUMN scheduled_for_ms INTEGER;
ALTER TABLE tasks ADD COLUMN schedule_time_zone TEXT
    CHECK (schedule_time_zone IS NULL OR trim(schedule_time_zone) <> '');
ALTER TABLE tasks ADD COLUMN missed_run_policy TEXT
    CHECK (missed_run_policy IN ('skip', 'run_once'));
ALTER TABLE tasks ADD COLUMN schedule_processed_at_ms INTEGER;
ALTER TABLE tasks ADD COLUMN recurrence_id TEXT;
ALTER TABLE tasks ADD COLUMN recurrence_revision INTEGER
    CHECK (recurrence_revision IS NULL OR recurrence_revision >= 1);
ALTER TABLE tasks ADD COLUMN recurrence_scheduled_for_ms INTEGER;

CREATE TABLE task_recurrences (
    recurrence_id TEXT PRIMARY KEY CHECK (recurrence_id GLOB 'recurrence:*'),
    workspace_id TEXT NOT NULL CHECK (workspace_id = 'workspace:personal'),
    project_id TEXT REFERENCES projects(project_id) ON DELETE RESTRICT,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 500),
    executor_agent_id TEXT NOT NULL REFERENCES agents(agent_id) ON DELETE RESTRICT,
    executor_acp_connection_revision INTEGER
        CHECK (executor_acp_connection_revision IS NULL OR executor_acp_connection_revision >= 1),
    cwd_override TEXT CHECK (cwd_override IS NULL OR trim(cwd_override) <> ''),
    starts_at_ms INTEGER NOT NULL,
    cron_expression TEXT NOT NULL CHECK (trim(cron_expression) <> ''),
    time_zone TEXT NOT NULL CHECK (trim(time_zone) <> ''),
    missed_run_policy TEXT NOT NULL CHECK (missed_run_policy IN ('skip', 'run_once')),
    overlap_policy TEXT NOT NULL CHECK (overlap_policy IN ('skip', 'queue_one', 'allow')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'paused', 'ended')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    next_run_at_ms INTEGER,
    pending_coalesced_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK (lifecycle <> 'active' OR next_run_at_ms IS NOT NULL)
) STRICT;

CREATE TABLE task_recurrence_occurrences (
    occurrence_id TEXT PRIMARY KEY CHECK (occurrence_id GLOB 'occurrence:*'),
    recurrence_id TEXT NOT NULL REFERENCES task_recurrences(recurrence_id) ON DELETE RESTRICT,
    recurrence_revision INTEGER NOT NULL CHECK (recurrence_revision >= 1),
    scheduled_for_ms INTEGER NOT NULL,
    local_slot TEXT NOT NULL CHECK (trim(local_slot) <> ''),
    trigger_kind TEXT NOT NULL DEFAULT 'scheduled' CHECK (trigger_kind IN ('scheduled', 'manual')),
    resolution TEXT NOT NULL CHECK (resolution IN ('materialized', 'skipped', 'coalesced')),
    task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (recurrence_id, local_slot),
    CHECK ((resolution = 'materialized') = (task_id IS NOT NULL))
) STRICT;

CREATE INDEX tasks_next_scheduled ON tasks(scheduled_for_ms, task_id)
WHERE scheduled_for_ms IS NOT NULL AND schedule_processed_at_ms IS NULL
  AND state = 'captured';
CREATE INDEX tasks_executor_agent ON tasks(executor_agent_id, state, updated_at_ms);
CREATE INDEX task_recurrences_next_due ON task_recurrences(next_run_at_ms, recurrence_id)
WHERE lifecycle = 'active';
CREATE INDEX task_recurrence_occurrences_history
ON task_recurrence_occurrences(recurrence_id, created_at_ms DESC, occurrence_id DESC);
`
