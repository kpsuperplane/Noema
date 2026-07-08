# SQLite Store

SQLite is Noema's canonical structured store for app-owned state. The database
file lives at:

```text
${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3
```

SQLite owns Noema records such as humans, agents, provider accounts, provider
capability bindings, conversations, transcript items, MCP setup/calibration
state, approvals, auxiliary model preferences, and memory service
settings/status.

SQLite does not mirror Mnemosyne's memory store. Durable memory truth, extraction,
updates, and memory search indexes belong to local Mnemosyne.

Current Noema home layout:

```text
~/.noema/
  db/
    noema.sqlite3
  mnemosyne/
    data/
    run/
```

This is a clean pre-V1 reset. No SurrealDB migration path is maintained.
