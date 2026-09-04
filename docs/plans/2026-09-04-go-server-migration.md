# Hypothetical Go Server Migration

- **Status:** Approved. Evidence gate passed. Migration in progress.
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

The foundation and evidence slice are complete. The evidence gate result is Go.

The slice contains:

- a fresh protected home and pure-Go SQLite schema version 1;
- transaction-based Task state and durable Task events;
- the complete generated GraphQL schema and its WebSocket transport;
- Task capture, Task read, and replayable Task events;
- durable `TASK.md` writes with staged crash recovery and current previews;
- validation for every checked-in web and iOS GraphQL operation;
- bounded provider SSE parsing, cancellation, and continuation details;
- bounded MCP stdio calls with environment and process-tree isolation;
- bounded diagnostics with explicit safe value types;
- a loopback-only binary guarded by `-migration-spike`.

An adversarial review found path, permission, stream, and output-bound risks.
The corrected slice escapes SQLite paths and applies private operating-system access controls.
It also bounds provider results and diagnostics files.

Authored production code is 2,712 lines. Tests are 1,393 lines across 20 test
functions. Generated GraphQL code is 80,445 lines.

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

The dependency audit found 20 production modules and no Rust or CGo build
need. It found no reachable vulnerability with Go 1.26.6.

Go 1.26.0 had 19 reachable standard-library findings. CI and release builds
must use Go 1.26.6 or newer.

The module graph has no GPL-family license. Release packages must include
third-party notices for MPL-2.0 and the MCP SDK license transition.

Windows support starts at Windows 10 version 1803 or Windows Server 2019.
Before cutover, native Windows tests must stress concurrent WAL writes and run
`PRAGMA integrity_check` after a cold reopen.

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

The slice passed every gate on 2026-09-04. Task creation writes a staged
document before the SQLite transaction. Startup recovery promotes committed
documents and removes documents without a Task row.

Windows directory metadata flushes use a write-capable directory handle.
Native Windows WAL stress and cold-reopen integrity tests remain cutover gates.

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
| `internal/provider` | Model accounts, routing, streams, and local models |
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

### First authentication unit

The complete browser authentication unit passed on 2026-09-04. It includes:

- exact Host and Origin checks;
- the zero-passkey setup barrier;
- initial passkey claim and normal passkey login;
- bounded persistent browser sessions;
- recovery-code rotation and recovery enrollment;
- passkey listing, addition, removal, and final-passkey protection;
- current-session and global logout;
- GraphQL HTTP and WebSocket admission;
- WebSocket closure after session revocation.

Use go-webauthn and the standard library. Do not add a session framework.
Append schema version 2 for passkeys and browser session digests.

The unit added 2,235 production lines and 770 test lines. It added nine focused
tests. Production exceeded its estimate by 235 lines but stayed below the
2,500-line stop threshold.

The tests cover authority, ceremonies, races, persistence, recovery, passkey
management, ingress limits, and GraphQL admission. The schema advanced to
version 2 through a forward-only migration.

Browser sessions use protected keyed digests. Recovery codes rotate through
protected atomic configuration writes. The server limits HTTP work to 256
requests and WebSockets to 64 connections.

### Native OAuth unit

The complete native OAuth unit passed on 2026-09-04. It preserves:

- authorization, token, and revocation routes;
- browser resume, recent-passkey consent, CSRF, and client redirects;
- public native clients and exact S256 PKCE;
- digest-only authorization, access, and refresh credentials;
- atomic code exchange and refresh rotation;
- durable request-bound and desktop retry recovery;
- replay, family, client, and global revocation;
- GraphQL client listing and revocation;
- bearer HTTP and WebSocket admission;
- WebSocket closure after expiry or revocation.

Schema version 3 adds native clients, authorization codes, refresh families,
access credentials, refresh credentials, and browser consent state.

The unit added 1,954 production lines and 660 test lines. It added eight focused
tests. Production exceeded its estimate by 54 lines and stayed below the
2,400-line stop threshold.

The maintained `github.com/go-oauth2/oauth2/v4` module owns OAuth request,
grant, response, redirect, and S256 validation rules. Noema owns durable token
state, atomic rotation, retry recovery, replay, and revocation.

The Go production closure now contains 24 external modules. Native bearer
authentication is active for GraphQL HTTP and WebSocket requests.

### Hosted onboarding unit

The first hosted onboarding unit passed on 2026-09-04. It includes:

- protected provider credentials with revision checks and rollback;
- safe provider account metadata and derived capabilities;
- OpenRouter API-key verification and compatible model discovery;
- OpenRouter S256 PKCE with bounded, short-lived attempt state;
- current GraphQL provider roots and authentication events;
- atomic model assignments for all nine current workloads;
- idempotent primary Chat creation after onboarding;
- one fresh-home path through the current client boot shape.

