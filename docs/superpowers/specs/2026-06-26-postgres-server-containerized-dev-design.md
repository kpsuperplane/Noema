# Postgres Server And Containerized Development Design

## Status

Approved direction for the next architecture pivot. No implementation has been
done in this spec. The next step is an implementation plan.

## Problem

Noema has been described and implemented around a local SQLite database. That
matched an early local daemon shape, but the product direction is now clearer:
Noema should default to an always-on personal server that supports multiple
devices from the start.

The current docs also overuse "V1" as a boundary. That makes the architecture
sound like a temporary small version rather than the real system being built
incrementally. The project is pre-stable, not toy-shaped.

Noema needs a storage and development posture that matches this direction:

- A canonical structured store suitable for an always-on server.
- Ergonomic local development that does not require hand-built database setup.
- A clear persistence boundary that keeps application code maintainable.
- Honest docs that describe implemented slices without weakening target
  architecture.

## Decisions

- Postgres is the canonical structured store for the current architecture.
- Noema's default deployment target is an always-on personal server.
- Desktop and mobile clients connect to the server rather than owning canonical
  structured state.
- Docker Compose should be the default development environment for the server,
  Postgres, and supporting services needed to run Noema locally.
- SQLx and explicit repositories are the preferred persistence implementation.
- A full ORM is not part of the current design.
- SQLite is not a supported canonical backend in this architecture. It may
  remain only as temporary legacy code during migration, test-only scaffolding,
  or a future separately-designed embedded mode.
- "V1" language should be replaced with "current slice", "initial slice",
  "implemented now", "target architecture", or "future slice" as appropriate.
- "Pre-V1" should become "pre-stable": schema and APIs may be rewritten until
  durable user-data compatibility is explicitly introduced.

## Non-Goals

- Do not support user-selectable Postgres vs. SQLite storage in the current
  architecture.
- Do not introduce a storage abstraction whose primary purpose is database
  portability.
- Do not preserve compatibility with local SQLite development databases.
- Do not containerize production deployment in this spec. The immediate goal is
  an ergonomic development environment. Production packaging can reuse the same
  pieces later.
- Do not add Kubernetes, cloud provisioning, or managed database assumptions.

## Architecture

Noema should be organized around an always-on server process:

```text
Desktop/mobile/web clients
        |
        v
Noema server
  - WebSocket/API sessions
  - agent runtime
  - schedulers and task workers
  - tool execution coordination
  - memory retrieval and extraction
  - governance, permissions, audit
        |
        v
Postgres canonical structured store
        |
        +--> object-owned files and artifacts
        +--> derived indexes/cache/tmp state
```

The server owns coordination and consistency. Clients may cache UI state, but
they do not become independent canonical stores.

Postgres owns structured rows for:

- humans
- agents
- tools
- conversations
- conversation turns
- conversation items
- memory items
- entities and relationships
- provenance edges
- tasks, grants, policies, and audit events as those slices are implemented

Durable object-owned files remain outside the database. Postgres stores
metadata, lifecycle, ownership, provenance, and file/object references.

## Database Design

Use Postgres-specific capabilities directly where they fit Noema:

- `uuid` or text IDs with explicit stable prefixes where product semantics need
  readable object references.
- `timestamptz` for persisted timestamps.
- `jsonb` for structured payloads and metadata.
- Foreign keys for concrete references where the target table is known.
- Typed object reference columns for polymorphic contracts such as actor,
  governable object, provenance source, and transcript item.
- Partial indexes for lifecycle states such as active vs. soft-deleted rows.
- Postgres full-text search for durable text retrieval.
- Row locking, `FOR UPDATE SKIP LOCKED`, or advisory locks for workers and
  serialized tool execution when useful.

The concrete object model remains valid:

- Concrete object tables are canonical structured state.
- Actor/principal, governable scope, provenance source, and transcript item are
  interfaces implemented by concrete objects, not universal parent tables.
- `conversation_items` remains the durable transcript/action stream.
- Provenance can link directly to `conversation_items`.
- Soft deletion should preserve auditability while redacting or invalidating
  dependent rows according to provenance and retention rules.

## Persistence Layer

Application code should not scatter SQL across runtime modules. It should call
repository APIs that express domain operations:

- create or load humans, agents, tools, and conversations
- append, update, soft-delete, and replay conversation items
- start, complete, fail, interrupt, or cancel turns
- set conversation-level `agent_status`
- save memory items and provenance edges
- retrieve memories under policy
- record tool calls, tool results, approvals, and audit events

Use SQLx inside these repository modules:

- SQL should be explicit and readable.
- Query result structs should be typed.
- Transactions should be passed deliberately through multi-step operations.
- Repository methods should own validation for typed object references that
  Postgres cannot enforce with a single foreign key.
- Tests should exercise repository behavior against a real Postgres instance.

This boundary gives Noema maintainability without pretending Postgres is an
interchangeable detail. If a future embedded mode is designed, it should get its
own adapter behind a deliberately chosen subset of repository behavior.

## Why Not A Full ORM

An ORM would reduce some boilerplate, but it would not remove the important
database differences Noema relies on: full-text search, JSON indexing, locking,
transactions, partial indexes, and worker coordination. Since Postgres is the
canonical database, hiding these details would mostly make the hard behavior
less visible.

