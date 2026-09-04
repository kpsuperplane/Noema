# Hypothetical Go Server Migration

- **Status:** Hypothetical decision and execution plan
- **Mode:** Plan only
- **Date:** 2026-09-04
- **Scope:** Replace the Rust server with a Go server
- **Clients:** Keep the current web, desktop, iPhone, and iPad clients
- **Recommended target:** Go core with bounded native workers

This plan does not approve implementation. It defines the evidence, limits,
and exit gates for a possible migration.

## 1. Decision Summary

Noema can move its server core to Go without changing the product clients.
The migration must preserve the current GraphQL, SQLite, filesystem, security,
and provider behavior.

The recommended target keeps these specialist processes:

- Obscura for interactive browsing;
- the bounded document parser until Go replacements pass format tests;
- Luau until an exact bounded Go host passes compatibility tests;
- the Swift Apple Foundation Models bridge;
- the existing `llama-server` executables.

These processes do not own Noema stored state.

Start implementation only if a representative Go slice passes the decision
gate in section 5.

## 2. Observable Outcome

One Go server replaces `noema_web`. Current clients connect without generated
operation changes or user-visible behavior loss.

The replacement must:

- create a fresh Go-owned Noema home;
- serve the current GraphQL schema and subscription protocol;
- preserve Task, conversation, memory, artifact, and project behavior;
- preserve authentication, authorization, action review, and audit behavior;
- preserve hosted, local, MCP, adapter, browser, and file capabilities;
- recover active and interrupted runs according to current contracts;
- keep secrets outside logs, model context, ordinary events, and exports;
- improve the normal edit, test, and server restart loop materially.

## 3. Non-goals

- Do not rewrite the web or native clients.
- Do not redesign GraphQL.
- Do not replace SQLite.
- Do not open or convert a Rust-created Noema home.
- Do not preserve the Rust database or file layout.
- Do not add a service mesh or distributed database.
- Do not translate every Rust package into one Go package.
- Do not preserve obsolete internal Rust APIs.
- Do not replace Obscura during the core migration.
- Do not replace Luau syntax during the core migration.
- Do not expand product capability during the migration.
- Do not add an ORM, dependency injection framework, or generic repository layer.

## 4. Current Baseline

Record the exact baseline again when implementation starts.

The 2026-09-04 baseline contains:

- 15 production Rust packages in the server closure;
- approximately 231,606 raw Rust lines, including tests;
- 1,135 Rust test functions;
- 60 active direct external crates;
- approximately 707 current-platform dependency nodes;
- GraphQL clients generated from `graphql/schema.graphql`;
- separate development and validation Cargo caches;
- disabled incremental compilation for development and tests;
- an embedded Obscura dependency with V8 and BoringSSL.

Measure these operations on one named reference machine:

1. Clean server build.
2. Warm server build with no changes.
3. Build after a store-only change.
4. Build after a runtime-only change.
5. Build after an API-only change.
6. One focused test build and run.
7. Full server unit test build and run.
8. Development server restart after one source change.

Save command, toolchain, processor, memory, cache state, wall time, CPU time,
peak memory, and produced artifact size.

## 5. Evidence Gate

Build one disposable Go slice before approving the migration.

The slice must use a new temporary Go home. It must never open the active home.

The slice must:

- open SQLite with foreign keys and current journal settings;
- create its schema from an empty directory;
- create and read one Task with its current documents;
- perform one transaction-based Task state change;
- expose that read and change through the existing GraphQL shape;
- stream one provider response with cancellation;
- publish one GraphQL subscription event;
- start and call one MCP subprocess with a cleared environment;
- read one file through a rooted filesystem handle;
- reject one stale current-run check;
- preserve one representative ordinary identifier in diagnostics;
- exclude one representative secret from diagnostics.

The slice passes only when all these gates pass:

| Gate | Required result |
| --- | --- |
| Build loop | At least five times faster, or below ten seconds |
| Clean build | At least three times faster than the Rust baseline |
| Database | Correct results for selected transaction and failure cases |
| GraphQL | Current generated web and iOS operations remain valid |
| Concurrency | Go race detection reports no issue in the slice |
| Security | No weaker file, process, secret, or authorization boundary |
| Complexity | No new framework exists only to imitate Rust structure |

