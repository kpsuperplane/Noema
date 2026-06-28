# SurrealDB Graph Memory Store Design

## Status

Approved design direction. No implementation has been done in this spec. The
next step is an implementation plan for replacing Postgres with embedded
SurrealDB as Noema's canonical structured store.

## Problem

Noema's current structured store is Postgres, with memory represented through
`memory_items`, `entities`, `relationships`, provenance tables, access grants,
context packets, and memory-use records. That model has useful pieces, but it
forces graph-native memory, provenance, and policy through many table-shaped
adapters.

The desired product direction is simpler:

- No separate database daemon for the default personal server.
- Memory as graph knowledge, similar to Graphiti's entity/fact-edge model.
- Strict schema and deterministic policy, not opaque or loosely typed memory.
- Provenance preserved without rebuilding broad audit machinery around every
  access.
- Clean local reset behavior during pre-stable development.

SurrealDB is the preferred replacement because it can run embedded inside the
Noema server process, supports schemafull tables, supports graph relation
records, and fits a strict graph memory model better than Postgres or SQLite for
this project.

## Research Summary

Graphiti stores raw source material as episode nodes, entities as graph nodes,
and durable facts as semantic relationship edges with temporal validity and
supporting episodes. Noema should adopt the important part of that model:
memories are claim edges, not separate rows that merely point at graph edges.
Noema does not need Graphiti-style episode nodes for chat because
`conversation_item` records can serve directly as provenance sources.

Other frameworks vary:

- Graphiti uses optional edge-type hints but can derive open predicates.
- LlamaIndex supports open extraction, dynamic schema hints, and strict schema
  extraction.
- Microsoft GraphRAG tends toward open descriptive relationships and typed
  entities.
- Mem0 graph memory links memories through entities without typed relationship
  predicates.

Noema should choose the stricter path: predicates are first-class ontology
records, not arbitrary strings. LLMs may propose predicates and fill out their
metadata, but only promoted predicates participate in ordinary retrieval.

Sources consulted:

- Graphiti repository: <https://github.com/getzep/graphiti>
- Graphiti edge model: <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/edges.py>
- Graphiti edge extraction prompt: <https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/prompts/extract_edges.py>
- SurrealDB Rust embedding docs: <https://surrealdb.com/docs/languages/rust/embedding>
- SurrealDB schemafull table docs: <https://surrealdb.com/docs/surrealql/statements/define/table>
- SurrealDB relation docs: <https://surrealdb.com/docs/surrealql/statements/relate>
- LlamaIndex property graph guide: <https://docs.llamaindex.ai/en/stable/module_guides/indexing/lpg_index_guide/>
- Microsoft GraphRAG prompt tuning docs: <https://microsoft.github.io/graphrag/prompt_tuning/manual_prompt_tuning/>
- Mem0 graph memory docs: <https://docs.mem0.ai/platform/features/graph-memory>

## Goals

- Replace Postgres as the canonical structured store.
- Run the database embedded inside the Noema server process.
- Store embedded database files directly under `NOEMA_HOME/db`.
- Keep all non-server access behind Noema APIs.
- Make durable memory a strict graph claim with provenance.
- Seed a small personal-agent predicate catalog.
- Support reviewed predicate proposals for future expansion.
- Use deterministic memory retrieval policy with a small `use_mode` vocabulary.
- Preserve provenance and conflict resolution without a broad generic audit
  table for every touch.
- Keep object-owned files, provider secrets, attachments, and artifacts on the
  filesystem.

## Non-Goals

- Do not keep Postgres as a supported canonical backend.
- Do not support user-selectable database backends in the current architecture.
- Do not add migrations or compatibility layers for pre-stable local data.
- Do not implement a separate graph database daemon.
- Do not treat summaries, reflections, cron outputs, task state, or artifacts
  as memory unless they are distilled into graph claims.
- Do not expose raw embedded database access to CLI, web, desktop, or mobile
  clients.
- Do not use an LLM to decide whether a claim is retrievable at read time.

## Decisions

- Embedded SurrealDB is the target canonical structured store.
- The Noema server is the only process that opens `NOEMA_HOME/db`.
- The project remains pre-stable: incompatible schema changes may require local
  database teardown and rebuild.
