# Codebase Audit Work Tracker

This checklist tracks the cleanup and refactor work identified by the July 2026
whole-codebase adversarial audit. Keep tasks unchecked until implementation and
relevant validation are complete.

The bounded deletion-first execution program is tracked in
[`docs/superpowers/plans/2026-07-10-noema-remediation/`](superpowers/plans/2026-07-10-noema-remediation/README.md).
It targets 12 aggregate active agent-hours, stops at 16, and requires maintained
source LOC to fall from 93,924 to 89,000 or fewer. Unselected audit work remains
in this tracker and is classified in the program's deferred backlog.

Priority labels:

- **P0**: trust boundary or data exposure; address before production exposure.
- **P1**: substantial correctness, reliability, or release risk.
- **P2**: important product quality, scalability, or maintainability work.
- **P3**: cleanup and consistency work with lower immediate risk.

## Security And Privacy

- [x] **P0** Authenticate GraphQL HTTP requests with a request-scoped principal.
- [x] **P0** Authenticate GraphQL WebSocket connections and subscriptions.
- [x] **P0** Authorize artifact downloads instead of exposing bearerless local URLs.
- [x] **P0** Reject DNS-rebinding-shaped `Origin` and `Host` combinations.
- [x] **P0** Refuse non-loopback web binding until transport authentication is configured.
- [x] **P0** Add authorization guards to sensitive provider, MCP, memory, and settings resolvers.
- [ ] **P0** Enforce `0700` permissions on the Noema root and private state directories.
- [ ] **P0** Enforce `0600` permissions on SQLite, WAL/SHM, configuration, artifacts, and diagnostic files where appropriate.
- [ ] **P0** Repair unsafe permissions on existing Noema installations during startup.
- [x] **P0** Remove the managed memory sidecar and its bearer-token trust boundary in the native Markdown cutover.
- [x] **P0** Validate native memory page ownership, scope, and source citations before publication.
- [ ] **P1** Bound, redact, permission, and rotate `errors.log`.
- [ ] **P1** Stop storing complete provider responses and MCP payloads in unredacted diagnostics.
- [x] **P1** Derive MCP OAuth callback URLs from trusted web/desktop transport state.
- [ ] **P1** Add expiration, one-shot state, bounded storage, and cleanup to OAuth and authentication attempts.
- [ ] **P1** Add a restrictive production Content Security Policy to the Tauri webview.
- [ ] **P1** Validate MCP tool arguments and structured results against reviewed schemas.
- [ ] **P1** Mark fetched web content as untrusted before it can influence later tool calls.

## Governance And Authorization

- [ ] **P0** Pass acting principal and active scopes into the Capability Gateway.
- [ ] **P0** Enforce MCP read, write, and export classifications during execution.
- [ ] **P0** Resolve source and destination ownership before executing governed tools.
- [ ] **P0** Integrate approval decisions with tool execution instead of only persisting approval rows.
- [ ] **P0** Revalidate approvals, provider/tool policy, health, and authentication immediately before side effects.
- [ ] **P1** Stop accepting client-supplied `reviewed_by` values as authoritative identity.
- [ ] **P1** Make memory retrieval purpose participate in policy decisions when additional scopes are introduced.
- [x] **P1** Advertise only the implemented `human:local` memory scope in the first slice.
- [x] **P1** Keep write/export MCP tools hidden or blocked until execution-time governance is complete.
- [ ] **P2** Persist an auditable policy decision record for every governed tool attempt.

## Correctness And Data Integrity