Stop the migration if this slice fails the build or security gate. Improve
the Rust build instead.

### Spike budget

- Go production code: at most 6,000 lines.
- Go test code: at most 3,000 lines.
- New Go tests: at most 20 focused tests.
- Rust production change: net-negative or zero.
- Client change: zero.

Stop and reassess if either Go line budget grows by 50 percent.

## 6. Target Runtime

```text
Web, desktop, and native clients
              |
              v
         Go Noema server
     GraphQL, auth, Tasks, runtime,
     providers, capabilities, storage
       |       |       |       |
       v       v       v       v
   Obscura   parser   Luau   Apple bridge
       |
       v
  external web pages

Go Noema server ---> SQLite and object-owned files
```

Workers receive bounded requests and return bounded results.

Workers must not receive database credentials, unrelated environment values,
or access to the complete Noema home.

## 7. Go Package Ownership

Use a small package set based on current authorities. Do not mirror the Cargo
package graph.

| Go area | Owns |
| --- | --- |
| `internal/home` | Paths, protected files, and rooted access |
| `internal/store` | SQLite, transactions, Go schema changes, and durable commands |
| `internal/domain` | Tasks, conversations, workspaces, projects, and artifacts |
| `internal/providers` | Model accounts, routing, streams, and local models |
| `internal/capabilities` | Policy, adapters, MCP, files, and browser commands |
| `internal/runtime` | Agent turns, Task runs, handoffs, and recovery |
| `internal/api` | GraphQL, sessions, passkeys, OAuth, Push, and assets |
| `cmd/noema` | Configuration, composition, startup, and shutdown |

Create a smaller package only when it owns a distinct current behavior.
Do not create interfaces for one implementation or tests alone.

## 8. Dependency Policy

Use the Go standard library first.

The initial dependency candidates are:

- gqlgen for the GraphQL server;
- the official MCP Go SDK;
- `golang.org/x/oauth2` for OAuth client behavior;
- go-webauthn for passkeys;
- a maintained Web Push implementation;
- a SQLite driver selected by the evidence gate.

Compare a pure-Go SQLite driver with a CGo driver. Select one through
transaction, concurrency, backup, and performance evidence.

Use Go `os.Root` for rooted file operations. Add platform-specific code only
for behavior that `os.Root` cannot enforce.

Keep provider HTTP code concrete. Do not add a generic REST client.

Pin direct modules. Commit `go.sum`. Record license and advisory checks in the
release workflow.

## 9. Preserved Contracts

| Boundary | Required compatibility |
| --- | --- |
| Stored state | Use a new Go-owned layout. Keep atomic replacement, checksums, file modes, and rooted access. |
| GraphQL | Keep `graphql/schema.graphql`, scalar encodings, null behavior, enums, cursors, subscriptions, and error categories. |
| Providers | Keep request conversion, transcript order, tool calls, cancellation, continuation, finalization, and safe errors. |
| Tools | Keep MCP transport, OAuth, process isolation, limits, and adapter manifest version 9 with exact Luau behavior. |
| Security | Keep authorization, egress, action review, approval consumption, URL policy, audit, and current-run checks. |
| Information | Keep secrets, private information, and ordinary information as the three information classes. |

The first Go release supports only homes created by Go. It does not read,
upgrade, or convert Rust-created homes.

The first Go release must not require a client update. Preserve product
behavior through public contracts, not old storage compatibility.

Secret Go types must reject JSON and text serialization. Their debug output
must use fixed text.

## 10. Ordered Migration Units

Each unit must pass its gate before the next unit starts.

