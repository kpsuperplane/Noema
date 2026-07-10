# Deferred and Rejected Audit Work

This file prevents the 16-hour remediation program from turning deferred ideas
into hidden scope. Items move out only when a later plan gives them a concrete
budget and acceptance gate.

## Deliberately Rejected in This Program

- Playwright, browser automation, screenshot matrices, and live browser QA.
- A new frontend unit-test harness or restoration of deleted frontend tests.
- Raw HTTP/WebSocket adversarial fixtures after Axum owns protocol parsing.
- Password login, JWTs, `axum-login`, remote account recovery, and role UI before
  a remote/multi-human login product slice exists.
- A general policy DSL, universal principal table, content-taint lattice, or
  generic saga framework.
- A complete schema fingerprint/rewrite solely to detect every possible manual
  SQLite mutation.
- New frontend state/design frameworks without measured net LOC reduction.
- Compatibility shims for pre-V1 schema or removed internal APIs.
- Crate splitting performed only to make the dependency diagram symmetrical.
- Custom license, SBOM, updater, HTTP, WebSocket, cookie, OAuth-protocol, or JSON
  Schema implementations when selected public tooling covers the requirement.

## Security and Governance Deferred Unless Completed Incidentally

- General multi-human role and administrator management.
- Authorization across future workspace/project/task ownership models that do
  not yet have complete product surfaces.
- Persisting a full policy-decision record for every read-only operation.
- Memory-purpose policy beyond concrete supported human/conversation filters.
- Automatic approval-model hooks and policy-language composition.
- Cross-device or remote web authentication.
- Comprehensive diagnostic rotation/retention UI.
- Formal mixed-content trust propagation across every model prompt.

Write/export MCP tools remain blocked when exact one-shot enforcement is not
implemented; the program must not expose them optimistically.

## Data Integrity Deferred Unless Required by Phase 3

- Complete foreign-key coverage across every persisted table.
- Full cascade/restrict/nullification review for future objects.
- Structural fingerprints beyond schema version/reset detection.
- General file/database saga persistence and recovery UI.
- A migration framework before pre-V1 data compatibility is required.
- Workspace/project/task memory scopes not representable by Mnemosyne.
- Complete timestamp normalization of untouched tables.
- External-memory `port` contract cleanup if not touched.
- Full MCP discovery snapshot/version history beyond safe current reconciliation.

## Runtime and Integration Follow-ups

- Fair scheduling, queue observability, and configurable per-conversation
  backpressure beyond bounded queues.
- Durable resumption of arbitrary in-flight external actions after restart.
- Foundation Local cancellation until the upstream bridge supports it safely.
- A general retry framework; ambiguous side effects remain terminal.
- Live generation-scoped memory swaps if the Phase 3 lifecycle work cannot
  finish them coherently.
- Exhaustive MCP cursor/cycle/fault matrices beyond focused bounds tests.
- Provider/Mnemosyne fault injection requiring external fixture services.
- Clean-machine packaged integration checks.

## UX and Accessibility Follow-ups

- Broad visual redesign or navigation reorganization.
- Touch-device automation and screenshot-based responsive matrices.
- Complete screen-reader audit of untouched surfaces.
- Comprehensive RTL/CJK article fixtures beyond Unicode-safe touched code.
- Rich memory provenance/history/entity drill-ins.
- Recovery UI for external side effects whose outcome cannot be known.
- Full provider-status taxonomy on untouched administration surfaces.

## Remaining Crate Sequence

After `noema-server` and `noema-store`, reconsider in this order using Phase 5's
measurement gate:

1. `noema-domain`
2. `noema-runtime`
3. `noema-integrations`
4. `noema-api`
5. composition-only `noema-core`

Each later extraction requires its own two-hour maximum plan, a +300 LOC cap,
at least a 20% focused incremental improvement, no more than 10% clean-build
regression, and an acyclic dependency graph. Provider- or MCP-specific crates
are not planned until profiling shows an independent hotspot.

## Release Follow-ups

- macOS signing and notarization once credentials are available.
- Published updater manifests and explicit-confirmation update UX.
- Windows/Linux installed bundles and platform-specific sidecar provisioning.
- Complete backup/restore product flows and consistency-aware live backup.
- Installed-application self-tests.
- GitHub Release publication; pushing/releasing always requires explicit user
  approval.

## Tracker Reconciliation Rule

At every phase gate, classify touched tracker rows as:

- **complete** — implementation and required validation passed;
- **partial** — leave unchecked and describe the exact landed subset here;
- **deferred** — leave unchecked and reference the section above;
- **rejected** — leave unchecked and state the design reason.

Do not add hundreds of row-to-evidence ledger lines. A phase commit, its test
output summary, LOC delta, and concise tracker note are sufficient evidence.
