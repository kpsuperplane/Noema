package store

// schemaV39SQL preserves MCP tools while allowing longer provider descriptions.
const schemaV39SQL = `
DROP INDEX mcp_tools_server;
ALTER TABLE mcp_tools RENAME TO mcp_tools_v38;
CREATE TABLE mcp_tools (
 mcp_tool_id TEXT PRIMARY KEY CHECK(length(mcp_tool_id)=41 AND substr(mcp_tool_id,1,9)='mcp_tool:' AND substr(mcp_tool_id,10) NOT GLOB '*[^0-9a-f]*'),
 mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 256), description TEXT CHECK(description IS NULL OR length(CAST(description AS BLOB))<=65536),
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
INSERT INTO mcp_tools SELECT * FROM mcp_tools_v38;
DROP TABLE mcp_tools_v38;
CREATE INDEX mcp_tools_server ON mcp_tools(mcp_server_id,name);
`
