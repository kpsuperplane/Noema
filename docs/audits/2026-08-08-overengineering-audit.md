# Noema codebase overengineering audit

- **Status:** Dated review snapshot; remediation complete
- **Baseline:** `ef0db3918e4997c214f9762f3be1e5753d6022de`
- **Mode:** Adversarial review, report only
- **Scope:** Entire tracked repository, with focused source tracing across every
  Rust crate, both clients, GraphQL, scripts, evaluation assets, and
  current/historical documentation

This report applies the repository's own
[simplicity contract](../development/simplicity.md): prefer one authority, require
two concrete production consumers before adding a general abstraction, avoid
pre-V1 compatibility by default, keep refactors net-negative, and protect unique
correctness, security, privacy, and data-loss invariants.

Supporting material:

- [Evidence appendix](2026-08-08-overengineering-evidence.md)
- [Remediation report](2026-08-08-overengineering-remediation.md)

This file preserves the original assessment.
The remediation report owns completed dispositions.

## Executive assessment

Noema is scope-heavy more than it is uniformly abstraction-heavy. Its current
surface includes a durable conversational runtime, multi-role Work execution,
two clients, multiple provider protocols, local models, native HTTP adapters,
MCP, OAuth, artifact storage, memory, PWA delivery, and model evaluation. Much of
the resulting code pays for a real product behavior or a material safety boundary.

The strongest overengineering is concentrated elsewhere:

1. **Parallel authorities** — stale documentation, duplicate persistence paths,
   duplicate telemetry, synthetic and persisted account models, and independently
   rebuilt snapshots.
2. **Dormant generality** — adapter events, continuation modes, quota/gate fields,
   a scheduler without a producer or worker, and test-only state authorities.
3. **Compatibility without a support policy** — legacy transcript parsing,
   provider-placeholder cleanup on every boot, stale generated native operations,
   and historical client display formats.
4. **String inference after discarding types** — English substring checks decide
   secrecy, retries, error codes, side effects, and finalization behavior even
   though typed schemas or behavior metadata already exist nearby.
5. **Inactive governance machinery** — 2,350 lines of architectural/test-name
   checkers that are not wired into normal validation and all fail at the audit
   baseline.

The repository can remove several thousand authored or tooling lines without
cutting a current product capability, and can remove roughly 42,000–49,000 lines
of stale or historical documentation from the active working tree after extracting
unique durable constraints. Estimates in this report overlap and are not an
additive commitment. Product-scope choices could remove much more, but they are
explicit decision gates rather than refactoring recommendations.

### Measured footprint

| Surface | Authored production | Tests | Generated | Notes |
| --- | ---: | ---: | ---: | --- |
| Rust workspace | 150,915 | 54,949 | — | 960 Rust test declarations |
| Web TypeScript/TSX | 36,999 | 2,386 | 3,016 | Route tree and GraphQL types excluded from authored count |
| iOS Swift | 17,856 | 0 | 23,484 | No committed Swift test target sources found |
| Markdown under `docs` | — | — | 56,985 | 47,739 lines are historical `docs/superpowers` Markdown |

Large files are useful discovery signals, not findings. The audit found justified
large authorities in schema migration, concurrency, security, and protocol code,
as well as large files containing actual duplicate or dormant behavior.

## Priority summary

