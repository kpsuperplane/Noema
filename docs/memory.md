# Memory Plan Index

This file is an index for the current memory direction, not a standalone
architecture plan.

Current authorities:

- [Project overview](project.md): product-level memory ownership and source of
  truth rules.
- [Current context](context/current.md): latest implemented memory state and
  open loops.
- [Frontend contract](frontend/current-contract.md): current memory routes,
  read models, redaction, and inspection boundaries.
- [Harness memory/context integration](harness/memory-context.md): target
  runtime boundary between context packets, retrieval, proposals, and durable
  memory truth.

Current implementation shape:

- Durable memory is modeled as graph claims over entities and promoted
  predicates in embedded SurrealDB.
- Conversation items are direct provenance sources.
- Explicit memory writes and provider-extracted proposals share the same
  proposal and canonicalization path.
- Retrieval starts as an explicit `search_memory` tool path and returns
  claim-shaped results subject to deterministic policy gates.
- Owner/admin inspection is available through bounded GraphQL read models such
  as `memoryClaims`, `memoryClaim`, `memoryPredicateProposals`, and
  `memoryGraph`.

Older broad memory proposals should be treated as target context only when they
agree with the current context and frontend contract above.
