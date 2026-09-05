# SQLite Store

SQLite owns Noema's stored structured state. The database file lives at:

```text
${NOEMA_HOME:-$HOME/.noema}/noema.sqlite3
```

SQLite owns Noema records such as humans, agents, provider accounts, provider
capability bindings, conversations, transcript items, MCP setup/policy
state, approvals, auxiliary model preferences, and the Memory model preference.

SQLite does not mirror memory pages. Durable memory prose and
semantic metadata live in Markdown under `memory/human/`. The separate
`system/indexes/memory.sqlite3` database contains only a rebuildable FTS
projection and may be deleted without losing memory truth.

Memory-related Noema home layout:

```text
~/.noema/
  noema.sqlite3
  memory/
    human/
      root.md
      .state.md
  system/
    indexes/
      memory.sqlite3
```

## Go row mapping

Bun maps Tasks, Task runs, Task events, and Agents into their existing Go types.
It uses the existing ncruces SQLite driver and connection pool. Transactions keep
immediate locking and exact current-run checks. Explicit SQL owns state transitions.

Mapped time fields retain integer milliseconds through bounded model configuration.
No ORM schema generation runs. Persisted schema changes still need forward migrations.

## Schema migrations

The Go store uses ordered, forward-only migrations. SQLite records the current
version in `PRAGMA user_version`. Store startup applies pending migrations in one transaction.

`internal/store/store.go` owns one ordered migration list. Its length is the
current version. Fresh databases and upgrades use this same list.
`internal/store/schema_v*.go` owns each immutable migration. A schema change
adds one immutable migration to the list and tests upgrades plus fresh convergence.

The Go server uses a fresh Noema home. It does not open or convert a Rust home.
Unknown and newer schema versions fail without mutation.
