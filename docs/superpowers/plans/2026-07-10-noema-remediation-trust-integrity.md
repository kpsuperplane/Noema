# Noema Milestones 1+2 — Trust and Integrity Implementation Plan

> Draft for root approval. Plan-only artifact; do not commit this file. Implement sequentially on `main`. A fresh Terra-medium implementer edits one unit at a time, a fresh Sol-high reviewer inspects it read-only, and root owns integration, validation, tracker evidence, and the commit.

## Goal and exit state

This macro-phase makes every privileged path derive authority from an authenticated transport, makes persisted state structurally trustworthy, and makes external content/actions fail closed. It performs exactly one destructive pre-V1 schema rewrite, in Unit 2. It does not add migrations or compatibility shims and does not add browser/UI tests.

At exit:

- GraphQL HTTP, subscriptions, downloads, and Tauri IPC have request-scoped principals and centralized authorization.
- Web binds only to a canonical loopback authority and uses one-shot bootstrap authentication followed by an HttpOnly session.
- GraphQL exposes stable safe error codes and correlation IDs while full causes remain only in bounded redacted diagnostics.
- Schema v2 enforces ownership, referential integrity, uniqueness, RFC 3339 timestamps, policy/approval state, complete MCP discovery, recovery journals, and a structural fingerprint.
- Private files are atomic and permission-repaired; provider/MCP mutations recover coherently without recording secrets in SQLite or logs.
- OAuth callbacks are server-derived, bounded, expiring, and one-shot.
- Mnemosyne authenticates every request, accepts only server-derived ownership, and exposes only representable effective scopes.
- MCP dispatch validates reviewed schemas, evaluates governed ownership/trust, waits on an exact in-memory prepared attempt when approval is required, atomically consumes approval once, revalidates immediately before dispatch, and never retries an ambiguous effect.

## Locked design decisions

1. **Authority is server-derived.** Introduce `RequestPrincipal`, `RequestContext`, `RunAuthority`, `GovernedScope`, `ContentTrust`, and `PolicyDecision` in focused modules under `src/authority/`. GraphQL inputs never accept an actor, reviewer, scope owner, or callback authority as authoritative. `RunAuthority` is derived when a turn is submitted and is passed unchanged into memory and capability execution.
2. **No universal actor/scope table.** Existing concrete humans, agents, and conversations remain canonical. Persisted v2 ownership uses constrained foreign keys and exactly-one checks only for those existing concrete tables. Workspace, project, and task scopes remain typed but unsupported for persistence until their canonical tables and lifecycle contracts exist; attempts to persist those owner kinds fail closed. This avoids both a second identity hierarchy and unenforced discriminator/ID pairs.
3. **Web authentication uses an opaque bootstrap capability.** Startup creates at most four random, single-use bootstrap tokens with a two-minute lifetime. Consuming `/?bootstrap=...` sets an HttpOnly, `SameSite=Strict`, path `/` session cookie and redirects to `/`; sessions have a 12-hour idle TTL and a hard cap of sixteen. Tokens never enter browser JavaScript or WebSocket payloads. Expired/evicted sessions receive the same `401` response.
4. **Canonical web authority comes from the bound socket.** Only numeric loopback addresses are accepted. After bind, `WebAuthority` holds the exact scheme/IP/port. Every request validates `Host`; state-changing HTTP and WebSocket upgrades additionally require the exact canonical `Origin`. Missing or alias origins are rejected. OAuth redirect URIs are stored at attempt creation and never reconstructed from a request `Host`. Cross-site provider callbacks authenticate through canonical listener authority plus their stored expiring one-shot state/PKCE attempt; they do not require the `SameSite=Strict` application session cookie. Session authentication remains mandatory when an application user initiates, polls, cancels, or administers an attempt.
5. **Static assets may be fetched before session establishment, but no application data may.** Bootstrap and assets still pass canonical Host validation; GraphQL, WebSocket, downloads, and callbacks require the appropriate session/attempt capability.
6. **Tauri trust terminates at the command boundary.** Only the configured main window may invoke GraphQL commands. The Rust command derives a `Desktop` principal from `Window` plus `DesktopState`, injects it into every query/subscription, and returns a serializable safe error. Client JSON cannot supply identity. Production CSP is restrictive; development exceptions are limited to the configured loopback Vite origin.
7. **Errors are safe by construction.** `SafeApiError { code, public_message, correlation_id }` is the only transport error. GraphQL extensions contain `code` and `correlationId`; HTTP/Tauri use the same shape. Expected validation/auth errors have stable messages. Unexpected errors expose a generic message and log one redacted event with the same correlation ID.
8. **Schema v2 is a reset, not a migration.** A new database is created directly at v2. Any v1, missing, or fingerprint-mismatched database returns `StoreError::ResetRequired` with instructions to stop writers and move the DB/WAL/SHM together while preserving `providers/`, configuration, MCP secrets, and other non-database state. No startup path drops or rewrites an existing database.
9. **Recovery journals are secret-free.** `persistence_sagas` records only random operation IDs, object IDs, operation kind, and phase. Secret bytes, authorization headers, raw arguments, and credential paths never enter SQLite or diagnostics. Same-directory private rollback files contain any necessary previous bytes.
10. **Memory scopes are representationally honest.** The current Mnemosyne adapter can prove `Human` and `Conversation` scopes only. Agent/project/workspace/task requests return `UNSUPPORTED_SCOPE`; they are never relabeled. Every query returns requested and effective scopes, deduplicates identical backend filters before I/O, and verifies returned ownership before releasing facts.
11. **Approval pauses the exact prepared attempt in memory.** A required-approval gateway call holds its exact arguments and authority only in an `ApprovalCoordinator`, persists only its fingerprint and redacted metadata, and waits up to five minutes. An authenticated decision wakes that same attempt. Restart or timeout expires it; no later proposal inherits approval. Immediately before the first transport write, the gateway re-reads policy inputs and atomically marks the approval consumed. Any ambiguous transport failure is terminal and is not retried.
12. **External content is data, never authority.** Valid external MCP and web-fetch results carry `ContentTrust::UntrustedExternal`; mixed results retain component labels. Invalid or unowned results are quarantined and withheld. Provider prompts/continuations consume the structured trust envelope rather than English phrase matching.

