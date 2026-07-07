# Supermemory And SQLite Reset Design

## Summary

Noema will remove its in-house memory engine, remove embedded SurrealDB, and
replace them with two clearer boundaries:

- SQLite is the canonical structured store for Noema-owned application state.
- A local Supermemory instance is the memory engine and graph owner.

This is a clean pre-V1 reset. Noema will not migrate SurrealDB data, preserve
old graph-memory tables, or keep compatibility paths for the existing memory
claim pipeline.

## Goals

- Replace embedded SurrealDB entirely with SQLite.
- Remove Noema's graph-claim memory engine, predicate catalog, canonicalizer,
  extraction pipeline, retrieval packet persistence, and graph inspection API.
- Default to a Noema-managed local Supermemory service, while allowing advanced
  users to point Noema at an external or separately managed Supermemory URL.
- Keep `search_memory` as the only model-visible recall path in the first
  slice.
- Add Settings > Memory as the configuration and status surface for
  Supermemory.
- Remove `/remember`, automatic memory markers, and the current `/memory/graph`
  surface for this slice.
- Keep secrets out of SQLite.
- Reduce build complexity and compile time by removing SurrealDB and the
  in-house graph store.

## Non-Goals

- No SurrealDB-to-SQLite migration.
- No local mirror of Supermemory's graph.
- No Noema-owned memory extraction, contradiction resolution, predicate review,
  or graph visualization.
- No automatic pre-turn memory injection.
- No transcript markers for automatic memory capture or explicit memory writes.
- No user-facing Supermemory graph browser in the first slice.

## Architecture

Noema's canonical structured store becomes SQLite at:

```text
${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3
```

SQLite owns Noema application state:

- Humans and agents.
- Provider accounts and capability bindings.
- Agent and auxiliary model preferences.
- MCP servers, discovered tools, calibrations, trusted identities, and approval
  requests.
- Conversations, turns, transcript items, and context summaries.
- Memory service configuration, readiness state, and ingest job bookkeeping.

SQLite does not own memory truth. It must not recreate the SurrealDB
`entities`, `predicates`, `predicate_proposals`, `claims`, graph edge, evidence,
or retrieval packet tables.

Supermemory becomes a separate memory subsystem. In managed mode, Noema starts a
`supermemory-server` child process with data under:

```text
${NOEMA_HOME:-$HOME/.noema}/supermemory/data
```

In external mode, Noema never starts a process. It stores the external
`base_url`, checks readiness, and uses the same client boundary.

The Rust store layer should remain a repository facade. Existing higher-level
callers should keep stable method names where useful, such as `get_agent`,
conversation append/page methods, provider account methods, model preference
methods, and MCP repository methods. The implementation behind those methods
switches from SurrealDB queries to SQLite statements.

Use `rusqlite` for the SQLite implementation. Avoid adding an async SQL
abstraction unless a later design establishes a concrete need that outweighs
the build-time cost.

## Supermemory Configuration

Noema has one memory service configuration record:

- mode: managed or external.
- base URL.
- status: not configured, starting, ready, unavailable, or auth error.
- selected model backend/provider account for Supermemory extraction.
- selected model profile and reasoning effort where the backend supports it.
- last checked timestamp.
- sanitized last error code and message.

Secrets never live in SQLite. API keys, generated local credentials, and
external Supermemory credentials live in Noema-owned private files under:

```text
${NOEMA_HOME:-$HOME/.noema}/supermemory/secrets/
```

Use the existing secret-file pattern from provider and MCP setup code inside
that dedicated Supermemory secrets directory.

Managed mode starts Supermemory with:

- `SUPERMEMORY_DATA_DIR` pointing at Noema's Supermemory data directory.
- `PORT` or `SUPERMEMORY_PORT` set to the configured port.
- LLM provider environment variables derived from Settings > Memory.

Supermemory model configuration is separate from Noema's chat model selection.
Settings may default it from available provider accounts, but the saved memory
model preference is its own setting because extraction quality, cost, and
latency differ from chat generation.

## Memory Flow

Noema removes provider `memory_proposals[]` from its chat response contract.
Providers no longer propose Noema-owned memories, and the runtime no longer
parses, validates, canonicalizes, or persists provider memory proposals.

Completed conversation turns are submitted to Supermemory in the background.
The first slice does not surface transcript markers for this ingestion. A local
`memory_ingest_jobs` table records submission status, timestamps, and error
codes for diagnostics, but it must not copy Supermemory memory truth or graph
relationships into SQLite.

The `/remember` command is removed. Text such as `/remember I prefer concise
answers` is ordinary chat input. Explicit memory write affordances can return
later as Supermemory-native UI or tool flows.

`search_memory` remains the only model-visible recall path. It keeps the
current trusted-runtime posture:

- The model may request `query`, `scope_ids`, `purpose`, and `limit`.
- Noema validates requested scopes against trusted active scopes.
- Noema maps trusted Noema scopes to Supermemory container tags.
- Noema searches Supermemory and returns safe memory results to the model.
- If Supermemory is unavailable, Noema returns a failed tool result rather than
  injecting fallback memory.

No automatic pre-turn profile or memory injection ships in this slice.

## Scope And Container Tags

Noema maps scopes to deterministic Supermemory container tags. Tags must follow
Supermemory's allowed pattern of alphanumeric characters, underscores, hyphens,
and colons.

Default tags:

```text
human:local
conversation:<conversation_id>
project:<project_id>
workspace:<workspace_id>
agent:<agent_id>
```

If a Noema ID contains unsupported characters, Noema sanitizes it with the same
stable path/segment discipline used elsewhere, while preserving a deterministic
mapping.

