# Codebase Audit Work Tracker

This checklist tracks the cleanup and refactor work identified by the July 2026
whole-codebase adversarial audit. Keep tasks unchecked until implementation and
relevant validation are complete.

Program controls: [remediation program](superpowers/plans/2026-07-09-noema-remediation-program.md),
[audit coverage ledger](codebase-audit-ledger.md).

Priority labels:

- **P0**: trust boundary or data exposure; address before production exposure.
- **P1**: substantial correctness, reliability, or release risk.
- **P2**: important product quality, scalability, or maintainability work.
- **P3**: cleanup and consistency work with lower immediate risk.

## Security And Privacy

- [ ] `SEC-001` **P0** Authenticate GraphQL HTTP requests with a request-scoped principal.
- [ ] `SEC-002` **P0** Authenticate GraphQL WebSocket connections and subscriptions.
- [ ] `SEC-003` **P0** Authorize artifact downloads instead of exposing bearerless local URLs.
- [ ] `SEC-004` **P0** Reject DNS-rebinding-shaped `Origin` and `Host` combinations.
- [ ] `SEC-005` **P0** Refuse non-loopback web binding until transport authentication is configured.
- [ ] `SEC-006` **P0** Add authorization guards to sensitive provider, MCP, memory, and settings resolvers.
- [ ] `SEC-007` **P0** Enforce `0700` permissions on the Noema root and private state directories.
- [ ] `SEC-008` **P0** Enforce `0600` permissions on SQLite, WAL/SHM, configuration, artifacts, and diagnostic files where appropriate.
- [ ] `SEC-009` **P0** Repair unsafe permissions on existing Noema installations during startup.
- [ ] `SEC-010` **P0** Authenticate the managed Mnemosyne sidecar with a runtime-generated bearer token.
- [ ] `SEC-011` **P0** Reject Mnemosyne facts whose source ownership cannot be proven.
- [ ] `SEC-012` **P0** Prevent request metadata from overriding trusted memory `user_id`, `agent_id`, or `run_id` fields.
- [ ] `SEC-013` **P1** Bound, redact, permission, and rotate `errors.log`.
- [ ] `SEC-014` **P1** Stop storing complete provider responses and MCP payloads in unredacted diagnostics.
- [ ] `SEC-015` **P1** Derive MCP OAuth callback URLs from trusted web/desktop transport state.
- [ ] `SEC-016` **P1** Add expiration, one-shot state, bounded storage, and cleanup to OAuth and authentication attempts.
- [ ] `SEC-017` **P1** Add a restrictive production Content Security Policy to the Tauri webview.
- [ ] `SEC-018` **P1** Validate MCP tool arguments and structured results against reviewed schemas.
- [ ] `SEC-019` **P1** Mark fetched web content as untrusted before it can influence later tool calls.

## Governance And Authorization

- [ ] `GOV-001` **P0** Pass acting principal and active scopes into the Capability Gateway.
- [ ] `GOV-002` **P0** Enforce MCP read, write, and export classifications during execution.
- [ ] `GOV-003` **P0** Resolve source and destination ownership before executing governed tools.
- [ ] `GOV-004` **P0** Integrate approval decisions with tool execution instead of only persisting approval rows.
- [ ] `GOV-005` **P0** Revalidate approvals, policy, calibration, health, and authentication immediately before side effects.
- [ ] `GOV-006` **P1** Stop accepting client-supplied `reviewed_by` values as authoritative identity.
- [ ] `GOV-007` **P1** Make memory retrieval purpose participate in policy decisions.
- [ ] `GOV-008` **P1** Fail closed when a requested memory scope cannot be represented by the memory backend.
- [ ] `GOV-009` **P1** Keep write/export MCP tools hidden or blocked until execution-time governance is complete.
- [ ] `GOV-010` **P2** Persist an auditable policy decision record for every governed tool attempt.

## Correctness And Data Integrity

