# MCP Server Settings Config Design

## Summary

The first MCP Settings page shows MCP server metadata, but it does not let a
user add a server. Noema should add a real "Add MCP server" flow that stores
connection configuration now, including secrets, fetches the MCP tool
list/schema during create, and preserves the existing control-plane rule that
tools remain disabled until calibration is completed.

The structured store remains the source of truth for non-secret MCP server
metadata. Secret material is stored on disk under the Noema home, similar to
provider-owned Codex token storage, and is never returned through GraphQL.

## Goals

- Let a user add an MCP server from Settings.
- Support actual V1 connection configuration for `stdio` and `http_sse`.
- Store MCP secrets on disk under `${NOEMA_HOME}/mcp/<mcp_server_id>/`.
- Keep the SurrealDB `mcp_servers.safe_config` field free of secret values.
- Return only metadata and secret-presence indicators to the web UI.
- Initialize the MCP connection during create and fetch `tools/list`
  metadata, including input/output schemas where available.
- Persist discovered tools immediately, but keep newly added servers and tools
  disabled until calibration is complete.
- Keep tool invocation, setup probe calls, auth flows, and tool calibration as
  follow-on actions, not implicit side effects of saving config.

## Non-Goals

- Do not invoke MCP tools or perform setup probe calls.
- Do not execute a stdio MCP beyond initialize and metadata-only `tools/list`.
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

Add
`createMcpServer(input: GraphqlCreateMcpServerInput!): GraphqlMcpServerSetupResult`.

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

`GraphqlMcpServerSetupResult`:

- `server: GraphqlMcpServer!`
- `discoveryStatus: String!`
- `discoveredToolCount: Int!`
- `discoveryError: String`

`discoveryStatus` values:

- `discovered`: initialize and `tools/list` succeeded, and discovered tool
  metadata was persisted.
- `unavailable`: config was saved, but the metadata-only MCP connection failed.
- `malformed`: config was saved, but the MCP metadata response could not be
  parsed or validated.

Create returns a result instead of failing the whole mutation for transport or
metadata errors after local validation and secret writes succeed. That keeps
the user's entered config durable and lets Settings show the server with a
clear discovery status. Validation errors still fail before persistence.

## Metadata Discovery On Create

After validation and secret storage, the backend initializes the MCP connection
using the saved safe config plus disk-backed secrets and calls metadata-only
discovery:

1. Initialize the MCP connection.
2. Call `tools/list`.
3. Persist each discovered tool name, description, input schema, optional
   output schema, annotations, and metadata fingerprint in `mcp_tools`.
4. Mark the server health `healthy` when discovery succeeds.
5. Mark the server health `unavailable` when connection fails.
6. Leave existing or newly discovered tools uncalibrated and agent-invisible.

The create flow must not call any MCP tool. It also must not infer effective
policy from server-provided metadata. Schemas and annotations are setup
material for calibration only.

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
- Saving config fetches the MCP tool list/schema but does not enable tools;
  calibration is still required.

On success, the form clears and `McpSettings` refetches. On failure, the form
shows the GraphQL validation error in place. If local validation succeeds but
metadata discovery fails, the form clears, the server appears with unavailable
health, and Settings shows the discovery error as a non-secret status message.
Existing server cards remain metadata-only and must not show raw config or
secret values.

## Runtime Boundary

Saving config and fetching tool metadata does not make a tool available to
agents. The created server starts as:

- `enabled = false`
- `health_status = healthy` when metadata discovery succeeds, otherwise
  `unavailable`
- `auth_status = none`
- `tool_count = <discovered tool count>`

Calibration remains mandatory before the Capability Gateway allows any
agent-proposed MCP call to execute.

## Testing

Backend tests:

- Creating a `stdio` server persists safe config and writes secret env keys to
  disk.
- Creating an `http_sse` server persists safe config and writes secret header
  keys to disk.
- Creating a server initializes the MCP connection, calls `tools/list`, persists
  input/output schemas and annotations, and returns `discoveryStatus =
  discovered` when metadata discovery succeeds.
- Metadata discovery failure returns a setup result with `discoveryStatus =
  unavailable` or `malformed` without exposing secrets.
- GraphQL rejects invalid transport strings, mismatched transport configs,
  empty commands, invalid URLs, non-string env/header maps, and secret-shaped
  keys in safe config.
- GraphQL responses never include secret values.

Frontend tests:

- The MCP empty state includes an Add action.
- The form renders the correct fields for `stdio` and `http_sse`.
- `KEY=value` parsing rejects duplicate and malformed lines.
- Submit calls `createMcpServer`, refetches settings on success, and shows
  metadata discovery status when discovery fails after config persistence.

Validation:

- `cargo fmt --all --check`
- Focused MCP store and GraphQL tests.
- `bun run gen:types`
- `bun run lint`
- `bun run build`
