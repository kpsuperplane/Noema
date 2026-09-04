# Hypothetical Go Server Migration

- **Status:** Approved and in progress
- **Mode:** Implement
- **Date:** 2026-09-04
- **Scope:** Replace the Rust server with a Go server
- **Clients:** Keep the current web, desktop, iPhone, and iPad clients
- **Target:** Pure-Go server with no Rust and no CGo

This plan records the approved migration, evidence, limits, and exit gates.

## 1. Decision Summary

Replace all Noema-owned Rust server code with Go. The server must not compile,
link, launch, or retain a Rust component.

The Go server must preserve every current production capability. Small client
changes are permitted when behavior remains stable.

The server must use no CGo. Third-party services and executables remain valid
integration boundaries when a current capability requires them.

The desktop client must stop embedding the Rust host. It will launch the Go
server as a sidecar and preserve its current Tauri command contract.

Continue implementation only if a representative Go slice passes section 5.

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
- Do not remove a current production capability.
- Do not retain Rust workers, libraries, or build targets in the server.
- Do not add CGo.
- Do not add a service mesh or distributed database.
- Do not translate every Rust package into one Go package.
- Do not preserve obsolete internal Rust APIs.
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

### Required production capabilities

| Group | Required behavior |
| --- | --- |
| Server shell | GraphQL HTTP and WebSocket, passkeys, recovery, OAuth, sessions, callbacks, artifacts, and assets |
| Chat | Streaming, transcripts, continuation, context, tools, choices, A2UI, and memory use |
| Tasks | Capture, scheduling, recurrences, delegation, approvals, retries, cancellation, files, and artifacts |
| Governance | Action review, human decisions, exact-call checks, authorization recovery, URL checks, and safe diagnostics |
| Memory | Markdown storage, reads, search, atomic changes, consolidation, citations, and live events |
| Files | Uploads, downloads, versions, previews, parsing, OCR, and rooted access |
| Providers | OpenAI, Codex, OpenRouter, Apple Foundation Models, and local GGUF models |
| Web | Search, fetch, downloads, browser sessions, snapshots, interactions, history, and switching |
| Adapters | Reviewed definitions, OpenAPI, credentials, OAuth, HTTP, transforms, pagination, and controls |
| MCP | Stdio, HTTP, OAuth, discovery, setup, policy, recovery, and dynamic calls |
| ACP | Agent processes, health, authentication, run bridging, and Task completion |
| Notifications | Web Push, APNs, native notifications, Live Activities, presence, and durable delivery |

Exclude only test support, evaluations, development watchers, GraphiQL, debug
schema writing, old migrations, and removed legacy paths.

### First foundation result

The foundation slice is complete. MCP process isolation remains before the
evidence gate can produce a final go decision.

The slice contains:

- a fresh protected home and pure-Go SQLite schema version 1;
- transaction-based Task state and durable Task events;
- the complete generated GraphQL schema and its WebSocket transport;
- Task capture, Task read, and replayable Task events;
- bounded provider SSE parsing, cancellation, and continuation details;
- bounded diagnostics with explicit safe value types;
- a loopback-only binary guarded by `-migration-spike`.

An adversarial review found path, permission, stream, and output-bound risks.
The corrected slice escapes SQLite paths and applies private operating-system access controls.
It also bounds provider results and diagnostics files.

Authored production code is 1,858 lines. Tests are 810 lines across 18 test
functions. Generated GraphQL code is 80,449 lines.

The first measurements used Go 1.26.0 on Linux amd64. The host had four AMD
EPYC Rome virtual processors and 7.6 GiB of memory.

| Check | Result |
| --- | ---: |
| Warm Linux build | 0.84 seconds |
| Focused provider test | 0.18 seconds |
| Warm unit suite | 0.76 seconds |
| Clean Linux amd64 build | 30.69 seconds |
| Clean Windows amd64 build | 30.58 seconds |
| Clean macOS arm64 build | 31.37 seconds |
| Linux and Windows binary | 31 MiB |
| macOS binary | 29 MiB |

All measured builds used `CGO_ENABLED=0`. The scoped tests, race tests, and
`go vet` passed.

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

Build one bounded Go slice before porting the remaining server.

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
| Warm build | Complete in five seconds or less |
| Focused test | Complete in five seconds or less |
| Warm unit suite | Complete in 30 seconds or less |
| Clean build | Complete in 60 seconds or less for each platform |
| Database | Correct results for selected transaction and failure cases |
| GraphQL | Current operations validate, or one bounded client patch updates them |
| Concurrency | Go race detection reports no issue in the slice |
| Platforms | Pure-Go builds pass for Linux, macOS, and Windows |
| Security | No weaker file, process, secret, or authorization boundary |
| Complexity | No new framework exists only to imitate Rust structure |

Do not expand the port while this slice fails a build or security gate. Correct
the slice or select a different Go dependency.

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
       Pure-Go Noema server
     GraphQL, auth, Tasks, runtime,
     providers, capabilities, storage
              |
              v
 Third-party services and executables

