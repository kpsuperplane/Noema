# Memory Plan Index

This file is an index for the current memory direction, not a standalone
architecture plan.

Current authorities:

- [Project overview](project.md): product-level memory ownership and source of
  truth rules.
- [Current context](context/current.md): latest implemented memory state and
  open loops.
- [Frontend contract](frontend/current-contract.md): current memory settings
  route and first-slice visibility boundaries.
- [Harness memory/context integration](harness/memory-context.md): historical
  target context for runtime memory integration.

Current implementation direction:

- Durable memory truth and graph behavior belong to local Supermemory.
- Noema stores only memory service configuration, readiness, and ingest job
  diagnostics in SQLite.
- Noema uses `search_memory` as an explicit tool-only recall path.
- Noema maps trusted active scopes to deterministic Supermemory container tags
  and fans out searches across those tags.
- Memory transcript markers, `/remember`, graph browsing, and automatic
  pre-turn recall are not part of the current slice.

Older broad memory proposals should be treated as historical target context only
when they agree with the current context and frontend contract above.