- Use strict SurrealDB schema definitions and Rust repository validation.
- Every durable memory is a graph claim.
- Conversation items are direct provenance sources.
- Provenance is represented as specialized graph-queryable evidence relations.
- Access is derived by deterministic policy, not by attaching a complete grant
  matrix to every claim.
- Replace the current broad purpose vocabulary with a small `use_mode` enum:
  `answer`, `personalize`, `plan`, `act`, `notify`, `inspect`, and `export`.
- Detailed memory-use tables should be replaced by compact retrieval packets
  that record assembled context and redacted omissions.

## Architecture

Noema should organize structured state around an embedded store owned by the
server:

```text
Web, CLI, desktop, mobile clients
        |
        v
Noema API / GraphQL
        |
        v
Noema server process
  - agent runtime
  - GraphQL/API resolvers
  - memory extraction and retrieval
  - deterministic policy engine
  - repository layer
        |
        v
Embedded SurrealDB at NOEMA_HOME/db
        |
        +--> object-owned filesystem docs, imports, attachments, artifacts
        +--> provider secret files under NOEMA_HOME/providers
        +--> rebuildable system indexes/cache/tmp
```

SurrealDB replaces Postgres for structured rows and graph state. The
filesystem remains the source of truth for durable object-owned bytes.

The first implementation should do the full replacement. It may order work so
high-risk SurrealDB validation happens first, but the deliverable is not a
throwaway proof slice.

## Core Components

### Embedded Store Runtime

The store runtime opens embedded SurrealDB at `NOEMA_HOME/db`, selects the
Noema namespace/database, applies strict schema bootstrap, validates schema
version, and exposes a server-internal handle to repositories. Startup should
fail early if the database cannot be opened, schema bootstrap fails, or the
embedded engine version is unsupported.

### Strict Schema Bootstrap

Schema bootstrap should define schemafull tables, relation tables, field
assertions, indexes, and built-in records needed by the current server. During
pre-stable development, bootstrap may rewrite schema directly and instruct the
developer to rebuild local database state when incompatible changes land.

Initial structured tables should include:

- `human`
- `agent`
- `tool`
- `provider_account`
- `conversation`
- `conversation_turn`
- `conversation_item`
- `entity`
- `predicate`
- `predicate_proposal`
- `claim`
- evidence relation tables
- `retrieval_packet`

The exact SurrealQL shape can be finalized during planning, but strictness is a
requirement: invalid entity types, predicate use modes, sensitivities,
lifecycles, and claim statuses should be rejected by schema or repository
validation.

### Repository Layer

Runtime modules should call domain repositories rather than scatter raw
SurrealQL. Repositories should own operations such as:

- create/load humans, agents, tools, provider accounts, and conversations
- append and replay conversation items
- create entities and resolve canonical entity IDs
- read built-in predicates and predicate proposals
- create, reinforce, supersede, dispute, archive, and delete claims
- attach evidence relations
- retrieve claims under policy
- record retrieval packets
- inspect/export graph neighborhoods

This is not a database portability abstraction. It is a domain boundary that
keeps SurrealDB details in one layer.

### Ontology Layer

Predicates are first-class ontology records. A predicate should include:

- stable id and human label
- description
- allowed subject entity types
- allowed object entity types
- optional inverse predicate behavior
- conflict/cardinality policy
- default sensitivity
- allowed `use_mode`s
- proactivity default
- extraction hints and examples
- review policy
- merge/synonym hints

The initial predicate catalog should focus on personal-agent memory, such as
preferences, likes/dislikes, interaction style, tool preferences, project
involvement, relationships between people, and stable personal facts.

Unknown predicates should create `predicate_proposal` records. LLMs may help
fill the full ontology proposal, but claims using unpromoted predicates remain
candidate-only and excluded from ordinary retrieval.

### Graph Memory Layer

Durable memory is a claim. Logically, a claim is an edge:

```text
human:kevin -[likes]-> concept:trains
```

Claims should carry:

- subject entity
- object entity
- predicate
- canonical fact text
- status: `candidate`, `active`, `confirmed`, `disputed`, `superseded`,
  `archived`, or `deleted`
