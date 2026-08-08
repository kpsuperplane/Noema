# Overengineering audit remediation report

- **Status:** Implementation complete within approved engineering scope
- **Audit baseline:** `ef0db3918e4997c214f9762f3be1e5753d6022de`
- **Measured implementation head:** `43267781ee9e37606f6ac3303373a6a657530ebf`
- **Mode:** Implement, validate, and record explicit blockers and product decisions
- **Scope:** All 23 engineering findings and five product decision gates from the
  [2026-08-08 audit](2026-08-08-overengineering-audit.md)

This is the durable completion ledger for the audit. The original audit remains a
review snapshot, and the [evidence appendix](2026-08-08-overengineering-evidence.md)
retains its baseline measurements. Current subsystem contracts remain authoritative
over this report.

## Executive outcome

The remediation removed the code and documentation that had clear evidence of no
production consumer, consolidated parallel authorities, narrowed executable
contracts to behavior the runtime actually supports, and corrected the two
information-handling failures. It did not delete live product capabilities merely
because they are large.

Measured against the audit baseline:

| Measure | Baseline | Remediated | Change |
| --- | ---: | ---: | ---: |
| Rust production lines | 150,915 | 148,699 | **-2,216** |
| Rust test lines | 54,949 | 54,335 | **-614** |
| Total Rust lines | 205,864 | 203,034 | **-2,830** |
| Rust test declarations | 960 | 951 | **-9** |
| Markdown under `docs` | 56,985 | 6,946 including this report | **-50,039** |

The measured code range changed 333 files with 6,146 insertions and 63,108
deletions. Most insertions were bounded migrations, consolidation rewrites, and
preservation tests rather than new abstraction. The historical documentation
reduction accounts for 52,136 deleted lines in its own commit.

Three engineering findings remain blocked by the same native toolchain boundary,
one compatibility finding is partially complete because of that boundary, and two
candidate refactors were explicitly retained after they failed the repository's
net-negative test. The five decision gates remain product choices, not implied
engineering authorization.

## Disposition vocabulary

- **Closed** — the demonstrated unnecessary authority or failure mode was removed
  or consolidated and validated.
- **Partial / blocked** — every safe independent part was completed, but the
  remaining work requires an unavailable platform toolchain or a product choice.
- **Retained after spike** — the candidate was inspected or prototyped and would
  increase total complexity, so the simplicity stop condition rejected it.
- **Decision required** — implementation would remove or materially narrow a live
  product capability and needs explicit product direction.

## Finding-by-finding disposition

