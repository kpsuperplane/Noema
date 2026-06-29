# Retired Postgres Schema

Postgres is no longer Noema's canonical structured store. Embedded SurrealDB
under `${NOEMA_HOME:-$HOME/.noema}/db` is the active runtime store.

This document is retained only as a historical note for the pre-SurrealDB
schema direction. Do not add new runtime code, configuration, or tests that
depend on this schema.

## Current Source-Of-Truth Split

| Data | Source of truth |
| --- | --- |
| Structured humans, agents, tools, conversations, transcript items, graph claims, provenance, grants, retrieval packets, links, and audit/object events | Embedded SurrealDB |
| Human-authored docs, imported files, attachments, and durable artifacts | Object-owned filesystem folders |
| Derived search/vector state, caches, temporary files, and rebuildable projections | `system/` |
| Inspection into database-backed state | Chat/work drill-ins, advanced inspection, and explicit export tools |