## Tracker ownership map

Every M1+M2-owned ledger row appears exactly once below (58 rows total).

| Unit | Tracker IDs |
| --- | --- |
| 1 | `ARCH-010` |
| 2 — single schema rewrite | `DATA-001`, `DATA-002`, `DATA-003`, `DATA-004`, `DATA-005`, `DATA-014`, `DATA-015`, `DATA-021`, `TEST-012` |
| 3 | `SEC-001`, `SEC-002`, `SEC-003`, `SEC-004`, `SEC-005`, `SEC-006`, `SEC-017`, `TEST-009` |
| 4 | `SEC-007`, `SEC-008`, `SEC-009`, `SEC-013`, `SEC-014`, `DATA-006`, `DATA-007`, `DATA-008`, `DATA-009`, `DATA-010`, `DATA-011`, `DATA-012`, `DATA-013`, `TEST-007`, `TEST-008` |
| 5 | `SEC-015`, `SEC-016` |
| 6 | `SEC-010`, `SEC-011`, `SEC-012`, `GOV-007`, `GOV-008`, `DATA-016`, `DATA-017`, `DATA-018`, `DATA-019`, `TEST-006` |
| 7 | `SEC-018`, `SEC-019`, `GOV-001`, `GOV-002`, `GOV-003`, `GOV-004`, `GOV-005`, `GOV-006`, `GOV-009`, `GOV-010`, `TEST-010` |
| Macro-phase exit gate | `MILESTONE-001`, `MILESTONE-002` |

## Unit 1 — Authority, trust, and safe-error contracts

**Depends on:** Foundation only. **Tracker:** `ARCH-010`.

### Files and interfaces

- Add focused modules such as `crates/noema-core/src/authority/{mod.rs,principal.rs,scope.rs,policy.rs,trust.rs}` and export them from `src/lib.rs`.
- Define opaque/random `PrincipalId`, `RequestPrincipal { id, subject, transport }`, `RequestContext { principal, correlation_id, callback_authority }`, `RunAuthority { principal, conversation_id, active_scopes, purpose }`, `GovernedScope`, `ContentTrust`, `TrustedEnvelope<T>`, `PolicyDecision`, and stable reason enums. IDs are generated with `ring::rand::SystemRandom`, never time/counter IDs.
- Replace `graphql/errors.rs` stringification with `ApiErrorCode` and `SafeApiError`; implement conversion to `async_graphql::Error` extensions and a serializable Tauri/HTTP body. Add a single `report_internal(error, correlation_id, event_kind)` boundary rather than logging at every layer.
- Split resolver construction in `graphql/schema.rs`/`graphql/resolvers.rs` so every operation retrieves a required `RequestContext` and passes it to services. At this unit, use test-only trusted contexts so existing transports compile; transport construction becomes mandatory in Unit 3.
- Introduce centralized `Authorizer` traits/methods (`require_system_admin`, `require_owner`, `require_conversation_access`, `require_artifact_access`) returning policy-neutral safe errors. Do not duplicate checks in resolvers.
- Update provider continuation/tool result types in `provider/contract.rs`, `provider/tools.rs`, and `capability.rs` to carry structured `ContentTrust`; initially label existing local results conservatively and do not alter dispatch behavior until Units 6–7.
- Inspect/split `graphql/resolvers.rs` and other touched files over 750 lines rather than adding more branches to them.

