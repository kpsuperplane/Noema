# Overengineering audit evidence appendix

- **Status:** Dated evidence snapshot
- **Baseline:** `ef0db3918e4997c214f9762f3be1e5753d6022de`

This file preserves measurements from the stated baseline.
It does not describe current repository size or behavior.

This appendix records the baseline, method, repository measurements, and concrete
evidence behind the [main audit](2026-08-08-overengineering-audit.md). It is a
review artifact, not a replacement for source-level contracts.

## Baseline and worktree handling

- Baseline commit: `ef0db3918e4997c214f9762f3be1e5753d6022de`.
- Branch: `main`, 41 commits ahead of `origin/main` when the snapshot was taken.
- One unrelated untracked file existed and was excluded:
  `noema-acp-validation-20260803T1713Z.tmp`.
- No production, test, schema, generated, evaluation, or existing documentation
  file was modified during review.
- Earlier unrelated source changes were committed by another active workflow before
  the baseline stabilized. All measurements and finding evidence were rechecked at
  the baseline above.

## Review method

The review used four parallel tracks:

| Track | Primary scope | Review focus |
| --- | --- | --- |
| Cross-cutting | repository contracts, workspace shape, docs, scripts, dependency/type patterns | authority conflicts, inactive governance, total footprint, product-scope gates |
| Domain/persistence | store, tasks, conversations, memory, workspaces, artifacts, home | duplicate validation/read/write authorities, dead APIs/dependencies, test seams, compatibility |
| Runtime/integrations | runtime, capabilities, adapters, MCP, providers, API, host, server, desktop, dev | dormant schemas/state machines, routing layers, policy enforcement, provider/web duplication, secret handling |
| Clients/tooling | web, iOS, GraphQL documents/generated output, scripts, evals, docs | reachability, source/generated drift, repeated projections/mappers, current-versus-historical contracts |

The following techniques were used:

1. read the repository's current project, context, simplicity, security, persistence,
   and client contracts;
2. inventory every tracked source/document surface by language and ownership;
3. measure Rust production/test lines using the repository size reporter;
4. enumerate large files as inspection candidates, not automatic findings;
5. trace trait implementations and production consumers with exact symbol search;
6. trace duplicated record fields, SQL decoders, conversion functions, and fallback
   reads to identify parallel authorities;
7. inspect feature-looking structures for an end-to-end producer, executor, reader,
   and UI/API consumer;
8. compare GraphQL schema, authored client operations, generated operation strings,
   and runtime uses;
9. trace imports/declarations for client reachability;
10. run inactive policy scripts at the snapshot rather than assuming they work;
11. separate generated output, historical docs, tests, and production code in all
    estimates;
12. reject reductions that would weaken a unique security, privacy, concurrency,
    protocol, migration, or data-loss invariant.

This was a static and executable-tooling audit. It was not a browser visual review,
an end-to-end product-usage study, or a production telemetry analysis.

## Repository footprint

### Rust

The repository-owned command:

```text
bun run scripts/report-rust-size.ts --base HEAD
```

reported:

| Measure | Count |
| --- | ---: |
| Production lines | 150,915 |
| Test lines | 54,949 |
| Total Rust lines | 205,864 |
| Rust test declarations | 960 |

The largest crate footprints were:

| Crate | Production | Tests | Total |
| --- | ---: | ---: | ---: |
| `noema-store` | 34,209 | 10,853 | 45,062 |
| `noema-runtime` | 28,769 | 13,250 | 42,019 |
| `noema-providers` | 25,564 | 11,523 | 37,087 |
| `noema-capability-adapters` | 13,899 | 6,869 | 20,768 |
| `noema-api` | 17,093 | 2,606 | 19,699 |
| `noema-capabilities-mcp` | 9,337 | 3,832 | 13,169 |
| `noema-capabilities` | 3,714 | 1,341 | 5,055 |
| `noema-model-evals` | 3,714 | 928 | 4,642 |
| `noema-tasks` | 4,267 | 58 | 4,325 |

The July remediation baseline `5d3e852b` measured 38,262 production and 25,796
test lines. The current snapshot is larger by 112,653 production and 29,153 test
lines, with 297 additional test declarations. This is feature-growth context, not
proof of overengineering; the audit requires a current unused consumer, duplicate
authority, or unsupported generality before recommending removal.

### Clients and documentation

