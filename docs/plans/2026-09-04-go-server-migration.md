# Go Server Migration Record

- **Status:** Source migration complete. External release acceptance remains open.
- **Mode:** Implement and ship
- **Started:** 2026-09-04
- **Scope:** Replace the Rust server with a pure-Go server
- **Clients:** Preserve the web, desktop, iPhone, and iPad clients
- **Platforms:** Linux, macOS, and Windows

This record keeps the approved decisions, completed source scope, size limits,
and remaining release gates.

## Decision

Go replaces all Noema-owned Rust server code. The server does not compile or
link Rust or CGo.

Current external executables remain valid integration boundaries. These
executables include Chrome, Obscura, `llama-server`, document tools, and the
Apple Foundation bridge.

The migration keeps all current production capabilities. Small client changes
are valid when product behavior stays stable.

The Go server uses a fresh home. It does not open, upgrade, or convert a
Rust-created home.

## Non-goals

- Do not rewrite the web or native clients.
- Do not redesign GraphQL or replace SQLite.
- Do not preserve Rust server packages or internal Rust APIs.
- Do not preserve a Rust database or file layout.
- Do not add CGo.
- Do not add a server-managed database backup subsystem.
- Do not add an ORM, dependency injection framework, or generic repository layer.
- Do not add distributed coordination for local operations.
- Do not expand product scope during the migration.

Noema backup remains a stopped-server copy of the complete Noema home. The
project storage contract owns that procedure.

## Baseline and Size Gate

The fixed Rust baseline is commit
`a007a4fa984f0d2eaeb2c101337dbbe7881d9379`.

| Class | Rust baseline | Go current | Go ratio | Hard limit |
| --- | ---: | ---: | ---: | ---: |
| Authored production | 173,990 | 76,704 | 44.09% | 139,192 |
| Tests | 65,836 | 24,347 | Separate evidence cost | — |
| Generated GraphQL | — | 79,612 | Separate generated cost | — |
| Inclusive total | 239,826 | 180,663 | 75.33% | 191,860 |

Count only tracked `.go` files. Report authored production, tests, generated
GraphQL, and the inclusive total separately.

Both Go ratios must stay below 80 percent. Generated code does not justify
authored growth or removal of necessary tests.

Each protection must address a current failure, concrete threat, retained
capability, or client contract. Rust parity alone does not justify extra code.

## Completed Source Outcome

One Go server now owns production startup, shutdown, configuration, storage,
GraphQL, runtime behavior, capabilities, and integrations.

The source migration includes:

- fresh-home creation and forward-only Go schema changes through version 32;
- GraphQL HTTP, WebSocket subscriptions, assets, GraphiQL, and the private socket;
- passkeys, recovery, browser sessions, native OAuth, and onboarding;
- Chat streaming, transcripts, continuation, context, tools, choices, and A2UI;
- Task capture, scheduling, recurrences, roles, gates, retries, and recovery;
- Projects, Agents, Artifacts, Memory, audit events, and notifications;
- OpenAI, OpenRouter, Codex, Apple Foundation Models, and local GGUF models;
- MCP, ACP, HTTP adapters, adapter OAuth, direct credentials, and Lua 5.4;
- search, fetch, downloads, document parsing, OCR, and browser routes;
- Playwright, Kernel, and installable Obscura browser providers;
- Web Push, APNs, native notifications, presence, and Live Activities;
- Go development startup, release builds, and desktop sidecar packaging.

Rust server packages and the unused Rust MCP transport are removed. Rust
remains only in supported client and tooling targets outside the server.

The desktop packages the Go server as its sidecar. macOS packages the Swift
Apple Foundation bridge beside that server.

Linux amd64 and arm64, macOS amd64 and arm64, and Windows amd64 release builds
use `CGO_ENABLED=0`.

## Preserved Contracts

| Boundary | Contract |
| --- | --- |
| Stored state | Use one Go-owned home, SQLite database, and object-owned files. |
| GraphQL | Keep the shared schema, scalar forms, null behavior, cursors, subscriptions, and error categories. |
| Providers | Keep message order, tool calls, cancellation, continuation, finalization, citations, usage, and safe errors. |
| Tasks | Keep current-run checks, one-use approvals, recovery, files, schedules, recurrences, and role handoffs. |
| Capabilities | Keep adapters, MCP, ACP, browser, files, documents, scripts, local models, and notifications. |
| Information | Keep secrets, private information, and ordinary information as the three information classes. |

Current clients keep their GraphQL operations and general behavior. A small
client patch is valid only when the server contract makes it necessary.

## Storage and Information Handling

`${NOEMA_HOME:-$HOME/.noema}` is the Go home. SQLite uses
`${NOEMA_HOME}/noema.sqlite3`.

If `${NOEMA_HOME}/db/noema.sqlite3` exists, startup rejects that Rust-created
home before the Go server writes data.

