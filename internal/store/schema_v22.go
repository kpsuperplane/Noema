package store

// schemaV22SQL permits governed actions and MCP authentication from exact Task runs.
const schemaV22SQL = `
PRAGMA defer_foreign_keys = ON;

DROP INDEX mcp_auth_requests_attention;
DROP INDEX mcp_auth_requests_action;
DROP INDEX action_requests_attention;
DROP INDEX action_requests_conversation;
DROP INDEX action_requests_identity;
DROP INDEX action_request_events_request;
ALTER TABLE mcp_auth_requests RENAME TO mcp_auth_requests_v21;
ALTER TABLE action_request_assessments RENAME TO action_request_assessments_v21;
ALTER TABLE action_request_decisions RENAME TO action_request_decisions_v21;
ALTER TABLE action_request_events RENAME TO action_request_events_v21;
ALTER TABLE action_requests RENAME TO action_requests_v21;
CREATE TABLE action_requests (
    action_id TEXT NOT NULL CHECK (length(action_id)=39 AND substr(action_id,1,7)='action:' AND substr(action_id,8) NOT GLOB '*[^0-9a-f]*'),
    revision INTEGER NOT NULL CHECK (revision=1),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id='human:local'),
    conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
    turn_id TEXT,
    call_item_id TEXT,
    task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT,
    run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT,
    task_generation INTEGER,
    run_item_id TEXT REFERENCES task_run_items(item_id) ON DELETE RESTRICT,
    approval_item_id TEXT,
    requesting_agent_id TEXT NOT NULL CHECK (length(trim(requesting_agent_id)) BETWEEN 1 AND 128),
    capability_name TEXT NOT NULL CHECK (length(trim(capability_name)) BETWEEN 1 AND 256),
    operation_token TEXT NOT NULL CHECK (length(trim(operation_token)) BETWEEN 1 AND 256),
    review_route TEXT NOT NULL CHECK (review_route IN ('human_review','llm_review')),
    read_only INTEGER NOT NULL CHECK (read_only IN (0,1)),
    repeat_safe INTEGER NOT NULL CHECK (repeat_safe IN (0,1)),
    destructive INTEGER NOT NULL CHECK (destructive IN (0,1)),
    open_world INTEGER NOT NULL CHECK (open_world IN (0,1)),
    arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json) AND json_type(arguments_json)='object' AND length(CAST(arguments_json AS BLOB))<=1048576),
    arguments_sha256 TEXT NOT NULL CHECK (length(arguments_sha256)=64 AND arguments_sha256=lower(arguments_sha256)),
    input_schema_json TEXT NOT NULL CHECK (json_valid(input_schema_json) AND json_type(input_schema_json)='object' AND length(CAST(input_schema_json AS BLOB))<=262144),
    authorization_context_json TEXT NOT NULL CHECK (json_valid(authorization_context_json) AND json_type(authorization_context_json)='object' AND length(CAST(authorization_context_json AS BLOB))<=262144),
    safe_summary TEXT NOT NULL CHECK (length(trim(safe_summary)) BETWEEN 1 AND 1000),
    state TEXT NOT NULL CHECK (state IN ('proposed','awaiting_approval','executable','executing','succeeded','failed','outcome_uncertain','declined','superseded','cancelled')),
    output_json TEXT CHECK (output_json IS NULL OR json_valid(output_json)),
    failure_code TEXT CHECK (failure_code IS NULL OR length(failure_code) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL, completed_at_ms INTEGER,
    PRIMARY KEY (action_id,revision), UNIQUE(call_item_id), UNIQUE(run_item_id), UNIQUE(approval_item_id),
    FOREIGN KEY (conversation_id,turn_id) REFERENCES conversation_turns(conversation_id,turn_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id,call_item_id) REFERENCES conversation_items(conversation_id,item_id) ON DELETE RESTRICT,
    FOREIGN KEY (conversation_id,approval_item_id) REFERENCES conversation_items(conversation_id,item_id) ON DELETE RESTRICT,
    CHECK ((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND call_item_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL AND run_item_id IS NULL)
        OR (conversation_id IS NULL AND turn_id IS NULL AND call_item_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL AND run_item_id IS NOT NULL AND approval_item_id IS NULL))
) STRICT;
INSERT INTO action_requests SELECT action_id,revision,owner_human_id,conversation_id,turn_id,call_item_id,
 NULL,NULL,NULL,NULL,approval_item_id,requesting_agent_id,capability_name,operation_token,review_route,
 read_only,repeat_safe,destructive,open_world,arguments_json,arguments_sha256,input_schema_json,
 authorization_context_json,safe_summary,state,output_json,failure_code,created_at_ms,updated_at_ms,completed_at_ms
 FROM action_requests_v21;

CREATE INDEX action_requests_attention ON action_requests(owner_human_id,state,created_at_ms DESC,action_id DESC) WHERE state='awaiting_approval';
CREATE INDEX action_requests_conversation ON action_requests(conversation_id,created_at_ms DESC,action_id DESC);
CREATE INDEX action_requests_task ON action_requests(task_id,created_at_ms DESC,action_id DESC);
CREATE UNIQUE INDEX action_requests_identity ON action_requests(action_id);

CREATE TABLE action_request_assessments (
 action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK(action_revision=1),
 status TEXT NOT NULL CHECK(status IN ('completed','reviewer_unavailable','invalid_response')),
 reviewer_selection_json TEXT CHECK(reviewer_selection_json IS NULL OR json_valid(reviewer_selection_json)),
 authorization TEXT CHECK(authorization IS NULL OR authorization IN ('explicit','substantive','weak','absent')),
 risk TEXT CHECK(risk IS NULL OR risk IN ('low','medium','high','critical')),
 recommendation TEXT NOT NULL CHECK(recommendation IN ('auto_execute','require_approval')),
 reason_codes_json TEXT NOT NULL CHECK(json_valid(reason_codes_json) AND json_type(reason_codes_json)='array'),
 explanation TEXT NOT NULL CHECK(length(trim(explanation)) BETWEEN 1 AND 4000), created_at_ms INTEGER NOT NULL,
 PRIMARY KEY(action_id,action_revision), FOREIGN KEY(action_id,action_revision) REFERENCES action_requests(action_id,revision) ON DELETE RESTRICT,
 CHECK((status='completed' AND reviewer_selection_json IS NOT NULL AND authorization IS NOT NULL AND risk IS NOT NULL)
 OR (status<>'completed' AND reviewer_selection_json IS NULL AND authorization IS NULL AND risk IS NULL AND recommendation='require_approval'))
) STRICT;
INSERT INTO action_request_assessments SELECT * FROM action_request_assessments_v21;

CREATE TABLE action_request_decisions (
 action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK(action_revision=1),
 state TEXT NOT NULL CHECK(state IN ('pending','approved','declined','consumed','superseded')),
 decided_by_human_id TEXT CHECK(decided_by_human_id IS NULL OR decided_by_human_id='human:local'),
 decided_at_ms INTEGER, consumed_at_ms INTEGER, created_at_ms INTEGER NOT NULL,
 PRIMARY KEY(action_id,action_revision), FOREIGN KEY(action_id,action_revision) REFERENCES action_requests(action_id,revision) ON DELETE RESTRICT,
 CHECK((state='pending' AND decided_by_human_id IS NULL AND decided_at_ms IS NULL AND consumed_at_ms IS NULL)
 OR (state IN ('approved','declined') AND decided_by_human_id IS NOT NULL AND decided_at_ms IS NOT NULL AND consumed_at_ms IS NULL)
 OR (state='consumed' AND decided_by_human_id IS NOT NULL AND decided_at_ms IS NOT NULL AND consumed_at_ms IS NOT NULL)
 OR (state='superseded' AND consumed_at_ms IS NULL))
) STRICT;
INSERT INTO action_request_decisions SELECT * FROM action_request_decisions_v21;

CREATE TABLE action_request_events (
 event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 event_id TEXT NOT NULL UNIQUE CHECK(length(event_id)=45 AND substr(event_id,1,13)='action_event:' AND substr(event_id,14) NOT GLOB '*[^0-9a-f]*'),
 action_id TEXT NOT NULL, action_revision INTEGER NOT NULL CHECK(action_revision=1),
 event_kind TEXT NOT NULL CHECK(event_kind IN ('proposed','reviewed','approval_requested','approved','declined','execution_started','succeeded','failed','outcome_uncertain','superseded','cancelled')),
 actor_id TEXT NOT NULL CHECK(length(trim(actor_id)) BETWEEN 1 AND 128), safe_payload_json TEXT NOT NULL CHECK(json_valid(safe_payload_json)), created_at_ms INTEGER NOT NULL,
 FOREIGN KEY(action_id,action_revision) REFERENCES action_requests(action_id,revision) ON DELETE RESTRICT
) STRICT;
INSERT INTO action_request_events SELECT * FROM action_request_events_v21;
CREATE INDEX action_request_events_request ON action_request_events(action_id,action_revision,event_sequence);

CREATE TABLE mcp_auth_requests (
 request_id TEXT PRIMARY KEY CHECK(length(request_id)=41 AND substr(request_id,1,9)='mcp_auth:'), revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 owner_human_id TEXT NOT NULL CHECK(owner_human_id='human:local'), conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
 turn_id TEXT REFERENCES conversation_turns(turn_id) ON DELETE RESTRICT, call_item_id TEXT REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
 task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT, run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT,
 task_generation INTEGER, run_item_id TEXT REFERENCES task_run_items(item_id) ON DELETE RESTRICT,
 mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE, capability_name TEXT NOT NULL,
 binding_json TEXT NOT NULL CHECK(json_valid(binding_json) AND json_type(binding_json)='object'), arguments_json TEXT NOT NULL CHECK(json_valid(arguments_json) AND json_type(arguments_json)='object'),
 provider TEXT NOT NULL, provider_round INTEGER NOT NULL CHECK(provider_round>=0), output_index INTEGER NOT NULL CHECK(output_index>=0),
 provider_call_id TEXT NOT NULL, provider_name TEXT NOT NULL, oauth_attempt_id TEXT REFERENCES mcp_oauth_attempts(attempt_id) ON DELETE SET NULL,
 state TEXT NOT NULL CHECK(state IN ('awaiting_user','authorizing','resuming','completed','cancelled','superseded')), failure_code TEXT,
 action_request_id TEXT REFERENCES action_requests(action_id) ON DELETE RESTRICT, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
 UNIQUE(conversation_id,turn_id,output_index), UNIQUE(run_id,run_item_id),
 CHECK((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND call_item_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL AND run_item_id IS NULL)
 OR (conversation_id IS NULL AND turn_id IS NULL AND call_item_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL AND run_item_id IS NOT NULL))
) STRICT;
INSERT INTO mcp_auth_requests SELECT request_id,revision,owner_human_id,conversation_id,turn_id,call_item_id,
 NULL,NULL,NULL,NULL,mcp_server_id,capability_name,binding_json,arguments_json,provider,provider_round,output_index,
 provider_call_id,provider_name,oauth_attempt_id,state,failure_code,action_request_id,created_at_ms,updated_at_ms FROM mcp_auth_requests_v21;
CREATE INDEX mcp_auth_requests_attention ON mcp_auth_requests(owner_human_id,state,created_at_ms) WHERE state IN ('awaiting_user','authorizing');
CREATE UNIQUE INDEX mcp_auth_requests_action ON mcp_auth_requests(action_request_id) WHERE action_request_id IS NOT NULL;

DROP INDEX mcp_tools_server;
ALTER TABLE mcp_tools RENAME TO mcp_tools_v21;
CREATE TABLE mcp_tools (
 mcp_tool_id TEXT PRIMARY KEY CHECK(length(mcp_tool_id)=41 AND substr(mcp_tool_id,1,9)='mcp_tool:' AND substr(mcp_tool_id,10) NOT GLOB '*[^0-9a-f]*'),
 mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 256), description TEXT CHECK(description IS NULL OR length(CAST(description AS BLOB))<=8192),
 input_schema_json TEXT NOT NULL CHECK(json_valid(input_schema_json) AND json_type(input_schema_json)='object' AND length(CAST(input_schema_json AS BLOB))<=262144),
 output_schema_json TEXT CHECK(output_schema_json IS NULL OR (json_valid(output_schema_json) AND json_type(output_schema_json)='object' AND length(CAST(output_schema_json AS BLOB))<=262144)),
 annotations_json TEXT NOT NULL CHECK(json_valid(annotations_json) AND json_type(annotations_json)='object' AND length(CAST(annotations_json AS BLOB))<=16384),
 source_revision TEXT NOT NULL CHECK(length(source_revision)=64 AND source_revision=lower(source_revision)),
 read_only INTEGER CHECK(read_only IS NULL OR read_only IN (0,1)), read_only_source TEXT CHECK(read_only_source IS NULL OR read_only_source IN ('annotation','safe_default','model','human')),
 idempotent INTEGER CHECK(idempotent IS NULL OR idempotent IN (0,1)), idempotent_source TEXT CHECK(idempotent_source IS NULL OR idempotent_source IN ('annotation','safe_default','model','human')),
 destructive INTEGER CHECK(destructive IS NULL OR destructive IN (0,1)), destructive_source TEXT CHECK(destructive_source IS NULL OR destructive_source IN ('annotation','safe_default','model','human')),
 open_world INTEGER CHECK(open_world IS NULL OR open_world IN (0,1)), open_world_source TEXT CHECK(open_world_source IS NULL OR open_world_source IN ('annotation','safe_default','model','human')),
 status TEXT NOT NULL CHECK(status IN ('ready','defaulted','disabled')), policy_revision INTEGER NOT NULL CHECK(policy_revision>0),
 created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL, UNIQUE(mcp_server_id,name),
 CHECK((read_only IS NULL)=(read_only_source IS NULL)), CHECK((idempotent IS NULL)=(idempotent_source IS NULL)),
 CHECK((destructive IS NULL)=(destructive_source IS NULL)), CHECK((open_world IS NULL)=(open_world_source IS NULL))
) STRICT;
INSERT INTO mcp_tools SELECT * FROM mcp_tools_v21;
DROP TABLE mcp_tools_v21;
CREATE INDEX mcp_tools_server ON mcp_tools(mcp_server_id,name);

DROP TABLE mcp_auth_requests_v21;
DROP TABLE action_request_assessments_v21;
DROP TABLE action_request_decisions_v21;
DROP TABLE action_request_events_v21;
DROP TABLE action_requests_v21;
`