- [ ] **P1** Add SQLite foreign keys for conversations, turns, items, summaries, artifacts, versions, MCP records, provider preferences, and bindings.
- [ ] **P1** Define deliberate cascade, restrict, and nullification behavior for every relationship.
- [ ] **P1** Add uniqueness constraints for single-active-summary and primary-conversation invariants.
- [ ] **P1** Replace permissive schema bootstrap with version and structural fingerprint validation.
- [ ] **P1** Reject stale or incomplete databases with a clear pre-V1 reset instruction.
- [ ] **P1** Make MCP setup a recoverable file/database saga.
- [ ] **P1** Make provider secret create, update, clear, and delete workflows recoverable.
- [ ] **P1** Write secret files atomically through temporary files, `fsync`, and rename.
- [ ] **P1** Wrap multi-row provider and MCP deletions in SQLite transactions.
- [ ] **P1** Allocate immutable random MCP server and tool IDs before writing filesystem state.
- [ ] **P1** Reconcile MCP discovery as a complete versioned snapshot.
- [ ] **P1** Disable or delete tools absent from the latest MCP discovery result.
- [ ] **P1** Replace lossy punctuation-derived MCP tool IDs with opaque IDs plus exact `(server, name)` identity.
- [ ] **P1** Make primary-conversation lookup/create/update one transaction.
- [ ] **P1** Make context-summary supersede/activate one transaction.
- [x] **P1** Limit first-slice memory retrieval to the one canonical `human:local` tree.
- [ ] **P1** Define filtering and result attribution when project, workspace, or agent memory scopes are introduced.
- [ ] **P1** Ensure every turn failure path marks the durable turn failed and restores a valid agent status.
- [ ] **P2** Standardize persisted timestamps on RFC3339 UTC.
- [ ] **P2** Validate provider-returned Exa final URLs before persistence and display.
- [ ] **P2** Report summarized-fetch truncation accurately.
- [ ] **P2** Render native memory update and retrieval errors as failures rather than empty success states.

## Reliability, Concurrency, And Lifecycle

- [ ] **P1** Replace the single global turn actor with supervised per-conversation workers.
- [ ] **P1** Add first-class turn cancellation and interruption commands.
- [x] **P1** Track and join runtime-owned background tasks during shutdown.
- [ ] **P1** Stop detaching compaction, provider generation, and connection-handler tasks without ownership.
- [x] **P1** Run native memory updates through runtime-owned background tasks.
- [ ] **P1** Add deadlines to all provider calls, including Exa search and fetch.
- [ ] **P1** Stream and cap Exa response bodies before JSON decoding.
- [x] **P1** Remove managed/external memory endpoint lifecycle and readiness state in the native cutover.
- [ ] **P1** Apply saved Memory model changes consistently to GraphQL and chat runtime paths.
- [ ] **P1** Assign unique Foundation bridge request IDs.
- [ ] **P1** Restart or invalidate a Foundation bridge after a timed-out request.
- [x] **P1** Refresh Foundation sessions when instructions, identity, model, or available tools change.
- [ ] **P1** Make Codex OAuth refresh single-flight per provider account.
- [ ] **P1** Persist refreshed Codex OAuth tokens atomically.
- [ ] **P2** Bound MCP SSE cursor traversal, pagination, buffering, and total operation duration.
- [ ] **P2** Detect MCP cursor cycles.
- [ ] **P2** Remove unused GraphQL subscription channels after their last subscriber leaves.
- [ ] **P2** Introduce a scoped turn finalizer to centralize completion and failure cleanup.

## Performance And Scalability

- [ ] **P1** Move synchronous `rusqlite` work off Tokio executor threads.
- [ ] **P1** Replace the global transcript append lock with database-backed or per-conversation sequencing.
- [x] **P2** Build and cache one GraphQL schema per runtime host.
- [ ] **P2** Establish a frontend bundle-size budget.
- [ ] **P2** Profile and reduce the approximately 926 kB minified initial JavaScript bundle.
- [ ] **P2** Lazy-load heavy onboarding, dialog, and infrequent settings code where practical.
- [ ] **P2** Avoid loading a full conversation transcript to validate one multiple-choice selection.
- [ ] **P2** Add indexes aligned with finalized foreign keys and common query filters.
- [ ] **P2** Avoid repeated identical memory queries across nominal scopes.
- [ ] **P2** Avoid constructing a summarizer provider for Exa fetches that do not use it.
- [ ] **P2** Profile prompt assembly and compaction against long transcripts.

## UX And Product Behavior

- [ ] **P1** Prevent message submission while a turn submission is already pending.
- [ ] **P1** Keep drafting and submission availability as separate composer states.
- [ ] **P1** Replace the perpetual loading skeleton for a successfully loaded empty conversation.
- [ ] **P1** Keep memory settings, memory browsing, ingestion, and agent retrieval on the same active backend.
- [ ] **P1** Clearly indicate when a saved memory setting requires restart, or make it live.
- [ ] **P1** Avoid presenting MCP classifications and approvals as enforced controls before enforcement exists.
- [ ] **P2** Add a recovery surface for interrupted or orphaned turns.
- [ ] **P2** Distinguish unavailable, unauthenticated, unhealthy, unsupported, and misconfigured providers consistently.
- [x] **P2** Show actionable native Markdown memory failure states rather than “Found no memories.”