Secret-bearing production types must not expose secret values through logs,
debug output, model context, ordinary events, exports, or ordinary JSON and
text sinks.

A dedicated protected-store DTO can serialize a secret into its governed,
protected persistence file. That serialization is not an ordinary JSON or
text sink.

Authorized private information and ordinary information must remain intact.
Secret exclusion must not conceal unrelated values.

Helper processes keep the host environment when required. They remove exact
server-owned secrets and `NOEMA_HOME` before startup.

## Dependencies and External Processes

The server uses pure-Go dependencies. Direct modules remain pinned in
`go.mod`, and checksums remain in `go.sum`.

The CI workflow checks advisories and static analysis.

External processes receive bounded requests and results. They do not receive
database access or the complete Go home.

The server owns each process lifecycle and protocol. The implementation uses
the limits needed by the current contract.

Do not add a fixed idle period, stderr-ring size, or process-tree rule only
because an earlier design named one. Add a rule only for an enforced current
contract or demonstrated failure.

## Obscura Delivery

Obscura remains the default interactive browser route. Selection installs one
pinned Noema Obscura worker under the Go home when it is absent.

The installer:

- selects one exact operating-system and architecture asset;
- downloads the pinned archive and published SHA-256 file;
- verifies the archive before extraction;
- rejects traversal, links, unexpected files, and oversized data;
- publishes the staged installation atomically;
- reuses the installed worker without a network request.

The installer does not hash the installed worker on every lookup. Initial
archive verification and protected publication own installation integrity.

The release needs public, anonymously readable assets for:

- Linux amd64;
- Linux arm64;
- macOS amd64;
- macOS arm64;
- Windows amd64.

The repository release workflow can build these assets. Asset publication to
a public location remains an external release gate.

A private GitHub release is insufficient because the installer has no GitHub
credential. The configured release URLs must work without authentication.

## Validation

Completed Linux validation covers:

- the full Go unit suite with `CGO_ENABLED=0`;
- `go vet`, `staticcheck`, and `govulncheck`;
- selected race tests;
- fresh-home startup, GraphQL access, stop, and restart;
- five no-CGo release target builds;
- web schema generation and production build;
- retained Rust formatting, checks, lints, and unit tests;
- desktop runtime mapping and Go sidecar packaging.

The store suite includes concurrent WAL writes, a cold reopen, and
`PRAGMA integrity_check`.

This Linux host cannot run native macOS or Windows tests. Cross-builds prove
compilation only.

After branch publication, GitHub must run native jobs on `macos-15` and
`windows-2022`. The workflow also runs `ubuntu-22.04`.

Native acceptance must cover the platform filesystem behavior, SQLite WAL
test, desktop packaging, process startup, and shutdown. macOS must also cover
the Apple Foundation bridge.

Live hosted-provider and external-integration acceptance needs configured
credentials. Record any unavailable or waived case before release.

The final integrated Linux check must include the full race suite.

## Remaining Release Gates

Source implementation is complete. Release acceptance remains open until:

1. The five public Obscura assets and checksum files are available anonymously.
2. Native Linux, macOS, and Windows jobs pass after branch publication.
3. The packaged desktop starts and stops its Go sidecar on each desktop platform.
4. The macOS package starts the Apple Foundation bridge.
5. The final integrated Linux suite passes, including race detection.
6. Credential-backed provider and integration acceptance passes or has an explicit waiver.
7. Each final release artifact has a complete notice set. It covers Go modules,
   the Go toolchain, Noema, web and font assets, MPL source availability, and
   packaged native content.
8. Both Go size ratios remain below 80 percent at the release commit.

These are release evidence gates. They are not missing server source features.

## Cutover and Rollback

Create an empty Go home for cutover. Do not copy a Rust-created home into it.

Rollback starts the previous accepted Go build against the same Go home. The
repository has no Rust server binary or Rust-home rollback path.

## Stop Conditions

Stop and request a product decision if:

- client compatibility needs a material GraphQL or product behavior change;
- a Go path weakens an information, authorization, or data-loss boundary;
- a retained capability has no viable pure-Go implementation;
- a current production capability must be removed;
- authored or inclusive Go reaches its 80-percent limit;
- the previous accepted Go build cannot open the current Go home.

## Approved Decisions

| Decision | Direction |
| --- | --- |
| Server language | Go only. Keep no Rust server component. |
| Native linkage | Use no CGo. |
| Capabilities | Preserve all current production capabilities. |
| Clients | Preserve behavior. Permit small compatibility changes. |
| Platforms | Support Linux, macOS, and Windows. |
| Stored state | Create a fresh Go-owned home. |
| Cutover | Use one final replacement after acceptance. |
| Obscura | Install one pinned external binary into the Go home when selected. |
| Scripts | Use a pure-Go Lua 5.4 runtime. |

The implementation owns dependency choices and internal package structure.
The release gates above own final shipment.