| ID | Disposition | Result | Principal evidence |
| --- | --- | --- | --- |
| OA-01 | **Blocked** | Authored iOS GraphQL remains ahead of committed Apollo Swift. Regeneration and native compile require macOS/Apple JavaScriptCore tooling unavailable in this Linux environment. No generated Swift was hand-edited. | Toolchain investigation; open loop recorded in `docs/context/current.md` |
| OA-02 | **Closed** | Removed English key-name secrecy inference from closed Work-event schemas while retaining exact fields, types, invariants, and the 16 KiB boundary. Added ordinary-value preservation coverage. | `1865fdd8` |
| OA-03 | **Closed** | Deleted the unsupported adapter event framework, HMAC/challenge/deduplication implementation, exports, and tests. Strict manifests reject the absent field. | `b6359196` |
| OA-04 | **Closed** | Reduced continuation to `None | ResponseToken`, removed inert gate/quota policy, narrowed connection and definition storage, and advanced the strict adapter contract to schema version 8. Removed the later one-value argument-source vocabulary. | `b6359196`, `13ca7ec3`, `44682e0d` |
| OA-05 | **Closed** | Deleted all three inactive gate systems and their stale tests/manifest: crate boundaries, focused dependency trees, and critical test names. Active size and Cargo validation remain. | `62ec612a` |
| OA-06 | **Closed** | Removed superseded plans/specs, the stale tracker, contradictory frontend/harness histories, and the obsolete Postgres document. Rebuilt concise current contracts and reduced `docs/context/current.md` to active direction and open loops. | `f36c2a1f` |
| OA-07 | **Closed** | Debug output now preserves ordinary routing IDs, safe URLs/endpoints, public client IDs, attempt IDs, and local paths while continuing to exclude credential material and secret URL components. | `b880914e` |
| OA-08 | **Closed** | Replaced string-derived task failure/retry behavior, side-effect detection, and finalization intent with typed runtime state already available at those boundaries. | `0423c405` |
| OA-09 | **Closed** | Deleted five unreachable web components, the unused native Tasks sidebar view, and associated dead task helpers/exports: 831 net client lines. | `4976aa66` |
| OA-10 | **Closed** | Removed the test-only in-memory continuation cursor authority. Durable cursor storage is the only state authority and retains reopen, expiry, rotation, tamper, and binding-drift coverage. | `b6359196` |
| OA-11 | **Closed** | Made `ArtifactMetadataStore` the single metadata writer, routed GraphQL external-URL creation through it, and removed mirrored inherent write APIs and error round-trips. | `584891a1` |
| OA-12 | **Closed** | Consolidated Work-event row decoding and criteria loading, made record validation/immutability authoritative, and removed ignored read parameters and duplicated query paths. | `4b5d0c7c` |
| OA-13 | **Closed** | Persisted selectable built-in web/provider accounts through forward-only store schema version 33 and removed the synthetic account merge/bypass representation. Existing-version upgrade and fresh convergence are covered. | `1653e79c` |
| OA-14 | **Blocked** | Runtime debug spans remain the intended telemetry authority, but stopping legacy transcript metadata would remove current iOS token/cache visibility. The native query/model update depends on the same blocked Apollo regeneration as OA-01. Web-only removal was rejected because it would create cross-client behavior skew. | Toolchain investigation; open loop in `docs/context/current.md` |
| OA-15 | **Closed** | Removed the single-implementation `CapabilityRouter` and `ProviderRouteResolver` trait boundaries and used their concrete registry routers. | `ca962687` |
| OA-16 | **Closed** | Centralized adapter management snapshots and native web action execution, removing parallel readers and request-local execution branches while preserving reviewed arguments and immutable catalogs. | `96a4dd95`, `7a09fd93` |
| OA-17 | **Blocked** | Fragment-first iOS mapping reduction cannot be completed or compiled safely without regenerating Apollo Swift. Generic mappers were not replaced speculatively or by hand-editing generated code. | Same macOS/Apple JavaScriptCore blocker as OA-01 |
| OA-18 | **Closed** | Collapsed all eight one-consumer Settings pane/content controller pairs, unified task status projection, reused the canonical transcript reconciliation loader, and removed 27 local StyleX cast adapters. | `0f88412b`, `4dd52ef3`, `f419bd28`, `b7121d27`, `7e92f83f` |
| OA-19 | **Closed** | Removed the unused store-to-memory dependency, unread native-memory index path, dead store APIs, test-only task pool selectors, test-only Home surfaces, operation-ID test polymorphism, and the one-use `chrono` dependency. | `1865fdd8`, `a9972c6e`, `0605b313`, `43c46c36` |
| OA-20 | **Partial / blocked** | Removed every-startup provider-placeholder reconciliation and recursive `mnemosyne` cleanup. Current SQLite v9 adoption and iconless native-memory reads remain intentionally supported. Client telemetry compatibility remains coupled to OA-14/OA-01. | `c8fe5471` |
| OA-21 | **Closed** | Aligned Astryx to the repository's 0.1.9 contract, removed the nested StyleX version skew, and deleted all per-file `unknown` cast adapters. | `0f88412b` |
| OA-22 | **Retained after spike** | The native-future provider trait plus object-safe erased contract remains. The erasure boundary is also the one streaming Markdown normalization authority; moving the boxed contract into five providers would distribute that behavior or add code. No clearly net-negative implementation was found. | Source spike; decision recorded in `docs/context/current.md` |
| OA-23 | **Closed (bounded sweep)** | Narrowed conversation ownership to its sole constructed human form, collapsed three `ActorRef` constructors, removed dead capability key DTOs, a one-value adapter argument source, ignored provider payload fields, test-only operation-ID polymorphism, task-pool test paths, and transitional web aliases. Provider capability flags and the evaluator continuation hook were retained after aggregate compilation proved concrete consumers. | `93d91e55`, `44682e0d`, `029f624a`, `0605b313`, `43c46c36`, `7bdbea6b`, `c6679ff7` |

## What changed by authority

### Executable contracts now tell the truth

Adapter manifests no longer advertise event ingress, provider-link/delta cursors,
quota, gates, or argument-source variability that runtime could not execute. The
strict schema was advanced rather than wrapped in a compatibility framework.
Removed fields fail closed at definition setup.

