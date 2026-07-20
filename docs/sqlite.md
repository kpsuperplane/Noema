# SQLite Store

SQLite is Noema's canonical structured store for app-owned state. The database
file lives at:

```text
${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3
```

SQLite owns Noema records such as humans, agents, provider accounts, provider
capability bindings, conversations, transcript items, MCP setup/calibration
state, approvals, auxiliary model preferences, and the Memory model preference.

The canonical database does not mirror memory pages. Durable memory prose and
semantic metadata live in Markdown under `memory/human/`. The separate
`system/indexes/memory.sqlite3` database contains only a rebuildable FTS
projection and may be deleted without losing memory truth.

Current Noema home layout:

```text
~/.noema/
  db/
    noema.sqlite3
  memory/
    human/
      root.md
      .state.md
  system/
    indexes/
      memory.sqlite3
```

This is a clean pre-V1 reset. No SurrealDB migration path is maintained.
