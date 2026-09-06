package store

// schemaV15SQL adds the Task lifecycle state and bounded run history.
const schemaV15SQL = `
ALTER TABLE tasks ADD COLUMN generation INTEGER NOT NULL DEFAULT 1 CHECK (generation > 0);
ALTER TABLE tasks ADD COLUMN stage_key TEXT NOT NULL DEFAULT 'inbox'
    CHECK (stage_key IN ('inbox','queue','doing','waiting','done','cancelled'));
ALTER TABLE tasks ADD COLUMN active_gate_id TEXT;
ALTER TABLE tasks ADD COLUMN completed_at_ms INTEGER;
ALTER TABLE tasks ADD COLUMN cancelled_at_ms INTEGER;
ALTER TABLE tasks ADD COLUMN execution_complexity TEXT
    CHECK (execution_complexity IS NULL OR execution_complexity IN ('simple','medium','difficult'));

CREATE TABLE task_runs (
    run_id TEXT PRIMARY KEY CHECK (run_id GLOB 'run:*'),
    task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    task_generation INTEGER NOT NULL CHECK (task_generation > 0),
    instance_name TEXT NOT NULL,
    run_kind TEXT NOT NULL CHECK (run_kind IN ('planner','executor','reviewer')),
    status TEXT NOT NULL CHECK (status IN ('queued','leased','running','completed','waiting_for_approval','interrupted','failed','cancelled')),
    agent_id TEXT NOT NULL,
    attempt_index INTEGER NOT NULL DEFAULT 0 CHECK (attempt_index >= 0),
    review_round INTEGER NOT NULL DEFAULT 0 CHECK (review_round >= 0),
    parent_run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT,
    provider_kind TEXT NOT NULL DEFAULT '', provider_account_id TEXT NOT NULL DEFAULT '',
    selection_mode TEXT NOT NULL DEFAULT 'provider_default', model_profile TEXT,
    reasoning_effort TEXT, fast_mode INTEGER NOT NULL DEFAULT 0 CHECK (fast_mode IN (0,1)),
    executor_backend TEXT NOT NULL, executor_agent_id TEXT NOT NULL, effective_cwd TEXT,
    error_code TEXT, error_message TEXT,
    provider_call_count INTEGER NOT NULL DEFAULT 0, tool_call_count INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER NOT NULL DEFAULT 0, cached_input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0, active_milliseconds INTEGER NOT NULL DEFAULT 0,
    queued_at_ms INTEGER NOT NULL, started_at_ms INTEGER, ended_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX task_runs_task ON task_runs(task_id, created_at_ms DESC, run_id DESC);

CREATE TABLE task_gates (
    gate_id TEXT PRIMARY KEY CHECK (gate_id GLOB 'gate:*'),
    task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    task_generation INTEGER NOT NULL CHECK (task_generation > 0),
    gate_kind TEXT NOT NULL CHECK (gate_kind IN ('clarification','approval','recovery')),
    gate_state TEXT NOT NULL CHECK (gate_state IN ('open','resolved','superseded')),
    recovery_reason TEXT, retry_run_kind TEXT,
    prompt TEXT NOT NULL, context_markdown TEXT NOT NULL DEFAULT '', suggested_answers_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(suggested_answers_json)),
    opened_by TEXT NOT NULL, originating_run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT,
    opened_at_ms INTEGER NOT NULL, resolved_by TEXT, resolved_at_ms INTEGER, resolution_message_id TEXT
) STRICT;

CREATE TABLE task_messages (
    message_id TEXT PRIMARY KEY CHECK (message_id GLOB 'task_message:*'),
    task_id TEXT NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    task_generation INTEGER NOT NULL CHECK (task_generation > 0),
    gate_id TEXT REFERENCES task_gates(gate_id) ON DELETE RESTRICT,
    message_kind TEXT NOT NULL, body_markdown TEXT NOT NULL, author_actor_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX task_messages_task ON task_messages(task_id, created_at_ms DESC, message_id DESC);

CREATE TABLE task_run_items (
    item_id TEXT PRIMARY KEY CHECK (item_id GLOB 'run_item:*'),
    run_id TEXT NOT NULL REFERENCES task_runs(run_id) ON DELETE CASCADE,
    sequence_index INTEGER NOT NULL CHECK (sequence_index >= 0), round_index INTEGER NOT NULL DEFAULT 0 CHECK (round_index >= 0),
    item_kind TEXT NOT NULL, status TEXT NOT NULL, correlation_id TEXT, parent_item_id TEXT,
    content_text TEXT, payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
    created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
    UNIQUE (run_id, sequence_index)
) STRICT;
CREATE INDEX task_run_items_run ON task_run_items(run_id, sequence_index DESC);
`