SQLx plus repositories keeps the database behavior explicit while still giving
typed decoding, async pooling, transaction support, and a clean boundary for the
rest of the application.

## Containerized Development

Development should be one-command ergonomic. The target setup is:

```text
docker compose up
```

or a thin project command that wraps it.

The Compose stack should include:

- `postgres`: canonical development database.
- `noema-server`: Rust server/daemon with source mounted for iterative
  development or built from the workspace.
- `web`: frontend dev server if it remains separate during development.
- Optional supporting services only when a slice needs them.

The container environment should provide:

- a stable Postgres URL
- database initialization for a fresh checkout
- persistent named volumes for database data
- separate volumes or bind mounts for Noema object-owned files
- health checks so server startup waits for Postgres readiness
- clear ports for the web app, server API, and WebSocket endpoint

Local development should still support running pieces directly on the host when
that is useful, but the documented happy path should be containerized.

## Configuration

Noema configuration should move from "path to local SQLite file" to "server
storage connection":

- `NOEMA_DATABASE_URL` for Postgres.
- `NOEMA_HOME` for object-owned files, artifacts, cache, and local config.
- Server bind address and public base URL settings.
- Provider credentials passed through environment variables or secrets.

The server should validate required configuration at startup and return clear
errors for missing database access, failed migrations/schema setup, or unwritable
object storage paths.

## Documentation Language

Docs should stop using "V1" as a product boundary. Replace it with:

- "current slice" for behavior implemented now.
- "initial slice" for the next narrow vertical slice.
- "target architecture" for the intended long-term shape.
- "future slice" for intentionally deferred work.
- "pre-stable" for the compatibility posture.

The compatibility rule should be:

```text
Noema is pre-stable. Schema and APIs may be rewritten directly until durable
user-data compatibility is explicitly introduced.
```

Docs that describe SQLite as canonical should be updated to Postgres. SQLite
docs can be removed, renamed as legacy notes, or replaced with Postgres schema
docs during implementation.

## Data Flow

Conversation flow:

1. Client sends user input to the always-on server.
2. Server writes a `conversation_items` row for the user input.
3. Server starts a `conversation_turn`.
4. Server updates conversation-level `agent_status`.
5. Server writes durable action rows for tool calls, tool results, approvals,
   A2UI cards, errors, memory proposals, and final assistant responses.
6. Server updates rows in place when the same canonical action changes status.
7. Client receives live events and can replay durable history from Postgres.

Memory provenance flow:

1. Memory extraction links memory candidates or saved memories to source
   `conversation_items`.
2. Memory use records link retrieved memory back to the turn or response that
   used it.
3. Soft-deleting a source conversation item redacts the item and updates or
   invalidates memories that depend solely on that provenance.

Worker flow:

1. Server or worker process claims runnable jobs using Postgres transactions and
   row locks.
2. Claimed work writes durable audit/tool/task rows.
3. Failed or interrupted work stores enough state for inspection and recovery.

## Testing

Testing should include:

- repository tests against real Postgres
- schema bootstrap tests
- transaction rollback tests
- conversation replay tests
- typed object reference validation tests
- soft-delete/provenance propagation tests
- worker/job locking tests once workers are introduced
- container smoke tests that start Postgres and the server

Unit tests can still use isolated pure Rust components where no database is
needed. Database behavior should not be faked for repository correctness.

## Migration Approach

Because Noema is pre-stable, this can be a direct rewrite:

1. Add Postgres dependencies and configuration.
2. Add a Postgres schema/bootstrap path for the current concrete object model.
3. Replace SQLite repository modules with Postgres/SQLx repositories.
4. Update daemon/runtime code to depend on the new repositories.
5. Update CLI inspection and web replay paths.
6. Add Docker Compose and local development docs.
7. Remove or quarantine SQLite code and docs after parity is reached.
8. Update current context and project docs to the always-on/Postgres posture.

No data migration from SQLite is required unless explicitly requested later.

## Risks

- Postgres increases setup complexity. Containerized development is the
  mitigation.
- SQLx compile-time query checking can require database availability during
  builds if enabled. The implementation plan should decide whether to use
  online checking, offline metadata, or runtime-checked queries for early
  development.
- The current repository tests are SQLite-oriented and will need restructuring.
- Some docs refer heavily to V1 and SQLite; cleanup should be broad enough to
  avoid contradictory guidance.
- Running the whole app in containers may expose filesystem path assumptions
  around `NOEMA_HOME`, bundled frontend assets, and provider credentials.

## Acceptance Criteria

The implementation is acceptable when:

- Postgres is the only canonical structured store described in current docs.
- The app can run locally through a documented Docker Compose path.
- The server can connect to Postgres from configuration.
- The concrete object schema exists in Postgres.
- Conversation history, memory provenance, and replay use Postgres-backed
  repositories.
- Runtime modules do not directly embed persistence SQL outside repository
  boundaries.
- SQLite is not described as the target canonical architecture.
- V1/pre-V1 language is replaced where it describes product or architecture
  posture.
- Validation commands and relevant database-backed tests pass.
