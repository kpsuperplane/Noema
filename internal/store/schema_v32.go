package store

// schemaV32SQL adds bounded runtime profiles for Chat turns and Task runs.
const schemaV32SQL = `
CREATE TABLE runtime_debug_spans (
    span_id TEXT PRIMARY KEY CHECK (span_id GLOB 'debug_span:*'),
    conversation_turn_id TEXT REFERENCES conversation_turns(turn_id) ON DELETE CASCADE,
    task_run_id TEXT REFERENCES task_runs(run_id) ON DELETE CASCADE,
    category TEXT NOT NULL CHECK (category IN ('provider','tool','runtime','persistence')),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 128),
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running','completed','failed','cancelled','interrupted')),
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json) AND json_type(metadata_json)='object'),
    started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    CHECK ((conversation_turn_id IS NULL) <> (task_run_id IS NULL)),
    CHECK ((status = 'running') = (ended_at_ms IS NULL AND duration_ms IS NULL))
) STRICT;
CREATE INDEX runtime_debug_spans_turn
ON runtime_debug_spans(conversation_turn_id, started_at_ms, span_id)
WHERE conversation_turn_id IS NOT NULL;
CREATE INDEX runtime_debug_spans_run
ON runtime_debug_spans(task_run_id, started_at_ms, span_id)
WHERE task_run_id IS NOT NULL;
`
