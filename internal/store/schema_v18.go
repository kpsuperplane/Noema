package store

// schemaV18SQL adds durable action requests and their review ledger.
const schemaV18SQL = `
CREATE TABLE action_requests (
    action_id TEXT NOT NULL CHECK (
        length(action_id) = 39 AND substr(action_id, 1, 7) = 'action:'
        AND substr(action_id, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    revision INTEGER NOT NULL CHECK (revision = 1),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id = 'human:local'),
    conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
    turn_id TEXT NOT NULL,
    call_item_id TEXT NOT NULL,
    approval_item_id TEXT,
    requesting_agent_id TEXT NOT NULL CHECK (length(trim(requesting_agent_id)) BETWEEN 1 AND 128),
    capability_name TEXT NOT NULL CHECK (length(trim(capability_name)) BETWEEN 1 AND 256),
    operation_token TEXT NOT NULL CHECK (length(trim(operation_token)) BETWEEN 1 AND 256),
    review_route TEXT NOT NULL CHECK (review_route IN ('human_review','llm_review')),
    read_only INTEGER NOT NULL CHECK (read_only IN (0,1)),
    repeat_safe INTEGER NOT NULL CHECK (repeat_safe IN (0,1)),
    destructive INTEGER NOT NULL CHECK (destructive IN (0,1)),
    open_world INTEGER NOT NULL CHECK (open_world IN (0,1)),
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json) AND json_type(arguments_json) = 'object'
        AND length(CAST(arguments_json AS BLOB)) <= 1048576
    ),
    arguments_sha256 TEXT NOT NULL CHECK (
        length(arguments_sha256) = 64 AND arguments_sha256 = lower(arguments_sha256)
    ),
    input_schema_json TEXT NOT NULL CHECK (
        json_valid(input_schema_json) AND json_type(input_schema_json) = 'object'
        AND length(CAST(input_schema_json AS BLOB)) <= 262144
    ),
    authorization_context_json TEXT NOT NULL CHECK (
        json_valid(authorization_context_json) AND json_type(authorization_context_json) = 'object'
        AND length(CAST(authorization_context_json AS BLOB)) <= 262144
    ),
    safe_summary TEXT NOT NULL CHECK (length(trim(safe_summary)) BETWEEN 1 AND 1000),
    state TEXT NOT NULL CHECK (state IN (
        'proposed','awaiting_approval','executable','executing','succeeded','failed',
        'outcome_uncertain','declined','superseded','cancelled'
    )),
    output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
    failure_code TEXT CHECK (failure_code IS NULL OR length(failure_code) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    PRIMARY KEY (action_id, revision),
    UNIQUE (call_item_id),
    UNIQUE (approval_item_id),
    FOREIGN KEY (conversation_id, turn_id)
        REFERENCES conversation_turns(conversation_id, turn_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, call_item_id)
        REFERENCES conversation_items(conversation_id, item_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id, approval_item_id)
        REFERENCES conversation_items(conversation_id, item_id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX action_requests_attention
ON action_requests(owner_human_id, state, created_at_ms DESC, action_id DESC)
WHERE state = 'awaiting_approval';
CREATE INDEX action_requests_conversation
ON action_requests(conversation_id, created_at_ms DESC, action_id DESC);

CREATE TABLE action_request_assessments (
    action_id TEXT NOT NULL,
    action_revision INTEGER NOT NULL CHECK (action_revision = 1),
    status TEXT NOT NULL CHECK (status IN ('completed','reviewer_unavailable','invalid_response')),
    reviewer_selection_json TEXT CHECK (
        reviewer_selection_json IS NULL OR json_valid(reviewer_selection_json)
    ),
    authorization TEXT CHECK (authorization IS NULL OR authorization IN ('explicit','substantive','weak','absent')),
    risk TEXT CHECK (risk IS NULL OR risk IN ('low','medium','high','critical')),
    recommendation TEXT NOT NULL CHECK (recommendation IN ('auto_execute','require_approval')),
    reason_codes_json TEXT NOT NULL CHECK (
        json_valid(reason_codes_json) AND json_type(reason_codes_json) = 'array'
    ),
    explanation TEXT NOT NULL CHECK (length(trim(explanation)) BETWEEN 1 AND 4000),
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (action_id, action_revision),
    FOREIGN KEY (action_id, action_revision)
        REFERENCES action_requests(action_id, revision) ON DELETE RESTRICT,
    CHECK ((status = 'completed' AND reviewer_selection_json IS NOT NULL
        AND authorization IS NOT NULL AND risk IS NOT NULL)
      OR (status <> 'completed' AND reviewer_selection_json IS NULL
        AND authorization IS NULL AND risk IS NULL AND recommendation = 'require_approval'))
) STRICT;

CREATE TABLE action_request_decisions (
    action_id TEXT NOT NULL,
    action_revision INTEGER NOT NULL CHECK (action_revision = 1),
    state TEXT NOT NULL CHECK (state IN ('pending','approved','declined','consumed','superseded')),
    decided_by_human_id TEXT CHECK (decided_by_human_id IS NULL OR decided_by_human_id = 'human:local'),
    decided_at_ms INTEGER,
    consumed_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (action_id, action_revision),
    FOREIGN KEY (action_id, action_revision)
        REFERENCES action_requests(action_id, revision) ON DELETE RESTRICT,
    CHECK ((state = 'pending' AND decided_by_human_id IS NULL AND decided_at_ms IS NULL AND consumed_at_ms IS NULL)
      OR (state IN ('approved','declined') AND decided_by_human_id IS NOT NULL
        AND decided_at_ms IS NOT NULL AND consumed_at_ms IS NULL)
      OR (state = 'consumed' AND decided_by_human_id IS NOT NULL
        AND decided_at_ms IS NOT NULL AND consumed_at_ms IS NOT NULL)
      OR (state = 'superseded' AND consumed_at_ms IS NULL))
) STRICT;

CREATE TABLE action_request_events (
    event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE CHECK (
        length(event_id) = 45 AND substr(event_id, 1, 13) = 'action_event:'
        AND substr(event_id, 14) NOT GLOB '*[^0-9a-f]*'
    ),
    action_id TEXT NOT NULL,
    action_revision INTEGER NOT NULL CHECK (action_revision = 1),
    event_kind TEXT NOT NULL CHECK (event_kind IN (
        'proposed','reviewed','approval_requested','approved','declined',
        'execution_started','succeeded','failed','outcome_uncertain','superseded','cancelled'
    )),
    actor_id TEXT NOT NULL CHECK (length(trim(actor_id)) BETWEEN 1 AND 128),
    safe_payload_json TEXT NOT NULL CHECK (json_valid(safe_payload_json)),
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY (action_id, action_revision)
        REFERENCES action_requests(action_id, revision) ON DELETE RESTRICT
) STRICT;

CREATE INDEX action_request_events_request
ON action_request_events(action_id, action_revision, event_sequence);
`
