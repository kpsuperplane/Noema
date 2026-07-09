# Noema Codebase Remediation Program

**Mode:** Implement

**Goal:** Close every item in `docs/codebase-audit-tracker.md` through a
rolling, evidence-gated program with explicit architecture planning, focused
implementation, adversarial review, automated validation, and daemon-backed
browser acceptance.

## Operating Model

- The root agent owns orchestration, integration, live `~/.noema` access,
  browser QA, commits, and milestone gates.
- A fresh Sol-high planner creates the decision-complete plan immediately
  before each milestone from the current repository state.
- A separate Sol-high agent reviews each milestone plan and completed unit
  read-only.
- One fresh Terra-medium implementer works at a time in the shared main
  worktree. Implementers edit and validate but do not commit.
- Root commits only after focused validation and task review pass. Root never
  pushes without explicit user authorization.
- Automatic advancement stops only for a genuine product decision, missing
  external authority, or a blocking failure that cannot be resolved safely.

Requested model/reasoning levels are recorded in every agent brief. If the
spawn interface cannot prove the routing, the task record calls it requested
rather than verified.

## Execution Order

1. Foundation guardrails, validation, browser harness, and baselines.
2. Milestones 1 and 2 as one trust/integrity macro-phase with a single schema
   rewrite.
3. Milestone 3 runtime concurrency, cancellation, and lifecycle.
4. Milestone 4 provider and integration hardening.
5. Milestone 7 architecture and measured performance.
6. Milestone 6 UX, accessibility, and language-independent semantics.
7. Milestone 5 self-contained release engineering.
8. Milestone 8 final CI, documentation, licensing, and repository closure.

Architecture precedes release packaging so crate, server, and asset boundaries
do not invalidate the packaged application. UX follows the stabilized runtime
and API contracts. The final CI milestone audits and completes coverage rather
than deferring feature tests from their owning milestones.

## Durable Control Artifacts

- `docs/codebase-audit-tracker.md` is the canonical checkbox list.
- `docs/codebase-audit-ledger.md` maps every canonical checkbox ID to exactly
  one primary milestone, known consumers, automated coverage, browser
  scenario, acceptance evidence, and completion commit.
- `docs/superpowers/plans/` contains this program and the just-in-time
  milestone plans.
- `docs/context/current.md` records concise implemented state, decisions, and
  open loops at every milestone boundary.
- `.superpowers/sdd/progress.md` is ignored, ephemeral recovery state and is
  never committed.

No tracker item is checked merely because it is assigned. Completion requires
implementation or an explicitly permitted evidence-based no-split decision,
fresh validation evidence, review approval, and a completion commit.

## Browser And Runtime Guardrails

### Authenticated product environment

Root may start the exact modified daemon with `NOEMA_HOME="$HOME/.noema"` and
use the existing Codex credentials for narrow positive-path acceptance checks.
Root must not inspect or capture credential contents, browser storage, cookies,
authorization headers, provider payloads, transcript text, memory content, or
account identifiers. Credential state under `~/.noema/providers/**` is
protected from deletion, copying into the repository, screenshots, traces, and
CI artifacts.

Foundation live QA is limited to state-safe navigation plus one minimal Codex
chat. It accepts valid ready, unavailable, and empty states without mutating
provider, MCP, memory, or settings configuration. Live database recreation is
reserved for the schema milestone, where the daemon is stopped first and the
SQLite database, WAL, and SHM are moved together to a private rollback
location. Whole-home deletion is never allowed.

### Ephemeral environment

Destructive, adversarial, cross-user, malformed-data, fault-injection, and
simulated-side-effect scenarios run against a temporary `NOEMA_HOME`, random
loopback port, deterministic provider, and local test integrations. The test
harness owns and joins every daemon, browser, and child process.

### Per-task evidence

Every observable change defines routes, preconditions, actions, expected
visible and accessible state, expected network behavior, recovery state, and
console-error expectations. Root starts the exact daemon, waits for explicit
readiness, uses the in-app browser, captures sanitized evidence when useful,
and shuts the daemon down cleanly. Pure documentation, licensing, CI metadata,
and proven behavior-neutral moves may skip browser QA with a recorded reason.

## Milestone Outcomes

### Foundation

- Persist this program and a complete one-owner coverage ledger.
- Establish minimal CI and generated-artifact cleanliness.
- Repair the stale frontend navigation tests and add normal test scripts.
- Add an isolated deterministic daemon/browser acceptance harness.
- Capture a credential-safe live `~/.noema` browser baseline.
- Capture reproducible toolchain, Rust build, and frontend bundle baselines
  without changing or bypassing `sccache`.