| ID | Priority | Confidence | Opportunity | Directional effect |
| --- | --- | --- | --- | ---: |
| OA-01 | P0 | Very high | Repair stale iOS GraphQL generation and add one freshness gate | Correctness; retired generated output removed |
| OA-02 | P0 | Very high | Remove Work-event English-key secrecy heuristic | ~29 production LOC plus obsolete tests |
| OA-03 | P1 | High | Delete dormant adapter event framework | ~500–550 LOC |
| OA-04 | P1 | High | Remove unimplemented continuation modes and inert adapter policy fields | ~370–550 LOC |
| OA-05 | P1 | Very high | Retire inactive architectural/test-name gate systems | 1,600–2,350 tooling/doc LOC |
| OA-06 | P1 | Very high | Remove historical and contradictory documentation from the active tree | ~42,000–49,000 doc LOC |
| OA-07 | P1 | High | Correct over-redaction of ordinary identifiers, URLs, and paths | ~20–35 LOC; restores diagnostics |
| OA-08 | P1 | High | Replace string-inferred runtime semantics with existing typed authority | Small net reduction; correctness gain |
| OA-09 | P2 | High | Delete unreachable web/native UI | ~820–870 authored LOC |
| OA-10 | P2 | High | Delete test-only in-memory cursor authority | ~180–220 LOC |
| OA-11 | P2 | High | Make artifact metadata writes one authority | ~70–110 LOC |
| OA-12 | P2 | High | Consolidate Work-event/read authorities | ~110–155 LOC |
| OA-13 | P2 | Medium-high | Collapse provider account dual representation/system-account path | ~200–400 LOC |
| OA-14 | P2 | High | Make runtime debug spans the sole provider telemetry authority | ~250–500 cross-platform LOC |
| OA-15 | P2 | High | Remove single-implementation routing traits | ~50–85 LOC |
| OA-16 | P2 | Medium-high | Consolidate native web binding/execution and adapter snapshots | ~100–190 LOC |
| OA-17 | P2 | Very high | Use iOS GraphQL fragments instead of generic closure/overload mappers | ~530–850 authored LOC before generated churn |
| OA-18 | P2 | High | Collapse one-consumer web settings layers and duplicate projections/loaders | ~185–365 LOC |
| OA-19 | P2 | High | Remove dead dependencies, dead store APIs, and unused memory index writes | ~170–260 LOC plus build/runtime work |
| OA-20 | P2 | Medium-high | Sunset perpetual pre-V1 compatibility paths | ~180–350 LOC depending scope |
| OA-21 | P2 | High | Resolve Astryx/StyleX version skew and delete per-file cast adapters | ~50–100 LOC and one duplicate dependency version |
| OA-22 | P3 | Medium | Reassess the dual provider generation contracts | ~150–300 LOC only if a spike proves net-negative |
| OA-23 | P3 | High | Remove speculative/test-only domain and API vocabulary | ~150–300 LOC across small slices |

## Immediate correctness and contract truthfulness

### OA-01 — iOS GraphQL has three unsynchronized authorities

The current server schema, authored iOS operation documents, and committed Apollo
Swift output disagree.

- All 105 authored iOS operations validate against `graphql/schema.graphql`.
- Five of the 105 committed generated operations do not: pending chat
  interventions, adapter definitions, adapter approval, and two retired OAuth
  client-import mutations.
- Generated operations still request `clientSetupUrl`, `oauthRedirectUri`, and
  `acceptsOauthClientJson`, and two still call the removed
  `importAdapterOauthClientJson` mutation.
- Current authored documents use `credentialSetup` and contain two
  `SetupAdapterConnection` mutations missing from generated output.
- Native Settings and Chat code still consumes the retired generated types.
- The five invalid generated operation files span 1,215 lines, with a further
  33-line retired generated input type.

This is not an argument against committed generated Swift. The documented native
build expects it. The overengineering is a manual, ungated three-stage authority
where stale output remains compilable and callable.

Repair the native model against `credentialSetup`, regenerate once, remove orphaned
generated operations through generation, build the target, and add one existing-CLI
clean-diff check. Do not add a second code generator or hand-edit generated files.

### OA-02 — Work-event secrecy is inferred from English key substrings

`noema-tasks/src/event_validation.rs::validate_payload` already enforces a closed
event kind, exact field set, scalar/closed-enum/typed-ID field validation, event
invariants, and a 16 KiB bound. It then recursively rejects keys containing words
such as `token`, `answer`, `result`, `feedback`, `description`, and `lease`.

The substring scan is redundant because unknown fields and arbitrary nested
objects cannot pass the remaining schema. It also conflicts with the current
information contract: English names are not a secrecy authority, and ordinary or
authorized private values must not be discarded because of a field name.

Delete `contains_sensitive_key` and its call. Keep the exact schema and size
boundary. Pair a secret-exclusion assertion at a real secret-bearing boundary with
an ordinary-value preservation assertion here.

### OA-07 — Whole-value Debug redaction hides ordinary information

