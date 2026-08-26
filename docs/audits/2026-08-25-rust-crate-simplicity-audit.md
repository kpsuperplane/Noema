# Rust crate simplicity audit

- Date: 2026-08-25
- Source revision: `7ba526a16b01661300d81a9f8343622200203047`
- Comparison baseline: `43267781ee9e37606f6ac3303373a6a657530ebf`
- Mode: Explore only
- Scope: All 18 workspace Rust crates and their current production consumers

This report is a dated engineering snapshot.
Current subsystem documents and current code remain the behavior authorities.

## Executive assessment

Noema is scope-heavy, but it is not uniformly overengineered.
Five crates contain 79% of production Rust.
Those crates own storage, runtime, providers, GraphQL, and capability adapters.

Most large areas protect real behavior.
Examples include transactions, secret handling, replay, browser isolation, OAuth, and hostile input processing.
File size alone does not justify a split.

The strongest reduction path removes duplicate authorities and inactive contracts.
It does not introduce a new framework.
It also does not merge protocols with different failure rules.

Three findings show current behavior defects.
They should precede ordinary cleanup.

- A notification cleanup transaction can return without committing.
- Single-client revocation can commit only part of its dependent cleanup.
- Foundation Local readiness can use different configuration than generation.

The bounded same-feature findings could remove about 1,600–2,800 production lines.
This range is directional and contains some overlapping changes.
Each implementation slice must measure its own net change.

The largest additional reduction needs product direction.
The dormant adapter scheduler alone represents about 760–900 production lines.
Other decisions concern Task workflows, evaluation verticals, Windows support, and unused GraphQL surface.

## Method

The audit used the following evidence.

- The workspace dependency graph and every Rust package manifest.
- Current production callers across Rust, web, iOS, binaries, features, and support targets.
- The final 2026-08-08 remediation baseline.
- Current source size from the repository Rust size reporter.
- Current subsystem contracts and active project decisions.
- Focused searches for unused types, repeated decisions, and parallel state machines.

The audit applied four rules.

1. A large file is only an inspection signal.
2. A test fake does not justify production polymorphism.
3. Security, data, concurrency, and external protocol boundaries remain intact.
4. Product capability removal needs an explicit decision.

The estimates count handwritten Rust unless stated otherwise.
They do not promise an exact patch size.

## Current footprint

The reporter counts source lines by production and test classification.
The delta compares total Rust with the stated baseline.

| Crate | Production | Tests | Total | Test declarations | Total delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| `noema-api` | 21,658 | 3,962 | 25,620 | 98 | +5,952 |
| `noema-artifacts` | 2,173 | 695 | 2,868 | 6 | 0 |
| `noema-capabilities` | 4,069 | 1,504 | 5,573 | 42 | +591 |
| `noema-capabilities-mcp` | 9,691 | 3,939 | 13,630 | 52 | +446 |
| `noema-capability-adapters` | 16,499 | 4,774 | 21,273 | 87 | +2,346 |
| `noema-conversations` | 516 | 116 | 632 | 3 | +8 |
| `noema-desktop` | 2,029 | 271 | 2,300 | 10 | +1,451 |
| `noema-dev` | 702 | 245 | 947 | 14 | +284 |
| `noema-home` | 1,096 | 353 | 1,449 | 8 | +662 |
| `noema-host` | 2,355 | 1,039 | 3,394 | 18 | +561 |
| `noema-memory` | 1,351 | 401 | 1,752 | 5 | -21 |
| `noema-model-evals` | 3,714 | 928 | 4,642 | 28 | 0 |
| `noema-providers` | 29,098 | 13,270 | 42,368 | 279 | +5,509 |
| `noema-runtime` | 32,541 | 15,474 | 48,015 | 277 | +6,121 |
| `noema-server` | 4,863 | 2,150 | 7,013 | 45 | +3,815 |
| `noema-store` | 37,052 | 14,578 | 51,630 | 186 | +6,987 |
| `noema-tasks` | 3,546 | 105 | 3,651 | 7 | -718 |
| `noema-workspaces` | 271 | 0 | 271 | 0 | 0 |
| **Total** | **173,224** | **63,804** | **237,028** | **1,165** | **+33,994** |

Recent growth mainly added product behavior.
Examples include native OAuth, browser backends, provider sessions, Task files, web notifications, and adapter management.
The audit does not classify that growth as waste by default.

## Ranked findings

### P0: Repair current behavior defects