- Add the root MIT license and a provisional redistribution inventory.

### Trust and integrity (Milestones 1–2)

- Establish request principals, run authority, scopes, policy decisions,
  content trust labels, and safe errors.
- Authenticate HTTP, WebSocket, downloads, and Tauri IPC; reject invalid
  Host/Origin combinations and non-loopback binds.
- Repair private filesystem permissions; redact and rotate diagnostics.
- Authenticate and isolate Mnemosyne; trust only server-derived OAuth callback
  state; validate MCP schemas and quarantine untrusted content.
- Perform one schema-v2 rewrite containing governance ownership, approvals,
  foreign keys, uniqueness, timestamp, and structural fingerprint contracts.
- Add atomic private-file operations, recoverable file/database sagas, opaque
  MCP identity, complete discovery snapshots, and truthful memory scopes.
- Enforce exact one-shot MCP approvals and execution-time revalidation before
  write/export tools become model-visible.

### Runtime (Milestone 3)

- Replace the global actor with supervised per-conversation workers, bounded
  queues, explicit submit/cancel/interrupt commands, owned tasks, database
  backpressure, transactional sequencing, and exactly-once finalization.
- Persist distinct completed, failed, cancelled, and interrupted outcomes;
  remove detached subscription relays; shut down in dependency order.

### Integrations (Milestone 4)

- Centralize outbound HTTP timeouts, redirects, byte caps, retries, and safe
  diagnostics.
- Harden Exa, Codex OAuth refresh, Foundation bridge identity/restart/session
  behavior, Mnemosyne readiness/death/live generation swaps, and MCP cursor,
  buffer, page, and duration limits.

### Architecture and performance (Milestone 7)

- Stabilize canonical domain types and provider-neutral ports; split backend
  and frontend hotspots; address measured query, transcript, schema, prompt,
  and bundle bottlenecks.
- Extract domain, store, integrations, runtime, API, composition, server, and
  desktop boundaries when their documented measurement gates pass.
- Preserve acyclic dependencies, narrow public APIs, workspace lints, and
  focused package tests. Stop optional further splitting when measured gains
  flatten under the approved thresholds.

### UX and semantics (Milestone 6)

- Replace boolean state combinations with typed loading, execution, recovery,
  provider, memory, and enforcement states.
- Separate drafting from submission; add Stop, recovery, empty, and actionable
  failure surfaces without replaying unfinished external actions.
- Present provider and MCP enforcement honestly; remove English phrase
  matching as semantic authority; add Unicode-safe anchors, bidi support,
  screen-reader announcements, and reduced-motion behavior.

### Release (Milestone 5)

- Fail closed on missing assets; define versioned release and backup/restore
  manifests; bundle pinned Python, Mnemosyne, model assets, Foundation bridge,
  notices, and frontend resources without ambient runtime fallbacks.
- Add installed-app tests, signing, notarization, explicit-confirmation
  updates, checksums, SBOM, and GitHub Release workflows. External credential
  absence may pause publication but never convert a required gate into a pass.

### Final closure (Milestone 8)

- Complete the Linux, macOS, Windows, frontend, Python, packaging, and
  installed-app CI matrix.
- Reconcile authoritative documentation, terminology, routes, licenses,
  ignored artifacts, unused dependencies, and generated files.
- Run a final Sol-high audit and complete live browser regression. Every
  checkbox must have concrete evidence before closure.

## Standard Validation

Every implementation unit runs focused tests and `git diff --check`. Every
milestone runs:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Relevant milestones additionally run Python, frontend, generated-artifact,
daemon/browser, packaging, and installed-application suites. Observable
functionality cannot be declared complete from compilation and unit tests
alone; the task's daemon-backed browser matrix must pass.

## Locked Decisions

- Web remains authenticated and loopback-only.
- Ordinary sends queue; Stop cancels; stop-and-replace is distinct.
- MCP approval authorizes one exact dispatch attempt; ambiguous external
  effects are never automatically retried.
- Memory settings use an atomic live generation swap shared by GraphQL and
  chat.
- Apple Silicon macOS and GitHub Releases are the first distribution target.
- Updates require explicit confirmation.
- The project is pre-V1 and permits one explicit schema rewrite, but never
  automatic user-data deletion or a compatibility layer unless requested.