### TDD and failure matrix

1. Write tests for safe GraphQL extensions, HTTP/Tauri serialization equivalence, correlation stability, and rejection of internal cause text/secret canaries.
2. Test that missing `RequestContext` fails closed and client variables cannot deserialize a principal/run authority.
3. Test governed-scope canonical ordering/fingerprints, unsupported scope serialization, and trust propagation without string heuristics.
4. Test authorizer owner/admin allow and cross-owner deny decisions using concrete IDs.

Unexpected errors return `INTERNAL` plus correlation ID; malformed inputs return stable typed codes; neither includes debug chains. No transport is considered secured by this unit alone.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core authority
cargo test -p noema-core graphql::errors
cargo check -p noema-core -p noema-desktop
git diff --check
```

Root records test evidence and commits: `Establish request authority and safe API contracts`.

## Unit 2 — Schema v2 and transactional invariants (the only schema rewrite)

**Depends on:** Unit 1. **Tracker:** `DATA-001`–`DATA-005`, `DATA-014`, `DATA-015`, `DATA-021`, `TEST-012`.

### Files and interfaces

- Rewrite `store/schema.rs` directly as canonical v2 SQL. Remove permissive `CREATE ... IF NOT EXISTS` bootstrap. `store/runtime.rs` distinguishes an empty database from any existing schema and validates v2 before opening the store.
- Add `SchemaVersion::V2`, an expected structural fingerprint generated from the canonical normalized `sqlite_schema` rows, and `validate_schema_v2`: compare version/fingerprint, required indexes/triggers, `PRAGMA foreign_key_check`, and required pragma state. Persist version/fingerprint once in `schema_state`; do not update the marker on ordinary startup.
- Add `StoreError::ResetRequired { found, expected }` with the safe reset instructions described above. Add a repository-owned reset-protocol document reference to the error; do not implement automatic deletion or live-home handling.
- Enable foreign keys on every pooled connection and specify deliberate actions: conversation-owned turns/items/summaries cascade with the conversation; item-to-turn and artifact provenance links restrict or set null so durable history cannot silently disappear; artifact versions cascade with their artifact and current-version linkage is deferred; provider preferences/bindings cascade with their account; MCP tools/snapshots cascade with their server; policy/approval audit references set null when an operational object is removed; canonical human/agent ownership references restrict deletion.
- Add exactly-one `CHECK` constraints for polymorphic owners/authors using nullable foreign keys to the existing human, agent, and conversation tables only. Reject workspace/project/task owner kinds until those canonical tables exist. Do not create a generic actor or scope root and do not retain unenforced discriminator/ID pairs.
- Standardize all persisted timestamps as RFC 3339 UTC through one `Timestamp`/`now_rfc3339` helper in `store/ids.rs`; remove Unix-seconds strings and update row parsing.
- Replace time/counter IDs for security-sensitive and MCP entities with the random opaque allocator from Unit 1. Business IDs remain typed newtypes.
- Add v2 tables/columns/indexes needed later: `policy_decisions`; exact-attempt MCP approvals with status, decision actor, expiry, consumption and policy/tool/calibration fingerprints; `mcp_discovery_snapshots` plus snapshot membership/current snapshot; `persistence_sagas`; provenance/owner columns; normalized lifecycle timestamps.
- Add explicit unique/partial indexes for provider identities, stable MCP `(server_id, remote_name)`, one current discovery snapshot, one active context summary per `(conversation_id, provider_id, model_id)`, and one primary conversation per human.
- Refactor `store/conversations.rs::get_or_create_primary_conversation` and `store/context_summaries.rs` to `TransactionBehavior::Immediate` transactions. The unique indexes are the final concurrency guard; constraint races are re-read, not retried blindly.
- Update all affected row structs and store modules (`store/{agents,artifacts,provider_accounts,provider_capability_bindings,memory_service}.rs`, `store/mcp/**`) in the same unit so no intermediate second rewrite is required.

### Schema acceptance matrix

| Case | Expected result |
| --- | --- |
| Empty temporary DB | Creates v2 once, fingerprint matches, all FKs clean |
| Existing v1 DB | `ResetRequired`; DB/WAL/SHM unchanged |
| Missing table/index/trigger or altered SQL | Fingerprint mismatch; store refuses to open |
| FK violation introduced in test | Validation fails before runtime starts |
| Delete conversation | Owned ephemeral rows follow declared cascades; restricted/audit records remain intentionally |
| Duplicate provider/MCP/active-summary/primary-conversation | Deterministic unique violation or read-after-race result |
| Concurrent primary conversation creation | All callers receive one ID |
| Concurrent context summary activation | Exactly one active row remains |
| Timestamp round trip | RFC 3339 UTC with subsecond precision; no integer strings |

Write failing store tests first in `store/tests.rs` and focused submodules, including schema mutation fixtures and multi-connection barriers for `TEST-012`. Never point these tests at `~/.noema`.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core store::tests -- --test-threads=1
cargo test -p noema-core concurrent
cargo check -p noema-core -p noema-desktop
git diff --check
```

Root checks that this is the only schema reset unit and commits: `Rewrite the pre-v1 store as schema v2`.

## Unit 3 — Authenticated web, subscriptions, downloads, and Tauri IPC

**Depends on:** Units 1–2. **Tracker:** `SEC-001`–`SEC-006`, `SEC-017`, `TEST-009`.

### Files and interfaces

- Split `daemon/web/mod.rs` into focused `authority.rs`, `session.rs`, `routes.rs`, and startup composition; keep `origin.rs`, `http.rs`, and `graphql_ws.rs` small.
- Add `WebAuthority::from_listener(local_addr)` and reject any non-loopback bind before serving. Parse Host/Origin as authorities, reject userinfo, paths, duplicate Host, names such as `localhost`, alternate loopback aliases, missing Origin on GraphQL POST/WS, and canonicalization ambiguity. Do not use forwarded headers.
- Add `BootstrapTokenStore` and `WebSessionStore` with injected clock/randomness for tests, hard caps, constant-time token comparison, TTL/idle expiry, one-shot consumption, secure cookie attributes, and bounded cleanup. Log only token/session hashes truncated to a non-correlatable diagnostic tag, never values.
- Route policy: canonical Host for every request; bootstrap consumption creates the cookie; assets are data-free; GraphQL HTTP/WS, artifact downloads, provider/MCP attempt initiation/polling/administration, and other APIs require a valid session. Cross-site provider redirects require the canonical listener plus a valid stored one-shot attempt instead of a session cookie. State-changing application requests and WS upgrades require canonical Origin.
- Build one GraphQL schema at startup. HTTP injects a fresh `RequestContext`; WS authenticates the upgrade once and injects the immutable context into every subscription request. `connection_init` may carry only protocol metadata, never auth/identity. Session expiry closes/rejects subsequent work with a stable auth code.
- Change artifact download lookup to `get_artifact_for_principal`/`Authorizer::require_artifact_access` before opening bytes. Normalize attachment filenames and never reveal existence across owners (`NOT_FOUND` for unauthorized IDs).
- Apply centralized authorizer guards to provider, memory, MCP, settings, conversation, usage, and artifact GraphQL surfaces. Add owner-scoped store queries instead of fetch-then-filter where possible.
- In `noema-desktop`, add a trusted `DesktopPrincipal` factory in `desktop_state.rs`; update `graphql_ipc.rs` commands/subscriptions to accept Tauri `Window`, require the main label, inject `RequestContext`, and return `SafeApiError`. Restrict commands/capabilities to the main window and add production CSP/connect-src in `tauri.conf.json`/capabilities.
- In `web/src/graphql/browserTransport.ts`, set same-origin credentials explicitly; do not read/store auth tokens. Adjust desktop error decoding only. Run generated GraphQL updates if error/contract schema changes; no UI tests.

### TDD and adversarial matrix

- Table-driven HTTP tests for canonical Host/Origin, IPv4/IPv6 loopback, missing/duplicate/malformed headers, DNS-rebinding hostnames, non-loopback bind, and method/route policy.
- Fake-clock tests for bootstrap consume/replay/expiry/capacity and session expiry/eviction. Verify identical auth failure bodies and no token material in logs.
- HTTP and WS tests prove unauthenticated rejection; authenticated subscription context cannot be replaced by `connection_init`; cross-owner query/mutation/subscription is denied.
- Artifact tests prove owner success, other-owner/not-found indistinguishability, traversal-safe disposition, and expiry rejection.
- Tauri command tests prove wrong-window rejection, client identity fields ignored/rejected, per-subscription context retention, and safe error serialization.
- `TEST-009`: full adversarial transport suite runs against a temporary daemon/home and raw HTTP/WS clients only. No browser automation and no live credentials.

On any authority parse ambiguity, deny. If the session registry is unavailable, APIs fail closed. Static asset access never grants a session.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core daemon::web
cargo test -p noema-core graphql::
cargo test -p noema-desktop
cargo check -p noema-core -p noema-desktop
cd crates/noema-core/web && bun run gen:all && bun run lint && bun run build
cd ../../.. && python3 scripts/check_generated_state.py --all
git diff --check
```

Root updates evidence and commits: `Authenticate every local application transport`.

## Unit 4 — Private persistence, redacted diagnostics, and recoverable mutations

**Depends on:** Units 1–3 and v2 saga/discovery tables. **Tracker:** `SEC-007`–`SEC-009`, `SEC-013`, `SEC-014`, `DATA-006`–`DATA-013`, `TEST-007`, `TEST-008`.

### Files and interfaces

- Add `private_fs.rs` as the only primitive for sensitive paths: symlink-safe known-directory creation/repair (`0700`), private files (`0600`), random `O_EXCL` same-directory staging, write + file sync + atomic rename + parent sync, bounded reads, and safe removal. Provide Unix permission implementation and a compiling restrictive non-Unix abstraction.
- Refactor `home.rs` startup repair to visit an allowlisted Noema tree only; never recurse through arbitrary entries or follow symlinks. Before SQLite open, validate the allowlisted parent chain, reject symlinks/wrong ownership, create the database directory privately, and use SQLite no-follow/open protections where supported. Repair and re-verify existing DB/WAL/SHM modes before runtime or API exposure; checkpointing may occur only after the trusted open. Repair the remaining home/run/provider/MCP/Mnemosyne/conversation directories, config/private artifacts/secrets/logs under the same policy. An unsafe owner/symlink is a hard startup error.
- Migrate `provider/secret_input.rs`, Codex token-store writers, `mcp/secrets.rs`, artifact writers, and other sensitive `fs::write`/`File::create` sites to `PrivateFs`.
- Replace `system_errors.rs` with a redacted allowlist event schema. Store code, component, correlation ID, bounded public context and counters only; remove raw request/result/payload fields. Cap fields and line size, rotate at 1 MiB under one mutex, retain three `0600` files, and tolerate a failed diagnostic write without altering primary control flow.
- Add `PersistenceSagaCoordinator` over the v2 `persistence_sagas` store and `PrivateFs`. Operation flow: allocate random IDs; journal `Prepared`; atomically stage/install with a private rollback file; transact DB mutation and mark `DbCommitted`; remove rollback and journal. Startup recovery runs before default-account seeding: pre-commit operations restore/delete filesystem state; post-commit operations finish cleanup. Journal/log rows contain no secret/path/payload bytes.
- Move provider create/update/clear/delete and MCP create/continue/delete behind application services using that coordinator. Multi-row deletes and preference/binding cleanup occur in one DB transaction. Fault injection is an explicit enum hook at every durable boundary, compiled for tests.
- Replace MCP identity generation with random opaque stable IDs. Add `McpDiscoveryService::reconcile_complete_snapshot(server_id, remote_snapshot)`: write a new snapshot and members in one transaction, retain IDs for exact `(server_id, remote_name)` matches, mark absent tools unavailable only after a successful complete snapshot, then atomically advance `current_snapshot_id`. Failed/partial discovery never mutates the current snapshot.
- Store only redacted MCP metadata and structural schema fingerprints; secrets remain in private files. Validate unique server identity and tool identity at the store boundary.

### TDD and failure matrix

- `TEST-007`: Unix tests create permissive known files/dirs, restart repair, and assert exact modes; include symlink substitution, wrong owner where testable, DB/WAL/SHM, rotation files, and artifact/secret creation. Non-Unix builds exercise path-policy logic.
- Diagnostic canary tests inject tokens, authorization headers, cookies, MCP arguments/results, and error chains; none may appear in current or rotated logs. Oversized fields/events are bounded.
- `TEST-008`: table-test crashes/failures after journal, staging, install, DB commit, and cleanup for provider create/update/delete and MCP create/continue/delete. Reopen twice to prove recovery idempotence and coherent DB/file state. Assert journal/log byte scans contain no canaries.
- Discovery tests cover identical refresh ID stability, rename as remove+add, partial/page failure preserving current snapshot, concurrent refresh serialization, deletion FK behavior, and random-ID collision retry.
- Transaction tests verify MCP multi-row delete, provider preference cleanup, and no orphaned secret/account state.

Unsafe permissions or symlinks fail closed. Diagnostics failure is secondary and never leaks into API text. A recovery ambiguity preserves the rollback material and refuses the affected object rather than guessing.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core private_fs
cargo test -p noema-core system_errors
cargo test -p noema-core persistence_saga
cargo test -p noema-core store::tests::mcp
cargo test -p noema-core provider::
cargo check -p noema-core -p noema-desktop
git diff --check
```

Root commits: `Make private persistence atomic and recoverable`.

## Unit 5 — Bounded server-derived OAuth attempts

**Depends on:** Units 3–4. **Tracker:** `SEC-015`, `SEC-016`.

### Files and interfaces

- Refactor `provider/auth.rs`, `daemon/web/provider_auth.rs`, `mcp/oauth.rs`, `mcp/setup.rs`, and desktop callback code around `OAuthAttemptStore<T>` with injected secure randomness and clock.
- Provider attempts are random, expire, retain bounded terminal status, and cap active+terminal entries at 32. MCP attempts cap at 16 and hold expected callback URI, state verifier, expiry, and one-shot consumed state. Eviction first cancels expired runtimes, then oldest terminal attempts; if all slots are live, creation returns `RESOURCE_EXHAUSTED` rather than evicting an active attempt.
- Remove client `redirectUri`/callback authority GraphQL inputs. Resolver services derive the exact callback from authenticated `RequestContext`: canonical bound web authority or the desktop callback listener. Update generated operations/forms only to remove these fields.
- Callback handling first validates canonical listener authority and bounded query sizes, then atomically `take_for_callback(state)`; only the stored expected callback URI is sent to providers. The provider redirect is authenticated by the stored state/PKCE attempt and does not require the Strict application cookie. Replay, expiry, state mismatch, redirect mismatch, or missing attempt returns one non-oracular error and cancels retained runtime state.
- Use `PrivateFs`/sagas from Unit 4 for token persistence. Callback parameters, verifier, access/refresh tokens, and raw provider errors never reach diagnostics.

### TDD and failure matrix

- Fake-clock/random tests: capacity, collision retry, expiry cleanup, active-cap refusal, terminal eviction, cancellation, exact redirect derivation, callback replay, and concurrent double callback (one winner).
- Raw HTTP tests: hostile/missing Host, oversized query, wrong state, alternate loopback alias, open-redirect input, expired attempt, callback without an application cookie, expired session on initiation/admin routes, and provider error. Assert no outbound token exchange occurs on rejection; prove a valid callback succeeds from its stored attempt without a session cookie.
- Desktop tests bind a temporary loopback callback port and prove a callback for any other authority is rejected.
- Frontend static verification proves no redirect URI remains in generated operation inputs and no token is stored in browser state.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core provider::auth
cargo test -p noema-core mcp::oauth
cargo test -p noema-core daemon::web::provider_auth
cargo test -p noema-desktop mcp_oauth_callback
cd crates/noema-core/web && bun run gen:all && bun run lint && bun run build
cd ../../.. && python3 scripts/check_generated_state.py --all
git diff --check
```

Root commits: `Bound OAuth attempts to trusted callback authorities`.

## Unit 6 — Authenticated Mnemosyne and truthful memory scopes

**Depends on:** Units 1–5. **Tracker:** `SEC-010`–`SEC-012`, `GOV-007`, `GOV-008`, `DATA-016`–`DATA-019`, `TEST-006`.

### Files and interfaces

- In `mnemosyne/lifecycle.rs`, generate a separate random sidecar bearer capability per child generation, bind only to `127.0.0.1`, pass it through a minimal child environment, and monitor child identity. Do not persist or log it. `mnemosyne/client.rs` adds the bearer to health and all API requests, enforces response byte/deadline limits, and rejects cross-generation clients.
- In `mnemosyne-sidecar/noema_mnemosyne_sidecar/{config.py,app.py}`, require constant-time Bearer authentication before parsing every health/memory request. Add strict request models for `OwnershipEnvelope { human_id, conversation_id?, provenance }`; reject client-supplied agent/user/run metadata outside that envelope. The Rust service constructs it only from `RunAuthority`.
- Store and return the ownership envelope with every fact. The Rust adapter validates that every returned fact exactly matches the requested owner/effective scope; any missing, malformed, or conflicting owner quarantines the whole response and returns `UNTRUSTED_RESULT` without logging content.
- Replace string scopes/purpose in `daemon/memory/tool.rs` and `memory/context.rs` with `MemoryPurpose`, `MemoryScope`, and `MemoryQueryPlan { requested, effective, backend_filters }`. Only `Human` and `Conversation` are representable now. Unsupported agent/project/workspace/task scopes fail before I/O.
- Deduplicate identical backend filter sets before requests, deduplicate returned facts by stable memory ID before ranking/truncation, and return both requested and effective scopes to GraphQL/tool callers. Purpose is a structured policy input, not matched text.
- Route memory ingest/search/list/delete through the same ownership/policy service. User/agent/run metadata from model/tool payloads is ignored as authority and rejected if it conflicts.
- Keep the real adapter contract testable by dependency-injecting the Python memory backend; tests launch the actual FastAPI sidecar endpoints with a deterministic in-memory backend and use the real Rust HTTP client. No live credentials or `~/.noema`.

### TDD and failure matrix

- Python tests: no/wrong bearer, correct bearer, oversized/malformed body, ownership required, ownership round trip, backend exception redaction, and no secret in logs.
- Rust tests: sidecar readiness requires authentication; child restart invalidates old client capability; exact ownership succeeds; other human/conversation, missing ownership, and forged metadata are rejected.
- `TEST-006`: two principals share one test sidecar; each can ingest/search/delete only its own facts via real adapter HTTP. Cross-user and cross-conversation probes prove zero leakage.
- Scope matrix: human, conversation, duplicate scopes/filters, unsupported scopes, mixed requested scopes, invalid purpose, empty result, duplicate IDs, malformed result, and truncation. Assert returned effective scopes exactly match issued backend filters.
- Policy records include principal, purpose, requested/effective scope fingerprints, and allow/deny reason, never query/fact content.

If authentication, generation identity, or ownership proof is unavailable, memory is `Unavailable`/denied; it never silently downgrades to a broader scope.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core mnemosyne
cargo test -p noema-core daemon::memory
cd crates/noema-core/mnemosyne-sidecar && python -m pytest
cd ../../.. && cargo check -p noema-core -p noema-desktop
git diff --check
```

Root commits: `Authenticate memory and enforce truthful ownership scopes`.

## Unit 7 — Governed MCP dispatch, exact approvals, and trust quarantine

**Depends on all prior units.** **Tracker:** `SEC-018`, `SEC-019`, `GOV-001`–`GOV-006`, `GOV-009`, `GOV-010`, `TEST-010`. The milestone rollups remain owned by the exit gate.

### Files and interfaces

- Refactor `capability/gateway.rs`, `capability.rs`, `mcp/eligibility.rs`, `provider/tools.rs`, and runtime composition around `GovernedToolAttempt`, `PolicyEngine`, `ApprovalCoordinator`, and `CapabilityGateway::dispatch(run_authority, proposal)`.
- Resolve the opaque stable tool ID first. Validate canonical JSON arguments against the reviewed input schema using a single maintained JSON Schema validator dependency. Derive source/destination ownership with reviewed structured extractors. Fingerprint canonical arguments, principal/run/scopes, ownership, tool metadata/schema/calibration, classifications, and policy version. Raw arguments are never persisted or logged.
- `PolicyEngine` evaluates actor, active scopes, purpose, content trust, source/destination ownership, read/write/export classifications, server/auth/health, discovery snapshot, and calibration. Persist every redacted allow/deny/require-approval decision. Unknown/mixed ownership, stale metadata, absent extractor, unsupported write/export, or unreviewed schemas fail closed.
- Remove client `reviewed_by` everywhere. Calibration save/revoke and approval decide/revoke mutations derive the decision actor from `RequestPrincipal`; schema and frontend generated operations expose no reviewer input.
- `ApprovalCoordinator` holds a prepared attempt in memory for at most five minutes and persists an exact fingerprint request. The authenticated decision mutation can approve/deny/revoke it. On approval, the gateway re-fetches server/tool/snapshot/schema/calibration/health/auth and reruns policy. In one immediate DB transaction it verifies pending+unexpired+fingerprint, marks consumed with actor/time, and records the execution decision. Only then may transport be called once. Restart, mutation, expiry, revoke, second consume, or any revalidation change denies; ambiguous transport errors are final and never automatically retried.
- Validate successful structured results against the reviewed output schema. Invalid/oversized/malformed results are stored only as a bounded quarantine diagnostic and withheld. Valid external MCP results are `TrustedEnvelope<...>` with `UntrustedExternal`; mixed local/external data retains component trust. Provider continuation explicitly serializes trust metadata as protocol structure.
- Apply the same trust envelope to every external search and fetch path, including DuckDuckGo, Exa, OpenAI-hosted search, and direct/Exa fetch: titles, snippets, URLs, response bodies, and summaries are untrusted content, byte/deadline bounded, and cannot supply policy/identity/tool instructions. Remove any raw external payload interpolation that lacks a structural boundary.
- Model tool exposure uses the same policy preflight. Tools remain hidden when governance inputs are incomplete. Exposed approval-capable tools can create a prepared exact attempt, but the model never receives or chooses the approval token.
- Split gateway/store/GraphQL files over 750 lines by policy, approval, validation, and transport responsibility. Do not add parallel eligibility logic.
- Regenerate frontend schema/operations for typed policy/approval/trust states only; no UI behavior or tests. Defer tracker/ledger completion and the milestone context update to the macro-phase closure commit after root validation and Sol review.

### TDD and adversarial matrix

- Schema validation: valid/invalid args, unknown fields, numeric/string edges, deep/oversized JSON, missing input/output schema, invalid output, and validator resource bounds. Transport call count must remain zero on any preflight failure.
- `TEST-010` table: same/cross owner; trusted/untrusted/mixed/unresolved content; human/conversation scope; read/write/export; healthy/unhealthy/unauthenticated/stale server; current/stale calibration; allow/deny/approval; approve/deny/revoke/expire/restart; mutated args/ownership/metadata/schema/calibration/health/auth; concurrent double decision/consume; and ambiguous transport failure. Assert exactly one dispatch maximum and no retry.
- Reviewer attribution: forged `reviewed_by` variables rejected; authenticated principal appears in calibration/decision audit; a different principal cannot decide another owner’s request unless explicit admin policy allows it.
- Quarantine tests inject prompt-like text, malformed structured output, secret canaries, and ownership conflicts. None may become a tool instruction, API error, diagnostic payload, or follow-on dispatch.
- Preflight/execution parity tests prove model exposure and execution call the same policy engine and fingerprints; no separate allowlist can drift.

### Focused validation and commit

```bash
cargo fmt --all --check
cargo test -p noema-core capability
cargo test -p noema-core mcp
cargo test -p noema-core governance
cargo test -p noema-core web_fetch
cargo check -p noema-core -p noema-desktop
cd crates/noema-core/web && bun run gen:all && bun run lint && bun run build
cd ../../.. && python3 scripts/check_generated_state.py --all
python3 scripts/check_audit_coverage.py
git diff --check
```

Root commits: `Enforce governed one-shot capability dispatch`.

## Per-unit root integration protocol

For each unit, root must:

1. Confirm `git status --short --branch`, preserve unrelated work, and brief one fresh Terra-medium implementer on only that unit.
2. Require tests to be written failing first, then implementation and focused validation. The implementer does not commit.
3. Have a fresh Sol-high reviewer inspect the diff read-only against the unit’s tracker rows, threat/failure matrix, net-code/consolidation goal, and 750-line threshold.
4. Integrate corrections; run the focused commands and `git diff --check` personally.
5. Record focused automated and operational-verification evidence in the ignored progress ledger, but do not check tracker rows or invent the not-yet-existing completion commit hash.
6. Inspect staged stat/name-status and commit exactly the reviewed unit. After the macro-phase review passes, create a distinct closure commit that updates tracker/ledger rows using the already-existing implementation hashes and summarizes durable decisions in `docs/context/current.md`.

## Macro-phase exit gate

After Unit 7, root runs from a clean working tree at the exact candidate commit:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast

cd crates/noema-core/mnemosyne-sidecar
python -m pytest
cd ../../../..

cd crates/noema-core/web
bun run gen:all
bun run lint
bun run build
cd ../../..

python3 -m unittest discover -s scripts/tests
python3 scripts/check_generated_state.py --all
python3 scripts/check_audit_coverage.py
git diff --check
```

Then a fresh Sol-high adversarial reviewer checks:

- all 58 M1+M2 rows are owned exactly once and have concrete test/commit evidence;
- no second schema rewrite, migration, compatibility path, automatic destructive reset, live-home access, credential logging, raw external-content release, client-derived authority, or English semantic matching was introduced;
- cross-owner, callback, Host/Origin, permission, saga, ownership, schema, quarantine, approval replay, execution revalidation, and ambiguous-effect cases fail closed;
- generated GraphQL/assets are current and all standard workspace/Python/frontend checks passed;
- source size/architecture did not regress and duplicated policy/auth/storage paths were consolidated.

`MILESTONE-001` and `MILESTONE-002` may be checked only after this review has no unresolved high/critical findings. Root then creates the macro-phase closure commit that records exact commands/results and references the already-existing implementation commits in the ledger.

## Genuine blockers

None. The current code and locked constraints determine the product decisions needed for this phase. The approval wait may temporarily serialize the current global runtime actor, but it is security-correct and bounded; Milestone 3’s per-conversation workers remove that known throughput limitation without changing the approval contract.