- [ ] `DATA-001` **P1** Add SQLite foreign keys for conversations, turns, items, summaries, artifacts, versions, MCP records, provider preferences, and bindings.
- [ ] `DATA-002` **P1** Define deliberate cascade, restrict, and nullification behavior for every relationship.
- [ ] `DATA-003` **P1** Add uniqueness constraints for single-active-summary and primary-conversation invariants.
- [ ] `DATA-004` **P1** Replace permissive schema bootstrap with version and structural fingerprint validation.
- [ ] `DATA-005` **P1** Reject stale or incomplete databases with a clear pre-V1 reset instruction.
- [ ] `DATA-006` **P1** Make MCP setup a recoverable file/database saga.
- [ ] `DATA-007` **P1** Make provider secret create, update, clear, and delete workflows recoverable.
- [ ] `DATA-008` **P1** Write secret files atomically through temporary files, `fsync`, and rename.
- [ ] `DATA-009` **P1** Wrap multi-row provider and MCP deletions in SQLite transactions.
- [ ] `DATA-010` **P1** Allocate immutable random MCP server and tool IDs before writing filesystem state.
- [ ] `DATA-011` **P1** Reconcile MCP discovery as a complete versioned snapshot.
- [ ] `DATA-012` **P1** Disable or delete tools absent from the latest MCP discovery result.
- [ ] `DATA-013` **P1** Replace lossy punctuation-derived MCP tool IDs with opaque IDs plus exact `(server, name)` identity.
- [ ] `DATA-014` **P1** Make primary-conversation lookup/create/update one transaction.
- [ ] `DATA-015` **P1** Make context-summary supersede/activate one transaction.
- [ ] `DATA-016` **P1** Translate each supported memory scope into an actual backend filter.
- [ ] `DATA-017` **P1** Stop relabeling broad human-memory results as project, workspace, or agent results.
- [ ] `DATA-018` **P1** Deduplicate memory results before ranking and truncation.
- [ ] `DATA-019` **P1** Return the effective memory scopes used by retrieval.
- [ ] `DATA-020` **P1** Ensure every turn failure path marks the durable turn failed and restores a valid agent status.
- [ ] `DATA-021` **P2** Standardize persisted timestamps on RFC3339 UTC.
- [ ] `DATA-022` **P2** Apply `memoryGraph` page input to backend pagination.
- [ ] `DATA-023` **P2** Use the external memory `port` setting or remove it from the contract.
- [ ] `DATA-024` **P2** Validate provider-returned Exa final URLs before persistence and display.
- [ ] `DATA-025` **P2** Report summarized-fetch truncation accurately.
- [ ] `DATA-026` **P2** Render structured memory-service errors as failures rather than empty success states.

## Reliability, Concurrency, And Lifecycle

- [ ] `RUN-001` **P1** Replace the single global turn actor with supervised per-conversation workers.
- [ ] `RUN-002` **P1** Add first-class turn cancellation and interruption commands.
- [ ] `RUN-003` **P1** Track and join runtime-owned background tasks during shutdown.
- [ ] `RUN-004` **P1** Stop detaching memory ingestion, compaction, provider generation, and connection-handler tasks without ownership.
- [ ] `RUN-005` **P1** Shut down the runtime before terminating Mnemosyne and the memory model proxy.
- [ ] `RUN-006` **P1** Add deadlines to all provider calls, including Exa search and fetch.
- [ ] `RUN-007` **P1** Stream and cap Exa response bodies before JSON decoding.
- [ ] `RUN-008` **P1** Add a real Mnemosyne readiness probe covering database, embeddings, and model connectivity.
- [ ] `RUN-009` **P1** Monitor the managed Mnemosyne child after startup and surface later process death.
- [ ] `RUN-010` **P1** Retry managed-sidecar startup if the selected loopback port is stolen.
- [ ] `RUN-011` **P1** Apply saved memory endpoint and model changes consistently to GraphQL and chat runtime paths.
- [ ] `RUN-012` **P1** Assign unique Foundation bridge request IDs.
- [ ] `RUN-013` **P1** Restart or invalidate a Foundation bridge after a timed-out request.
- [ ] `RUN-014` **P1** Refresh Foundation sessions when instructions, identity, model, or available tools change.
- [ ] `RUN-015` **P1** Make Codex OAuth refresh single-flight per provider account.
- [ ] `RUN-016` **P1** Persist refreshed Codex OAuth tokens atomically.
- [ ] `RUN-017` **P2** Bound MCP SSE cursor traversal, pagination, buffering, and total operation duration.
- [ ] `RUN-018` **P2** Detect MCP cursor cycles.
- [ ] `RUN-019` **P2** Remove unused GraphQL subscription channels after their last subscriber leaves.
- [ ] `RUN-020` **P2** Introduce a scoped turn finalizer to centralize completion and failure cleanup.

## Performance And Scalability