| Finding | Evidence | Required direction |
| --- | --- | --- |
| Empty APNs claim rolls back orphan cleanup | [notifications.rs](../../crates/noema-store/src/notifications.rs#L303) | Commit cleanup before the empty return. |
| Single-client revocation is not atomic | [clients.rs](../../crates/noema-store/src/clients.rs#L68) | Use one transaction for revocation and dependent cleanup. |
| Foundation Local readiness uses a second configuration | [composition.rs](../../crates/noema-host/src/composition.rs#L197) | Probe the exact configured provider instance. |

### P1: Restore one authority

| Concern | Current owners | Preferred authority |
| --- | --- | --- |
| Task reconciliation | Runtime and store | One store transaction operation |
| Action output persistence | Runtime and capability binding | Captured capability binding |
| Provider account identity | Host, store, and providers | `ProviderKind` plus store metadata |
| MCP setup state | MCP control, API, and chat setup | `McpSetupStatus` |
| Notification retry outcome | API strings and three store modules | One typed store decision |
| Task role file context | Initial execution and continuation | One role-to-file loader |
| Provider session transitions | OpenAI, Codex, and shared Responses transport | Shared Responses transport |

### Decision gates

These items can remove substantial code.
Each item changes a stated or implied product contract.

| Decision | Current evidence | Potential reduction |
| --- | --- | ---: |
| Keep or retire the adapter scheduler | No production install, claim, or commit path exists | 760–900 production lines |
| Keep stored Task workflow definitions | One fixed workflow supplies all stages | Broad schema and code reduction |
| Keep all current GraphQL reads | Authored web and iOS operations do not use several fields | 100–170 API lines |
| Keep both evaluation workflows | Local and hosted commands support different decisions | Product-sized reduction |
| Support Windows or declare Unix-only | Home setup mixes Unix-only code with Windows ACL code | About 100 support lines |
| Keep `preferred_backend` in host settings | Host parses it, but managed execution ignores it | 15–30 cross-crate lines |

## Cross-crate simplification map

### Task state changes

Runtime loads and plans a Task before the store repeats both steps.
The store must retain the current-run check and transaction.
Runtime should pass only the requested transition.

Evidence: [runtime Task handling](../../crates/noema-runtime/src/daemon/task_runtime.rs#L702) and [store reconciliation](../../crates/noema-store/src/work_reconciliation.rs#L120).

### Capability output persistence

The capability binding already owns its output policy.
Runtime selects the same policy again during action replay.
Runtime should call the captured binding.

Evidence: [binding.rs](../../crates/noema-capabilities/src/binding.rs#L469) and [action_resolution.rs](../../crates/noema-runtime/src/daemon/runtime/action_resolution.rs#L162).

### Provider identity

Five default provider account identities appear in host, store, providers, and onboarding.
`ProviderKind` should map kinds to default account identifiers.
The store should retain account metadata.

Evidence: [host map](../../crates/noema-host/src/composition.rs#L594) and [store accounts](../../crates/noema-store/src/provider_accounts.rs#L15).

### MCP setup state

Setup, discovery, health, authentication, and issue status are derived several times.
One `McpSetupStatus` value can feed API and chat views.

Evidence: [setup model](../../crates/noema-capabilities/mcp/src/setup_model.rs#L154) and [control support](../../crates/noema-capabilities/mcp/src/control/support.rs#L178).

### SQLite write policy

The store has a general busy retry helper and an artifact-specific loop.
Connection setup should own common busy policy.
Artifact conflict handling should remain local.

Evidence: [provider selections](../../crates/noema-store/src/provider_selections.rs#L34) and [artifact metadata port](../../crates/noema-store/src/artifact_metadata_port.rs#L128).

## Crate reports

### `noema-api`

Role: GraphQL schema, resolvers, subscriptions, and client-facing views.

Verdict: The crate is broad because the product API is broad.
The main reduction comes from unused output and repeated adapter reads.

Findings:

- **Decision gate:** Remove unused roots and output fields after the operator check.
  Candidates include `tasksActivity`, `searchMemory`, and the single-artifact read.
  Other candidates include unused transcript, Task artifact, model build, and intervention fields.
  Evidence starts in [query.rs](../../crates/noema-api/src/graphql/schema/query.rs#L361).
  Estimated reduction: 100–170 production lines.
- **P2:** Replace the adapter sibling cache with one composed management read.
  Current reads repeat one snapshot across three sibling fields.
  Evidence: [adapters.rs](../../crates/noema-api/src/graphql/adapters.rs#L29).
  Estimated reduction: 35–70 production lines.
- **P2:** Remove inert local status fields and the one-value workspace role.
  Clients request these fields but do not use them.
  Evidence: [local_status.rs](../../crates/noema-api/src/graphql/local_status.rs#L5) and [foundation.rs](../../crates/noema-api/src/graphql/tasks/projections/foundation.rs#L19).
  Estimated reduction: 35–55 production lines.
- **P2:** Use one closed presence key, counter map, and lease type.
  Browser and native presence use identical counter behavior.
  Evidence: [web_push.rs](../../crates/noema-api/src/graphql/web_push.rs#L102).
  Estimated reduction: 20–35 production lines.
- **P3:** Remove `testRequestPrincipal` if replacement tests remain smaller.
  The field exists only under a test feature.
  Evidence: [query.rs](../../crates/noema-api/src/graphql/schema/query.rs#L101).
  Estimated reduction: 10–18 production lines.

Retain:

- Resource ownership checks.
- Exact governed-action arguments, target, disclosure, and consequence.
- Adapter secret exclusion and request snapshot consistency.
- Web Push and APNs payload safety.
- `pendingGovernedActions`, because the live evaluation script uses it.

Directional reduction: 100–178 production lines before unused schema removal.

### `noema-artifacts`

Role: Artifact metadata ports, filesystem operations, and safe publication.

Verdict: This crate is already compact and cohesive.
Its two ports break real dependency cycles.

Findings:

- **P3:** Update stale crate prose that describes filesystem extraction as future work.
  Evidence: [lib.rs](../../crates/noema-artifacts/src/lib.rs#L3).
  Estimated reduction: 1–3 documentation lines.

Retain:

- Metadata and operation ports.
- Root identity and symbolic-link checks.
- Staging, cleanup, integrity, and concurrent publication rules.
- Collision and cleanup test seams.

Directional reduction: No meaningful Rust reduction.

### `noema-capabilities`

Role: Capability definitions, bindings, policy, and output persistence.

Verdict: The main model is sound.
Several policy names describe only three actual behaviors.

Findings:

- **P1:** Consolidate five output sanitizer types into three behavior policies.
  Artifact behavior matches standard behavior.
  File and browse behavior delegate to web fetch behavior.
  Evidence: [binding.rs](../../crates/noema-capabilities/src/binding.rs#L160).
  Estimated reduction: 45–75 production lines and similar test removal.
- **P1:** Let the captured binding persist replayed action output.
  This removes the runtime policy switch.
  Evidence: [binding.rs](../../crates/noema-capabilities/src/binding.rs#L475).
  Estimated cross-crate reduction: 12–18 production lines.
- **P2:** Reuse nested argument and reason helpers.
  Evidence: [web/mod.rs](../../crates/noema-capabilities/src/web/mod.rs#L10) and [file.rs](../../crates/noema-capabilities/src/file.rs#L187).
  Estimated reduction: 10–18 production lines.
- **P3:** Remove the unused integration-kind enum and unused browse constant.
  Evidence: [integration.rs](../../crates/noema-capabilities/src/integration.rs#L112) and [browse.rs](../../crates/noema-capabilities/src/web/browse.rs#L12).
  Estimated reduction: 11–20 production lines.

Retain:

- Exact credential-field exclusion.
- Preservation tests for ordinary and authorized private values.
- Separate Memory envelope handling.
- Binding ownership of output policy.

Directional reduction: 80–130 production lines, plus 45–75 test lines.

### `noema-capabilities-mcp`

Role: MCP transport, discovery, setup, OAuth, policy, and stored server state.

Verdict: Transport and security boundaries are justified.
Setup state and compatibility names create avoidable surface.

Findings:

- **P1:** Make `McpSetupStatus` the sole setup-state authority.
  Evidence: [setup_model.rs](../../crates/noema-capabilities/mcp/src/setup_model.rs#L154) and [support.rs](../../crates/noema-capabilities/mcp/src/control/support.rs#L178).
  Estimated reduction: 45–80 production lines and 20–40 test lines.
- **P2:** Derive command debug output through the redacted secret type.
  Evidence: [setup_model.rs](../../crates/noema-capabilities/mcp/src/setup_model.rs#L69) and [secret_model.rs](../../crates/noema-capabilities/mcp/src/secret_model.rs#L41).
  Estimated reduction: 35–40 production lines.
- **P3:** Remove unused discovered record fields from the read model.
  Keep stored columns and fingerprint behavior.
  Evidence: [model.rs](../../crates/noema-capabilities/mcp/src/model.rs#L215).
  Estimated reduction: 10–30 production lines.

Retain:

- HTTP and standard-input transports.
- Repository and secret boundaries.
- OAuth backend separation.
- URL, DNS, operation-token, and schema-keyword checks.

Directional reduction: 90–150 production lines.

### `noema-capability-adapters`

Role: Adapter definitions, connections, OAuth, transition recovery, and OpenAPI execution.

Verdict: The active adapter path has strong safety boundaries.
The scheduler is a large inactive product path.

Findings:

- **Decision gate:** Retire or activate the adapter scheduler.
  No production path installs, claims, or commits scheduled work.
  Evidence: [schedule.rs](../../crates/noema-capabilities/adapters/src/schedule.rs#L1) and [service.rs](../../crates/noema-capabilities/adapters/src/service.rs#L612).
  Potential reduction: 760–900 production lines and 190–240 test lines.
- **P2:** Share three connection descriptor constructions.
  Evidence: [service.rs](../../crates/noema-capabilities/adapters/src/service.rs#L1374).
  Estimated reduction: 35–60 production lines.
- **P2:** Remove duplicate definition sets in management snapshots.
  Evidence: [service.rs](../../crates/noema-capabilities/adapters/src/service.rs#L111).
  Estimated reduction: 8–15 production lines.
- **P2:** Use one private descriptor replacement helper if the result is net-negative.
  Evidence: [service.rs](../../crates/noema-capabilities/adapters/src/service.rs#L1151).
  Estimated reduction: 20–40 production lines.
- **P3:** Remove the pointer-identity snapshot test.
  The test checks an implementation detail.
  Evidence: [runtime_tests.rs](../../crates/noema-capabilities/adapters/src/runtime_tests.rs#L75).
- **Decision gate:** Remove `imported_at` if no persisted definition needs it.
  Current install paths always write `None`.
  Evidence: [definition_store.rs](../../crates/noema-capabilities/adapters/src/definition_store.rs#L31).

Retain:

- Content-addressed definitions and connection records.
- OAuth store and transition journal.
- Separate adapter and MCP OAuth behavior.
- HTTP safety, OpenAPI lowering, proposal input, and source digests.

Directional reduction: 70–135 production lines without the scheduler decision.

### `noema-conversations`

Role: Conversation identifiers, status, ownership, and context summary records.

Verdict: This crate is small.
One future-facing owner model has only one current value.

Findings:

- **P2:** Replace the one-value owner enum with a validated human owner identifier.
  The store already writes only human ownership.
  Evidence: [references.rs](../../crates/noema-conversations/src/references.rs#L28) and [conversations.rs](../../crates/noema-store/src/conversations.rs#L201).
  Estimated reduction: 35–60 production lines and 10–20 test lines.

Retain:

- Conversation status validation.
- Actor reference validation.
- Separate initial and compacted context summary records.

Directional reduction: 35–60 production lines.

### `noema-desktop`

Role: Desktop startup, protected credentials, GraphQL transport, and application lifecycle.

Verdict: The Rust boundary is valid.
Two small HTTP parsers and inert recovery data can be reduced.

Findings:

- **P1:** Use one private loopback HTTP parser for both desktop OAuth flows.
  Preserve each flow's callback checks.
  Evidence: [remote_oauth.rs](../../crates/noema-desktop/src/remote_oauth.rs#L127) and [mcp_oauth_callback.rs](../../crates/noema-desktop/src/mcp_oauth_callback.rs#L178).
  Estimated reduction: 50–90 production lines.
- **P2:** Remove recovery metadata until a production source supplies it.
  Every current producer supplies `None`.
  Evidence: [desktop_profile.rs](../../crates/noema-desktop/src/desktop_profile.rs#L16) and [desktop_state.rs](../../crates/noema-desktop/src/desktop_state.rs#L333).
  Estimated reduction: 15–30 production lines.
- **P3:** Use the status command for the remote retry action.
  Evidence: [main.rs](../../crates/noema-desktop/src/main.rs#L121).
  Estimated reduction: 8–15 Rust lines, plus a small client change.

Retain:

- Rust-owned credentials and keyring access.
- GraphQL transport and subscription generations.
- Ordered shutdown and status delivery.
- Exact callback state and bounded request checks.

Directional reduction: 73–135 production lines.

### `noema-dev`

Role: Development process supervision and change watching.

Verdict: The crate is direct and small.
Three explicit watchers are simpler than a general process framework.

Findings:

- No current simplification justifies a code change.

Retain:

- Build-cache safety.
- Process-group cleanup.
- Active-runner limits and change budgets.
- Explicit watcher ownership.

Directional reduction: None.

### `noema-home`

Role: Home paths, private files, initialization, and diagnostics.

Verdict: Path and file safety are justified.
Platform support is currently inconsistent.

Findings:

- **P1:** Resolve the Windows support contract.
  Diagnostics import Unix permission extensions without platform guards.
  Private files contain a separate Windows ACL path.
  Evidence: [diagnostics.rs](../../crates/noema-home/src/diagnostics.rs#L3) and [private_files.rs](../../crates/noema-home/src/private_files.rs#L126).
- **P2:** Share the four lower-hex identifier predicates.
  Keep distinct error messages.
  Evidence: [paths.rs](../../crates/noema-home/src/paths.rs#L467).
  Estimated reduction: 10–20 production lines.
- **P3:** Remove the unused legacy OAuth quarantine path method.
  Evidence: [paths.rs](../../crates/noema-home/src/paths.rs#L244).

Retain:

- `NoemaPaths` as the path authority.
- Atomic private writes.
- Permission and symbolic-link checks.
- Bounded diagnostics.

Directional reduction: 15–30 production lines without the platform decision.

### `noema-host`

Role: Configuration, dependency composition, startup, readiness, and ordered shutdown.

Verdict: The composition root is valid.
Public test APIs and provider setup contain removable layers.

Findings:

- **P0:** Probe the configured Foundation Local instance.
  The second construction discards the resolved bridge path.
  Evidence: [composition.rs](../../crates/noema-host/src/composition.rs#L197).
- **P1:** Make provider account identity one typed authority.
  Evidence: [composition.rs](../../crates/noema-host/src/composition.rs#L594).
  Estimated host reduction: 15–30 production lines.
- **P1:** Remove the one-consumer `ProviderBootstrap` wrapper.
  `ProviderConfig` already supplies the kind and model.
  Evidence: [hosted.rs](../../crates/noema-providers/src/adapters/hosted.rs#L16) and [composition.rs](../../crates/noema-host/src/composition.rs#L400).
  Estimated cross-crate reduction: 60–100 lines.
- **P2:** Remove test-only public startup and configuration APIs.
  Supported binaries use one startup function.
  Evidence: [runtime_host.rs](../../crates/noema-host/src/runtime_host.rs#L89).
  Estimated reduction: 30–55 production lines and 25–50 test lines.
- **P2:** Build fallback hosted providers with one concrete loop.
  Do not add a factory trait.
  Evidence: [composition.rs](../../crates/noema-host/src/composition.rs#L420).
  Estimated reduction: 15–25 production lines.
- **P2:** Replace two worker wrappers with one ordered host entrypoint.
  Evidence: [lib.rs](../../crates/noema-host/src/lib.rs#L32).
  Estimated cross-crate reduction: 12–20 lines.
- **P3:** Delete two dead startup error variants and small pass-through wrappers.
  Evidence: [runtime_host.rs](../../crates/noema-host/src/runtime_host.rs#L166) and [composition.rs](../../crates/noema-host/src/composition.rs#L387).
  Estimated reduction: 20–33 production lines.
- **Decision gate:** Make onboarding provider fields non-null.
  Every producer supplies every field.
  This change needs web and iOS schema generation.
  Evidence: [onboarding.rs](../../crates/noema-host/src/onboarding.rs#L23).

Retain:

- Startup rollback and ordered shutdown.
- Late MCP classification binding.
- Recovery-code rotation and private replacement.
- Configuration secret-source rules.
- Adapter recovery order and provider readiness proof.
- Credential-revision fencing and artifact append race tests.

Directional host-only reduction: 80–135 production lines.

### `noema-memory`

Role: Markdown Memory authority, publication, recovery, search index, and citations.

Verdict: The persistence design protects real recovery and race behavior.
Only small local cleanup is clear.

Findings:

- **P2:** Apply the typed pending payload directly after its durable write.
  Current code parses the bytes it just serialized.
  Recovery should still parse the durable file.
  Evidence: [native.rs](../../crates/noema-memory/src/native.rs#L517).
  Estimated reduction: 4–8 production lines.
- **P3:** Narrow `root`, `index_path`, and `state_body` visibility.
  Current consumers do not need the broader API.
- **Decision gate:** End the retired `memory_pages` cleanup after a defined support window.
  The disposable index still runs this compatibility cleanup on every rebuild.
  Evidence: [native.rs](../../crates/noema-memory/src/native.rs#L644).

Retain:

- Markdown as the source authority.
- Compare-and-publish behavior.
- Crash recovery and derived full-text search.
- Hierarchy and citation validation.

Directional reduction: 4–8 production lines before the support-window decision.

### `noema-model-evals`

Role: Local GGUF qualification and hosted provider evaluation.

Verdict: The two command workflows are distinct.
The hosted helper models providers that production never constructs.

Findings:

- **P2:** Make the hosted helper OpenRouter-specific.
  Production creates only OpenRouter in hosted evaluation.
  Other provider variants only support tests.
  Evidence: [hosted_provider.rs](../../crates/noema-model-evals/src/hosted_provider.rs#L21) and [matrix_runner.rs](../../crates/noema-model-evals/src/matrix_runner.rs#L190).
  Estimated reduction: 90–150 production lines and 20–50 test lines.
- **Decision gate:** Retire either evaluation workflow only with product direction.
  Local qualification and hosted comparison support different decisions.

Retain:

- Separate local and hosted report forms.
- Direct provider configuration for evaluation targets.
- Deterministic fixtures and report output.

Directional reduction: 90–150 production lines without workflow removal.

### `noema-providers`

Role: Provider protocols, sessions, local models, browser backends, and registry operations.

Verdict: The protocol boundaries are necessary.
The largest same-feature reduction exists inside repeated codecs and test-only layers.

Findings:

- **P1:** Remove test-only local-model process polymorphism.
  Production has one process factory, one managed process, and one reaper clock.
  Evidence: [process.rs](../../crates/noema-providers/src/local_models/manager/process.rs#L24).
  Estimated reduction: 80–140 production lines.
- **P1:** Share the Chat Completions stream codec with local GGUF.
  Both paths parse the same frames, deltas, tool fragments, usage, and completion rules.
  Evidence: [shared SSE](../../crates/noema-providers/src/chat_completions/sse.rs) and [local streaming](../../crates/noema-providers/src/local_models/provider/streaming.rs#L222).
  Estimated reduction: 180–300 production lines.
- **P1:** Move common OpenAI and Codex Responses transitions into the existing transport authority.
  Keep their replay and credential differences.
  Evidence: [openai.rs](../../crates/noema-providers/src/adapters/openai.rs#L244), [responses.rs](../../crates/noema-providers/src/adapters/codex/responses.rs#L330), and [transport.rs](../../crates/noema-providers/src/adapters/responses/transport.rs#L229).
  Estimated reduction: 50–100 production lines.
- **P2:** Remove dormant tool capability fields and repeated schema overrides.
  Evidence: [tools.rs](../../crates/noema-providers/src/tools.rs#L124).
  Estimated reduction: 45–75 production lines.
- **P2:** Remove two unused methods from the erased operations surface.
  Typed sessions already own both behaviors.
  Evidence: [operations/mod.rs](../../crates/noema-providers/src/operations/mod.rs#L60).
  Estimated reduction: 20–30 production lines.
- **P2:** Share browser utility code and privatize the unused facade.
  Keep Kernel and Obscura as distinct backends.
  Evidence: [obscura.rs](../../crates/noema-providers/src/adapters/web/browse/obscura.rs#L686), [kernel.rs](../../crates/noema-providers/src/adapters/web/browse/kernel.rs#L754), and [browse.rs](../../crates/noema-providers/src/web/browse.rs#L184).
  Estimated reduction: 35–60 production lines.
- **P3:** Replace the one-value Codex credential source enum.
  Evidence: [responses.rs](../../crates/noema-providers/src/adapters/codex/responses.rs#L46).
  Estimated reduction: 15–25 production lines.
- **P3:** Remove the unused protocol transport variant.
  Evidence: [error.rs](../../crates/noema-providers/src/generation/error.rs#L18).
  Estimated reduction: 3–5 production lines.
- **P3:** Parse provider names once through `ProviderKind`.
  Evidence: [selection.rs](../../crates/noema-providers/src/selection.rs#L182).
  Estimated reduction: 3–10 production lines.
- **Decision gate:** Remove or honor managed `preferred_backend`.
  Host configuration stores it, but managed conversion ignores it.
  Evidence: [config.rs](../../crates/noema-providers/src/config.rs#L345).

Retain:

- One provider session for each run, turn, or notification.
- Local replay and the no-replay rule after partial output.
- Distinct Responses and Chat Completions protocol stacks.
- Distinct OpenAI, Codex, OpenRouter, Kernel, and Obscura policies.
- Browser process isolation, ownership, limits, and public-URL checks.
- Registry leases, retirement, and the runtime type-erasure boundary.

Directional reduction: 430–735 production lines.

### `noema-runtime`

Role: Conversation execution, provider sessions, Task agents, action review, tools, and compaction.

Verdict: The actor and session boundaries are justified.
Several decisions are repeated across runtime, store, and capability bindings.

Findings:

- **P1:** Let one store operation load, plan, and apply each Task transition.
  Runtime currently performs an earlier broad read and plan.
  Evidence: [task_runtime.rs](../../crates/noema-runtime/src/daemon/task_runtime.rs#L702) and [work_reconciliation.rs](../../crates/noema-store/src/work_reconciliation.rs#L120).
  Estimated cross-crate reduction: 40–75 production lines.
- **P1:** Use one Task role-to-file context loader.
  Initial Reviewer context omits `REVIEW.md`, while continuation includes it.
  Evidence: [execution.rs](../../crates/noema-runtime/src/daemon/task_runtime/execution.rs#L243) and [background_task.rs](../../crates/noema-runtime/src/daemon/runtime/background_task.rs#L168).
  Estimated reduction: 20–45 production lines.
- **P1:** Remove legacy Task contract vocabulary with a forward migration.
  Current event constructors receive `None` for contract identifiers.
  Evidence: [ids.rs](../../crates/noema-tasks/src/ids.rs#L47), [event.rs](../../crates/noema-tasks/src/event.rs#L19), and [tool_marker.rs](../../crates/noema-runtime/src/tool_marker.rs#L301).
  Estimated cross-crate reduction: 50–80 production lines.
- **P1:** Remove state-free citation registry plumbing and unused generation values.
  Use module functions and the actor's event registry.
  Evidence: [citation_markers.rs](../../crates/noema-runtime/src/daemon/runtime/citation_markers.rs#L23).
  Estimated reduction: 70–120 production lines.
- **P1:** Use the captured capability binding for replayed action output.
  Evidence: [action_resolution.rs](../../crates/noema-runtime/src/daemon/runtime/action_resolution.rs#L162).
  Estimated reduction: 12–18 production lines.
- **P2:** Share provider-summary generation and compaction record assembly.
  Evidence: [context_compaction.rs](../../crates/noema-runtime/src/daemon/runtime/context_compaction.rs#L140) and [continuation_context.rs](../../crates/noema-runtime/src/daemon/runtime/continuation_context.rs#L301).
  Estimated reduction: 20–45 production lines.
- **P2:** Store browser owner state in one entry instead of three maps.
  Preserve owner serialization and revision checks.
  Evidence: [actor.rs](../../crates/noema-runtime/src/daemon/runtime/actor.rs#L66).
  Estimated reduction: 20–50 production lines.
- **P3:** Remove unused handle routes, ignored arguments, and pass-through test helpers.
  Candidates include one-shot Memory generation and client-message wrappers.
  Evidence: [handle.rs](../../crates/noema-runtime/src/daemon/runtime/handle.rs#L246) and [task_continuation.rs](../../crates/noema-runtime/src/daemon/runtime/task_continuation.rs#L61).
  Estimated reduction: 55–90 production lines.
- **P3:** Remove one unused A2UI reference asset source.
  Keep the live versioned A2UI assets.
  Potential reduction: about 188 JSON lines.

Retain:

- Actor serialization and event revision checks.
- Provider sessions and replay rules.
- Action review and uncertain outcomes.
- Browser public-URL and ownership controls.
- Protected authentication arguments.
- Leases, cancellation, and current-run checks.
- Task file protection, parser child limits, citations, A2UI, ACP, and runtime diagnostics.

Directional reduction: 270–500 production lines, with cross-crate overlap.

### `noema-server`

Role: HTTP routing, browser security, sessions, native OAuth, and static assets.

Verdict: Most recent growth implements public-server security and native OAuth.
Those boundaries should remain.

Findings:

- **P2:** Share bounded OAuth callback parsing and static result-page framing.
  Keep provider-specific completion and status mapping.
  Evidence: [router.rs](../../crates/noema-server/src/web/router.rs#L447).
  Estimated reduction: 20–35 production lines.
- **P3:** Remove the first duplicate native OAuth request-shape check.
  Keep registrar validation, redirect checks, and S256 PKCE.
  Evidence: [native_oauth.rs](../../crates/noema-server/src/web/native_oauth.rs#L319).
  Estimated reduction: 2–6 production lines.
- **P3:** Move the favicon cache seam behind test compilation.
  Evidence: [favicons.rs](../../crates/noema-server/src/web/favicons.rs#L242).
  Estimated reduction: 4–8 production lines.

Retain:

- Host and Origin enforcement.
- Middleware order, setup isolation, and passkey ceremony binding.
- Bounded durable sessions and revocation broadcasts.
- Protected cookies and recovery comparison.
- OAuth library ownership, rotating refresh credentials, and replay detection.
- Unix-socket authentication, WebSocket limits, and favicon public-URL checks.

Directional reduction: 26–49 production lines.

### `noema-store`

Role: SQLite schema, transactions, persistence, migrations, and durable recovery state.

Verdict: One store crate is the correct transaction boundary.
Do not split it by table or file size.

Findings:

- **P0:** Commit APNs orphan cleanup before returning no due delivery.
  The current early return rolls back the cleanup.
  Evidence: [notifications.rs](../../crates/noema-store/src/notifications.rs#L303).
- **P0:** Make single-client revocation atomic.
  Nine dependent writes can currently commit only in part.
  One Live Activity update is also deleted later in the same flow.
  Evidence: [clients.rs](../../crates/noema-store/src/clients.rs#L68).
- **P1:** Remove `append_item_lock` and use one immediate transaction.
  The store already serializes connection access.
  Evidence: [runtime.rs](../../crates/noema-store/src/runtime.rs#L39) and [items.rs](../../crates/noema-store/src/conversations/items.rs#L301).
  Estimated reduction: 15–35 production lines.
- **P1:** Use one typed notification delivery outcome and retry decision.
  APNs, Web Push, and Live Activity repeat the same delays and terminal rule.
  Evidence: [notifications.rs](../../crates/noema-store/src/notifications.rs#L359), [web_push.rs](../../crates/noema-store/src/web_push.rs#L342), and [live_activity.rs](../../crates/noema-store/src/live_activity.rs#L489).
  Estimated reduction: 20–45 production lines.
- **P1:** Use one Task reconciliation transaction authority with runtime.
  Evidence: [work_reconciliation.rs](../../crates/noema-store/src/work_reconciliation.rs#L120).
- **P2:** Put common SQLite busy policy in connection setup.
  Keep artifact conflict handling local.
  Estimated reduction: 20–50 production lines.
- **P2:** Remove redundant indexes with a forward migration.
  Three explicit indexes repeat unique-key prefixes.
  Evidence: [schema.rs](../../crates/noema-store/src/schema.rs#L193).
- **P2:** Share MCP server fields and row decoding.
  Four queries repeat the same 21-field selection.
  Evidence: [repository_rows.rs](../../crates/noema-store/src/mcp/repository_rows.rs#L197).
  Estimated reduction: 45–75 production lines.
- **P2:** Remove dead read wrappers and test-only public write helpers.
  Evidence: [run_items.rs](../../crates/noema-store/src/run_items.rs#L125), [work_reconciliation.rs](../../crates/noema-store/src/work_reconciliation.rs#L43), and [items.rs](../../crates/noema-store/src/conversations/items.rs#L234).
  Estimated reduction: 70–100 production lines.
- **P2:** Remove the legacy native-client token hash with a forward migration.
  OAuth clients write a zero digest, and authentication never reads this column.
  Evidence: [schema.rs](../../crates/noema-store/src/schema.rs#L2565) and [native_oauth.rs](../../crates/noema-store/src/native_oauth.rs#L111).
  Estimated reduction: 5–20 production lines after migration.
- **P3:** Share provider preference row decoding.
  Keep each owner result type.
  Evidence: [agent_runtime_preferences.rs](../../crates/noema-store/src/agent_runtime_preferences.rs#L54) and [auxiliary_model_preferences.rs](../../crates/noema-store/src/auxiliary_model_preferences.rs#L123).
  Estimated reduction: 15–35 production lines.
- **P3:** Replace dynamic artifact timestamps with static SQL.
  Evidence: [sqlite.rs](../../crates/noema-store/src/sqlite.rs#L82).
  Estimated reduction: 15–25 production lines.
- **Decision gate:** Remove the never-written `revoked` approval state during the next schema-floor rebuild.
  Do not add a migration only for this value.

Retain:

- One store crate and one connection authority.
- Schema handshake, recovery, and every applied forward migration.
- Task transactions and current-run checks.
- Task file path, size, symbolic-link, and atomic-write rules.
- Action requests, OAuth rotation, replay checks, and artifact ports.

Directional reduction: 220–380 production lines.

### `noema-tasks`

Role: Task state, workflow stages, schedules, gates, events, and transition planning.

Verdict: The core planner is compact.
Legacy contract terms and generic stored workflows exceed current behavior.

Findings:

- **P1:** Remove legacy Task contract identifiers and event variants.
  Current event constructors receive `None` for contract identifiers.
  This change needs one forward store migration.
  Evidence: [ids.rs](../../crates/noema-tasks/src/ids.rs#L47) and [event.rs](../../crates/noema-tasks/src/event.rs#L19).
  Estimated cross-crate reduction: 50–80 production lines.
- **P2:** Trust private planner output inside the store transaction.
  Failed-run facts are validated before and inside `plan_failed_run_action`.
  Reuse the first successful validation.
  Evidence: [planning.rs](../../crates/noema-tasks/src/planning.rs#L62).
  Estimated reduction: 12–25 production lines.
- **P3:** Parse schedule recurrence once during preview.
  Evidence: [schedule.rs](../../crates/noema-tasks/src/schedule.rs#L145).
- **Decision gate:** Replace stored workflow definitions with code-owned stage metadata.
  One fixed workflow supplies every current Task.
  Keep `WorkflowStageBehavior` as the behavior authority.

Retain:

- Pure transition planning.
- Gate state and retry behavior.
- Schedule semantics and missed-run policy.
- Current Task commands unless moving them produces a measured net reduction.

Directional local reduction: 15–35 production lines before larger decisions.

### `noema-workspaces`

Role: Workspace and project identifiers.

Verdict: This crate is already minimal.
The identifier macro has multiple current domains.

Findings:

- **P3:** Use workspace package metadata and dependency versions in the manifest.
  This is manifest consistency, not a Rust reduction.

Retain:

- The shared semantic identifier macro.
- Workspace and project identifier types.

Directional reduction: No Rust reduction.

## Recommended sequence

### 1. Correct behavior

Repair the three P0 findings first.
Add one focused regression test for each unique risk.
Do not combine these repairs with cleanup.

### 2. Restore existing authorities

Make the store own Task transition application.
Make bindings own output persistence.
Make `ProviderKind` own default account identifiers.
Make `McpSetupStatus` own setup state.

Each slice should be independently net-negative.
Stop if a slice needs a new public framework.

### 3. Remove concentrated duplication

Start with provider stream codecs and Responses transitions.
Then remove store wrappers, MCP aliases, host wrappers, and runtime plumbing.

Measure each crate slice against its starting commit.
Do not combine unrelated protocol changes.

### 4. Cut unused API surface

Confirm local operator use before removing hidden GraphQL reads.
Update authored web and iOS operations in the same slice.
Regenerate clients after the schema changes.

### 5. Resolve product decisions

Decide the adapter scheduler first.
It offers the largest isolated reduction.

Decide stored Task workflows next.
That choice affects schema ownership and Task customization.

Then decide evaluation breadth, Windows support, and managed backend preference.

## Validation plan for implementation

This audit changed documentation only.
Future implementation should use focused validation for each authority.

- Store defects need rollback and commit-result tests.
- Task changes need current-run and role-file tests.
- Provider changes need wire, stream, partial-output, and replay tests.
- Capability changes need secret exclusion and ordinary-value preservation tests.
- OAuth changes need redirect, PKCE, rotation, replay, and bounded-input tests.
- Schema changes need authored client generation and operation validation.

Run the repository Rust size reporter at each milestone.
Use net-negative budgets for every cleanup slice.

## Audit limits

This audit did not execute external provider calls.
It did not exercise browser, mobile, or local-model processes.
It did not change production code.

Repository searches cannot prove every external GraphQL operator is absent.
The API removal candidates therefore retain an operator check.

Persisted-data changes need forward-only migrations.
Every migration needs upgrade and fresh-schema convergence tests.