Several custom Debug implementations reflect an older information model and now
create both code and observability debt:

- `InvokerKey`, `OperationToken`, and `CapabilityTarget` hide internal routing
  identifiers that do not grant authority.
- provider configurations hide normalized base URLs, OAuth issuer/token endpoints,
  and a public OAuth client ID;
- local-model configuration and evaluation types hide bridge, model, runtime, and
  cache paths;
- MCP and adapter OAuth types hide attempt IDs already returned for owner-gated UI
  polling and callback correlation.

Keep actual API keys, access/refresh tokens, auth codes, state, PKCE verifiers,
user codes, callback URLs, and authorization-URL secret query material excluded.
Show the ordinary values intact. Where a URL mixes ordinary and secret components,
derive one exact sanitized diagnostic form rather than hiding the entire URL.

This should be a small net-negative patch with paired preservation and
non-disclosure tests, not a mass replacement of `[REDACTED]` throughout the repo.

### OA-08 — Runtime reconstructs discarded semantics from strings

Several decisions parse English or name fragments even though structured state is
available or should remain available:

- `task_runtime.rs::execution_error_code` searches rendered error text for
  `terminal`, `provider`, `model`, `lease`, and `fence`.
- `execution_is_retryable` searches for the phrase `after one repair`.
- `progress.rs::result_side_effect` treats tool names containing `create`,
  `update`, or `delete` as side effects instead of using
  `CapabilityToolBehavior`.
- `task_continuation.rs::build_task_finalization_prompt` searches a human-readable
  reason for `human input` instead of receiving the existing
  `ProgressAuditDecision::AskHuman` state.
- the web transcript marker model strips old English display prefixes and filters
  a small English list of “low information” strings from otherwise structured
  display metadata.

The smallest fix is not a universal error framework. Preserve a small typed
failure/retry code at the task boundary, pass tool behavior into progress tracking,
pass a typed finalization reason, and remove pre-V1 display compatibility once the
development-data policy is explicit.

## High-confidence deletion opportunities

### OA-03 — Adapter event support is code for a path that always rejects

The native adapter event module and tests occupy 476 lines. It implements HMAC
verification, challenges, deduplication, DTOs, and exports. The compiler rejects
every event subscription as unsupported, while comments say the parsed field is
retained for a future milestone. No HTTP/webhook ingress or production consumer
exists.

Delete the module, event field, exports, and tests. Let the existing strict manifest
parser reject an `event` field until a real ingress and consumer form a vertical
slice. Keep one supported-manifest test and one unknown-event-field rejection test.

### OA-04 — Adapter schemas advertise behavior invocation cannot enforce

Two related forms of dormant extensibility should be removed from the executable
contract:

1. `ProviderLink` and `DeltaCursor` continuation modes are parsed, validated, and
   described in setup, but invocation only extracts and persists `ResponseToken`.
2. adapter `gates` and `quota` are parsed, compiled, hashed, and prompted for, but
   do not participate in eligibility, invocation admission, API projection, or
   accounting. Gates are consulted only when replacing a definition; quota is
   copied only.

Reduce the executable continuation enum to `None | ResponseToken`. Remove inert
policy-looking fields instead of letting users or reviewers infer enforcement.
Both changes require an explicit adapter schema/digest version decision and
fail-closed tests for removed fields.

### OA-05 — Three inactive gate systems mirror repository state

The following systems total 2,350 lines:

- crate-boundary checker and test: 1,324 lines;
- focused dependency-tree checker and test: 414 lines;
- critical-test-name verifier, test, and manifest: 612 lines.

They are absent from Cargo aliases, package scripts, the current validation
contract, and checked-in CI. At the baseline, all three fail: the boundary model is
stale, the focused tree cannot run offline because `winsafe` is not cached, and the
critical-test manifest contains four renamed or removed tests.

Delete them unless an external pipeline is first proven to consume them. If one
architecture gate is still wanted, derive package/feature inventory from Cargo
metadata, encode only explicit deny/ownership rules, wire it into normal validation,
and keep a small table-driven test. Do not preserve exact test names as a proxy for
behavior.

