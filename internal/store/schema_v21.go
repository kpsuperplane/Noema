package store

// schemaV21SQL binds a suspended MCP call to its reviewed action claim.
const schemaV21SQL = `
CREATE UNIQUE INDEX action_requests_identity ON action_requests(action_id);
ALTER TABLE mcp_auth_requests ADD COLUMN action_request_id TEXT
 REFERENCES action_requests(action_id) ON DELETE RESTRICT;
CREATE UNIQUE INDEX mcp_auth_requests_action ON mcp_auth_requests(action_request_id)
 WHERE action_request_id IS NOT NULL;
`