| Surface | Files | Lines | Classification |
| --- | ---: | ---: | --- |
| Web TS/TSX total | 243 | 42,401 | all source |
| Web authored production | 228 | 36,999 | excludes tests and two generated files |
| Web tests | 13 | 2,386 | authored test source |
| Web generated | 2 | 3,016 | GraphQL types and route tree |
| iOS Swift total | 419 | 41,340 | all Swift |
| iOS authored | 54 | 17,856 | excludes Apollo output |
| iOS generated | 365 | 23,484 | committed Apollo output |
| Scripts | 8 | 2,397 | TypeScript/shell tooling |
| Markdown under `docs` | 98 | 56,985 | current and historical mixed |
| `docs/superpowers` Markdown | 73 | 47,739 | historical plan/spec corpus |

The authored Rust, web, and iOS production total is approximately 205,770 lines,
before GraphQL source documents, scripts, and evaluation data. Generated client
output adds 26,500 lines but is not counted as authored complexity.

### File-size inspection signals

- 123 Rust files exceed 500 lines.
- 44 Rust files exceed 750 lines.
- 15 Rust files exceed 1,000 lines.
- Five authored web files exceed 750 lines; one 837-line web test does as well.
- Four authored iOS files exceed 750 lines.
- Forty-eight Markdown files exceed 300 lines; twenty exceed 1,000.

Large-file review found both kinds of result:

- justified: immutable schema migrations, schema convergence tests, approval and
  filesystem race tests, protocol adapters, transcript virtualization;
- reducible: duplicate adapter/native web execution, generic iOS mappers caused by
  repeated GraphQL selections, stale client components, and dormant integration
  state machines.

## Finding evidence

### E-01 — iOS schema/source/generated divergence

Current schema authority:

- `graphql/schema.graphql:204-216` exposes
  `AdapterDefinition.credentialSetup`.
- the schema no longer exposes `clientSetupUrl`, `oauthRedirectUri`,
  `acceptsOauthClientJson`, `importAdapterOauthClientJson`, or
  `ImportAdapterOauthClientJsonInput`.

Current authored operations:

- `apps/ios/Noema/Operations/Chat.graphql:288-344` selects credential setup;
- `Chat.graphql:397-401` authors `SetupAdapterConnection`;
- `apps/ios/Noema/Operations/Settings.graphql:508-668` selects credential setup
  and authors `SettingsSetupAdapterConnection`.

Stale generated operations query retired fields or mutations at line 11 of:

- `PendingChatInterventionsQuery.graphql.swift`;
- `SettingsAdapterDefinitionsQuery.graphql.swift`;
- `SettingsApproveAdapterDefinitionMutation.graphql.swift`;
- `ImportAdapterOauthClientJsonMutation.graphql.swift`;
- `SettingsImportAdapterOauthClientJsonMutation.graphql.swift`.

Retired generated types remain in production use:

- `apps/ios/Noema/Features/Settings/SettingsCapabilityFlows.swift:86-122` and
  `:327-354`;
- `apps/ios/Noema/Features/Chat/HumanInterventions.swift:80-87`;
- `apps/ios/Noema/Features/Chat/ChatInterventionModelSupport.swift:227-244`.

The authored and generated sets each contain 105 operation names. AST validation
of authored documents against the current schema produced zero errors. Validation
of operation strings embedded in committed generated Swift produced the five errors
above. The generated set retains two retired import operations and omits the two
authored setup replacements.

`apps/ios/README.md:17-28` documents only a manual
`apollo-ios-cli generate --path apollo-codegen-config.json` workflow. No checked-in
CI, Xcode build phase, Cargo alias, or root validation entrypoint checks native
generation freshness. The Apollo CLI was not installed in the audit environment,
so generated repair/build validation belongs to implementation.

### E-02 — closed Work-event schema plus heuristic scan

- `crates/noema-tasks/src/event_validation.rs:18-65` bounds JSON, requires an
  object with `v=1`, verifies the exact field set for the event kind, validates
  each field, and checks kind-specific invariants.
- `:43-48` then rejects `contains_sensitive_key`.
- `:333-355` recursively searches English substrings including `secret`,
  `credential`, `password`, `token`, `prompt`, `answer`, `description`, `result`,
  `feedback`, `transcript`, `provider_payload`, and `lease`.
- `docs/harness/security.md` states that field-name substring matching and entropy
  heuristics are not classification authorities.

No arbitrary top-level or nested object can pass the exact event field/type
validators. The scan therefore adds no independent boundary.

### E-03 — dormant adapter event framework