The active Rust size reporter and `noema-dev` validation runner are in use and must
remain.

### OA-06 — Documentation is the largest parallel architecture

Historical plans/specs account for 47,739 of 56,985 Markdown lines under `docs`.
Many are far larger than the current planning contract, duplicate a spec with an
implementation plan, retain unchecked execution steps, and describe retired
`noema-core`, SurrealDB, Supermemory, Mem0, Mnemosyne, or earlier provider designs.

The nominally current documentation is also inconsistent:

- `docs/harness/runtime.md` still makes SurrealDB the transcript authority;
- `docs/harness/events.md` describes event-derived current state even though
  current Work events are audit/invalidation, not a second state authority;
- several frontend documents describe Home/Memory navigation, memory graph queries,
  and tasks/integrations as future;
- `docs/postgres.md` says SurrealDB is current;
- `docs/codebase-audit-tracker.md` contains 117 unchecked items, including work
  already completed, and claims repository CI that is not checked in;
- `docs/context/current.md` is exactly 299 lines but 4,073 words, with individual
  lines approaching 2,200 characters. The line cap is not limiting context density.

Keep current contracts, not milestone history. Extract unique durable rules into
the closest active authority, delete completed/superseded plans from the working
tree, rewrite the frontend contract against current routes/schema, reduce
`context/current.md` to active direction and open loops, and retire the old audit
tracker after this report is accepted. Git history remains the archive.

### OA-09 — Unreachable client UI remains in the product tree

Static import/declaration tracing found five unimported web components and one
unreferenced native view:

- `TaskResumeControls.tsx` — 100 lines;
- `TaskToolMarker.tsx` — 175 lines;
- `ProviderStatus.tsx` — 23 lines;
- `ChatWorkPanel.tsx` — 168 lines;
- `StageBadge.tsx` — 10 lines;
- the first 91 lines of `TasksSidebar.swift`.

Additional dead task helpers, types, labels, and exports bring the credible total
to roughly 820–870 authored lines. Delete these directly, then run static build and
client compilation. Do not create a registry or barrel to make dead components
look reachable.

### OA-10 — Tests own a second continuation cursor state machine

The adapter continuation module contains an in-process `CursorStore` of roughly
180–220 lines used only by its own tests. Production uses `DurableCursorStore`.
The in-memory authority even exposes resync states that durable production cannot
enter.

Delete it and exercise the durable store through a temporary directory. Preserve
open/reopen, stale-handle, rotation, expiry, and atomic-commit risks; do not port
test-only behavior into production.

## Authority consolidation opportunities

### OA-11 — Artifact metadata has two writers and round-trip error translation

`NoemaStore` exposes inherent create/append methods while separately implementing
`ArtifactMetadataStore` over the same transaction helpers. The inherent create path
has one production caller; the inherent append path has none. Domain errors are
converted into `StoreError` and then converted back into artifact-domain errors.
The two paths also differ in transaction and busy-retry policy.

Make `ArtifactMetadataStore` the sole writer and route external-URL GraphQL creation
through it. Remove the inherent writer surface and mirrored errors. Keep
`ArtifactMetadataStore` and `ArtifactOperations`: they break real crate cycles and
own filesystem/persistence cancellation behavior.

### OA-12 — Work events and criteria are repeatedly reconstructed

Work-event payloads and records validate the same closed data at adjacent
boundaries, retain a hidden duplicate kind to compensate for publicly mutable
fields, and check the 16 KiB limit again in the store. Two store modules then decode
the same 12-column event row independently.

Submission and review loaders also accept `_expected_criteria` parameters they
ignore. Two callers allocate the vectors, while batch loaders independently query
the canonical criteria through two near-identical helpers.

Make record fields immutable, validate once on construction and once when reading
untrusted persisted rows, share the canonical event column list/decoder, remove
ignored parameters, and consolidate the criteria query. Preserve malformed-row,
pagination, recovery, missing-link, and exact-coverage tests.

### OA-13 — Provider accounts have persisted and synthetic representations