Supermemory v4 search centers on one `containerTag`. For a `search_memory` call
with multiple trusted scopes, Noema runs bounded parallel searches over the
allowed tags, merges results, sorts by Supermemory similarity and stable ID, and
truncates to the requested limit.

For normal chat, the default trusted candidate tags are:

- `human:local`.
- `conversation:<current conversation id>`.
- `project:<current project id>` when a trusted project scope is available.

Noema does not allow model-provided text, fuzzy entity names, or arbitrary
scope strings to unlock additional container tags.

## GraphQL API

GraphQL exposes the new Memory settings and service readiness boundary:

- Query current Memory settings.
- Query service status/readiness.
- Update managed/external mode.
- Update external base URL.
- Save Supermemory model preference using the existing provider/model option
  shapes.
- Check connection.
- Start or retry the managed service.
- Reset managed local Supermemory data behind explicit destructive
  confirmation.

GraphQL removes the old Noema graph-memory APIs:

- `memoryClaims`
- `memoryClaim`
- `memoryGraph`
- `memoryPredicateProposals`
- `memoryPredicateProposal`

Frontend generated GraphQL schema and operation types must be regenerated.

## Frontend

Add a top-level Settings section named Memory. It should follow the existing
Settings route and component pattern, using this route:

```text
/settings/memory
```

Settings > Memory belongs near Agents because memory is a core subsystem, not a
safety approval policy.

The page shows:

- Mode selector: managed local or external URL.
- Service status.
- Managed data location, redacted by default with a technical-details reveal.
- External base URL when in external mode.
- Memory extraction model selector following the existing
  `ModelPreferenceSelect` pattern.
- Actions to check connection, start/retry managed service, switch mode, and
  reset local Supermemory data with destructive confirmation.

Remove or hide the current `/memory/graph` route and graph drill-in. The primary
`/memory` product surface is removed from primary navigation in this slice.
Opening `/memory` should redirect to `/settings/memory` until a
Supermemory-backed memory browsing surface exists.

The chat transcript no longer renders memory-save, memory-proposal, or memory
update markers in this slice.

## Lifecycle And Error Handling

Noema startup should not require Supermemory to be ready. SQLite startup remains
required for Noema. If Supermemory fails, Noema still boots and chat can run.
`search_memory` returns a failed tool result when invoked, and Settings > Memory
shows the failure.

Managed lifecycle:

1. Resolve configured port and data paths.
2. Prepare the Supermemory data and secret directories.
3. Start `supermemory-server` as a child process.
4. Poll readiness before marking Memory ready.
5. Capture sanitized startup errors in SQLite status fields.
6. Write raw diagnostic details to `errors.log`.
7. Stop the child process during Noema shutdown.

External lifecycle:

1. Validate the base URL and credential presence.
2. Poll readiness.
3. Store sanitized readiness status.
4. Use the same Supermemory HTTP client for search and ingestion.

Errors returned through GraphQL and tool results must be sanitized. Raw
Supermemory output, credentials, and provider details belong only in local
diagnostics.

## SQLite Schema

SQLite schema version starts at 1 and assumes a clean reset.

Required tables:

- `schema_state`
- `humans`
- `agents`
- `agent_runtime_preferences`
- `auxiliary_model_preferences`
- `provider_accounts`
- `provider_capability_bindings`
- `mcp_servers`
- `mcp_tools`
- `tool_calibrations`
- `trusted_identity_selectors`
- `approval_requests`
- `conversations`
- `conversation_turns`
- `conversation_items`
- `conversation_context_summaries`
- `memory_service_settings`
- `memory_ingest_jobs`

The schema should use ordinary relational constraints, unique indexes, and JSON
text columns where Noema already treats payloads as flexible metadata. Since the
project is pre-V1, incompatible schema changes may rewrite the schema directly
until a real migration policy is needed.

If an old SurrealDB directory exists under `NOEMA_HOME/db`, Noema ignores it.
Technical status may report that obsolete SurrealDB files exist, but startup
does not migrate or delete them automatically.

## Implementation Units

1. Replace SurrealDB with SQLite for non-memory store behavior.
2. Add Supermemory client, config, readiness, and lifecycle management.
3. Remove Noema graph-memory extraction and claim persistence; adapt
   `search_memory` to Supermemory.
4. Update GraphQL and web Settings > Memory.
5. Remove or hide obsolete memory graph surfaces.
6. Update docs and current context.

## Testing

Repository tests should cover SQLite behavior for:

- Conversations, turns, transcript item append, and transcript paging.
- Provider accounts and provider capability bindings.
- Agent and auxiliary model preferences.
- MCP servers, tools, calibrations, trusted identities, and approvals.
- Context summaries.
- Memory service settings and status.

Runtime tests should prove:

- Provider responses no longer require or parse `memory_proposals[]`.
- `/remember` is ordinary chat text.
- `search_memory` validates trusted scopes and maps only allowed scopes to
  Supermemory container tags.
- Supermemory unavailability returns a failed tool result.
- Completed-turn ingestion failures do not fail chat turns.

Supermemory client tests should use a fake HTTP server rather than the real
binary.

GraphQL tests should cover:

- Memory settings queries and mutations.
- Readiness/status states.
- Removal of old graph claim APIs.

Frontend validation should run the existing web generation, lint, and build
commands after the Settings > Memory route lands.

Full Rust validation remains:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Frontend validation for the UI slice remains:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

## Open Follow-Ups

- Rebuild memory visibility from Supermemory-native signals.
- Decide whether `/memory` returns as a Supermemory-backed browse/review
  surface.
- Design automatic pre-turn recall or profile injection separately.
- Design export/backup semantics for SQLite plus Supermemory data.
- Decide whether Supermemory inferred-memory review belongs in Noema's UI.