Pure-Go server ---> SQLite and object-owned files
```

External processes receive bounded requests and return bounded results. They
must not receive database access or the complete Noema home.

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
| `internal/documents` | Bounded document and image parsing |
| `internal/script` | Bounded adapter and calculation execution |
| `internal/runtime` | Agent turns, Task runs, handoffs, and recovery |
| `internal/api` | GraphQL, sessions, passkeys, OAuth, Push, and assets |
| `cmd/noema` | Configuration, composition, startup, and shutdown |

Create a smaller package only when it owns a distinct current behavior.
Do not create interfaces for one implementation or tests alone.

## 8. Dependency Policy

Use the Go standard library first.

The initial dependency choices are:

- `ncruces/go-sqlite3` for pure-Go SQLite;
- gqlgen for GraphQL and `graphql-transport-ws`;
- the official MCP Go SDK for HTTP and subprocess transports;
- `golang.org/x/oauth2` for OAuth client behavior;
- go-webauthn for passkeys;
- webpush-go for Web Push;
- apns2 for APNs;
- chromedp with external Chrome for interactive browsing;
- Starlark-Go for bounded scripts;
- a WebAssembly PDFium runtime and focused Go document readers.

Select a pure-Go SQLite driver through transaction, concurrency, backup, and
performance evidence.

Use Go `os.Root` for rooted file operations. Add platform-specific code only
for behavior that `os.Root` cannot enforce.

Keep provider HTTP code concrete. Do not add a generic REST client.

Pin direct modules. Commit `go.sum`. Record license and advisory checks in the
release workflow.

Reject any module that requires CGo or a Rust build.

The shipped server and its dependencies use `CGO_ENABLED=0`. The Linux race
test can use the Go toolchain's CGo-based test instrumentation.

External Chrome, `llama-server`, LibreOffice, and the Apple provider bridge can
remain integrations. The Go server owns their limits, lifecycle, and protocol.

Browser fingerprints and document formatting can differ. Preserve the supported
actions, bounded outputs, security checks, and main content.

## 9. Preserved Contracts

| Boundary | Required compatibility |
| --- | --- |
| Stored state | Use a new Go-owned layout. Keep atomic replacement, checksums, file modes, and rooted access. |
| GraphQL | Keep `graphql/schema.graphql`, scalar encodings, null behavior, enums, cursors, subscriptions, and error categories. |
| Providers | Keep request conversion, transcript order, tool calls, cancellation, continuation, finalization, and safe errors. |
| Tools | Keep MCP transport, OAuth, process isolation, limits, adapters, browsing, files, documents, and bounded scripts. |
| Security | Keep authorization, egress, action review, approval consumption, URL policy, audit, and current-run checks. |
| Information | Keep secrets, private information, and ordinary information as the three information classes. |

The first Go release supports only homes created by Go. It does not read,
upgrade, or convert Rust-created homes.

Avoid client changes when the current contract is practical. Keep any required
client patch small and preserve product behavior.

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
| 5. Capabilities and integrations | Add adapters, MCP, files, documents, scripts, search, fetch, browsing, and local models. | Current capability security and provider contract cases pass. |
| 6. API and notifications | Add remaining mutations, subscriptions, Web Push, APNs, Live Activities, and native support. | Client operations and notification lifecycle cases pass. |
| 7. Desktop sidecar | Launch Go from desktop and proxy existing commands, subscriptions, and OAuth returns. | Current desktop behavior passes without an embedded host. |
| 8. Candidate acceptance | Run race, contract, restart, and controlled failure tests. | No current verified path loses its main outcome. |
| 9. Cutover | Follow section 13 and complete live acceptance. | Clients, integrations, notifications, Tasks, and recovery pass. |
| 10. Removal | Remove all replaced Rust server code after the rollback window. | Current documents name the Go server as the authority. |

Run live acceptance only after explicit approval. Record every waived provider
or platform case.

## 11. Validation Strategy

Do not copy all 1,135 Rust tests mechanically.

Port tests that protect a distinct risk:

- authorization, information handling, and path safety;
- transaction atomicity and data loss;
- repeat-safe commands and one-use approvals;
- current-run checks and stale execution claims;
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
go test ./cmd/... ./internal/...
go test -race ./cmd/... ./internal/...
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

- client compatibility requires a material or user-visible GraphQL change;
- the Go path weakens a security or information-handling boundary;
- a required capability has no viable pure-Go implementation;
- a current verified product path must be retired;
- the build loop misses the evidence gate;
- the migration exceeds its forecast by 50 percent;
- rollback cannot start the retained Rust server with its archived home.

## 15. Approved Decisions

| Decision | Approved direction |
| --- | --- |
| Server language | Go only. Retain no Rust server component. |
| Native linkage | Use no CGo. |
| Capabilities | Preserve every current production capability. |
| Clients | Preserve behavior. Small compatibility changes are allowed. |
| Platforms | Support Linux, macOS, and Windows. |
| Stored state | Create a fresh Go-owned `NOEMA_HOME`. |
| Cutover | Use one final replacement after acceptance passes. |
| Coordination | Assume no concurrent feature work during migration. |
| Schedule | Continue until the migration is complete. |

The implementation owns build thresholds, dependency choices, migration order,
and the final removal date.