`PersistedProviderAccountRecord` and `ProviderAccountRecord` duplicate thirteen
fields; the latter adds derived capabilities. Manual conversion copies every field
in both directions. Three web backends are exposed as synthetic, non-persisted
“system provider accounts,” while local system providers are persisted normally.
API and runtime code merge or fall back across both sources, and persistence has a
special `ValidatedSystem` account-reference mode.

Choose one authority:

- preferably persist every selectable built-in account through a forward-only
  migration and derive capabilities from one record type; or
- explicitly model non-account system backends outside provider accounts.

Do not retain fake accounts plus a bypass mode. Preserve provider capability
assignment validation and test existing-version upgrade plus fresh convergence if
the schema changes.

### OA-14 — Provider telemetry is written and interpreted twice

Runtime still writes `provider_usage` into transcript item metadata while the
canonical `runtime_debug_spans` store records provider, model, phase, indices,
duration, token usage, and cache usage. Web maintains a legacy metadata parser and
fallback renderer; iOS separately parses the same metadata.

Make runtime debug spans/profile the sole telemetry authority. Stop writing new
legacy metadata and remove cross-client legacy parsing after deciding how much
pre-V1 development data must remain inspectable. Preserve current token/cache
visibility through the canonical debug query.

The same compatibility decision should cover English-prefixed transcript display
values and retired task-model aliases. Avoid a new versioned compatibility
framework.

### OA-15 — Two routing traits have one production implementation

`CapabilityRouter` has only `CapabilityRegistryRouter`; production constructs and
uses the concrete router, and trait-object use is test-oriented. Likewise,
`ProviderRouteResolver` has only `RegistryProviderRouteResolver`; the meaningful
seam is the selection loader beneath it.

Move dispatch/resolve to inherent methods and store concrete handles. Keep real
polymorphism: capability invokers, payload sanitizers, provider implementations,
web backends, and MCP transports all have multiple production consumers.

### OA-16 — Native web and adapter snapshots have parallel execution/read paths

Foreground native web execution and approved-action execution independently rebuild
bindings, destinations, behavior, revisions, and output handling. The approval
state machine is distinct and justified; the concrete native binding/executor is
not. Share one exact binding constructor and executor while leaving admission and
governed-action transitions separate.

GraphQL and host code also reconstruct adapter definition/connection stores beside
an `AdapterCapabilityService` that already owns those stores and a snapshot. Expose
only the missing projections through that service snapshot and remove independent
scan timing.

A later, higher-risk slice may remove the request-local
`RuntimeExecutionInvoker` side channel, but only after shared native execution is
proven and without bypassing immutable catalog snapshots or reviewed arguments.

### OA-17 — iOS GraphQL repetition created generic Swift mapping machinery

`Settings.graphql` is 1,119 lines with 40 operations and only one fragment. Model
settings, adapter definitions, model pools, installations, provider accounts, and
capability shapes repeat. `Memory.graphql` repeats page/article/status shapes.
Handwritten Swift compensates with a fourteen-closure mapper and many
operation-specific overloads.

The Tasks operation layer already demonstrates the simpler authority: shared
GraphQL fragments. Add fragments for the repeated shapes, map each generated
fragment once, and delete generic closure/key-path machinery and overload mirrors.
Do not replace them with Swift protocols or another DTO hierarchy.

After that local reduction, consider sharing only the 28 structurally identical
web/iOS operation documents through a root GraphQL source directory. Stop if
generator configuration and runtime document plumbing cost more than the roughly
494 duplicated authored lines per client.

### OA-18 — Web has one-consumer layers and duplicate projections/loaders

Eight Settings pane/content pairs total 3,190 lines. Each content component has one
production consumer, while wrappers mirror Apollo state and callbacks into broad
props and re-export unused symbols. Colocate each domain's query/mutation state with
its pane; retain meaningful local subcomponents and keep domains separate.

Three smaller duplicate authorities should be fixed in the same style:

- task status is derived differently in list/transcript and detail surfaces,
  producing a real `REQUEST_CHANGES` display inconsistency;
- PWA reconciliation repeats transcript loading/merging already owned by the
  canonical loader in `App.tsx`;
- 27 local `*XStyle` cast helpers in 21 files compensate for framework type skew.

These are consolidation targets, not reasons to build a global Settings or app
controller.