- `crates/noema-capabilities/adapters/src/event.rs`: 341 lines.
- `event/tests.rs`: 135 lines.
- `definition.rs:344-346` parses the field without activating it.
- `definition.rs:620-650` retains event types for a future milestone.
- `compiler.rs:522-524` rejects every event subscription as unsupported.
- exports appear at `lib.rs:68-71`.

No production call site consumes `verify_hmac_event`, `verify_challenge`,
`ChallengeVerifier`, `EventDeduplicator`, or the event DTOs.

### E-04 — incomplete continuation and inert policy fields

Continuation:

- `definition.rs:543-607` represents `ProviderLink` and `DeltaCursor`;
- `continuation.rs:30-229` implements their validators/retry helpers;
- `compiler.rs:687-708` accepts them and setup advertises them;
- `request.rs:122-131,161-179` and `invocation.rs:235-286,524-543` execute only
  `ResponseToken`.

Policy-looking adapter fields:

- definition at `definition.rs:263-307,347-349`;
- compiled copies at `compiler.rs:55-58,96-97,227-228,915`;
- setup model/prompt at `setup.rs:192-193,210,240,291`;
- gates are used only for compatible replacement fencing at
  `service.rs:123-136` and `connection_store.rs:1138-1162`;
- quota has no eligibility, invocation, projection, or accounting reader.

### E-05 — inactive gate systems fail at the baseline

Line inventory:

```text
 663 scripts/check-crate-boundaries.ts
 661 scripts/check-crate-boundaries.test.ts
 219 scripts/check-focused-dependency-trees.ts
 195 scripts/check-focused-dependency-trees.test.ts
 203 scripts/verify-rust-critical-tests.ts
 159 scripts/verify-rust-critical-tests.test.ts
 250 docs/superpowers/baselines/rust-critical-test-contracts.json
2350 total
```

Exact-symbol search outside their own tests, implementations, manifest, and
historical plans found no consumer. `.cargo/config.toml` validation aliases run
`noema-dev`, Cargo checks, clippy, and tests, but none of these scripts.

Execution results:

- `check-crate-boundaries.ts` exited 1 with 17 stale violations, including unknown
  workspace packages `noema-capability-adapters` and `noema-dev` and rules rejecting
  current deliberate dependencies.
- `check-focused-dependency-trees.ts` exited 1 because offline `cargo tree` tried
  to fetch uncached `winsafe v0.0.19`.
- `verify-rust-critical-tests.ts` exited 1 because four exact test names no longer
  exist.

### E-06 — historical/current documentation conflict

Historical corpus:

- `docs/superpowers/plans`: 52 files, 41,893 lines;
- `docs/superpowers/specs`: 21 files, 5,846 lines;
- combined Markdown: 47,739 lines, 83.8% of documentation Markdown;
- 39 files mention retired `noema-core`;
- 18 mention SurrealDB;
- 24 mention retired/superseded Supermemory, Mem0, Mnemosyne, or DuckDuckGo work.

Representative current-tree contradictions:

- `docs/harness/runtime.md:24,28,44,714` names SurrealDB;
- `docs/harness/events.md:33` says current state derives from events;
- `docs/harness.md:571-572` explicitly prefers a narrow slice of the full
  architecture rather than a smaller architecture;
- `docs/frontend/README.md:107,191,239` names SurrealDB and removed memory graph
  GraphQL operations;
- `docs/frontend/object-model.md:41-45` calls tasks/integrations future;
- `docs/frontend/navigation-workflows.md:31` names Home and Memory as primary;
- even `docs/frontend/current-contract.md:66-100` omits current `/work`, calls
  Tasks and privacy future, and lists routes absent from the current
  `apps/web/src/app/routes.ts:22-36` union;
- `docs/postgres.md:3-14` calls SurrealDB current;
- `docs/codebase-audit-tracker.md:146-152` lists file splits already performed;
- tracker line 214 says the frontend lacks a test script, but
  `apps/web/package.json:19` defines `bun test src`;
- tracker line 213 claims CI, but no `.github` workflow is tracked.

`docs/context/current.md` is 299 lines, but its 4,073 words and 31,319 characters
include long completed implementation summaries. The simplicity contract says the
file should contain active direction/current constraints/open loops and Git history
should own completed execution detail.

### E-07 — verified ordinary-value over-redaction

Routing identifiers:

- `crates/noema-capabilities/src/router.rs:13-18` redacts `InvokerKey`;
- `binding.rs:13-18` redacts `OperationToken`;
- `binding.rs:36-45` redacts `CapabilityTarget`;
- runtime uses values such as literal `runtime-execution` and a canonical tool name
  at `runtime/model_tools.rs:757-758`.

Endpoints and public OAuth metadata:

- `crates/noema-providers/src/config.rs:134-151` hides OpenAI base URL;
- `:180-196` hides OpenRouter base URL;
- `:249-262` hides issuer, token URL, and client ID;
- `:297-313` hides Codex base URL;
- `response_support/http.rs:6-31` already rejects credential-bearing, query, and
  fragment components from generation endpoints.

Paths:

- `config.rs:327-338,361-381`;
- `local_model/management.rs:64-80`;
- `local_models/eval.rs:32-44`;
- `local_models/eval/materialize.rs:46-55`.

OAuth attempt IDs:

- MCP: `mcp/oauth_model.rs:78-88,97-108,116-136`;
- adapter: `adapters/oauth.rs:114-135`;
- the adapter service already prints the public attempt ID at
  `adapters/service.rs:156-186`.

Callback and authorization URLs may contain actual code/state and remain excluded
unless an exact component sanitizer is introduced.

### E-08 — string semantic inference

- `crates/noema-runtime/src/daemon/task_runtime.rs:630-657` turns a rendered error
  into a safe message, error code, and retry flag through substring checks.
- `runtime/progress.rs:185-187` infers side effects from operation-name words even
  though capability bindings carry `read_only`, `idempotent`, `destructive`, and
  `open_world`.
- `runtime/task_continuation.rs:136-158` branches on `reason.contains("human input")`
  after callers map an existing `ProgressAuditDecision` to prose.
- `apps/web/src/components/transcript/markerModel.ts:518-524,546-556` filters
  English display strings and strips legacy English prefixes.

### E-09 — unreachable client code

Repository-wide symbol search found declarations but no imports/uses for:

| File/symbol | Lines |
| --- | ---: |
| `TaskResumeControls.tsx` | 100 |
| `TaskToolMarker.tsx` | 175 |
| `ProviderStatus.tsx` | 23 |
| `ChatWorkPanel.tsx` | 168 |
| `StageBadge.tsx` | 10 |
| `TasksSidebar` portion of `TasksSidebar.swift` | 89–91 |

Additional unreferenced task helpers and exports were traced individually. Static
search found no string-based dynamic imports for the files above.

### E-10 — test-only cursor authority

- `adapters/src/continuation.rs:259-421` defines `CursorStatus`, `StoredCursor`, and
  `CursorStore` and labels it an in-process reference store for deterministic tests.
- only `continuation/tests.rs:118-167` consumes it;
- production uses `DurableCursorStore` through `service.rs` and `invocation.rs`;
- `FullResyncRequired` and `commit_resync` exist only in the test authority.

### E-11 — artifact writer duplication

- inherent store writer: `crates/noema-store/src/artifacts.rs:60-126`;
- port implementation: `artifact_metadata_port.rs:20-131`;
- common transactions: `artifact_writes.rs`;
- GraphQL external-URL creation is the sole production inherent-create caller at
  `noema-api/src/graphql/artifacts.rs:293`;
- inherent append has no production caller;
- `noema-store/src/error.rs` maps artifact-domain errors to store errors;
- `artifact_metadata_port.rs:179-205` maps them back.

The port has real production/test consumers and must remain; the parallel inherent
writer is the finding.

### E-12 — duplicate Work decoders and ignored criteria

- canonical-ish event decoder: `noema-store/src/task_events.rs:66-144`;
- second decoder: `noema-store/src/work_reads.rs:371-412`;
- both decode the same 12 fields and reconstruct the same typed record;
- `task_reads.rs:197-215` accepts `_expected_criteria` for submission/review and
  ignores both;
- `work_read_task.rs:79-102` and `work_run_context.rs:133-156` allocate and pass
  the ignored values;
- `work_read_submission_batch.rs:190-224` and
  `work_read_list_rows.rs` contain near-identical criterion-ID queries.

### E-13 — provider account dual authority

- `noema-providers/src/accounts.rs:93-156` defines two records with thirteen
  identical fields; one adds derived capabilities;
- `:158-203` manually maps between them;
- `:236-243` returns three synthetic system accounts;
- `noema-providers/src/persistence/capabilities.rs:10-71` introduces
  `Persisted | ValidatedSystem` reference modes;
