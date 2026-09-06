package store

// schemaV20SQL adds restart-safe MCP call authentication interruptions.
// It stores call authority and private arguments. It stores no credentials.
const schemaV20SQL = `
CREATE TABLE mcp_auth_requests (
    request_id TEXT PRIMARY KEY CHECK (length(request_id)=41 AND substr(request_id,1,9)='mcp_auth:'),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id='human:local'),
    conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE RESTRICT,
    turn_id TEXT NOT NULL REFERENCES conversation_turns(turn_id) ON DELETE RESTRICT,
    call_item_id TEXT NOT NULL REFERENCES conversation_items(item_id) ON DELETE RESTRICT,
    mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(mcp_server_id) ON DELETE RESTRICT,
    capability_name TEXT NOT NULL,
    binding_json TEXT NOT NULL CHECK (json_valid(binding_json) AND json_type(binding_json)='object'),
    arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json) AND json_type(arguments_json)='object'),
    provider TEXT NOT NULL,
    provider_round INTEGER NOT NULL CHECK (provider_round >= 0),
    output_index INTEGER NOT NULL CHECK (output_index >= 0),
    provider_call_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    oauth_attempt_id TEXT REFERENCES mcp_oauth_attempts(attempt_id) ON DELETE SET NULL,
    state TEXT NOT NULL CHECK (state IN ('awaiting_user','authorizing','resuming','completed','cancelled','superseded')),
    failure_code TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(conversation_id, turn_id, output_index)
) STRICT;
CREATE INDEX mcp_auth_requests_attention ON mcp_auth_requests(owner_human_id,state,created_at_ms)
 WHERE state IN ('awaiting_user','authorizing');
`
