# Setup 2 — Notion MCP connection

Updated: 2026-09-10

Status: Pass with normal reauthentication on expiry

This setup ran through the live Go Noema server over the private GraphQL
socket. It used a hosted synthetic MCP service. It did not access a real
Notion workspace or change a page.

## Setup contract

| Item | Observed value |
| --- | --- |
| Backend | Go development server; authenticated socket request returned `{"state":"authenticated"}` |
| Fixture | `2026-09-08-gmail-v1-notion-mcp-v8`; [health](https://noema.kevinpei.com/__pa-replay/health) |
| Server | `mcp_server:1e041eac4708761774cdcc984cb5a666` |
| Transport | Streamable HTTP |
| Server card | [MCP server card](https://noema.kevinpei.com/__pa-replay/.well-known/mcp.json) |
| Endpoint | `https://noema.kevinpei.com/__pa-replay/notion/mcp` |
| OAuth scope | `notion.read` |
| Discovered tools | `notion-get-self`, `notion-search`, `notion-fetch` |
| Final health | `healthy`; auth `authenticated`; pending tools `0` |
| Noema behavior revision | `1add8c9e` |

The agent read the server card and tool catalog, generated the MCP setup
proposal, and the operator accepted the exact revision. Reauthentication used
the normal Noema human sign-in path. No transient OAuth URL, code, token, or
client secret is recorded here.

## Acceptance checks

| Check | Result | Evidence |
| --- | --- | --- |
| First read after setup | Pass | `notion-get-self`, search, and fetch returned the connected synthetic workspace and launch records. |
| Delegated reuse | Pass | `pa-setup-notion-20260910-delegated-reuse-3`; Task `task:9d6e866a4922569faa8fe459c24dda9b` finished and the reviewer approved it. |
| Pagination | Pass | `pa-setup-notion-20260910-pagination-1`; the child Task `task:abfbf22b9359bab8a7255ea68ba23bad` followed all three search pages and accounted for all six results. |
| Nested blocks | Pass | `pa-setup-notion-20260910-nested-1` fetched `notion-page-launch`, its details block, and the child analytics block. |
| Empty search | Pass | `pa-setup-notion-20260910-empty-1` returned a clear empty result for an exact unmatched phrase. |
| Account boundary | Pass | `pa-setup-notion-20260910-account-boundary-2` could not fetch the account-B private page and did not reconnect or change scope. |
| Invalid cursor | Pass | `pa-setup-notion-20260910-invalid-cursor-2` returned a structured `invalid_cursor` failure, kept the service healthy, and completed a fresh search. |
| Token expiry | Pass with normal reauthentication | `pa-setup-notion-20260910-expiry-1` opened `mcp_auth:820f54482446a5a5b19b4f72b56c184c`. The operator completed the normal synthetic account-A sign-in, after which the read completed and the server returned to `healthy` / `authenticated`. |
| Read-only boundary | Pass | The catalog exposes only the three read-only tools. No page or block mutation occurred. |

The fixture request trace is available at
[fixture/requests](https://noema.kevinpei.com/__pa-replay/fixture/requests).
The trace is synthetic and contains no OAuth URLs, codes, or credential
values.

## Limits

The MCP server, OAuth flow, workspace, and records are emulated. This setup
does not prove real Notion consent, hosted-server availability, or every
Notion API object. It proves the selected MCP transport, discovery, tool
schemas, account boundary, recovery, pagination, and nested-read behavior.