- sensitivity: `public`, `normal`, `private`, `sensitive`, or `secret`
- confidence
- observed and valid time fields
- extraction metadata
- optional claim-level policy overrides
- deterministic dedupe fingerprint

The SurrealDB implementation should make claims graph-queryable while still
giving each claim a stable record ID so evidence and supersession relations can
target it.

Repeated evidence should reinforce the existing claim by adding evidence
relations, not create duplicate claims. Human corrections or stronger evidence
can supersede older claims through explicit claim-to-claim relations.

### Provenance Layer

Provenance should be specialized and graph-queryable. Initial evidence
relations should include:

- `supported_by`: claim to conversation item or source object
- `corrected_by`: claim to conversation item or source object
- `contradicted_by`: claim to conversation item or source object
- `supersedes`: claim to claim
- `derived_from`: claim to claim or source object

Evidence relations should carry authority, excerpt, observed time, creator, and
metadata. Precedence can be computed from evidence:

```text
human_correction
> explicit_human_statement
> document_source
> repeated_observation
> agent_inference
> weak_inference
```

This replaces broad generic object provenance for memory truth while preserving
the important answer to "why does Noema believe this?"

### Retrieval Policy Engine

Retrieval policy should be pure Rust and deterministic. SurrealDB can generate
candidates through graph traversal, entity/predicate filters, and text/search
indexes, but every candidate must pass policy before being shown to an agent.

The retrieval request should include trusted fields:

- requesting agent
- active humans
- active objects
- coarse `use_mode`
- explicit-memory-request flag
- sensitivity ceiling
- approval flags, especially for secret access

Policy gates should reject if:

- claim status is not retrievable
- current time is outside valid bounds
- predicate metadata is missing or invalid
- subject/object types do not satisfy predicate schema
- requesting agent is not active or delegated
- `use_mode` is denied by predicate or claim override
- claim sensitivity exceeds the request ceiling
- sensitivity-specific unlock rules are not met
- explicit deny overrides apply

Sensitivity defaults:

- `public`: retrievable when mode and context match.
- `normal`: retrievable when an active human is the subject or participant and
  the agent is active/delegated.
- `private`: retrievable only in direct subject/object context or on explicit
  request.
- `sensitive`: retrievable only for allowed modes plus explicit request or a
  trusted active object match.
- `secret`: retrievable only with explicit request and approval.

LLMs may propose claims and predicate metadata during extraction. LLMs must not
decide read-time access.

### Retrieval Packet Recorder

Instead of detailed `memory_use_records`, Noema should record compact retrieval
packets when context is assembled. A packet should include:

- packet id and run/turn id
- requesting agent
- active humans and objects
- `use_mode`
- policy/schema version
- included claim ids
- redacted omission counts and generic reasons
- owner/admin-only denial details when appropriate

This preserves explainability without turning every possible memory touch into
a separate durable object.

### API Layer

GraphQL remains Noema's first-party product API. Web, CLI, desktop, and future
mobile clients should continue to use Noema APIs and should not open embedded
database files directly. GraphQL types may change to reflect graph memory, but
storage details should stay behind server repositories.

## Data Flow

### Writes

1. A conversation turn creates durable `conversation_item` records in
   SurrealDB.
2. Memory extraction reads the new source item and proposes entity, predicate,
   and claim candidates.
3. Known predicates create candidate or active claims depending on predicate
   review policy, sensitivity, and whether the source is an explicit human
   "remember this" command.
4. Unknown predicates create `predicate_proposal` records with full proposed
   ontology metadata. Claims using them remain excluded from ordinary
   retrieval until promotion.
5. Claims are written with strict subject, object, predicate, fact, sensitivity,
   status, valid time, confidence, extraction metadata, and overrides.
6. Evidence relations link claims to source items or other source objects.
7. Conflict and cardinality rules run against existing claims in the same
   predicate neighborhood. Corrections can supersede or invalidate older
   claims. Ambiguous conflicts become disputed.

### Reads

1. Runtime builds a trusted retrieval request with active agent, active humans,
   active objects, `use_mode`, explicit request flag, sensitivity ceiling, and
   approval flags.
