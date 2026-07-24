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

## Schema migrations

The canonical database uses ordered, forward-only `rusqlite_migration` steps
and records its current version in SQLite's `PRAGMA user_version`. Store startup
validates the recorded version and exact schema shape before applying pending
migrations transactionally. A database created from the final pre-migration v9
schema is adopted in place without rewriting application rows.

The v9 baseline and every applied migration are immutable history. A schema
change appends one migration to `store_migrations` in
`crates/noema-store/src/schema.rs`, increments `STORE_SCHEMA_VERSION`, and adds
focused coverage for the data or invariant at risk. Unknown, structurally
modified, and newer schemas are rejected without mutation.

No SurrealDB migration path is maintained, and SQLite schemas older than or
different from the exact v9 baseline still require an explicit reset or restore.
