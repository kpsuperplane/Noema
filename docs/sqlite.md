# SQLite Store

SQLite owns Noema's stored structured state. The database file lives at:

```text
${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3
```

SQLite owns Noema records such as humans, agents, provider accounts, provider
capability bindings, conversations, transcript items, MCP setup/policy
state, approvals, auxiliary model preferences, and the Memory model preference.

SQLite does not mirror memory pages. Markdown under `memory/human/` owns memory prose and metadata.
Memory search uses an in-process lexical index rebuilt from those files.
There is no separate persisted memory search database.

Sources: [home paths](../internal/home/home.go),
[memory search](../internal/memory/search.go), and
[store startup](../internal/store/store.go).

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

The Go migration list does not provide a supported conversion from a former Rust home.
Use a separate home for that transition. Existing Go homes use the forward migrations above.
Schema versions above the supported version fail without mutation.