Schema version 4 owns provider accounts. Schema version 5 owns primary Chat
identity. Schema version 6 owns the complete hosted model assignment set.

OpenRouter secrets remain in protected files. SQLite contains only safe
account metadata, compatible model profiles, and credential revisions.

The Go server now reaches an empty ready Chat from a fresh home through
OpenRouter. The first text turn is complete in the runtime unit below.

### Codex device authentication unit

The Codex device authentication unit passed on 2026-09-04. It includes:

- the current OpenAI device authorization wire contract;
- bounded polling with cancellation and expiry;
- authorization-code exchange;
- bounded model discovery with visible-profile filtering;
- validated client-version discovery with a safe fallback;
- protected access and refresh token storage;
- atomic token and catalog publication with credential revision checks and rollback;
- safe authentication events and errors.

The current provider-auth GraphQL roots now route Codex and OpenRouter attempts.
They preserve the existing web and iOS operation shapes. Graceful shutdown
cancels detached Codex polling and model discovery. Codex generation remains
later provider work.

### First Chat runtime unit

The first Chat runtime unit passed on 2026-09-04. It includes:

- schema version 7 for turns, items, and conversation status;
- schema version 8 for safe links, cascade deletion, and one active turn;
- startup and shutdown recovery for interrupted turns;
- bounded OpenRouter text generation with safe credential use;
- serialized detached turn execution and timezone context;
- GraphQL acceptance, status, delta, item, completion, and replay behavior;
- durable provider failures and restart-stable transcript identifiers.

This unit covers text-only OpenRouter turns. Native tools, reasoning records,
citations, hosted search, and provider continuation remain required in unit 4.

### OpenRouter native-tool provider unit

The OpenRouter native-tool provider unit passed on 2026-09-04. It includes:

- canonical and provider-safe tool names with collision handling;
- strict schema conversion with safe fallback and optional-null restoration;
- function-tool and hosted-search request controls;
- complete native call, result, and reasoning replay;
- bounded stream parsing for calls, reasoning, citations, searches, and usage;
- validated native calls with exact advertised-name and call-ID checks;
- safe errors, cancellation, secret handling, and request and response limits.

The provider uses complete local replay. It does not depend on an OpenRouter
continuation identifier.

Shared generation requests, responses, replay items, reasoning, and tool controls
now use one provider-neutral Go contract. OpenRouter remains its first transport.

### Codex Responses HTTP unit

The Codex Responses HTTP transport includes:

- structured message, reasoning, function-call, and function-result replay;
- streamed text and one validated native function call;
- encrypted reasoning, output identity, model identity, and usage;
- stored client-version, workspace, session, and origin headers;
- Fast mode through the priority service tier;
- access-token refresh near expiry and one refresh retry after authentication rejection;
- bounded requests, responses, errors, cancellation, and credential lifetime.

WebSocket sessions remain later work.

### Primary Chat provider-routing unit

The primary Chat provider-routing unit passed on 2026-09-04. It includes:

- one provider-neutral generation interface and request-size error;
- selection of OpenRouter or Codex from the stored primary assignment;
- one fixed provider route through immediate-tool continuations and finalization;
- provider-specific recommended models and Fast mode;
- exact provider provenance in usage, reasoning, calls, and results;
- production composition of both hosted generators;
- rejection of OpenAI until its Go transport and credentials exist.

This unit changes no schema or client operation.

### First immediate tool runtime unit

The `task.inspect` runtime unit passed on 2026-09-04. It includes:

- one advertised immediate-read tool with a final source-schema check;
- rooted access to the current Task row and `TASK.md`;
- atomic call and result storage with repeat-safe completion;
- exact stored results and bounded model-facing replay;
- durable provider call, result, commentary, and reasoning history;
- restart recovery that does not repeat an uncertain call;
- repeated durable continuations with distinct stream and round identities;
- deterministic stops for repeated results, failure streaks, and the hard ceiling;
- one tool-free finalization request after a deterministic stop;
- bounded finalization replay that keeps every permitted call and result;
- compact finalization after local size or provider context rejection;
- commentary phases and stream identities that reconcile across repeated rounds;
- combined provider usage across every request in the turn;
- existing GraphQL `Activity` delivery for live and stored tool items.

This unit does not add Task writes, action requests, scheduling, parallel calls,
or adaptive progress audits. Those capabilities remain later unit 4 slices.

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
gofmt check for `cmd` and `internal`
go vet ./cmd/... ./internal/...
go test ./cmd/... ./internal/...
go test -race ./cmd/... ./internal/...
staticcheck ./cmd/... ./internal/...
govulncheck ./cmd/... ./internal/...
govulncheck github.com/99designs/gqlgen
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
