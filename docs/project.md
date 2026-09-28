# Noema product and architecture

Noema is an opinionated, self-hosted personal assistant for one local owner.
One main Chat holds the ongoing conversation. Tasks handle delegated work without creating more chat threads.
The [README](../README.md) provides the feature tour and screenshots.

## Product direction

- Keep the main assistant as the human's point of contact.
- Delegate complex work to Tasks with separate execution and review.
- Preserve useful personal context in inspectable Markdown memory.
- Enforce action states and exact approvals in server code.
- Keep records on the owner's server and make work inspectable.

Hosted models and connected services receive data during their use.
Self-hosting does not mean that every operation stays on the host.

## Implemented architecture

One Go server composes authentication, GraphQL, Chat, Task workers, model providers, integrations, and notifications.
The web app uses React. Tauri provides the desktop shell. SwiftUI provides the iOS client.
The desktop shell can start a bundled Go server or connect to a remote server.

| Concern | Implementation | Contract |
| --- | --- | --- |
| Startup and service composition | [cmd/noema](../cmd/noema/main.go) | [Harness](harness.md) |
| Chat and Task execution | [internal/runtime](../internal/runtime) | [Tasks](tasks.md) |
| Structured state and migrations | [internal/store](../internal/store) | [SQLite](sqlite.md) |
| Human memory | [internal/memory](../internal/memory) | [Memory](memory.md) |
| HTTP APIs and MCP | [internal/adapter](../internal/adapter), [internal/mcp](../internal/mcp) | [Capabilities](harness/capabilities.md) |
| Public browsing and downloads | [internal/webtool](../internal/webtool) | [Web tools](harness/web-browsing.md) |
| Browser and native authentication | [internal/auth](../internal/auth) | [Server access](server-security.md) |
| Web interface | [apps/web](../apps/web) | [Frontend contract](frontend/current-contract.md) |

Chat and Tasks use separate execution loops with shared storage and services.
Schedules create or release Task work. Stored changes drive subscriptions and notifications.
Tasks use the built-in provider Executor. [ACP execution is retired](development/acp-retirement.md).
See the subsystem contracts for the limits of each path.

## Information handling

Noema uses exactly three information classes: **secrets**, **private information**, and **ordinary information**.
Secrets grant authority and belong in protected credential or transient-auth stores.
Authorized private information and ordinary information remain intact.

The [security contract](harness/security.md#information-classes-and-mechanisms) separates required handling from implemented controls and known limits.
Connection policies and exact action approval exist. General per-scope grants and universal information-flow tracking do not.
Do not describe those missing controls as product guarantees.

## Stored data

The default data directory is `~/.noema`. `NOEMA_HOME` selects another directory.
[internal/home](../internal/home/home.go) owns the shared paths.

| Path under `NOEMA_HOME` | Purpose |
| --- | --- |
| `config.yaml` | Server configuration |
| `db/noema.sqlite3` | Structured state, history, schedules, and action requests |
| `memory/human/` | Local-human memory pages and metadata |
| `tasks/`, `workspaces/`, `conversations/` | Working documents and owned files |
| `adapters/` | API definitions, accounts, connections, and protected credentials |
| `mcp/`, `providers/` | Connection settings and protected credentials |
| `notifications/` | Protected Web Push and APNs authority |
| `models/` | Model downloads and verified weights |
| `run/` | Local socket and protected transient authentication state |
| `system/` | Derived caches and temporary files |

SQLite adapter rows are rebuildable indexes of the filesystem adapter records.
Memory search uses a rebuildable in-process index. It does not use a separate SQLite FTS database.

Stop Noema before backup or restore. Copy the complete data directory, including protected files.
A partial export is not a complete backup.

## Scope and limits

The current product serves one local owner. Shared workspaces and multiple human accounts are not available.
Projects organize Task context; they do not provide independent authorization boundaries.
Memory belongs to the local human. Project documents provide separate working context.
There is no general proactivity-level hierarchy or universal event-subscription system.

[Current context](context/current.md) records open work and validation limits.
Use Git history for completed plans and past verification reports.
