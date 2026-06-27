# Canonical Postgres Schema

## Purpose

Postgres is Noema's canonical structured store for the always-on,
self-hosted personal server. Durable object-owned documents, attachments, and
artifacts remain on the filesystem; derived indexes, caches, and temporary
state remain rebuildable under `system/`.

In containerized and development configuration, Postgres stores its physical
database files under `${NOEMA_HOME:-$HOME/.noema}/db/postgres`.

## Schema Authority

The implemented schema is bootstrapped by
[`crates/noema-core/src/memory_persistence/postgres_schema.rs`](../crates/noema-core/src/memory_persistence/postgres_schema.rs)
during the pre-stable phase. Schema changes may rewrite that bootstrap file
directly until Noema has a stable compatibility policy. Migrations should be
introduced only when that stable compatibility policy exists.

Concrete object rows are the canonical structured state. Actor/principal,
governable scope, provenance source, and transcript item are interfaces
implemented by concrete objects rather than universal parent tables. Postgres
therefore stores typed object references such as `owner_object_type` and
`owner_object_id` where polymorphic behavior crosses concrete objects.

## Implemented Tables

Current bootstrap tables:

- `schema_migrations`: bootstrap/version marker for the pre-stable schema.
- `humans`, `agents`, `tools`: concrete actor and capability objects.
  `humans.primary_conversation_id` references
  `conversations.conversation_id` and stores the default home chat for each
  human.
- `conversations`, `conversation_turns`, `conversation_items`: durable chat
  and transcript state. `conversations.conversation_id` is the stable Noema
  conversation identifier; `conversation_items` is the canonical transcript
  item table for current chat history.
- `memory_items`: durable memory records with lifecycle, sensitivity,
  authority, retrieval policy, and generated Postgres full-text search vector.
- `entities`, `relationships`, `memory_subjects`, `memory_participants`:
  scoped context graph primitives backed by memory authority.
- `memory_retrieval_purpose_rules`, `memory_retrieval_object_links`: typed
  retrieval policy and trusted object-link gates.
- `object_access_grants`: allow/deny grants over concrete object refs.
- `object_provenance_edges`: provenance links between concrete objects.
- `object_events`: structured audit/event records for object changes.
- `object_links`: general typed links between concrete objects.
- `context_packets`, `context_packet_memory_edges`,
  `context_packet_omissions`: persisted context assembly manifests and
  redacted omission/audit records.
- `memory_use_records`: runtime records of memory retrieval, inclusion, and
  use.

The bootstrap also creates indexes for owner lookups, conversation replay,
memory policy filtering, Postgres full-text search, graph traversal,
provenance, object links, context packets, omissions, and memory-use records.

## Source-Of-Truth Split

| Data | Source of truth |
| --- | --- |
| Structured humans, agents, tools, conversations, transcript items, memory, entities, relationships, provenance, grants, context packets, memory-use records, links, and audit/object events | Postgres |
| Human-authored docs, imported files, attachments, and durable artifacts | Object-owned filesystem folders |
| Derived search/vector state, caches, temporary files, and rebuildable projections | `system/` |
| Inspection into database-backed state | Chat/work drill-ins, advanced inspection, and explicit export tools |
