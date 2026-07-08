# Current Frontend Contract

This contract keeps the first frontend aligned with the current Rust runtime.
It should be updated whenever the GraphQL contract, runtime events, embedded
store schema, or route model changes.

The current route source supports chat at `/`, Settings at `/settings`, and
nested Settings sections including `/settings/memory`. `/memory` redirects to
`/settings/memory`. `/memory/graph` is not a current route.

## Current Sources

| Source | Current authority |
| --- | --- |
| GraphQL client API | Local status, onboarding, provider auth, chat startup, chat turns, transcript items, MCP settings, web-tool settings, memory service settings, daemon errors |
| Daemon web server | Local React shell, GraphQL HTTP, and GraphQL WebSocket subscription endpoints |
| `A2uiCard` payloads | Structured cards inside the chat stream when backed by current runtime behavior |
| `config.yaml` and environment-derived config | Provider/model/default setup state; secrets remain outside SQLite |
| `NoemaPaths` and store config | Noema home, runtime directory, SQLite database path, and Mnemosyne data/runtime paths |
| SQLite store | Concrete object rows, conversations, conversation turns/items, provider accounts, MCP setup/calibration state, approvals, auxiliary preferences, and memory service settings/status |
| Local Mnemosyne | Durable memory truth, extraction, updates, and memory search indexes |

## Frontend Code Organization

Noema-owned React product components should live one component per file.
Component folders may contain pure `.ts` helpers, shared type files, and nearby
tests. The web UI foundation is Astryx with a Noema-owned Neutral-derived theme
and StyleX for Noema-specific layout and state styling.

## Addressable Route Matrix

Routes describe states implemented by `crates/noema-core/web/src/app/routes.ts`
and TanStack Router file routes. Unknown browser paths fall back to the chat
home route. They do not imply primary navigation priority.

| Route | Label | Backing | Capability | Status |
| --- | --- | --- | --- | --- |
| `/` | Home | setup health, local service state, primary conversation | route to the durable primary conversation when ready; show guided readiness state if blocked | Current |
| `/memory` | Memory redirect | route model | redirects to `/settings/memory` | Current |
| `/settings` | Settings default | GraphQL settings read models | route-derived Settings utility surface; defaults to Agents | Current |
| `/settings/agents` | Settings / Agents | agent metadata and model preference options from GraphQL | registered-agent list and model preference editor | Current |
| `/settings/tools/web` | Settings / Web | provider capability bindings and auxiliary summarizer preference | first-party `web.search`/`web.fetch` status and model preference | Current |
| `/settings/tools/mcps` | Settings / MCPs | MCP server metadata from GraphQL | MCP setup, authentication, calibration, and delete flows where implemented | Current |
| `/settings/safety/approvals` | Settings / Approvals | MCP approval read models | pending MCP approval checkpoints | Current limited |
| `/settings/safety/identities` | Settings / Trusted identities | trusted identity selectors from GraphQL | selector rows used to resolve tool-result ownership | Current |
| `/settings/safety/usage` | Settings / Usage | auxiliary model preferences from GraphQL | progress-audit model preference | Current |
| `/settings/system/providers` | Settings / Providers | provider account metadata from GraphQL | provider metadata, auth method, readiness, and safe error state | Current |
| `/settings/memory` | Settings / Memory | GraphQL `memorySettings`, `saveMemoryServiceSettings`, and `checkMemoryService` | view managed/external Mnemosyne status, configure external endpoint and extraction model preference, and check readiness | Current |

Future route groups:

- `/memory/:id`, `/memory/review`, and richer memory browsing backed by Mnemosyne
  visibility APIs.
- `/inspect`, `/inspect/context-graph`, `/inspect/context-packets`.
- `/threads`, `/threads/:id`.
- `/workspaces`, `/workspaces/:id`.
- `/projects`, `/projects/:id`.
- `/tasks`, `/tasks/:id`.
- `/runs`, `/runs/:id`.
- `/approvals`.
- `/tools`, `/tools/:capabilityId`.
- `/governance/grants`.
- `/governance/policy-simulator`.
- `/audit`.
- `/exports`, `/restore`.
- `/agents`, `/agents/:id`.

Future routes may appear as disabled rows only when doing so helps explain why a
feature is unavailable. Disabled rows must not present mutation controls.

## Chat Stream Contract

Inputs:

- Optional initial prompt.
- Optional model override.
- Current working directory or project hint when available.

Events to render:

- User text.
- Assistant text.
- Generic activity notice with status: started, completed, failed.
- Tool call and tool result markers for runtime tool execution.
- Structured cards when `A2uiCard` payloads are backed by current runtime
  behavior.
- Error notices.
- Turn completion.

Current behavior:

- The current web frontend uses Noema's GraphQL client API. Queries provide
  scoped read models, mutations execute explicit Noema commands, and
  subscriptions stream conversation and activity events.
- Static assets are served over ordinary HTTP; product state and product actions
  go through GraphQL.
- The web home chat loads `human:local.primary_conversation_id`.
- Durable chat history is reconstructed from SQLite-backed
  `conversation_items`.
- Daemon runtime state is live coordination state only. After restart, Noema
  reactivates the durable conversation and assembles context from SQLite plus
  provider-independent runtime state.
- Provider runtime ids are not part of the current product contract.
- `agent_status` is live coordination state and is not replayed as transcript
  history.
- `conversation_items` include user text, assistant text, durable activity rows,
  A2UI cards, tool calls/results, approvals, and meaningful errors.

## Memory Frontend Contract

Current memory UX is Settings plus a top-level memory list:

- `/settings/memory` shows Mnemosyne mode, readiness, and the model preference used
  for memory extraction.
- External-mode base URL edits are live for the next `search_memory` call.
- Managed mode is read-only in the current slice because the managed Mnemosyne child
  process is started by the runtime host at startup.
- `/memory` lists Mnemosyne-backed human memories through Noema Core GraphQL.
- There is no `/memory/graph` route and no React Flow graph browser in the
  current slice.
- Transcript memory markers and `/remember` are intentionally absent for now.
- `search_memory` remains model-visible as an explicit tool-only recall path;
  automatic pre-turn memory injection is not part of the frontend contract.

## Setup Health Read Model

Setup health is a blocking pre-chat readiness state, not the product home. Once
setup is healthy enough, `/` should show the chat home.

Checklist:

```text
Local folder
  -> Assistant connection
  -> Local service
  -> First chat
```

Default happy path:

1. Create local folder.
2. Check Codex sign-in.
3. Start Noema.
4. Start chat.
5. Send `Say hello and tell me Noema is working.`

Show:

- Local folder status, with the full path redacted by default outside
  owner/admin reveal.
- Config file existence and assistant connection status.
- Whether config was initialized by defaults.
- SQLite store availability/readiness state.
- Mnemosyne readiness state in Settings > Memory.
- Local service reachable/unreachable.
- Provider readiness in beginner language: connected, not connected, timed out,
  or error.

## Inspection Boundaries

Context graph browsing is a future owner/admin-only surface. The first slice
must not expose a graph browser or Noema-owned memory claim tables. Richer
Mnemosyne-backed visibility should avoid leaking private memory existence through
normal-user text and should keep graph details out of default chat.
