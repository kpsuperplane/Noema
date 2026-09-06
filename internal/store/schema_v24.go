package store

// schemaV24SQL adds rebuildable public indexes for filesystem adapter authorities.
const schemaV24SQL = `
CREATE TABLE adapter_definitions (
 semantic_digest TEXT PRIMARY KEY CHECK(length(semantic_digest)=64 AND semantic_digest=lower(semantic_digest)),
 definition_id TEXT NOT NULL CHECK(length(trim(definition_id)) BETWEEN 1 AND 96),
 adapter_id TEXT NOT NULL CHECK(length(trim(adapter_id)) BETWEEN 1 AND 96),
 definition_revision TEXT NOT NULL CHECK(length(trim(definition_revision)) BETWEEN 1 AND 96),
 source_reference TEXT NOT NULL CHECK(length(source_reference) BETWEEN 1 AND 4096),
 display_name TEXT NOT NULL CHECK(length(display_name)<=256),
 reviewed INTEGER NOT NULL CHECK(reviewed IN (0,1)),
 superseded INTEGER NOT NULL CHECK(superseded IN (0,1)),
 operation_count INTEGER NOT NULL CHECK(operation_count BETWEEN 1 AND 256),
 manifest_relative_path TEXT NOT NULL CHECK(manifest_relative_path GLOB 'adapters/definitions/*/manifest.json'),
 updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX adapter_definitions_identity ON adapter_definitions(definition_id,reviewed,superseded,semantic_digest);

CREATE TABLE adapter_connections (
 connection_id TEXT PRIMARY KEY CHECK(length(connection_id)=32 AND connection_id=lower(connection_id)),
 connection_slug TEXT NOT NULL CHECK(length(trim(connection_slug)) BETWEEN 1 AND 96),
 connection_label TEXT NOT NULL CHECK(length(connection_label)<=256),
 semantic_digest TEXT NOT NULL REFERENCES adapter_definitions(semantic_digest) ON DELETE RESTRICT,
 status TEXT NOT NULL CHECK(status IN ('active','suspended')),
 connection_revision INTEGER NOT NULL CHECK(connection_revision>0),
 policy_revision INTEGER NOT NULL CHECK(policy_revision>0),
 data_sharing_policy TEXT CHECK(data_sharing_policy IS NULL OR data_sharing_policy IN ('allow_automatically','review_every_call')),
 unsafe_action_policy TEXT CHECK(unsafe_action_policy IS NULL OR unsafe_action_policy IN ('always_ask','reviewer_may_approve','never_ask')),
 allowed_operations_json TEXT NOT NULL CHECK(json_valid(allowed_operations_json) AND json_type(allowed_operations_json)='array'),
 descriptor_relative_path TEXT NOT NULL CHECK(descriptor_relative_path GLOB 'adapters/connections/*/connection.json'),
 updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX adapter_connections_definition ON adapter_connections(semantic_digest,status,connection_id);
`
