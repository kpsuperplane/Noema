# Current Frontend Contract

This contract keeps the first frontend aligned with the current Rust runtime.
It should be updated whenever the GraphQL contract, runtime events, embedded
store schema, or route model changes.

The current route source supports chat at `/`, Settings at `/settings`, and
nested Settings sections including `/settings/memory`. `/memory` is the native
memory root and `/memory/<article-path>` addresses child articles. `/memory/graph`
is not a current route.

The shell keeps the same primary navbar on every route. Desktop keeps every
destination label visible, while mobile keeps only the active destination label
beside its icon. The navbar occupies the chrome band above the white content
deck. Settings is an active navbar destination whose labeled section menu begins
below that band on the left side of the shell; it does not add a separate back
control. The browser document owns vertical scrolling for every primary surface
at every viewport width, while bounded sidebars, dialogs, and detail panes retain
independent scrolling when their composition requires it.

## Current Sources

| Source | Current authority |
| --- | --- |
| GraphQL client API | Local status, onboarding, provider auth, chat startup, chat turns, transcript items, MCP settings, web-tool settings, native memory, daemon errors |
| Daemon web server | Local React shell, GraphQL HTTP, and GraphQL WebSocket subscription endpoints |
| `A2uiCard` payloads | Structured cards inside the chat stream when backed by current runtime behavior |
| `config.yaml` and environment-derived config | Provider/model/default setup state; secrets remain outside SQLite |
| `NoemaPaths` and store config | Noema home, runtime directory, SQLite database path, and native memory paths |
| SQLite store | Concrete object rows, conversations, conversation turns/items, provider accounts, MCP provider/tool policy state, approvals, auxiliary preferences, and the Memory model preference |
| Native Markdown memory | Durable local-human memory prose, metadata, provenance, hierarchy, and consolidation state |

## Frontend Code Organization

Noema-owned React product components should live one component per file.
Component folders may contain pure `.ts` helpers, shared type files, and nearby
tests. The web UI foundation is Astryx with a Noema-owned Neutral-derived theme
and StyleX for Noema-specific layout and state styling.

## Addressable Route Matrix

Routes describe states implemented by `apps/web/src/app/routes.ts`
and TanStack Router file routes. Unknown browser paths fall back to the chat
home route. They do not imply primary navigation priority.

| Route | Label | Backing | Capability | Status |
| --- | --- | --- | --- | --- |
| `/` | Home | setup health, local service state, primary conversation | route to the durable primary conversation when ready; show guided readiness state if blocked | Current |
| `/memory` | Memory | GraphQL native-memory read/update model | inspect the local-human tree and request one background update | Current |
| `/memory/$` | Memory article | GraphQL native-memory page read model | load a child article from its canonical Markdown path | Current |
| `/settings` | Settings default | GraphQL settings read models | route-derived Settings utility surface; defaults to Agents | Current |
| `/settings/agents` | Settings / Agents | agent metadata and model preference options from GraphQL | registered-agent list and model preference editor | Current |
| `/settings/tools/web` | Settings / Web | provider capability bindings and auxiliary summarizer preference | first-party `web.search`/`web.fetch` status and model preference | Current |
| `/settings/tools/mcps` | Settings / MCPs | MCP server metadata from GraphQL | MCP setup, authentication, provider policy, advanced tool behavior, and delete flows | Current |
| `/settings/safety/approvals` | Settings / Approvals | MCP approval read models | pending MCP approval checkpoints | Current limited |
| `/settings/safety/identities` | Settings / Trusted identities | trusted identity selectors from GraphQL | selector rows used to resolve tool-result ownership | Current |
| `/settings/safety/usage` | Settings / Usage | auxiliary model preferences from GraphQL | progress-audit model preference | Current |
| `/settings/system/providers` | Settings / Providers | provider account metadata from GraphQL | provider metadata, auth method, readiness, and safe error state | Current |
| `/settings/memory` | Settings / Memory | GraphQL `memorySettings` and `saveMemoryModelPreference` | choose the model used for native memory updates | Current |

Future route groups:

- `/settings/safety/privacy`, backed by the governed-action reviewer model
  preference and privacy posture once that runtime authority is implemented.
- `/memory/review` and richer provenance review flows.
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
- The browser GraphQL transport retries both WebSocket connections and
  terminally failed subscription operations indefinitely with capped backoff.
  While disconnected, chat becomes read-only and reports a reconnecting state;
  after a fresh subscription is acknowledged it refetches active GraphQL reads
  and backfills the latest durable transcript page before resuming normal use.
- Provider runtime ids are not part of the current product contract.
- `agent_status` is live coordination state and is not replayed as transcript
  history.
- `conversation_items` include user text, assistant text, durable activity rows,
  A2UI cards, tool calls/results, approvals, and meaningful errors.