2. SurrealDB supplies candidate claims through graph traversal, predicate/entity
   filters, and search indexes.
3. Rust policy evaluates every candidate deterministically.
4. Eligible claims are ranked using non-authoritative hints after policy passes.
5. A retrieval packet records active context, included claims, redacted
   omissions, and policy/schema version.
6. The agent receives claim text plus minimal provenance explanation, not raw
   unrestricted graph state.

### Inspection

Owner/admin inspection can traverse from a claim to its predicate,
subject/object entities, evidence sources, supersession history, conflicts, and
retrieval packets. Agent-visible surfaces must use the same policy engine and
redaction rules as runtime retrieval.

## Error Handling

Startup should fail loudly and early if `NOEMA_HOME/db` cannot be opened,
schema bootstrap fails, schema version is unsupported, or the embedded engine
version is unsupported.

Write operations should be transactional at the domain level. A conversation
item should not partially create a claim without evidence, and a claim should
not become active if predicate or schema validation fails. Unknown predicates
should produce reviewable proposals rather than hard failures unless the user
requested immediate memory creation that cannot be satisfied.

Retrieval should fail closed. If policy inputs are incomplete, schema versions
mismatch, sensitivity is unknown, predicate metadata is missing, or candidate
loading fails, the claim is excluded. Agent-facing omissions stay generic.
Owner/admin inspection can show precise denial reasons.

Conflict handling should preserve evidence. Corrections and stronger evidence
supersede older claims through explicit relations. Ambiguous conflicts become
disputed claims and are excluded from ordinary retrieval until resolved.

## Testing

Test this as a storage architecture, not just a memory feature.

Embedded store tests:

- Open a temporary `NOEMA_HOME/db`.
- Bootstrap schema.
- Reject invalid records.
- Support local teardown and rebuild.

Repository tests:

- Create humans, agents, provider accounts, conversations, conversation items,
  entities, predicates, claims, and evidence relations through domain APIs.
- Replay conversation history.
- Inspect graph neighborhoods.

Ontology tests:

- Built-in predicates validate subject and object types.
- Built-in predicates validate allowed `use_mode`s, default sensitivity, and
  conflict/cardinality rules.
- Unknown predicates create proposals and do not enable ordinary retrieval.

Memory write tests:

- Repeated evidence reinforces an existing claim.
- Explicit correction supersedes an older claim.
- Contradictory evidence creates disputed state when precedence cannot resolve
  it safely.
- Claims require evidence before promotion.

Retrieval policy tests:

- Deterministic gates for `public`, `normal`, `private`, `sensitive`, and
  `secret`.
- No LLM involvement in read-time policy.
- Denied claims produce redacted omissions.
- Candidate graph traversal cannot bypass policy.

API and cleanup tests:

- GraphQL/client behavior keeps working over the SurrealDB-backed repositories.
- No runtime dependency remains on Postgres, SQLx, Postgres docs, or Postgres
  environment variables after the replacement lands.

## Rollout Shape

The implementation plan should target full replacement, ordered to reduce risk:

1. Add embedded SurrealDB dependency, store runtime, and schema bootstrap.
2. Port core object persistence: humans, agents, provider accounts,
   conversations, turns, and items.
3. Add strict entity, predicate, claim, predicate proposal, and evidence schema.
4. Port memory writes to graph claims and evidence relations.
5. Replace retrieval with SurrealDB candidate generation plus the Rust policy
   engine over claims.
6. Replace context graph inspection with graph-native inspection.
7. Update GraphQL and CLI surfaces.
8. Remove Postgres/SQLx runtime code, docs, configuration, and tests.
9. Update durable project docs after implementation validates the design.

## Open Design Checks For Planning

- Confirm the exact embedded SurrealDB engine feature to use for local
  file-backed storage.
- Confirm how SurrealDB relation records should be targeted by evidence
  relations. If direct relation-to-relation edges are awkward, use a schemafull
  `claim` record with relation fields while preserving graph-queryable claim
  traversal.
- Decide the initial personal-agent predicate catalog.
- Decide the exact retrieval packet schema and owner/admin denial detail
  retention.
- Decide whether text search and vector search are stored in SurrealDB indexes
  immediately or introduced after graph retrieval is stable.
