# Concrete Object Chat Persistence Design

## Status

Approved design for a pre-V1 architecture cleanup. No backwards compatibility
is required for existing local development databases, schemas, or chat
persistence paths.

## Problem

Noema currently persists successful chat turns as generic `episodes` and
`messages`, while activity rows, A2UI cards, errors, and tool-like actions are
mostly live WebSocket transcript items. That makes chat history incomplete,
memory provenance indirect, and deletion semantics awkward.

The broader schema also overuses universal tables such as `principals`,
`scopes`, and `episodes`. Those names describe useful behavioral concepts, but
they are not good concrete roots for the product model. A conversation is a
conversation, not an episode-shaped scope. A human, agent, or tool can act as a
principal, but "principal" does not need to be its own root object table.

## Design Principle

Use concrete object tables as the source of truth. Shared concepts are
interfaces or contracts implemented by concrete objects, not universal parent
tables.

Important contracts:

- Actor-capable: an object can own, author, create, delete, grant, or approve.
- Governable: an object owns visibility, retention, proactivity, and permission
  policy.
- Provenance-capable: an object can serve as evidence for another object.
- Transcript-capable: an object can render into conversation/work history.

SQLite cannot enforce polymorphic foreign keys directly. Repository APIs and
tests must validate typed object references.

## Core Object Model

Concrete root tables:

- `humans`: people who own, use, or participate in Noema.
- `agents`: assistants that respond, remember, plan, and act.
- `tools`: callable capabilities and integrations. Tool rows may also author
  output items when a tool performs work.
- `conversations`: durable chat/work threads.
- `conversation_turns`: causal units created by user input or another trigger.
- `conversation_items`: canonical renderable and auditable conversation stream.
- `memory_items`: durable memory records.
- `entities` and `relationships`: semantic graph objects.

Support tables:

- `object_provenance_edges`: typed evidence links between concrete objects.
- `object_access_grants`: typed grants against concrete objects.
- `object_events`: audit/event records against concrete objects.
- `object_links`: typed relationships such as active context, created from,
  relevant to, or required for.
- `context_packets` and `memory_use_records`: retrieval and use audit records,
  updated to reference concrete objects rather than generic scopes.

Tables to remove from active chat persistence:

- `principals`
- `scopes`
- `episodes`
- `messages`

Their useful concepts remain as contracts and typed refs.

## Typed Object References

Shared references use explicit type and id columns:

```text
owner_object_type, owner_object_id
author_object_type, author_object_id
created_by_object_type, created_by_object_id
deleted_by_object_type, deleted_by_object_id
source_object_type, source_object_id
target_object_type, target_object_id
grantee_object_type, grantee_object_id
```

Known initial object types:

```text
human
agent
tool
conversation
conversation_turn
conversation_item
memory_item
entity
relationship
context_packet
```

Repository methods must reject unknown object types and verify referenced rows
exist where practical.

## Conversation Tables

`conversations` stores durable thread identity and lifecycle:

- `conversation_id`
- `title`
- `owner_object_type`, `owner_object_id`
- `primary_human_id`
- `primary_agent_id`
- provider metadata such as provider name, model, provider thread id, and cwd
- lifecycle status such as `active`, `archived`, `deleted`
- live `agent_status`
- retention and deletion metadata
- `created_at`, `updated_at`, `deleted_at`
- `metadata`

`agent_status` is current coordination state, not transcript history or audit
history. It may be stored as the current conversation state for backend/client
coordination, but it should not create `conversation_items` and may be reset or
recomputed on daemon restart. Initial values:

```text
idle
input_received
thinking
tool_running
waiting_for_previous_turn_completion
interrupting
error
```

`conversation_turns` stores causal units:

- `turn_id`
- `conversation_id`
- `trigger_item_id`
- `status`: `input_received`, `running`, `waiting_for_tool`, `interrupted`,
  `completed`, `failed`, `cancelled`
