package store

// schemaV25SQL lets the durable call-authentication interruption reference one adapter connection.
const schemaV25SQL = `
DROP INDEX mcp_auth_requests_attention;
DROP INDEX mcp_auth_requests_action;
ALTER TABLE mcp_auth_requests RENAME TO mcp_auth_requests_v24;
CREATE TABLE mcp_auth_requests (
 request_id TEXT PRIMARY KEY CHECK(length(request_id)=41 AND substr(request_id,1,9)='mcp_auth:'), revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 owner_human_id TEXT NOT NULL CHECK(owner_human_id='human:local'), conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
 turn_id TEXT REFERENCES conversation_turns(turn_id) ON DELETE RESTRICT, call_item_id TEXT REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
 task_id TEXT REFERENCES tasks(task_id) ON DELETE RESTRICT, run_id TEXT REFERENCES task_runs(run_id) ON DELETE RESTRICT,
 task_generation INTEGER, run_item_id TEXT REFERENCES task_run_items(item_id) ON DELETE RESTRICT,
 authority_kind TEXT NOT NULL CHECK(authority_kind IN ('mcp_server','adapter_connection')),
 authority_id TEXT NOT NULL CHECK(length(authority_id) BETWEEN 1 AND 128),
 mcp_server_id TEXT REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE, capability_name TEXT NOT NULL,
 binding_json TEXT NOT NULL CHECK(json_valid(binding_json) AND json_type(binding_json)='object'), arguments_json TEXT NOT NULL CHECK(json_valid(arguments_json) AND json_type(arguments_json)='object'),
 provider TEXT NOT NULL, provider_round INTEGER NOT NULL CHECK(provider_round>=0), output_index INTEGER NOT NULL CHECK(output_index>=0),
 provider_call_id TEXT NOT NULL, provider_name TEXT NOT NULL, oauth_attempt_id TEXT REFERENCES mcp_oauth_attempts(attempt_id) ON DELETE SET NULL,
 state TEXT NOT NULL CHECK(state IN ('awaiting_user','authorizing','resuming','completed','cancelled','superseded')), failure_code TEXT,
 action_request_id TEXT REFERENCES action_requests(action_id) ON DELETE RESTRICT, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
 UNIQUE(conversation_id,turn_id,output_index), UNIQUE(run_id,run_item_id),
 CHECK((authority_kind='mcp_server' AND mcp_server_id=authority_id) OR (authority_kind='adapter_connection' AND mcp_server_id IS NULL AND length(authority_id)=32)),
 CHECK((conversation_id IS NOT NULL AND turn_id IS NOT NULL AND call_item_id IS NOT NULL AND task_id IS NULL AND run_id IS NULL AND task_generation IS NULL AND run_item_id IS NULL)
 OR (conversation_id IS NULL AND turn_id IS NULL AND call_item_id IS NULL AND task_id IS NOT NULL AND run_id IS NOT NULL AND task_generation IS NOT NULL AND run_item_id IS NOT NULL))
) STRICT;
INSERT INTO mcp_auth_requests SELECT request_id,revision,owner_human_id,conversation_id,turn_id,call_item_id,
 task_id,run_id,task_generation,run_item_id,'mcp_server',mcp_server_id,mcp_server_id,capability_name,binding_json,arguments_json,
 provider,provider_round,output_index,provider_call_id,provider_name,oauth_attempt_id,state,failure_code,action_request_id,created_at_ms,updated_at_ms
 FROM mcp_auth_requests_v24;
CREATE INDEX mcp_auth_requests_attention ON mcp_auth_requests(owner_human_id,state,created_at_ms) WHERE state IN ('awaiting_user','authorizing');
CREATE UNIQUE INDEX mcp_auth_requests_action ON mcp_auth_requests(action_request_id) WHERE action_request_id IS NOT NULL;
DROP TABLE mcp_auth_requests_v24;
`