- `noema-api/src/graphql/web_tool_settings.rs` merges synthetic and stored accounts;
- `noema-runtime/src/daemon/runtime/web_tools.rs` loads store first and falls back
  to the synthetic list.

### E-14 — provider telemetry dual authority

- legacy transcript writer:
  `noema-runtime/src/daemon/runtime/transcript_persistence.rs:26-61` and
  `provider_items.rs`;
- canonical span store: `noema-store/src/runtime_debug.rs` and
  `schema.rs:802-827`;
- web fallback: `debugUsage.ts`, `RuntimeDebugDialog.tsx::LegacyUsage`, and
  `Transcript.tsx` legacy plumbing;
- native fallback: `ChatDetailParityViews.swift:58`.

Both persist provider/model/phase/index/token data. Runtime debug spans additionally
own duration/status and are the current documented debug authority.

### E-15 — one-implementation routing traits

- `CapabilityRouter`: declaration at `capabilities/src/router.rs:338-355`, only
  implementation at `:534-559`; the sole `Arc<dyn CapabilityRouter>` cast is a
  test.
- `ProviderRouteResolver`: declaration at
  `providers/src/routing/mod.rs:55-62`, sole implementation at `:86-125`; host and
  runtime construct the concrete resolver.
- `ProviderSelectionLoader` beneath the resolver has stateful test implementations
  for race behavior and is the meaningful seam; it is not included in the removal.

### E-16 — native web and adapter snapshot duplication

- foreground web orchestration: `runtime/local_tools.rs:553-737`;
- approved web execution: `local_tools/web_actions.rs:104-199`;
- approved binding reconstruction: `action_resolution.rs:150-301`;
- existing model/runtime binding authority: `model_tools.rs:168-200,678-765`.

Adapter stores are independently reconstructed/scanned at:

- `noema-api/src/graphql/adapters.rs:191-249,522-547`;
- `noema-host/src/composition.rs:108-123`;
- while `AdapterCapabilityService` owns stores/snapshot at
  `adapters/src/service.rs:276-310`.

### E-17 — iOS GraphQL/mapping repetition

- `Settings.graphql`: 1,119 lines, 40 operations, one fragment;
- the same model preference/options shape occurs five times in lines 8-216;
- `SettingsModelSupport.swift:67-111` supplies a fourteen-closure generic mapper
  and repeats its key-path invocation five times;
- adapter definitions repeat at `Settings.graphql:508-668` and
  `Chat.graphql:288-344`;
- `Memory.graphql:1-140` repeats article/page/status selections;
- `MemoryModel.swift:190-330` repeats apply and operation-specific mapping overloads;
- Tasks already uses sixteen fragments in `TasksFields.graphql`.

After fragment expansion, web has 130 operations, iOS has 105, 50 names are shared,
and 28 shared operations are structurally identical. Those exact matches account
for 494 expanded selection lines per client.

### E-18 — web duplicate layers/projections

- eight one-consumer Settings pane/content pairs total 3,190 lines;
- each content component has exactly one production importer;
- seven pane modules re-export an unused content symbol;
- `TaskStatusBadge.tsx:57-69` and
  `TaskDetailQueryPanel.tsx:251-263` derive status separately and disagree on
  `REQUEST_CHANGES`;
- `App.tsx:369-456` owns canonical transcript load/merge state while PWA
  reconciliation at `:459-515` repeats query/merge without updating that state;
- 27 local functions matching `*XStyle` occur in 21 web files.

Dependency inspection in `apps/web` reported:

```text
@astryxdesign/core@0.1.2 -> @stylexjs/stylex@0.18.3
direct @stylexjs/stylex@0.19.0
```

### E-19 — dead dependency/index/API examples

- `noema-store/Cargo.toml:23` depends on `noema-memory`; no store Rust source
  references `noema_memory`.
- `noema-memory/src/native.rs:646-672` creates, clears, and fills `memory_pages`;
  repository-wide search finds no reader; FTS search reads `memory_fts` at `:451`.
- dead or unnecessarily public store symbols include
  `list_recent_conversation_items_for_context`, `safe_authorization_context`,
  `WorkPageSize::DETAIL_DEFAULT`, `WORK_RUN_CONTEXT_MAX_CRITERIA`, and
  `StoreError::is_system_invariant`.
- task model pool has test-only no-readiness and selector APIs while production uses
  registry-ready transaction-local selection.