## Accessibility, Internationalization, And Semantics

- [ ] **P2** Remove English-only `I am`/`I'm` identity inference.
- [ ] **P2** Replace phrase matching used as semantic authority with structured or language-aware interpretation.
- [ ] **P2** Add native Markdown memory rendering coverage for CJK, RTL, accented, duplicate, and empty headings.
- [ ] **P2** Audit loading, empty, error, and status announcements for screen readers.
- [ ] **P3** Preserve reduced-motion behavior while localizing global animation rules.

## Release And Deployment

- [x] **P1** Fail release builds when required web assets are missing.
- [x] **P1** Remove the Python memory runtime and ship native Markdown memory inside Noema.
- [ ] **P1** Bundle the Foundation Swift bridge in the Tauri application.
- [ ] **P1** Test chat, memory, Foundation Local, GraphQL IPC, and route chunks from an installed application.
- [ ] **P2** Add clean-machine release-build validation.
- [ ] **P2** Add macOS signing, notarization, update, and distribution workflows.
- [ ] **P2** Add backup and restore validation before exposing those product features.

## Codebase Architecture And Maintainability

- [ ] **P1** Split `daemon/runtime/turn.rs` into orchestration, provider phases, tool-loop execution, and finalization.
- [ ] **P1** Split `daemon/runtime/transcript_persistence.rs` into persistence, event projection, display metadata, and failure lifecycle.
- [ ] **P1** Split `provider/contract.rs` into generation types, response parsing, and provider errors.
- [x] **P1** Extract a shared Responses dialect core used by OpenAI and Codex adapters.
- [ ] **P1** Split `daemon/runtime/local_tools.rs` into provider resolution, credential loading, execution, and result adaptation.
- [ ] **P2** Move GraphQL integration tests out of `graphql/schema.rs` and beside domain resolvers.
- [ ] **P2** Split `daemon/tests.rs` by turn lifecycle, tools, memory, compaction, and providers.
- [ ] **P2** Split frontend `App.tsx` into conversation, authentication, subscription, paging, and submission controllers.
- [ ] **P2** Split frontend `graphql/operations.ts` by domain.
- [ ] **P2** Replace raw GraphQL error strings with typed safe errors and diagnostic correlation IDs.
- [ ] **P2** Replace duplicated SQL/Rust/GraphQL/TypeScript string enums with canonical typed definitions.
- [ ] **P2** Replace generic MCP `safe_config: Value` with typed transport configuration and explicit secret references.
- [ ] **P2** Centralize provider HTTP timeouts, byte caps, redirect policy, retry behavior, and diagnostics.
- [ ] **P3** Enforce one Noema-owned React component per file.
- [ ] **P3** Remove obsolete frontend barrel wrappers and dead state/prop chains.
- [ ] **P3** Move component-specific animation rules out of global CSS where practical.

### Multi-Crate Workspace Architecture

The July 2026 decomposition is complete. The workspace uses explicit domain,
infrastructure, composition, API, and shell owners rather than a generic shared
crate or umbrella facade:

```text
noema-server / noema-desktop
  ├── noema-host (composition)
  └── noema-api (GraphQL)
        ├── noema-host service handles
        ├── noema-runtime
        ├── noema-store
        └── semantic contracts

noema-host
  ├── noema-runtime
  ├── noema-store
  └── concrete adapters and semantic contracts

noema-runtime
  ├── noema-store
  └── semantic contracts

noema-store
  └── semantic contracts
```

`noema-capabilities-mcp` is an independent child package that depends on
`noema-capabilities`; the parent does not depend on the child. Hosted providers,
Foundation Local, web backends, and private local GGUF implementations live in
`noema-providers`. `noema-store` owns SQLite implementation and persistence read
models, `noema-runtime` owns governed execution, `noema-host` owns concrete
application composition and lifecycle, and `noema-api` owns the transport-neutral
GraphQL schema. HTTP/WebSocket and Tauri concerns stay in their respective shell
crates.

