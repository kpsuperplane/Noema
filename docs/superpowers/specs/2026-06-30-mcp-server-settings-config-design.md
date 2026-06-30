# MCP Server Settings Config Design

## Summary

The first MCP Settings page shows MCP server metadata, but it does not let a
user add a server. Noema should add a real "Add MCP server" flow that stores
connection configuration now, including secrets, while preserving the existing
control-plane rule that tools remain disabled until metadata discovery and
calibration are completed.

The structured store remains the source of truth for non-secret MCP server
metadata. Secret material is stored on disk under the Noema home, similar to
provider-owned Codex token storage, and is never returned through GraphQL.

## Goals

- Let a user add an MCP server from Settings.
- Support actual V1 connection configuration for `stdio` and `http_sse`.
- Store MCP secrets on disk under `${NOEMA_HOME}/mcp/<mcp_server_id>/`.
- Keep the SurrealDB `mcp_servers.safe_config` field free of secret values.
- Return only metadata and secret-presence indicators to the web UI.
- Create newly added servers disabled with unknown health and no tools.
- Keep metadata discovery, process launch, auth flows, and tool calibration as
  follow-on actions, not implicit side effects of saving config.

## Non-Goals

- Do not invoke MCP tools or perform setup probe calls.
- Do not launch stdio processes from the Settings form.
- Do not implement OAuth or browser auth for remote MCPs in this slice.
- Do not expose saved secret values back to the browser.
- Do not add backwards-compatible migrations for pre-V1 state.

## Config Model

`stdio` config contains:

- `command`: executable name or absolute path.
- `args`: ordered string list.
- `cwd`: optional working directory.
- `env`: non-secret environment variables.
- `secret_env`: secret environment variables stored on disk.

`http_sse` config contains:

- `url`: HTTP or HTTPS endpoint URL.
- `headers`: non-secret headers.
- `secret_headers`: secret headers stored on disk.

GraphQL accepts both safe and secret fields during create. The store persists
only safe fields and secret references in `safe_config`. Runtime launch code can
later merge safe config with disk-backed secrets inside the Noema server
process.

Example persisted safe config:

```json
{
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-github"],
  "cwd": null,
  "env": {
    "GITHUB_OWNER": "example"
  },
  "secret_refs": {
    "env": ["GITHUB_TOKEN"]
  }
}
```

Example disk secret file:

```json
{
  "env": {
    "GITHUB_TOKEN": "..."
  },
  "headers": {}
}
```

## Secret Storage

MCP secret files live at:

```text
${NOEMA_HOME}/mcp/<mcp_server_id>/secrets.json
```

Directory and file requirements:

- The server directory is created with private permissions on Unix.
- The secret file is written with private permissions on Unix.
- Parent directory creation is owned by the Noema server process.
- Secret file reads and writes happen in backend code only.
- GraphQL responses expose booleans or key names, not secret values.

The initial file format is JSON with `env` and `headers` objects. Empty secret
objects are allowed so the runtime can distinguish "no secrets configured" from
"secret storage missing".

## GraphQL API

Add `createMcpServer(input: GraphqlCreateMcpServerInput!): GraphqlMcpServer`.

Input fields:

- `displayName: String!`
- `transportKind: String!`
- `stdio: GraphqlMcpStdioConfigInput`
- `httpSse: GraphqlMcpHttpSseConfigInput`

`GraphqlMcpStdioConfigInput`:

- `command: String!`
- `args: [String!]!`
- `cwd: String`
- `env: JSON`
- `secretEnv: JSON`

`GraphqlMcpHttpSseConfigInput`:

- `url: String!`
- `headers: JSON`
- `secretHeaders: JSON`

Validation rules:

- `displayName` must be non-empty after trimming.
- `transportKind` must be `stdio` or `http_sse`.
- Exactly one transport-specific config must match the selected transport.
- `stdio.command` must be non-empty.
- `http_sse.url` must be `http` or `https`.
- Safe env/header maps must contain string keys and string values.
- Secret env/header maps must contain string keys and string values.
- Safe maps reject secret-shaped keys such as `authorization`, `token`,
  `api_key`, `apikey`, `cookie`, `password`, `credential`, `secret`,
  `private_key`, `session`, and `set-cookie`.

The server id is generated server-side from the display name with a stable MCP
prefix and collision suffix. Users do not type durable ids in the basic UI.

## Settings UX

`/settings/mcps` gets an Add button above the list and in the empty state. The
form appears inline at the top of the pane so users keep context.

Fields:

- Display name.
- Transport segmented control or select: `stdio`, `http_sse`.
- `stdio`: command, args textarea, working directory, non-secret env textarea,
  secret env textarea.
- `http_sse`: URL, non-secret headers textarea, secret headers textarea.

Textareas use a simple `KEY=value` line format for env and headers in the UI.
The frontend converts this to JSON objects before calling GraphQL. Duplicate
keys are rejected client-side before submit.

The form copy should be concise:

- Non-secret fields are stored in the database.
- Secret fields are stored on disk and are not shown again.
- Saving config does not enable tools; calibration is still required.

On success, the form clears and `McpSettings` refetches. On failure, the form
shows the GraphQL validation error in place. Existing server cards remain
metadata-only and must not show raw config or secret values.

## Runtime Boundary

Saving config does not make a tool available to agents. The created server
starts as:

- `enabled = false`
- `health_status = unknown`
- `auth_status = none`
- `tool_count = 0`

Later discovery may use the saved safe config plus disk secrets, list MCP tool
metadata, and write `mcp_tools` records. Calibration remains mandatory before
the Capability Gateway allows any agent-proposed MCP call to execute.

## Testing

Backend tests:

- Creating a `stdio` server persists safe config and writes secret env keys to
  disk.
- Creating an `http_sse` server persists safe config and writes secret header
  keys to disk.
- GraphQL rejects invalid transport strings, mismatched transport configs,
  empty commands, invalid URLs, non-string env/header maps, and secret-shaped
  keys in safe config.
- GraphQL responses never include secret values.

Frontend tests:

- The MCP empty state includes an Add action.
- The form renders the correct fields for `stdio` and `http_sse`.
- `KEY=value` parsing rejects duplicate and malformed lines.
- Submit calls `createMcpServer` and refetches settings on success.

Validation:

- `cargo fmt --all --check`
- Focused MCP store and GraphQL tests.
- `bun run gen:types`
- `bun run lint`
- `bun run build`