- `started_at`, `completed_at`, `interrupted_at`
- `created_at`, `updated_at`
- `metadata`

`conversation_items` is the canonical transcript/action stream:

- `item_id`
- `conversation_id`
- optional `turn_id`
- optional `parent_item_id`
- `kind`: `user_text`, `assistant_text`, `activity`, `a2ui_card`, `tool_call`,
  `tool_result`, `approval_request`, `approval_result`, `error_notice`, and
  similar semantic item kinds
- `status`: `pending`, `running`, `completed`, `failed`, `cancelled`,
  `interrupted`
- `author_object_type`, `author_object_id`
- `content_text` for readable text
- `payload_json` for structured data
- `created_at`, `updated_at`
- `deleted_at`, `deleted_by_object_type`, `deleted_by_object_id`
- `redacted_at`, `redaction_reason`
- `metadata`

Items are ordered by `created_at` for now. Do not add a separate sequence column
until real ordering issues appear.

## Persistence Boundary

Persist semantic conversation items, not rendering mechanics.

Persist:

- user messages
- final assistant messages
- memory saves, memory extraction rows, memory proposals, and memory-use rows
- A2UI cards that represent real objects, proposals, actions, or results
- tool calls and tool results
- approval requests and decisions
- denials, recoveries, and errors that affected the turn
- failed tool calls if the tool was actually invoked

Do not persist:

- thinking or working placeholders
- spinners
- socket reconnect notices
- optimistic UI state before backend acknowledgement
- streaming chunks as separate rows
- debug traces unless explicitly routed to an audit/debug object

Streaming assistant output should either be buffered until complete or upserted
into one canonical `assistant_text` item.

Long-running semantic items should update the same row in place when they are
the same canonical action. For example, a `tool_call` begins as `running` and is
updated to `completed` or `failed` with result payload when done. Add child
items only when sub-events are meaningful audit records in their own right.

## Runtime Flow

On conversation start:

1. Create or resume a `conversations` row.
2. Store provider metadata and current live `agent_status`.
3. Return the durable `conversation_id` to clients.

On user input:

1. Create a new `conversation_turns` row.
2. Immediately create a `conversation_items` row with `kind = user_text`.
3. Emit the item to the client.
4. Set `agent_status` according to runtime state.

As the agent works:

1. Persist semantic tool, memory, card, approval, and error items when the
   corresponding action actually happens.
2. Update item status and payload as actions complete.
3. Persist final assistant text as one `assistant_text` item.
4. Mark the turn completed, failed, cancelled, or interrupted.

## Interruptions

Incoming user input creates a new `turn_id` immediately.

If the previous turn is still thinking, it can be interrupted without durable
history beyond status updates. If the previous turn has an in-flight tool call,
that tool call is allowed to finish and update its existing
`conversation_item`. The interrupted turn may not start additional tool calls
or emit a final assistant response after interruption.

The new turn can be visible immediately, but tool execution for the new turn
waits until the prior in-flight tool call settles. This serializes tool side
effects while preserving a responsive transcript.

## Memory Provenance

Memory provenance points directly to concrete source objects, usually
`conversation_item` rows:

```text
object_provenance_edges:
  edge_id
  target_object_type = "memory_item"
  target_object_id
  source_object_type = "conversation_item"
  source_object_id
  relation
  evidence_excerpt
  created_by_object_type
  created_by_object_id
  created_at
  metadata
```

Memory creation and provenance linking must be transactional. Noema should not
create unprovenanced memory because a provenance insert failed.

Explicit memory saves should link to the originating `user_text`
`conversation_item`. Extracted memories should link to the relevant source
items, usually the user item, assistant item, or memory extraction/card item
that produced the memory.

## Deletion And Redaction

Deletion is soft delete with readable-content redaction.

Deleting a `conversation_item` must:

1. Set deletion metadata on the item.
2. Clear or redact readable `content_text`.
3. Clear or redact sensitive fields in `payload_json`.
4. Exclude the item from normal replay and retrieval.
5. Update or remove provenance edges from the deleted item.
6. Soft-delete/redact any memory whose only provenance is the deleted item.
7. Keep memories active when they still have other provenance.

The cascade should run in one transaction. Future UI can preview the cascade,
but backend behavior should be deterministic.

Audit-mode queries may show that an item existed and was deleted, but must not
show redacted text unless an explicit recovery/export policy later permits it.

## Error Handling

If user item persistence fails before provider execution, fail fast and surface
a recoverable client error.

If assistant output is produced but cannot be persisted, do not claim the turn
is durable. Surface an error and set `agent_status = error`.

If memory creation succeeds but provenance linking fails, roll back the whole
memory transaction.

Late provider output from an interrupted turn is ignored unless it corresponds
to a tool item that was already in flight and allowed to finish.

If tool completion cannot be persisted, mark the tool item or turn failed where
possible and notify the client.

## Repository Boundaries

Split persistence into focused modules rather than continuing to grow large
catch-all files.

Suggested modules:

- `objects`: typed object ref validation and helpers.
- `schema`: bootstrap schema.
- `conversation_repository`: conversations, turns, items, replay, status, and
  deletion.
- `memory_repository`: memory CRUD, memory policy, memory search.
- `provenance_repository`: object provenance edges and cascade handling.
- `context_repository`: context packets and memory-use records.
- `graph_repository`: semantic entities and relationships.

The exact file names can follow local Rust module style during planning, but
the implementation must keep files below the project comfort threshold where
practical.

## Documentation Updates

Update:

- `docs/project.md`
- `docs/context/current.md`
- `docs/sqlite.md`
- `docs/memory.md`
- relevant `docs/frontend/*` files
- harness docs where they describe scopes, episodes, principals, or transcript
  persistence

Docs should describe `principal`, `scope-like`, and `episode/source-like` as
interfaces/contracts implemented by concrete object tables, not as required
universal tables.

## Validation

Repository tests:

- typed object refs reject unknown object types
- refs are validated where practical
- `conversation_items.turn_id` belongs to the same conversation
- replay orders by `created_at` and excludes deleted items by default
- semantic item persistence accepts supported kinds and rejects runtime-only
  placeholders
- soft delete redacts item content
- source deletion cascades to sole-provenance memories
- memories with other provenance survive source deletion
- memory creation and provenance linking are transactional

Runtime tests:

- conversation start creates durable conversation row
- user turn creates durable turn and user item before provider execution
- assistant response creates one durable assistant item
- explicit memory save links provenance to a conversation item
- provider memory proposals and A2UI cards persist as conversation items
- provider failure leaves clear turn and agent status
- interrupted turn cannot start more tools
- in-flight tool item can settle after interruption
- new turn waits for previous in-flight tool completion before tool execution

Frontend/protocol tests:

- WebSocket emits persisted items and live `agent_status`
- refresh/replay reconstructs semantic history from `conversation_items`
- runtime-only statuses are not replayed as transcript rows

Validation commands remain the project defaults:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Frontend validation is required for protocol/UI changes:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

## Implementation Phases

1. Schema and docs rewrite.
2. Repository split and typed object-ref helpers.
3. Conversation runtime migration.
4. Memory provenance migration.
5. Frontend replay and live `agent_status`.
6. Validation and cleanup of obsolete chat paths.

Each phase should leave the repo in a coherent state, with tests adjusted to the
new object model. Because this is pre-V1, remove old paths instead of preserving
compatibility unless the user explicitly asks otherwise.

## Out Of Scope

- Backwards-compatible migrations for existing development databases.
- Full project/task/workspace/tool orchestration beyond the object model needed
  for chat, memory provenance, and typed refs.
- A polished deletion-preview UI. The backend cascade is required; UI preview
  can follow.
- A separate sequence column for conversation ordering.