- [ ] `PERF-001` **P1** Move synchronous `rusqlite` work off Tokio executor threads.
- [ ] `PERF-002` **P1** Replace the global transcript append lock with database-backed or per-conversation sequencing.
- [ ] `PERF-003` **P2** Build and cache one GraphQL schema per runtime host.
- [ ] `PERF-004` **P2** Establish a frontend bundle-size budget.
- [ ] `PERF-005` **P2** Profile and reduce the approximately 926 kB minified initial JavaScript bundle.
- [ ] `PERF-006` **P2** Lazy-load heavy onboarding, dialog, and infrequent settings code where practical.
- [ ] `PERF-007` **P2** Avoid loading a full conversation transcript to validate one multiple-choice selection.
- [ ] `PERF-008` **P2** Add indexes aligned with finalized foreign keys and common query filters.
- [ ] `PERF-009` **P2** Avoid repeated identical memory queries across nominal scopes.
- [ ] `PERF-010` **P2** Avoid constructing a summarizer provider for Exa fetches that do not use it.
- [ ] `PERF-011` **P2** Profile prompt assembly and compaction against long transcripts.

## UX And Product Behavior

- [ ] `UX-001` **P1** Prevent message submission while a turn submission is already pending.
- [ ] `UX-002` **P1** Keep drafting and submission availability as separate composer states.
- [ ] `UX-003` **P1** Replace the perpetual loading skeleton for a successfully loaded empty conversation.
- [ ] `UX-004` **P1** Keep memory settings, memory browsing, ingestion, and agent retrieval on the same active backend.
- [ ] `UX-005` **P1** Clearly indicate when a saved memory setting requires restart, or make it live.
- [ ] `UX-006` **P1** Avoid presenting MCP classifications and approvals as enforced controls before enforcement exists.
- [ ] `UX-007` **P2** Add a recovery surface for interrupted or orphaned turns.
- [ ] `UX-008` **P2** Distinguish unavailable, unauthenticated, unhealthy, unsupported, and misconfigured providers consistently.
- [ ] `UX-009` **P2** Show actionable memory-service failure states rather than “Found no memories.”

## Accessibility, Internationalization, And Semantics

- [ ] `A11Y-001` **P2** Generate collision-resistant anchors for non-Latin memory headings.
- [ ] `A11Y-002` **P2** Remove English-only `I am`/`I'm` identity inference.
- [ ] `A11Y-003` **P2** Replace phrase matching used as semantic authority with structured or language-aware interpretation.
- [ ] `A11Y-004` **P2** Add memory article coverage for CJK, RTL, accented, duplicate, and empty headings.
- [ ] `A11Y-005` **P2** Audit loading, empty, error, and status announcements for screen readers.
- [ ] `A11Y-006` **P3** Preserve reduced-motion behavior while localizing global animation rules.

## Release And Deployment

- [ ] `REL-001` **P1** Fail release builds when required web assets are missing.
- [ ] `REL-002` **P1** Define and implement a distributable managed Mnemosyne runtime.
- [ ] `REL-003` **P1** Bundle or explicitly provision the required Python runtime and dependencies.
- [ ] `REL-004` **P1** Pin and lock Python dependencies used by the managed sidecar.
- [ ] `REL-005` **P1** Bundle the Foundation Swift bridge in the Tauri application.
- [ ] `REL-006` **P1** Test chat, memory, Foundation Local, GraphQL IPC, and route chunks from an installed application.
- [ ] `REL-007` **P2** Add clean-machine release-build validation.
- [ ] `REL-008` **P2** Add macOS signing, notarization, update, and distribution workflows.
- [ ] `REL-009` **P2** Add backup and restore validation before exposing those product features.

## Codebase Architecture And Maintainability