Runtime task decisions now carry typed failure and behavior information through
the boundary that owns it. Work-event validation relies on its exact closed schema,
not an English substring scanner. This fixes both simplicity and multilingual
correctness without weakening secret handling.

### Persistence has fewer parallel representations

Artifact writes use one trait authority. Work-event rows use one decoder and
criteria loader. Built-in provider accounts are persisted like other selectable
accounts. Native memory no longer writes an unread second index. Store schema
version 33 is forward-only, with upgrade and fresh convergence tests; no applied
migration was edited.

### Client composition is shallower

Unreachable components were deleted rather than registered. Settings domains
still remain distinct, but their one-consumer state wrappers no longer mirror broad
prop surfaces into separate content components. Task status and PWA transcript
reconciliation now use their existing canonical projections/loaders.

The repository-local `noema-product-ui` guidance influenced this work: pane
collapsing preserved each Settings domain's information hierarchy, focal actions,
Astryx surfaces, and responsive composition. No visual redesign was attempted.

### Historical design no longer competes with current contracts

Git history now owns completed milestone detail. The active docs describe current
SQLite, Work, memory, harness, and frontend behavior instead of retaining parallel
SurrealDB, Supermemory, Mem0, Mnemosyne, retired navigation, and earlier provider
architectures. This report and the audit are evidence artifacts, not subsystem
authorities.

## Product decision gates

The user's instruction to address the audit authorized engineering remediation;
it did not select which live product capabilities to retire. These choices have
materially different user outcomes, so no branch was inferred.

| Gate | Status | Explicit choice still required | Engineering action after a choice |
| --- | --- | --- | --- |
| DG-01 — adapter scheduler | **Decision required** | Commit to a near-term scheduler product, or remove the dormant filesystem scheduler. | If retained, deliver a producer plus event-driven worker as one vertical slice. If retired, remove the state machine and quarantine old schedule state with a bounded transition. |
| DG-02 — integration substrates | **Decision required** | Keep both native HTTP adapters and MCP as core, or designate one as experimental/retired. | Preserve both distinct stacks if both are core. Otherwise retire one complete vertical slice; do not add a universal third framework. |
| DG-03 — two full clients | **Decision required** | Continue full web+iOS parity, or defer one client. | If both remain, accept inherent duplication and share only economical GraphQL fragments. If one is deferred, remove its parity obligation explicitly. |
| DG-04 — Work breadth | **Decision required** | Keep planner/executor/reviewer, recurrence, ACP, gates, leases, and review as first-class, or name the exact roles/workflows to retire. | Remove entire unselected workflows and their cross-layer surfaces; do not flatten safety state for workflows that remain. |
| DG-05 — secondary systems | **Decision required** | For model evals, A2UI, PWA/web push, local-model qualification, and runtime debug, decide individually whether adoption justifies continued ownership. | Retain a complete vertical slice or retire it completely. Do not build generic frameworks around low-adoption systems. |

Until those decisions are made, all five are open product questions rather than
unfinished refactors.

## Evaluated reductions that were intentionally rejected

### Provider contract collapse

`ModelProvider` uses native async futures, while `ProviderOperations` supplies the
object-safe runtime boundary. The erasure wrapper also owns streaming Markdown
message splitting and normalization. Directly implementing the boxed interface in
each provider would either duplicate that logic across five implementations or
introduce another shared layer. The spike therefore failed OA-22's explicit
"clearly net-negative" condition.

### Backend-conditional web-fetch summarizer context

Exa fetch ignores `WebFetchContext`, while direct HTTP uses it for summarization.
A working optional-context prototype passed 17 provider fetch tests, three focused
runtime tests, and clippy, but added nine production Rust lines after tightening.
It was reverted because a small unused resolution cost does not justify increasing
the interface and branch surface.

### Evaluator continuation hook

An apparent no-caller hook was initially removed. Aggregate workspace compilation
proved that the `eval-support` feature is a concrete consumer. The hook was restored
behind that feature and its feature-specific check/clippy pass. This is a useful
example of the audit's rule: search evidence from the default feature set is not
sufficient proof of dead code.

## Compatibility intentionally retained

- Explicit SQLite v9 adoption is a current persistence contract.
- Iconless native-memory page reads are a current documented contract.
- Forward-only migrations and convergence tests remain intact.
- Legacy provider telemetry remains until iOS can consume runtime debug spans.
- Generated Apollo Swift remains committed and untouched until it can be regenerated
  by the supported toolchain.