- `noema-home` production always initializes with `force: false` and logs through
  best-effort `try_append`; alternate surfaces are test-only.

### E-20 — permanent compatibility paths

- `noema-host/src/composition.rs:108-112,393-431` runs legacy provider placeholder
  reconciliation on every startup;
- `noema-store/src/provider_accounts.rs:333-377,583-610` scans references and
  conditionally deletes the historical rows;
- host composition also removes the retired `mnemosyne` directory recursively on
  every startup;
- legacy telemetry/display parsers are listed under E-08 and E-14.

The SQLite v9 adoption path and iconless memory-page read behavior are documented
current compatibility and were excluded.

## Secondary reduction backlog

These findings are smaller or should be performed only beside related work:

| Area | Evidence | Direction |
| --- | --- | --- |
| Exa fetch | Exa ignores `WebFetchContext` summarizer, but runtime resolves it unconditionally | Resolve backend first; require summarizer only for Direct HTTP |
| Home initialization | production never uses `force: true` | Remove options type and preserve existing config unconditionally |
| Home diagnostics | production uses only best-effort `try_append` | Make fallible writer/path test surface private |
| Artifact operation IDs | production trait has only secure implementation; second is a test fake | Directly call secure generator; retain security tests without a production trait |
| Task inserts | capture, delegation, and recurrence repeat the canonical insert columns | Add one private helper only if the patch remains net-negative |
| Work command metadata | store repeats an 18-arm command metadata match | Generate `meta()` in the existing command macro |
| Timestamp parser | `noema-tasks` uses `chrono` once while `jiff` is already present | Use `jiff`, remove direct dependency |
| Transitional web types | two six-line runtime type-alias modules identify themselves as transitional | Import canonical provider types directly |
| Adapter argument source | enum has one value and no semantic reader | Remove field/enum from next strict schema version |
| Capability key DTOs | three key DTOs are only re-exported | Delete unused public surface |
| Runtime wrappers | several convenience methods/route variants have no production caller | Delete wrappers and dead command branch |
| Provider matrix | unused `Anthropic` variant and capability flags are assigned but unread | Remove misleading entries, not real provider behavior |
| Dev supervisor | polls `try_wait` every 50 ms although tests use bounded `child.wait()` | Use bounded wait while retaining TERM/KILL escalation |
| Progress audit | model output includes ignored `confidence` and `reason` | Remove fields from schema/parser |
| Chat blocks | multimodal block wire shape is constructed only by its test | Remove until production builds block input |
| Conversation vocabulary | non-human ownership variants and actor constructor kinds have no production consumer | Narrow code/API without rewriting immutable schema |
| Marker tests | 837-line file repeats 15 large marker literals | Add local builders/table cases; retain privacy/spoof/fallback risks |
| Eval narratives | invalid/incomplete/superseded same-day reports remain beside accepted evidence | Keep accepted provenance; let Git retain superseded narratives |

## Rejected false positives

The following were explicitly inspected and are not reduction targets on current
evidence:

- schema migration source and upgrade/fresh-convergence tests;
- Work fencing, command atomicity, approval revision checks, ambiguous external
  outcomes, and durable interaction state;
- provider registry leasing/retirement and local model process lifecycle;
- distinct provider protocol implementations;
- distinct OAuth families and typed secret wrappers;
- SSRF, DNS pinning, redirects, byte caps, filesystem traversal, symlink,
  no-clobber, cancellation, and rollback protections;
- artifact persistence/operation ports that avoid crate cycles;
- Markdown canonical memory plus disposable FTS;
- generated TypeScript and Swift output as such;
- platform-specific UI, accessibility, navigation, Keychain, lifecycle, PWA, and
  Tauri behavior;
- active `noema-dev`, Rust size reporting, web generation check, and client build
  configurations;
- file splitting where it does not remove a duplicate authority.

## Limitations

- Static reachability can miss reflection, external package consumers, or scripts
  run only outside the repository. Before deleting public tooling or APIs, confirm
  no external CI/release job invokes them.
- No browser visual inspection was authorized or performed; UI deletion findings
  are import/reachability findings, not visual-design judgments.
- The audit environment lacked the Apollo iOS CLI and Xcode, so native regeneration
  and compile validation remain implementation requirements.
- No production usage telemetry was available. Decision gates require product
  evidence, not line counts.
- LOC estimates are directional and overlap where one consolidation deletes code
  counted by another. Every implementation slice must measure its own patch.