- [ ] `ARCH-001` **P1** Split `daemon/runtime/turn.rs` into orchestration, provider phases, tool-loop execution, and finalization.
- [ ] `ARCH-002` **P1** Split `daemon/runtime/transcript_persistence.rs` into persistence, event projection, display metadata, and failure lifecycle.
- [ ] `ARCH-003` **P1** Split `provider/contract.rs` into generation types, response parsing, and provider errors.
- [ ] `ARCH-004` **P1** Extract a shared Responses dialect core used by OpenAI and Codex adapters.
- [ ] `ARCH-005` **P1** Split `daemon/runtime/local_tools.rs` into provider resolution, credential loading, execution, and result adaptation.
- [ ] `ARCH-006` **P2** Move GraphQL integration tests out of `graphql/schema.rs` and beside domain resolvers.
- [ ] `ARCH-007` **P2** Split `daemon/tests.rs` by turn lifecycle, tools, memory, compaction, and providers.
- [ ] `ARCH-008` **P2** Split frontend `App.tsx` into conversation, authentication, subscription, paging, and submission controllers.
- [ ] `ARCH-009` **P2** Split frontend `graphql/operations.ts` by domain.
- [ ] `ARCH-010` **P2** Replace raw GraphQL error strings with typed safe errors and diagnostic correlation IDs.
- [ ] `ARCH-011` **P2** Replace duplicated SQL/Rust/GraphQL/TypeScript string enums with canonical typed definitions.
- [ ] `ARCH-012` **P2** Replace generic MCP `safe_config: Value` with typed transport configuration and explicit secret references.
- [ ] `ARCH-013` **P2** Centralize provider HTTP timeouts, byte caps, redirect policy, retry behavior, and diagnostics.
- [ ] `ARCH-014` **P3** Enforce one Noema-owned React component per file.
- [ ] `ARCH-015` **P3** Remove obsolete frontend barrel wrappers and dead state/prop chains.
- [ ] `ARCH-016` **P3** Move component-specific animation rules out of global CSS where practical.

### Multi-Crate Workspace Plan

Splitting `noema-core` should optimize incremental development builds and
architectural isolation rather than assume that more crates will improve clean
workspace builds. The first pass should remain deliberately small: additional
rustc invocations and cross-crate APIs can make clean builds slower when the
boundaries are too granular.

Target dependency direction:

```text
noema-domain
    |---> noema-store ---------|
    |                           |---> noema-runtime --|
    `---> noema-integrations --|                     |
                                                      |---> noema-core
                    noema-store ---> noema-api -------|       |
                                         ^                    |---> noema-server
                                         |                    `---> noema-desktop
                                  noema-runtime
```

Proposed responsibilities:

- `noema-domain`: IDs, concrete object and conversation types, provider/tool
  contracts, capability types, and shared typed errors. Keep dependencies small.
- `noema-store`: SQLite schema, records, repositories, and store-specific tests.
- `noema-integrations`: provider adapters, MCP transports, Mnemosyne lifecycle,
  search, and web fetch. Split providers or MCP further only when measurements
  show an independent hotspot.
- `noema-runtime`: turn orchestration, tool lifecycle, context planning,
  transcript persistence, and supervised runtime workers.
- `noema-api`: GraphQL schema, resolvers, subscriptions, and API read models.
- `noema-core`: thin composition facade containing `NoemaRuntimeHost` and curated
  re-exports. Lower-level crates must never depend back on this facade.
- `noema-server`: HTTP/WebSocket transport, web asset embedding, server/dev
  binaries, and the asset-related build script.
- `noema-desktop`: existing Tauri shell depending on the composition facade.

Staged extraction:

- [x] `ARCH-017` **P2** Capture baseline `cargo build --timings` results before changing crate boundaries.
- [x] `ARCH-018` **P2** Measure incremental checks after representative GraphQL, store, provider, and frontend-asset edits.
- [ ] `ARCH-019` **P1** Extract `noema-server` so frontend assets and sidecar source changes no longer invalidate the entire core crate.
- [ ] `ARCH-020` **P1** Move `daemon/web/**`, `graphql/**`, `noema_web`, `noema_dev`, and web asset embedding into the server/API boundary.
- [ ] `ARCH-021` **P1** Make release asset generation fail when required entry assets are absent.
- [ ] `ARCH-022` **P1** Extract `noema-store` with no dependency on GraphQL, provider adapters, MCP transports, or web parsing.
- [ ] `ARCH-023` **P2** Extract stable shared types into `noema-domain` after server and store boundaries clarify dependency direction.
- [ ] `ARCH-024` **P2** Extract runtime orchestration into `noema-runtime`.
- [ ] `ARCH-025` **P2** Extract provider, MCP, memory, search, and fetch adapters into `noema-integrations`.
- [ ] `ARCH-026` **P2** Reduce `noema-core` to composition and curated public re-exports.
- [ ] `ARCH-027` **P2** Update `noema-desktop` to depend only on the composition/API surfaces it requires.
- [ ] `ARCH-028` **P2** Move subsystem tests into their owning crates so focused package tests avoid unrelated dependencies.
- [ ] `ARCH-029` **P2** Compare clean and incremental timings after each extraction and stop splitting when gains flatten.

Boundary guardrails:

- [ ] `ARCH-030` Keep dependency flow acyclic and directed from domain toward composition.
- [ ] `ARCH-031` Avoid broad internal preludes or facade imports in lower-level crates.
- [ ] `ARCH-032` Use `pub(crate)` and narrow public interfaces instead of exporting implementation details solely to complete the split.
- [ ] `ARCH-033` Avoid cross-crate generic-heavy APIs unless measurements justify them.
- [ ] `ARCH-034` Preserve workspace dependency inheritance, lints, MSRV, and validation commands across every crate.
- [ ] `ARCH-035` Keep the initial split to a small number of cohesive crates; create provider- or MCP-specific crates only from measured evidence.

## Testing And CI

- [ ] `TEST-001` **P1** Add a CI workflow covering Rust, frontend, Python, generated artifacts, and release packaging.
- [x] `TEST-002` **P1** Add a frontend `test` script and include it in normal validation.
- [x] `TEST-003` **P1** Fix the stale shell-navigation tests for Memory and Usage settings.
- [x] `TEST-004` **P1** Stabilize Foundation subprocess tests under the default parallel Rust test command.
- [ ] `TEST-005` **P1** Add multi-conversation concurrency and hung-turn shutdown tests.
- [ ] `TEST-006` **P1** Add two-user memory isolation tests against the real Mnemosyne adapter.
- [ ] `TEST-007` **P1** Add Unix permission tests for the Noema root, SQLite/WAL/SHM, artifacts, and logs.
- [ ] `TEST-008` **P1** Add fault-injection tests at every filesystem/database workflow boundary.
- [ ] `TEST-009` **P1** Add GraphQL authentication, DNS-rebinding, WebSocket, and artifact authorization tests.
- [ ] `TEST-010` **P1** Add MCP policy matrices covering ownership, scopes, approval, revocation, and execution-time revalidation.
- [ ] `TEST-011` **P2** Add stale-tool reconciliation, tool-ID collision, schema violation, and cursor-cycle tests.
- [ ] `TEST-012` **P2** Add concurrent primary-conversation and active-summary creation tests.
- [x] `TEST-013` **P2** Regenerate GraphQL schema, operation types, and route trees in CI and require a clean diff.
- [ ] `TEST-014` **P2** Add clean-release and installed-application tests.

## Documentation And Repository Hygiene

- [ ] `DOC-001` **P2** Replace remaining Supermemory references with Mnemosyne terminology.
- [ ] `DOC-002` **P2** Remove stale SurrealDB references from current documentation.
- [ ] `DOC-003` **P2** Resolve contradictory `/memory` route documentation.
- [ ] `DOC-004` **P2** Remove obsolete `/remember`, `/memory/graph`, and flat Settings route claims.
- [ ] `DOC-005` **P2** Reduce `docs/context/current.md` to concise current state, decisions, and open loops.
- [ ] `DOC-006` **P2** Separate historical plans and superseded architecture from current authoritative docs.
- [ ] `DOC-007` **P3** Untrack Python `__pycache__`, `.pyc`, and `.egg-info` artifacts.
- [ ] `DOC-008` **P3** Untrack ignored `.superpowers` task reports.
- [ ] `DOC-009` **P3** Add Python cache and packaging outputs to `.gitignore`.
- [ ] `DOC-010` **P3** Remove unused `@xyflow/react`, `d3-force`, and `@types/d3-force` dependencies.
- [x] `DOC-011` **P2** Add the missing root MIT `LICENSE` file.
- [ ] `DOC-012` **P2** Document the actual release build and packaging contract.

## Recommended Milestones

- [ ] `MILESTONE-001` **Milestone 1:** Close security, privacy, and governance trust-boundary gaps.
- [ ] `MILESTONE-002` **Milestone 2:** Establish relational integrity and recoverable persistence workflows.
- [ ] `MILESTONE-003` **Milestone 3:** Introduce per-conversation concurrency, cancellation, and owned lifecycle management.
- [ ] `MILESTONE-004` **Milestone 4:** Harden provider, Foundation, OAuth, web, and memory integrations.
- [ ] `MILESTONE-005` **Milestone 5:** Produce deterministic, self-contained release artifacts.
- [ ] `MILESTONE-006` **Milestone 6:** Resolve user-facing correctness, accessibility, and internationalization issues.
- [ ] `MILESTONE-007` **Milestone 7:** Split architectural hotspots and address measured performance bottlenecks.
- [ ] `MILESTONE-008` **Milestone 8:** Complete CI, documentation alignment, licensing, and repository cleanup.