Completed extraction work:

- [x] **P1** Extract domain and integration contracts into their explicit owners without introducing a generic common crate.
- [x] **P1** Extract SQLite persistence into `noema-store` without GraphQL, host, or provider-transport ownership.
- [x] **P1** Extract governed execution into transport-neutral `noema-runtime`.
- [x] **P1** Extract configuration, startup, composition, and shutdown into `noema-host`.
- [x] **P1** Extract the shared GraphQL schema into transport-neutral `noema-api`.
- [x] **P1** Keep HTTP/session/assets in `noema-server` and Tauri IPC/events in `noema-desktop`.
- [x] **P2** Move subsystem tests into their owning crates so focused package tests avoid unrelated dependencies.
- [x] **P2** Preserve workspace dependency inheritance, lints, MSRV, feature boundaries, and validation commands.

## Testing And CI

- [x] **P1** Add CI covering Rust, frontend, generated artifacts, and release web-asset embedding; clean-machine application packaging remains deferred.
- [ ] **P1** Add a frontend `test` script and include it in normal validation.
- [ ] **P1** Fix the stale shell-navigation tests for Memory and Usage settings.
- [ ] **P1** Stabilize Foundation subprocess tests under the default parallel Rust test command.
- [ ] **P1** Add multi-conversation concurrency and hung-turn shutdown tests.
- [ ] **P1** Add multi-owner memory isolation tests when shared ownership is introduced.
- [ ] **P1** Add Unix permission tests for the Noema root, SQLite/WAL/SHM, artifacts, and logs.
- [ ] **P1** Add fault-injection tests at every filesystem/database workflow boundary.
- [x] **P1** Add GraphQL authentication, DNS-rebinding, WebSocket, and artifact authorization tests.
- [ ] **P1** Add MCP policy matrices covering ownership, scopes, approval, revocation, and execution-time revalidation.
- [ ] **P2** Add stale-tool reconciliation, tool-ID collision, schema violation, and cursor-cycle tests.
- [ ] **P2** Add concurrent primary-conversation and active-summary creation tests.
- [x] **P2** Regenerate GraphQL schema, operation types, and route trees in CI and require a clean diff.
- [ ] **P2** Add clean-release and installed-application tests.

## Documentation And Repository Hygiene

- [x] **P2** Retire current Supermemory and Mnemosyne terminology with the native Markdown cutover.
- [ ] **P2** Remove stale SurrealDB references from current documentation.
- [x] **P2** Resolve contradictory `/memory` route documentation.
- [x] **P2** Remove obsolete `/remember`, `/memory/graph`, and flat Settings route claims from current contracts.
- [x] **P2** Keep `docs/context/current.md` below 300 lines with current state, decisions, and open loops.
- [ ] **P2** Separate historical plans and superseded architecture from current authoritative docs.
- [x] **P3** Untrack Python `__pycache__`, `.pyc`, and `.egg-info` artifacts.
- [ ] **P3** Untrack ignored `.superpowers` task reports.
- [x] **P3** Add Python cache and packaging outputs to `.gitignore`.
- [x] **P3** Remove unused `@xyflow/react`, `d3-force`, and `@types/d3-force` dependencies.
- [x] **P2** Add the missing root MIT `LICENSE` file.
- [x] **P2** Document the native memory release layout without a Python sidecar requirement.

## Recommended Milestones

- [ ] **Milestone 1:** Close security, privacy, and governance trust-boundary gaps.
- [ ] **Milestone 2:** Establish relational integrity and recoverable persistence workflows.
- [ ] **Milestone 3:** Introduce per-conversation concurrency, cancellation, and owned lifecycle management.
- [ ] **Milestone 4:** Harden provider, Foundation, OAuth, web, and memory integrations.
- [ ] **Milestone 5:** Produce deterministic, self-contained release artifacts.
- [ ] **Milestone 6:** Resolve user-facing correctness, accessibility, and internationalization issues.
- [ ] **Milestone 7:** Split architectural hotspots and address measured performance bottlenecks.
- [ ] **Milestone 8:** Complete CI, documentation alignment, licensing, and repository cleanup.
