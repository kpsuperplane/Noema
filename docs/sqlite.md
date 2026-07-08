# SQLite Store

SQLite is Noema's canonical structured store for app-owned state. The database
file lives at:

```text
${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3
```

SQLite owns Noema records such as humans, agents, provider accounts, provider
capability bindings, conversations, transcript items, MCP setup/calibration
state, approvals, auxiliary model preferences, memory service settings/status,
and Supermemory ingest job diagnostics.

SQLite does not mirror Supermemory's graph. Durable memory truth, memory graph
behavior, extraction, updates, and memory search indexes belong to local
Supermemory.

Current Noema home layout:

```text
~/.noema/
  db/
    noema.sqlite3
  supermemory/
    data/
    secrets/
```

This is a clean pre-V1 reset. No SurrealDB migration path is maintained.