- Every surfaced task reference uses the same compact live task control.
  The control keeps its task icon and expresses live state with a separate
  semantic icon; the full state phrase remains in its accessible label.
  A contiguous run of task-creation references immediately following same-turn
  assistant text is embedded inside that message bubble at its stored
  transcript position. Creation references without that concrete relationship
  remain standalone.
- Completed, waiting, and recovery task notifications pair deterministically
  with the last assistant message carrying the same notification id. Their
  stored reference is embedded inside that message using the shared control
  instead of rendered as a separate transcript row. Notification delivery does
  not insert a completion reference when the same task's creation reference is
  within the preceding two durable message bubbles; other notification
  references remain durable and are surfaced.
- Foreground task-tool results do not create task references. Notification
  delivery is the sole producer after task creation, so routine task mutations
  do not add transcript cards.
- Task executors may emit one concise user-visible commentary sentence before a
  non-terminal tool batch; it is rendered as an assistant bubble while the
  adjacent tool calls remain grouped and individually expandable. Hidden
  provider reasoning is never part of this transcript contract.
- When one provider phase has exactly one tool call, its commentary may label
  the compact tool marker. Consecutive tool calls from the same agent collapse
  to the newest call that has not completed; once every call is terminal, the
  summary retains the call it was already showing. Expanding the group restores
  each individual marker and its exact tool identity. Compact markers show the
  status, a structured web-tool icon when applicable, and the primary call
  subject; targets and results remain in disclosure.
- Collapsed tool-group labels and live task-stage badge labels use the shared
  critically damped rolling-text response for state changes. Words replace
  vertically with a `micro` response while their container width uses
  `standard`. Collapsed transcripts represent one or many
  consecutive calls through the same cluster path and preserve the first call's
  render identity. A one-call cluster sends disclosure directly to that call;
  later calls activate the group disclosure without remounting the summary.
  Tool-stack and activity disclosure use interruptible `standard` presence;
  outgoing controls become inert immediately. Reduced-motion preferences apply
  final geometry and text immediately.
- Finite frontend motion uses the three shared critically damped `micro`,
  `standard`, and `surface` presets. The shell and detail rail use `surface`;
  transcript arrival, disclosure, replacement, and owned scrolling use
  `standard`; compact feedback uses `micro`. CSS consumers and Astryx duration
  tokens use sampled counterparts of the same responses.
- Transcript arrival animates inside the virtualized row so TanStack Virtual's
  positioning transform remains authoritative. Bottom-following targets the
  live clamped bottom with a monotonic spring and cancels immediately when the
  human takes scroll ownership. Spinners, typing dots, progress indicators, and
  glimmers remain time-based signals and stop under reduced motion.
- The shared primary navbar remains in the shell chrome above the white content
  deck, so every routed body starts at the content boundary without an in-deck
  title row or transcript overlap.

## Memory Frontend Contract

Current memory UX is Settings plus a top-level native article surface:

- `/settings/memory` selects the model used for background memory updates.
- `/memory` renders canonical Markdown as a full-width Wikipedia-style reading
  surface with article typography, numbered citations, related-article links,
  and no in-article navigation chrome. Child articles use their filesystem-derived
  path at `/memory/<article-path>` so direct loads and browser history resolve
  the same page. The active Memory navbar control presents the current page as
  a compact trigger; opening it reveals a filesystem-derived hierarchy dropdown
  for client-side navigation across all pages on every viewport.
- Each page exposes a validated Lucide icon key. The Memory navigation rail,
  active article title, and Related Articles cards render that icon as
  supplementary decoration while retaining the title as the accessible label.
- The Memory article groups `Update` and the lower-emphasis `Refresh icons`
  action in one compact editorial maintenance notice near its title. The notice
  exposes pending-message count, last-updated time, operation-specific retry,
  and failure state without rendering implementation filenames.
- One initial tree query is kept current by authoritative GraphQL subscription
  snapshots after source arrivals and update transitions; the page does not poll.
- Both maintenance actions are disabled while the one server-owned job is
  queued or running; failed jobs retain their completed checkpoint for retry.
- There is no `/memory/graph` route and no React Flow graph browser in the
  current slice.
- Transcript memory markers and `/remember` are intentionally absent for now.
- The bounded root page enters ordinary turns automatically. `read_memory_page`
  and `search_memory` retrieve deeper pages for the current continuation.

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
- Native memory storage readiness through the normal local-service status.
- Local service reachable/unreachable.
- Provider readiness in beginner language: connected, not connected, timed out,
  or error.

## Inspection Boundaries

Context graph browsing is a future owner/admin-only surface. The first slice
does not expose a graph browser, private-memory existence, or Noema-owned claim
tables; exact page provenance remains available without entering default chat.