## Validation record

### Passed

- `cargo fmt --all --check`.
- `cargo validate check --workspace --exclude noema-desktop`.
- `cargo validate clippy --workspace --all-targets --exclude noema-desktop -- -D warnings`.
- `cargo validate check -p noema-runtime --features eval-support`.
- `cargo validate clippy -p noema-runtime --features eval-support --all-targets -- -D warnings`.
- Focused crate tests and warning-denied clippy ran for every Rust reduction slice.
- The final adapter suite passed 97/97 tests, including strict schema v8,
  persistent cursors, OAuth, invocation, refresh, and schedule coverage.
- Web `bun run lint` and `bun run build` passed after the frontend reductions. The
  existing large-chunk warning remains.
- The final Rust size report confirms a 2,830-line net reduction and nine fewer test
  declarations.
- `git diff --check` passed at each commit boundary.

### Standard gate blocked by desktop configuration

These standard commands all stop at the same existing Tauri build-script failure:

- `cargo check-workspace`
- `cargo gate-lint`
- `cargo gate-test`

On Linux, `noema-desktop` enables `macos-private-api`, but that feature is not in
the allowlist produced from the active `tauri.conf.json`; Tauri instructs running
through its dev/build entrypoint or removing the macOS-only feature. The audit
remediation did not change desktop feature ownership. Excluding that package, the
workspace check and warning-denied clippy pass.

### Non-desktop unit-suite debt

`cargo validate test --workspace --exclude noema-desktop --no-fail-fast` completed
and reported failures in five targets. These are recorded rather than folded into
unrelated refactors:

- `noema-api`: three model recommendation/reasoning fixture expectations;
- `noema-host`: two first-run/default-provider expectations;
- `noema-providers`: one Obscura URL-fragment assertion;
- `noema-runtime`: one ACP socket-expiry timing assertion and two interaction tests
  colliding on shared `provider_call_id` fixture state;
- `noema-store`: one canonical default reasoning-effort expectation.

All other tests in those runs continued because `--no-fail-fast` was used. The
adapter target passed completely. Focused tests for the changed authorities passed.

An additional nonstandard `noema-runtime --all-features` check exposes an existing
feature-wiring gap: `contract-test-support` references `noema_store::test_support`
without enabling the store's `test-support` feature. The specific `eval-support`
feature used by the restored continuation hook passes.

### Platform validation not available

- Apollo iOS generation and iOS compilation were not run because the supported
  generator/native dependency chain requires macOS/Apple JavaScriptCore.
- Frontend layout was statically validated by lint/build but not visually inspected
  in a browser, as browser inspection was not requested.

## Commit ledger

The remediation was deliberately split into independently reviewable units:

| Area | Commits |
| --- | --- |
| Dormant/dead authorities | `b6359196`, `1865fdd8`, `4976aa66`, `62ec612a` |
| Information and typed semantics | `b880914e`, `0423c405` |
| Artifact, Work, and routing consolidation | `584891a1`, `4b5d0c7c`, `ca962687` |
| Adapter contract and management | `13ca7ec3`, `96a4dd95`, `44682e0d` |
| Provider accounts and native web | `1653e79c`, `7a09fd93` |
| Frontend authority reduction | `0f88412b`, `4dd52ef3`, `f419bd28`, `b7121d27`, `7e92f83f` |
| Small domain/test seams | `a9972c6e`, `93d91e55`, `029f624a`, `0605b313`, `43c46c36`, `7bdbea6b`, `d773d5c2` |
| Compatibility and documentation | `c8fe5471`, `f36c2a1f` |
| Aggregate integration corrections | `c6679ff7`, `43267781` |

Two lint-only commits (`07251b4e`, `2a9cea0f`) kept touched crates warning-clean,
and `1a957d71` was corrected by `c6679ff7` after aggregate feature compilation
proved the evaluator consumer.

## Remaining actions

Engineering cleanup from the audit is complete within the current product contract.
The next actions require new information or authority:

1. run Apollo generation and build on macOS, then close OA-01, OA-14, and OA-17 as
   one native contract/telemetry unit;
2. choose a direction for each of DG-01 through DG-05 before deleting or expanding
   those live product surfaces;
3. triage the separately listed desktop gate, feature-wiring, and aggregate fixture
   failures as validation maintenance, not as a continuation of this refactor.