### OA-19 — Small dead authorities accumulate in domain and persistence code

High-confidence independent removals include:

- `noema-store` declares `noema-memory` but never references it;
- native memory creates, clears, and writes `memory_pages` for every page, but all
  search reads use `memory_fts` and no reader uses `memory_pages`;
- several store methods/constants are unused or public only for their own tests;
- task model-pool defaults/selection expose alternate no-readiness or test-only
  paths while all production callers use the registry-aware transactional path;
- `noema-home` has a `force` initialization option and fallible diagnostics API
  used only by tests;
- artifact operation-ID polymorphism has only the secure production generator and
  a test fake;
- a direct `chrono` dependency exists for one timestamp parse already supported by
  the workspace's `jiff` authority.

Ship these as small deletion slices. Do not turn them into a generic repository or
testing framework.

### OA-20 — One-time compatibility runs on every startup or every render

Host startup scans four historical provider placeholders, queries their references,
and may delete rows/directories on every boot. It also removes an old `mnemosyne`
directory each startup. Clients continue parsing old telemetry and display formats.

Decide a support window. Once it has passed, delete the paths. If account cleanup
must remain temporarily, use an explicit one-time version marker or forward-only
migration where possible; do not keep permanent startup scanning. Leaving an
unknown stale filesystem directory is safer than perpetual destructive cleanup.

Legacy iconless native-memory reads and the explicit SQLite v9 upgrade are current
documented contracts and are excluded from this recommendation.

### OA-21 — Frontend type adapters point to dependency skew

The app declares Astryx core/CLI/theme `0.1.2` and direct StyleX `0.19.0`, while
Astryx embeds StyleX `0.18.3`. The repository's current frontend instructions name
Astryx `0.1.9`. Twenty-seven local `*XStyle` functions across 21 files cast arrays
through `unknown` into component-specific `xstyle` types.

Use the required Astryx upgrade workflow, align the StyleX versions, and then
delete per-file cast adapters. If a compatibility helper remains genuinely needed,
one shared helper has many concrete consumers; do not retain 27 local versions.

## Medium-confidence and opportunistic opportunities

### OA-22 — Provider generation uses a native trait plus a universal erased trait

Five providers implement `ModelProvider`, every production construction immediately
passes through `erase_model_provider`, and runtime uses only `ProviderOperations`.
The erasure wrapper forwards the full interface and also owns Markdown message
normalization.

This is a legitimate Rust object-safety pattern, so it is not an automatic removal.
Run a small spike: either make the single production contract object-safe and let
providers implement it directly, or document why native futures materially avoid
cost/complexity. Proceed only if normalization remains one authority and the total
patch is clearly net-negative. Do not replace it with generics throughout runtime.

### OA-23 — Speculative and test-only vocabulary should wait for consumers

Examples suitable for nearby cleanup include:

- conversation owner variants for agent, conversation, workspace, project, task,
  and tool when production currently constructs human ownership only;
- three identical `ActorRef` constructors that retain no actor kind;
- enum-string/constructor/getter tests that enforce no external contract;
- adapter argument source with one value and no semantic reader;
- dead capability key DTOs, transitional web type aliases, unused provider matrix
  flags, and no-caller convenience wrappers;
- an operation-ID source trait whose second implementation exists only in tests.

Do not rewrite persisted constraints or future product goals merely to remove enum
variants. Narrow current code and public APIs while leaving immutable migrations
alone.

## Product decision gates

These are the largest possible reductions, but each changes a stated capability.
They require explicit product direction and must not be smuggled into refactors.

### DG-01 — Dormant adapter scheduler

The adapter filesystem scheduler is roughly 816 lines. It implements installation,
leases, checkpoints, retries, revoke, recovery, migration, and quarantine. No
production producer or polling worker calls those operations; production only runs
startup recovery. Service tests manually seed schedules.

The current context nevertheless calls adapter schedules canonical. Choose one:

- make scheduling a real near-term vertical slice with a producer and event-driven
  worker; or
- remove the state machine and quarantine any old schedule directory with a small
  transition.

Do not keep a polling scheduler indefinitely as a promise of future functionality.

