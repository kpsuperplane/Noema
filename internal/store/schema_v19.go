package store

// schemaV19SQL adds the durable MCP control plane. Secret material stays in
// protected home files and never enters these tables.
const schemaV19SQL = `
CREATE TABLE mcp_definitions (
    mcp_definition_id TEXT PRIMARY KEY CHECK (
        length(mcp_definition_id) = 47 AND substr(mcp_definition_id, 1, 15) = 'mcp_definition:'
        AND substr(mcp_definition_id, 16) NOT GLOB '*[^0-9a-f]*'
    ),
    definition_revision TEXT NOT NULL UNIQUE CHECK (
        length(definition_revision) = 56 AND substr(definition_revision, 1, 24) = 'mcp_definition_revision:'
        AND substr(definition_revision, 25) NOT GLOB '*[^0-9a-f]*'
    ),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 256),
    transport_kind TEXT NOT NULL CHECK (transport_kind IN ('stdio','streamable_http')),
    safe_config_json TEXT NOT NULL CHECK (
        json_valid(safe_config_json) AND json_type(safe_config_json) = 'object'
        AND length(CAST(safe_config_json AS BLOB)) <= 65536
    ),
    created_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE mcp_servers (
    mcp_server_id TEXT PRIMARY KEY CHECK (
        length(mcp_server_id) = 43 AND substr(mcp_server_id, 1, 11) = 'mcp_server:'
        AND substr(mcp_server_id, 12) NOT GLOB '*[^0-9a-f]*'
    ),
    mcp_definition_id TEXT NOT NULL REFERENCES mcp_definitions(mcp_definition_id) ON DELETE RESTRICT,
    connection_label TEXT CHECK (connection_label IS NULL OR length(trim(connection_label)) BETWEEN 1 AND 256),
    connection_revision TEXT NOT NULL CHECK (
        length(connection_revision) = 56 AND substr(connection_revision, 1, 24) = 'mcp_connection_revision:'
        AND substr(connection_revision, 25) NOT GLOB '*[^0-9a-f]*'
    ),
    secret_revision TEXT CHECK (
        secret_revision IS NULL OR (length(secret_revision) = 32
        AND secret_revision NOT GLOB '*[^0-9a-f]*')
    ),
    service_description TEXT CHECK (service_description IS NULL OR length(service_description) <= 8192),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
    data_sharing_policy TEXT CHECK (data_sharing_policy IS NULL OR data_sharing_policy IN ('allow_automatically','review_every_call')),
    unsafe_action_policy TEXT CHECK (unsafe_action_policy IS NULL OR unsafe_action_policy IN ('always_ask','reviewer_may_approve','never_ask')),
    policy_revision INTEGER NOT NULL DEFAULT 0 CHECK (policy_revision >= 0),
    health_status TEXT NOT NULL CHECK (health_status IN ('unknown','healthy','unavailable')),
    auth_status TEXT NOT NULL CHECK (auth_status IN ('none','needs_auth','authenticated','unavailable')),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX mcp_servers_definition ON mcp_servers(mcp_definition_id, mcp_server_id);

CREATE TABLE mcp_tools (
    mcp_tool_id TEXT PRIMARY KEY CHECK (
        length(mcp_tool_id) = 41 AND substr(mcp_tool_id, 1, 9) = 'mcp_tool:'
        AND substr(mcp_tool_id, 10) NOT GLOB '*[^0-9a-f]*'
    ),
    mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 256),
    description TEXT CHECK (description IS NULL OR length(CAST(description AS BLOB)) <= 8192),
    input_schema_json TEXT NOT NULL CHECK (
        json_valid(input_schema_json) AND json_type(input_schema_json) = 'object'
        AND length(CAST(input_schema_json AS BLOB)) <= 262144
    ),
    output_schema_json TEXT CHECK (
        output_schema_json IS NULL OR (json_valid(output_schema_json)
        AND json_type(output_schema_json) = 'object'
        AND length(CAST(output_schema_json AS BLOB)) <= 262144)
    ),
    annotations_json TEXT NOT NULL CHECK (
        json_valid(annotations_json) AND json_type(annotations_json) = 'object'
        AND length(CAST(annotations_json AS BLOB)) <= 16384
    ),
    source_revision TEXT NOT NULL CHECK (length(source_revision) = 64 AND source_revision = lower(source_revision)),
    read_only INTEGER CHECK (read_only IS NULL OR read_only IN (0,1)),
    read_only_source TEXT CHECK (read_only_source IS NULL OR read_only_source IN ('annotation','safe_default','human')),
    idempotent INTEGER CHECK (idempotent IS NULL OR idempotent IN (0,1)),
    idempotent_source TEXT CHECK (idempotent_source IS NULL OR idempotent_source IN ('annotation','safe_default','human')),
    destructive INTEGER CHECK (destructive IS NULL OR destructive IN (0,1)),
    destructive_source TEXT CHECK (destructive_source IS NULL OR destructive_source IN ('annotation','safe_default','human')),
    open_world INTEGER CHECK (open_world IS NULL OR open_world IN (0,1)),
    open_world_source TEXT CHECK (open_world_source IS NULL OR open_world_source IN ('annotation','safe_default','human')),
    status TEXT NOT NULL CHECK (status IN ('ready','defaulted','disabled')),
    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(mcp_server_id, name),
    CHECK ((read_only IS NULL) = (read_only_source IS NULL)),
    CHECK ((idempotent IS NULL) = (idempotent_source IS NULL)),
    CHECK ((destructive IS NULL) = (destructive_source IS NULL)),
    CHECK ((open_world IS NULL) = (open_world_source IS NULL))
) STRICT;
CREATE INDEX mcp_tools_server ON mcp_tools(mcp_server_id, name);

CREATE TABLE mcp_oauth_attempts (
    attempt_id TEXT PRIMARY KEY CHECK (
        length(attempt_id) = 42 AND substr(attempt_id, 1, 10) = 'mcp_oauth:'
        AND substr(attempt_id, 11) NOT GLOB '*[^0-9a-f]*'
    ),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id = 'human:local'),
    mcp_server_id TEXT REFERENCES mcp_servers(mcp_server_id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN ('waiting_for_user','completed','failed')),
    failure_code TEXT CHECK (failure_code IS NULL OR length(failure_code) BETWEEN 1 AND 128),
    result_mcp_server_id TEXT REFERENCES mcp_servers(mcp_server_id) ON DELETE SET NULL,
    expires_at_ms INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX mcp_oauth_attempts_expiry ON mcp_oauth_attempts(expires_at_ms);
`
