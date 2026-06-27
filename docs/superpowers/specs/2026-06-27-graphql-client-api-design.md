# GraphQL Client API Design

## Status

Approved design direction for making GraphQL the only first-party client-facing
API for Noema. No implementation has been done in this spec. The next step is
an implementation plan.

## Problem

Noema is moving from a local web chat slice toward a full workspace operating
system with multiple clients, conversations, users, agents, tasks, boards,
approvals, tools, memory, and inspection surfaces.

The current web UI uses a small native HTTP and WebSocket protocol owned by the
daemon. That is fine for the first chat slice, but it will become awkward as
desktop, mobile, CLI, workspace, and task-management clients need overlapping
views of the same graph-shaped product state.

Noema needs one coherent client API that can:

- Serve web, CLI, desktop, and mobile clients.
- Keep frontend and backend operation types generated from one schema.
- Support live updates for conversations, agent activity, tasks, boards, memory,
  and onboarding state.
- Support limited offline command queues for clients that temporarily lose
  connectivity.
- Preserve Noema's core boundaries for policy, audit, provenance, memory,
  commands, and durable persistence.

## Decisions

- Make GraphQL the only first-party client-facing API for product state and
  product actions.
- Serve the local web UI static assets separately, but have the web app use
  GraphQL for all Noema state and mutations.
- Have the CLI use GraphQL for product reads and actions instead of calling
  daemon-specific product protocols.
- Keep GraphQL resolvers thin. Queries call Noema read models, mutations call
  explicit Noema command handlers, and subscriptions wrap Noema's durable event
  stream.
- Keep Noema's Rust domain, runtime, repository, policy, provenance, and audit
  layers testable without GraphQL.
- Treat GraphQL subscriptions as the client-facing realtime transport, not as
  the underlying sync model.
- Add a durable cursor/event model underneath subscriptions so clients can catch
  up after sleep, reconnect, or mobile backgrounding.
- Support a limited offline outbox by replaying client commands with
  idempotency keys and observed versions or cursors.
- Leave room for true offline-first replication later by keeping read models,
  commands, and events separate from the GraphQL transport.

## Non-Goals

- Do not move domain behavior into GraphQL resolvers.
- Do not let generic object patch mutations replace explicit Noema commands.
- Do not expose direct table-shaped GraphQL access to canonical Postgres state.
- Do not use GraphQL subscriptions as the only record of realtime changes.
- Do not require true offline-first conflict-free replication in the current
  slice.
- Do not make GraphQL the interface used by internal Rust modules to talk to
  each other.
- Do not expose secrets, private memory, raw policy internals, or owner/admin
  inspection state without the same authorization and redaction rules enforced
  by the core.

## Architecture

The client-facing architecture is:

```text
Web / CLI / desktop / mobile clients
  -> GraphQL endpoint
  -> Noema read models, commands, and subscriptions
  -> policy, permissions, provenance, audit, and validation
  -> Postgres and durable object-owned files
  -> durable event stream
```

GraphQL is a facade over Noema's core. The schema gives clients a coherent,
typed way to ask for the data and operations they need, but the resolver layer
does not own business behavior.

Queries return scoped read models. A read model is allowed to be shaped for a
screen or workflow, but it must be backed by canonical Postgres state or derived
rebuildable projections.

Mutations execute commands. A mutation such as `moveTask`, `sendConversationTurn`,
or `approveToolUse` maps to one explicit command handler. The command handler
performs validation, policy checks, persistence, audit/provenance writes, and
event emission.

Subscriptions expose durable event streams. A subscription may filter events for
a conversation, workspace, task board, memory review queue, or local status
surface, but missed-event catch-up is owned by the event/cursor layer beneath
GraphQL.

## API Surface

The first GraphQL slice should cover the behavior that exists today:

- Local status and readiness.
- Onboarding status.
- Provider authentication attempts.
- Primary conversation start and replay.
- Sending a chat turn.
- Transcript item updates.
- Agent status updates.

Future slices should add GraphQL fields and commands for:

- Conversation lists and thread switching.
- Memory list, detail, review, and access preview.
- Workspace and project read models.
- Task boards, task details, dependencies, ordering, and assignees.
- Tool access requests, approvals, denials, and audit trails.
- Agent activity, delegation, and handoff views.
- Export, restore, rebuild, and privileged inspection workflows.

The intended mutation style is command-shaped:

```graphql
mutation MoveTask($input: MoveTaskInput!) {
  moveTask(input: $input) {
    outcome
    task {
      id
      status
      position
    }
    rejectedReason
    nextCursor
  }
}
```

The avoided style is generic patching:

```graphql
mutation UpdateTask($input: UpdateTaskPatch!) {
  updateTask(input: $input) {
    task {
      id
    }
  }
}
```

Generic patching hides the domain action, makes audit weaker, and encourages
policy logic to drift into resolvers.

## Realtime And Sync

Client-facing realtime should use GraphQL subscriptions. The subscription API
should expose scoped streams such as:

- Conversation events since cursor.
- Workspace events since cursor.
- Task board events since cursor.
- Agent activity events since cursor.
- Onboarding or provider-auth status changes.

The underlying event model should provide:

- Durable ordered event ids or cursors.
- Scope filters for conversation, workspace, human, agent, task board, memory,
  and owner/admin inspection surfaces.
- Catch-up after reconnect by resubscribing with the last observed cursor.
- Idempotent delivery semantics at the client reducer layer.
- Redaction and authorization before event payloads leave the server.

Subscriptions deliver events; they do not define truth. After reconnect, a
client may combine event catch-up with read-model refetches when the server
reports that the cursor is too old, invalid, or no longer authorized.

## Offline Queue