### DG-02 — Integration substrates

The three capability crates alone contain 26,950 production and about 12,042 test
lines, before provider-specific and UI integration code. Native HTTP adapters and
MCP are both first-class stacks with distinct auth, transport, discovery, policy,
and persistence.

If both are core, their difference is justified. If one is experimental, choosing
one near-term substrate would save far more than local refactors. Do not build a
third universal integration framework to unify them.

### DG-03 — Two full client surfaces

Web and iOS contain 54,855 authored production lines plus 26,500 generated lines.
Cross-platform parity duplicates GraphQL documents, projections, navigation,
lifecycle, and product workflows across languages.

That cost is inherent to the product choice. Shared GraphQL fragments can reduce
contract repetition, but shared generated output or a cross-platform UI framework
would likely add more complexity. Deferring one client is the only large structural
reduction.

### DG-04 — Work orchestration breadth

The task domain crate and dedicated Tasks client directories already exceed 14,000
lines before store, runtime, API, ACP, recurrence, review, notification, and
reconciliation code are counted. Planner/executor/reviewer roles, immutable
contracts, gates, leases, recurrence, ACP, and review are explicit current product
goals.

Simplify duplicate readers and strings, but do not flatten the state model unless
the product narrows. The largest reduction would be a product decision about which
roles and workflows remain first-class.

### DG-05 — Secondary product systems

Model evaluation, A2UI, PWA/web push, local-model qualification, and runtime debug
each add cross-layer code. Every one has a current consumer, so none qualifies as
dead infrastructure. Validate adoption and strategic importance before expanding
them; retire a whole vertical slice if it is not paying for itself rather than
building generic frameworks around it.

## Complexity that should be protected

The audit explicitly rejects these tempting but harmful “simplifications”:

- **Forward-only migrations and schema convergence tests.** `schema.rs` is large
  because already-applied migrations are immutable. Preserve upgrade and fresh
  convergence coverage.
- **Approval, fencing, ambiguous outcomes, and durable interaction state.** These
  prevent duplicate or unauthorized effects and own real races.
- **Exact secret handling, OAuth state, PKCE, credential stores, and secure
  bindings.** Remove broad concealment of ordinary data, not typed secret wrappers.
- **SSRF, DNS pinning, redirect, byte-limit, filesystem traversal, symlink,
  cancellation, and rollback tests.** These protect distinct boundaries.
- **Different provider and OAuth protocols.** OpenAI/Codex Responses, OpenRouter
  Chat Completions, provider device code, adapter PKCE, and MCP OAuth are materially
  different external contracts.
- **Real production polymorphism.** Providers, web backends, MCP transports,
  capability invokers, payload sanitizers, and binding sources have concrete
  consumers.
- **Artifact ports.** The filesystem and SQLite boundary avoids crate cycles and
  owns atomic publication/cancellation behavior; only the parallel concrete writer
  should disappear.
- **Markdown memory plus FTS.** Markdown is canonical and FTS disposable. Remove
  only the unread duplicate index table.
- **Generated client output.** Generated code is not authored complexity. Its
  freshness and source duplication are the problems.
- **Platform-specific UI and lifecycle.** Do not hide real differences behind a
  cross-platform abstraction.
- **Large cohesive files.** Splitting a file without eliminating a duplicate
  authority generally increases the system and is not a reduction.
- **`noema-dev` and Rust size reporting.** They are wired into the current workflow
  and reduce validation cost; the inactive parallel gates are the cleanup target.

## Bottom line

Noema does not need a broad architectural rewrite. It needs a sequence of deletion
and authority-consolidation slices, each with a named invariant and a net-negative
budget. Start with current correctness and inert code, then remove compatibility
and duplicate representations. Keep product-scope choices separate so a cleanup
effort cannot silently redesign the product.

The highest-leverage near-term result is achievable without touching database
schema or core runtime state machines: repair native generation, remove heuristic
secrecy, delete dormant adapter and gate machinery, retire inactive tooling and
historical docs, remove dead client UI, and collapse test-only authorities. The
[remediation report](2026-08-08-overengineering-remediation.md) records the
completed review units and retained decisions.