| Unit | Work | Exit gate |
| --- | --- | --- |
| 0. Baseline and spike | Complete sections 4 and 5. | Record a go or no-go decision. |
| 1. Home and store | Add configuration, rooted files, SQLite, fresh schema creation, transactions, and backups. | A new home works from an empty directory. Selected store tests pass. |
| 2. Domain and read API | Add stored reads, current GraphQL queries, and static assets. | Current client operations return equivalent normalized results. |
| 3. Authentication and commands | Add sessions, passkeys, recovery, native OAuth, commands, approvals, notifications, and audit events. | Security, concurrency, restart, and stale-request cases pass. |
| 4. Provider and agent runtime | Add provider conversion, streaming, context, tools, Task roles, finalization, and recovery. | Provider fixtures produce equivalent conversations and Task outcomes. |
| 5. Capabilities and integrations | Add adapters, MCP, files, search, fetch, browser coordination, and local-model supervision. | Current capability security and provider contract cases pass. |
| 6. API and notifications | Add remaining mutations, subscriptions, Web Push, APNs, Live Activities, and native support. | Client operations and notification lifecycle cases pass. |
| 7. Candidate acceptance | Run race, contract, restart, and controlled failure tests. | No current verified path loses its main outcome. |
| 8. Cutover | Follow section 13 and complete live acceptance. | Clients, workers, notifications, Tasks, and recovery pass. |
| 9. Removal | Remove the replaced server after the rollback window. Keep approved workers. | Current documents name the Go server as the authority. |

Run live acceptance only after explicit approval. Record every waived provider
or platform case.

## 11. Validation Strategy

Do not copy all 1,135 Rust tests mechanically.

Port tests that protect a distinct risk:

- authorization, information handling, and path safety;
- transaction atomicity and data loss;
- repeat-safe commands and one-use approvals;
- current-run checks and stale worker claims;
- provider wire formats and stream termination;
- external input bounds and duplicate JSON keys;
- restart recovery and uncertain external actions;
- fresh schema creation and later Go schema changes;
- browser, MCP, and subprocess isolation;
- client-visible GraphQL behavior.

Use table-driven tests when setup and consequences are shared.
Use the real in-memory SQLite store where practical.

Run these checks for each candidate unit:

```text
gofmt check
go vet ./...
go test ./...
go test -race ./...
staticcheck ./...
govulncheck ./...
GraphQL operation validation
fresh Go schema check
```

Pin exact tool versions in the migration toolchain file.

## 12. Budget and Schedule

Reforecast after the evidence gate. These limits are provisional.

| Work | Production budget | Test budget |
| --- | ---: | ---: |
| Spike | 6,000 | 3,000 |
| Home and store | 18,000 | 8,000 |
| Domain and read API | 20,000 | 8,000 |
| Authentication and commands | 22,000 | 10,000 |
| Provider and agent runtime | 45,000 | 18,000 |
| Capabilities and integrations | 35,000 | 15,000 |
| Complete API and notifications | 18,000 | 8,000 |
| **Provisional total** | **164,000** | **70,000** |

Generated GraphQL code is measured separately. It does not justify growth in
authored production code.

Each unit must define a smaller budget before implementation. Stop when a unit
exceeds its estimate by 50 percent or 500 lines, whichever is smaller.

Expected focused full-time duration for one person is six to twelve months.
Reforecast when the spike and store unit finish.

## 13. Cutover and Rollback

Stop the Rust server. Rename the Rust home for archival use. Create a new,
empty Go home and start the Go server.

Rollback requires these steps:

1. Stop the Go server.
2. Preserve its diagnostics and Go home.
3. Start the retained Rust binary with the archived Rust home.
4. Verify clients and the restored Rust state.

Rollback does not transfer Go-created state into Rust. Do not open a home with
the other implementation.

## 14. Stop Conditions

Stop and request a product decision if any condition occurs:

- client compatibility requires GraphQL changes;
- the Go path weakens a security or information-handling boundary;
- exact Luau compatibility requires changing saved adapter behavior;
- browser parity requires reimplementing Obscura inside the migration;
- a current verified product path must be retired;
- the build loop misses the evidence gate;
- the migration exceeds its forecast by 50 percent;
- rollback cannot start the retained Rust server with its archived home.

## 15. Decisions Required Before Execution

The human must approve these choices:

1. Go core with retained workers, or a strict pure-Go server.
2. The reference machine and build targets.
3. The SQLite driver selected by the spike.
4. The server feature-freeze window.
5. The implementation branch and merge sequence.
6. The live acceptance scope before cutover.
7. The final removal date for the Rust server.

Until these decisions exist, this document remains a hypothetical plan.