Noema should support limited offline queues for clients that temporarily lose
connectivity, such as a mobile client on a subway.

Offline clients may queue command-shaped GraphQL mutations for low-risk actions:

- Send a chat message.
- Create a task.
- Edit task title or notes.
- Move a task on a board.
- Add a task comment.
- Toggle a checklist item.
- Save a local draft or submit a memory-review choice when safe.

Each queued command should include:

- A client command id or idempotency key.
- The actor identity.
- The intended scope.
- The observed cursor, version, or entity revision when the user acted.
- The command payload.
- The local creation time.

On reconnect, the client replays queued commands through GraphQL. The server
returns an explicit outcome:

- Accepted.
- Rejected with a stable reason.
- Accepted with a server-adjusted result.
- Requires fresh review because policy, permissions, or target state changed.

The server remains authoritative. Offline replay must not bypass current
permissions, policy, redaction, approval, or audit requirements.

## Type Generation

The backend owns the GraphQL schema. Clients own operation documents. Generated
types should come from the schema plus the operations each client actually uses.

For the web UI:

- Use Apollo Client as the React GraphQL client.
- Use Apollo's normalized cache as a client projection cache for the
  authenticated human/session.
- Generate TypeScript operation types and React hooks from GraphQL documents.
- Prefer operation-specific types over importing broad schema object types.
- Keep generated files clearly marked and excluded from hand edits.
- Keep reveal/collapse/redaction presentation state in React state, not in
  normalized cache identity.
- Clear or recreate the Apollo cache when the authenticated human/session
  changes.

For the CLI:

- Use the same GraphQL schema and operation documents where practical.
- Generate or centralize typed Rust client helpers for CLI operations.
- Avoid maintaining a separate daemon protocol with parallel product types.

This replaces the current pattern where Rust protocol enums generate TypeScript
bindings for the web-specific daemon protocol. Rust types may still exist for
internal commands, read models, and events, but client compatibility is defined
by the GraphQL schema and operation contracts.

## Components

- GraphQL server: owns schema, query resolvers, mutation resolvers, subscription
  resolvers, context/auth wiring, and error mapping.
- Apollo web client: owns React query/mutation/subscription hooks and normalized
  projection cache for the active authenticated human/session.
- Noema command layer: executes explicit state-changing operations.
- Read model layer: returns scoped projections for chat, memory, workspace,
  task, setup, and inspection screens.
- Durable event layer: records ordered events and exposes cursor-based catch-up.
- Subscription bridge: adapts durable events into GraphQL subscription payloads.
- Client outbox: stores queued command operations while offline and replays them
  with idempotency keys.
- Generated client types: TypeScript for web, Rust helpers or operation types
  for CLI.

## Migration Path

1. Add GraphQL alongside the current web protocol for the existing chat and
   onboarding slice.
2. Move the web UI to GraphQL queries, mutations, and subscriptions for existing
   behavior.
3. Move CLI product operations to GraphQL while keeping local lifecycle commands
   such as starting the daemon as direct local commands when needed.
4. Retire the native web product HTTP and WebSocket protocol after parity is
   proven.
5. Add future task, workspace, memory, tool, approval, and inspection surfaces
   directly through GraphQL.

Static web assets and external protocol callbacks, such as provider auth
callbacks, may remain non-GraphQL HTTP endpoints. They are not product-state
APIs for first-party clients.

## Error Handling And Edge Cases

GraphQL errors should not leak private object existence, raw policy details,
secrets, provider identifiers, or privileged inspection information. Error
payloads should use stable client-safe codes plus plain-language messages.

Partial query data must respect redaction. If a field is not authorized, the
schema should prefer an explicit redacted or unavailable shape over accidental
null ambiguity where the difference matters to trust.

Subscription reconnect should handle:

- Cursor accepted and events replayed.
- Cursor too old, requiring read-model refetch.
- Cursor unauthorized because permissions changed.
- Scope deleted, archived, or no longer visible.
- Client command accepted but projected event delayed.

Offline outbox replay should handle:

- Duplicate command ids.
- Commands against stale task positions.
- Messages queued for conversations that were archived or permission-revoked.
- Board moves that conflict with newer ordering changes.
- Approval decisions whose requested action changed while offline.

## Testing

Backend tests should cover:

- GraphQL queries call read models rather than duplicating repository behavior.
- GraphQL mutations call command handlers and emit audit/provenance/events.
- Resolver tests for redaction, authorization, and safe error mapping.
- Subscription catch-up from a durable cursor.
- Reconnect flows for accepted, stale, and unauthorized cursors.
- Idempotent mutation replay with duplicate client command ids.
- Rejection and adjusted-result outcomes for stale offline commands.

Frontend tests should cover:

- Generated GraphQL TypeScript types are up to date.
- Web onboarding uses GraphQL queries and mutations.
- Web chat start, replay, send, transcript stream, and agent status use
  GraphQL operations.
- Offline queue UI states for queued, sending, accepted, rejected, and needs
  review outcomes.

CLI tests should cover:

- CLI product reads and actions use GraphQL client helpers.
- CLI output remains stable when GraphQL returns redacted or unavailable fields.
- CLI errors map from GraphQL client-safe codes to useful terminal messages.

Validation should include the existing Rust checks and the web type generation,
lint, and build commands for affected frontend work.

## Documentation Updates

Update Noema docs so they state:

- GraphQL is the first-party client API for web, CLI, desktop, and mobile.
- Internal Rust modules use Noema command, read model, repository, runtime,
  policy, provenance, and event interfaces directly.
- Realtime clients consume GraphQL subscriptions backed by durable event cursors.
- Limited offline queueing is supported through command-shaped mutations with
  idempotency keys.
- Direct daemon product protocols are transitional and should not be extended
  for new product surfaces.
