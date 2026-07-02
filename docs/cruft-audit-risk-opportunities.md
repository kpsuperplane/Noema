# Cruft Audit Medium And High Risk Opportunities

This document extends `docs/cruft-audit-learnings.md` with a risk-oriented
triage of opportunities that were intentionally excluded from
`docs/superpowers/plans/2026-07-02-cruft-audit-low-risk-fixes.md`.

Scope:

- Medium-risk behavior-preserving refactors.
- High-risk product, security, API, packaging, or provider-behavior decisions.

Out of scope:

- Low-risk deletion slices already covered by
  `docs/superpowers/plans/2026-07-02-cruft-audit-low-risk-fixes.md`.
- Items classified as keep/no action in `docs/cruft-audit-learnings.md`.

## Medium-Risk Refactor Opportunities

These are live-code refactors. They should preserve product behavior and public
schema/API shape unless a separate decision document says otherwise.

### P1

#### Store Claims Query/Write Cleanup

- Scope IDs: `store-claims-001`, `store-claims-002`
- Files: `crates/noema-core/src/store/claims/consolidation.rs`,
  `crates/noema-core/src/store/claims/write.rs`
- Payoff: reduces duplicated SurrealQL branch logic and splits a large live
  write module into clearer claim lifecycle, relation, evidence/entity, and
  fingerprinting areas.
- Why medium-risk: core graph-memory persistence depends on this path, and
  branch behavior differs subtly across exact-object, object-with-terms,
  no-object, sensitivity, ordering, and limit cases.
- Prerequisites: preserve current consolidation branch semantics and related
  store tests before moving code.
- Suggested shape: extract shared claim-match query/read mapping helpers first;
  then split write responsibilities into focused sibling modules without
  changing public store APIs initially.
- Validation: full Rust validation plus focused `store/tests/claims.rs` and
  daemon memory-write tests.

#### Store MCP Module Split

- Scope IDs: `store-general-003`, `store-general-004`
- Files: `crates/noema-core/src/store/mcp.rs`,
  `crates/noema-core/src/store/agents.rs`,
  `crates/noema-core/src/store/ids.rs`
- Payoff: breaks a large live store module into server, tool, calibration,
  trusted identity, approval, row-mapping, and record-fragment concerns.
- Why medium-risk: MCP setup, GraphQL, Capability Gateway, and runtime prompt
  exposure all depend on this store surface; row parsing and persisted string
  values are easy to disturb.
- Prerequisites: keep `store/ids.rs::record_fragment` unchanged because the
  audit found it has different lossy semantics.
- Suggested shape: first add a small prefixed-hex helper for agent/MCP record
  fragments; then split MCP store methods by domain while preserving method
  signatures.
- Validation: full Rust validation plus focused MCP store, GraphQL MCP, setup,
  and gateway tests.

#### GraphQL Schema/MCP Split

- Scope IDs: `GQL-003`, `GQL-004`, `GQL-005`
- Files: `crates/noema-core/src/graphql/schema.rs`,
  `crates/noema-core/src/graphql/mcp.rs`
- Payoff: shrinks the large schema test module and MCP GraphQL module; moves
  MCP setup transport/fakes out of schema root.
- Why medium-risk: GraphQL is the first-party client API; web operations and
  generated types depend on stable field and mutation names.
- Prerequisites: no schema/API field changes in this slice; generated API names
  must stay stable.
- Suggested shape: split tests into subsystem test modules; split MCP into
  `types`, `setup`, `resolvers`, and `parsing`; relocate
  `GraphqlMcpSetupTransport` beside MCP setup code.
- Validation: full Rust validation, GraphQL schema tests, MCP GraphQL tests, and
  web type generation only if operation/schema artifacts change.

### P2

#### MCP Transport/Config Consolidation

- Scope IDs: `MCP-003`, `MCP-004`, `MCP-005`, `MCP-006`
- Files: `crates/noema-core/src/mcp/stdio.rs`,
  `crates/noema-core/src/mcp/http.rs`,
  `crates/noema-core/src/mcp/setup.rs`,
  `crates/noema-core/src/mcp/client.rs`
- Payoff: consolidates RMCP `tools/call` helpers and JSON config parsing;
  separates Streamable HTTP, SSE, OAuth/header helpers, and setup tests.
- Why medium-risk: stdio, Streamable HTTP, and SSE are all live through setup
  and Capability Gateway execution; SSE is explicitly not dead.
- Prerequisites: preserve caller-specific error mapping and SSE JSON-RPC
  behavior.
- Suggested shape: extract shared argument/result helpers first, then shared
  config parsing with adapter-specific errors, then split `http.rs` and move
  setup tests.
- Validation: full Rust validation plus MCP client/setup/gateway transport
  tests.

#### Daemon-Web GraphQL Glue Relocation

- Scope IDs: `DW-001`, `DW-002`, `DW-003`, `DW-005`
- Files: `crates/noema-core/src/daemon/web/provider_auth.rs`,
  `crates/noema-core/src/daemon/web/replay.rs`,
  `crates/noema-core/src/daemon/web/mod.rs`,
  `crates/noema-core/src/graphql/onboarding.rs`,
  `crates/noema-core/src/graphql/chat.rs`
- Payoff: moves live GraphQL-facing provider auth and replay mapping out of
  daemon-web; reduces large route/test module pressure.
- Why medium-risk: code is live, not removable; onboarding, provider auth, and
  conversation replay are user-visible startup/chat paths.
- Prerequisites: do `DW-001` before simplifying `DW-005` test-seam traits.
- Suggested shape: move provider-auth orchestration to GraphQL/provider-auth
  service code, move replay mapping beside GraphQL chat or shared transcript
  mapping, then split tests by module.
- Validation: full Rust validation plus onboarding, provider-auth, GraphQL chat
  replay, and daemon web route tests.

#### Vocabulary Parser Consolidation

- Scope IDs: `memory-005`, `ccd-004`
- Files: `crates/noema-core/src/memory/consolidation.rs`,
  `crates/noema-core/src/memory/extraction.rs`,
  `crates/noema-core/src/daemon/memory_pipeline.rs`,
  `crates/noema-core/src/mcp.rs`,
  `crates/noema-core/src/graphql/mcp.rs`,
  `crates/noema-core/src/store/mcp.rs`,
  `crates/noema-core/src/store/retrieval.rs`,
  `crates/noema-core/src/store/claims/labels.rs`
- Payoff: establishes one authoritative typed parser/renderer per closed
  vocabulary for memory sensitivity/type, MCP transport/trust classification,
  and claim policy labels.
- Why medium-risk: these strings cross persisted SurrealDB rows, GraphQL
  inputs, provider/model parsing, and frontend-generated contracts.
- Prerequisites: inventory persisted/wire strings before changing helper
  ownership.
- Suggested shape: introduce shared enum methods or parser modules, migrate one
  vocabulary family at a time, keep wire strings byte-for-byte identical.
- Validation: full Rust validation plus GraphQL MCP, store MCP, retrieval,
  memory extraction/consolidation tests.

#### Desktop OAuth Callback Shared Handling

- Scope ID: `desktop-cruft-001`
- Files: `crates/noema-desktop/src/mcp_oauth_callback.rs`,
  `crates/noema-core/src/daemon/web/mod.rs`
- Payoff: removes duplicate callback request parsing, `attemptId` extraction,
  callback URL reconstruction, OAuth completion, and HTML response logic.
- Why medium-risk: desktop loopback listener and daemon web callback are both
  live; callback URL construction and error HTML affect OAuth recovery.
- Prerequisites: keep desktop listener ownership; do not include lazy-listener
  startup timing.
- Suggested shape: extract shared callback completion/response logic into a
  core-owned helper; desktop keeps socket/listener lifecycle.
- Validation: full Rust validation plus desktop callback and daemon web MCP
  OAuth callback tests.

### P3

#### Cross-Client GraphQL DTO/Operation Consolidation

- Scope IDs: `cli-003`, `ccd-003`
- Files: `crates/noema-cli/src/graphql/mod.rs`,
  `crates/noema-cli/src/graphql/ws.rs`,
  `crates/noema-cli/src/inspection.rs`,
  `crates/noema-core/web/src/graphql/operations.ts`
- Payoff: reduces schema drift between CLI string queries/manual serde DTOs and
  web generated operations.
- Why medium-risk: both clients are live first-party API consumers;
  consolidation can accidentally change selected fields or output formatting.
- Prerequisites: choose a behavior-preserving sharing/codegen approach without
  changing GraphQL schema.
- Suggested shape: start with CLI memory list/detail DTO/selection dedupe; treat
  broader Rust GraphQL codegen as a separate follow-up.
- Validation: full Rust validation, CLI inspection/chat tests, and web
  typegen/build if shared operations move.

#### Provider Adapter Test/Diagnostic Fixture Consolidation

- Scope IDs: `provider-adapters-006`, `provider-adapters-007`,
  `provider-adapters-008`
- Files: `crates/noema-core/src/provider/adapters/openai.rs`,
  `crates/noema-core/src/provider/adapters/codex_responses.rs`,
  `crates/noema-core/src/provider/adapters/foundation_local.rs`,
  `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`
- Payoff: consolidates duplicated malformed-envelope logging, HTTP test
  servers, request parsers, and fake Foundation bridge fixtures.
- Why medium-risk: individually low risk, but medium when bundled because shared
  test fixtures can weaken coverage across provider adapters.
- Prerequisites: keep diagnostics text/metadata semantics stable where tests
  assert them.
- Suggested shape: extract test-only HTTP/bridge fixtures first; consolidate
  logging only after verifying OpenAI/Codex diagnostic differences.
- Validation: full Rust validation plus provider adapter streaming, malformed
  response, and Foundation Local tests.

#### Gateway Discarded Context Cleanup

- Scope ID: `core-misc-004`
- Files: `crates/noema-core/src/capability/gateway.rs`,
  `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Payoff: removes misleading `agent_id` and `scope_ids` plumbing that active
  gateway execution discards.
- Why medium-risk: the fields overlap future grant/policy evaluation context, so
  removing them can conflict with near-term security wiring.
- Prerequisites: confirm this slice is strictly current-behavior cleanup, not
  capability policy work.
- Suggested shape: isolate from `core-misc-001` through `core-misc-003`; adjust
  constructor/call sites only.
- Validation: full Rust validation plus gateway/local tool tests.

### Medium-Risk Bundling Guidance

Can bundle:

- `store-general-003` with `store-general-004`.
- `GQL-003`, `GQL-004`, and `GQL-005`.
- `MCP-003` through `MCP-006`, but land in sub-steps.
- `memory-005` with `ccd-004` only if preserving all wire/persisted strings.
- `cli-003` with a narrow first pass of `ccd-003`.

Should isolate:

- Store claims cleanup from Store MCP cleanup.
- Daemon-web relocation from GraphQL schema/MCP refactor.
- Desktop OAuth shared handling from desktop listener startup timing.
- Gateway discarded fields from capability policy/quarantine/owner-extraction
  work.

## High-Risk Decision Opportunities

These require an explicit decision artifact before implementation. Most are not
primarily cleanup; they clarify product, privacy, security, provider, or
release semantics.

### P1

#### Provider Selection And Privacy Semantics

- Scope IDs: `DR-003`, `DR-004`
- Files: `crates/noema-core/src/daemon/runtime/memory_writes.rs`,
  `crates/noema-core/src/daemon/runtime/turn.rs`
- Decision required: whether memory canonicalization/consolidation must use the
  selected conversation or agent provider, and whether model override is
  provider-local or an explicit provider override.
- Payoff: prevents local-provider users from silently routing memory maintenance
  through daemon default Codex/OpenAI.
- Blast radius: runtime provider resolution, GraphQL/CLI model override
  semantics, memory-write tests.
- Failure modes: privacy leak, wrong model/provider used for memory truth,
  confusing provider preference behavior.
- Recommended artifact: short provider-selection RFC with a privacy matrix.
- Validation: two-provider regression tests, focused memory-write/turn tests,
  full Rust validation.

#### Capability Gateway Policy, Quarantine, Owner Extraction

- Scope IDs: `core-misc-001`, `core-misc-002`, `core-misc-003`
- Files: `crates/noema-core/src/capability.rs`,
  `crates/noema-core/src/capability/examination.rs`,
  `crates/noema-core/src/capability/ownership.rs`,
  `crates/noema-core/src/capability/gateway.rs`,
  `crates/noema-core/src/store/mcp.rs`,
  `crates/noema-core/src/graphql/mcp.rs`
- Decision required: whether to wire deterministic policy evaluation,
  read-result quarantine examination, and persisted owner extractors into the
  live gateway, or delete the staged policy APIs.
- Payoff: closes the gap between MCP security design and runtime behavior.
- Blast radius: MCP tool execution, approvals, trusted identities,
  audit/quarantine store schema, GraphQL calibration flows.
- Failure modes: untrusted reads released directly, ownership bypass, policy
  matrix falsely assumed to be enforced, broken MCP setup semantics.
- Recommended artifact: security decision record mapping each policy stage to
  gateway checks.
- Validation: gateway unit tests for read/write/export, owner resolution,
  quarantine, approval paths.

#### Foundation Bridge Lifecycle API

- Scope IDs: `provider-adapters-004`, `provider-adapters-005`
- Files:
  `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`,
  `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs`, Swift
  bridge `main.swift`
- Decision required: whether replay, cancel, close-session, and shutdown are
  first-class runtime lifecycle APIs now, or should be cut from both Rust and
  Swift protocol surface until needed.
- Payoff: makes Foundation Local session lifecycle explicit.
- Blast radius: bridge protocol, daemon shutdown, conversation resume,
  cancellation UX.
- Failure modes: orphaned sessions/processes, unsupported cancel misreported,
  replay assumptions diverge between Rust and Swift, protocol compatibility
  churn.
- Recommended artifact: bridge lifecycle spec covering session ownership, replay
  source, cancellation semantics, and shutdown timing.
- Validation: macOS bridge tests plus non-macOS compile checks.

#### Strict Provider Response Parsing

- Scope ID: `provider-core-004`
- Files: `crates/noema-core/src/provider/contract.rs`,
  `crates/noema-core/src/daemon/prompts.rs`,
  `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Decision required: whether required Noema responses must reject implicit
  top-level output arrays and concatenated JSON compatibility paths.
- Payoff: tighter model/provider contract and fewer ambiguous parsed responses.
- Blast radius: provider adapters, response envelope tests, real provider
  traces.
- Failure modes: dropping valid provider output, accepting malformed output,
  silently discarding later concatenated envelopes.
- Recommended artifact: parser-hardening note backed by captured provider
  traces.
- Validation: contract parser tests across strict and malformed examples, plus
  Foundation Local response tests.

### P2

#### Old Public Memory API Removal

- Scope IDs: `memory-001`, `memory-002`, `ccd-001`
- Files: `crates/noema-core/src/memory.rs`,
  `crates/noema-core/src/memory/store.rs`,
  `crates/noema-core/src/memory/store_helpers.rs`,
  `crates/noema-core/src/memory/model.rs`,
  `crates/noema-core/src/memory/types.rs`,
  `crates/noema-core/src/store/retrieval.rs`,
  `crates/noema-core/src/lib.rs`
- Decision required: whether pre-V1 public `noema-core` API compatibility
  matters for `MemoryStore`, old DTOs, and
  `MemoryPersistenceError::MemoryStore`.
- Payoff: removes the largest legacy memory parallel path.
- Blast radius: public exports, memory tests, error types.
- Failure modes: breaking external consumers unexpectedly, deleting policy
  coverage that should move to graph retrieval tests.
- Recommended artifact: API break memo naming removed exports and replacement
  graph APIs.
- Validation: graph retrieval policy tests, memory tool tests, full Rust
  validation.

#### Raw Daemon Protocol Surface

- Scope IDs: `DC-004`, `DC-005`, `DC-006`, `DC-007`
- Files: `crates/noema-core/src/daemon/client.rs`,
  `crates/noema-core/src/daemon/protocol.rs`,
  `crates/noema-core/src/daemon/server.rs`,
  `crates/noema-core/src/paths.rs`,
  `crates/noema-core/src/lib.rs`
- Decision required: whether raw Unix-socket chat clients and serialized daemon
  protocol compatibility are still supported now that product chat uses
  GraphQL.
- Payoff: simplifies daemon protocol around lifecycle-only use.
- Blast radius: public `DaemonClient`, request/response enums, CLI lifecycle,
  tests.
- Failure modes: breaking hidden clients, stale protocol docs, removing
  errors/fields still needed for diagnostics.
- Recommended artifact: protocol support statement: lifecycle-only vs public
  chat API.
- Validation: daemon client/server tests, CLI chat lifecycle tests, full Rust
  validation.

#### GraphQL Memory/Status Stale Fields

- Scope IDs: `GQL-006`, `GQL-008`
- Files: `crates/noema-core/src/graphql/memory.rs`,
  `crates/noema-core/src/graphql/local_status.rs`,
  `crates/noema-core/web/src/graphql/operations.ts`, CLI GraphQL module
- Decision required: whether to remove redundant memory graph fields and replace
  always-`Codex` assistant connection with provider-aware status.
- Payoff: prevents public schema from encoding false provider/redaction
  semantics.
- Blast radius: GraphQL schema, generated frontend types, CLI/web operations.
- Failure modes: client type breakage, stale UI assumptions, hiding
  redaction/provider state that users rely on.
- Recommended artifact: GraphQL schema change proposal with before/after
  operations.
- Validation: schema generation, web typegen/build, GraphQL memory/status
  tests.

#### Embedded Generated Web Assets Packaging

- Scope ID: `ccd-005`
- Files: `crates/noema-core/web/vite.config.ts`,
  `crates/noema-core/src/daemon/web/assets.rs`,
  `crates/noema-cli/src/dev.rs`,
  `crates/noema-core/src/daemon/web/assets/*`
- Decision required: release packaging/build-from-clean strategy before deleting
  committed generated assets.
- Payoff: removes generated bundle churn from Rust source while preserving
  release serving.
- Blast radius: release builds, debug asset loading, CI, dev-daemon watcher
  behavior.
- Failure modes: clean checkout cannot serve web UI, release binary misses
  assets, Rust rebuild loops return.
- Recommended artifact: packaging design for when assets are built, embedded,
  checked, and ignored.
- Validation: clean build, release asset serving, frontend build.

#### Desktop MCP OAuth Listener Startup

- Scope ID: `desktop-cruft-002`
- Files: `crates/noema-desktop/src/desktop_state.rs`,
  `crates/noema-desktop/src/mcp_oauth_callback.rs`,
  `crates/noema-core/web/src/graphql/mcpOAuthCallback.ts`
- Decision required: eager startup vs lazy listener creation during hosted MCP
  OAuth setup.
- Payoff: avoids app startup failure from an unused callback port.
- Blast radius: desktop runtime init, OAuth URL lifecycle, setup error timing.
- Failure modes: OAuth redirect URL unavailable, late bind failure during setup,
  listener lifetime races.
- Recommended artifact: desktop OAuth lifecycle note.
- Validation: desktop runtime init tests and MCP OAuth setup-path tests; no
  browser inspection unless explicitly requested.

### P3

#### Evidence Authority Variants

- Scope ID: `store-claims-004`
- Files: `crates/noema-core/src/store/claims/model.rs`, schema/docs
- Decision required: whether `HumanCorrection`, `DocumentSource`,
  `WeakInference`, and `SystemRule` are planned provenance authorities or schema
  cruft.
- Payoff: clarifies memory provenance roadmap before document import/correction
  flows.
- Blast radius: claim schema, docs, future import/correction APIs.
- Failure modes: removing needed future provenance vocabulary, or keeping
  misleading unsupported authority values.
- Recommended artifact: memory provenance vocabulary decision.
- Validation: claim model/schema tests.

#### CLI `noema context graph` Placeholder

- Scope ID: `cli-002`
- Files: `crates/noema-cli/src/inspection.rs`, CLI command wiring
- Decision required: remove, implement against `memoryGraph`, or intentionally
  keep unavailable placeholder.
- Payoff: removes a reachable dead-end CLI command or turns it into useful graph
  inspection.
- Blast radius: CLI UX, parse tests, docs.
- Failure modes: users hit unavailable command, or incompatible output format
  lands prematurely.
- Recommended artifact: CLI inspection IA/output spec.
- Validation: CLI parser and inspection output tests.

#### Frontend Audit Settings Placeholder

- Scope ID: `frontend-004`
- Files: `crates/noema-core/web/src/components/settings/AuditSettingsPane*.tsx`,
  shell/settings navigation, GraphQL operations
- Decision required: whether Audit remains visible as placeholder, gets backed
  by a read model, or is hidden until implemented.
- Payoff: aligns settings IA with real data surfaces.
- Blast radius: settings nav, route model, future audit GraphQL.
- Failure modes: shipping empty trust surface, or hiding a planned governance
  affordance without replacement.
- Recommended artifact: audit surface product decision tied to audit-event
  persistence.
- Validation: web typecheck/build and route/navigation tests if present.

## Sequencing Guidance

Recommended order:

1. Provider selection/privacy semantics.
2. Capability Gateway policy/quarantine/owner-extraction decision.
3. Old public memory API removal decision.
4. Raw daemon protocol support decision.
5. GraphQL memory/status schema decision.
6. Release asset packaging decision.
7. Foundation bridge lifecycle decision.
8. Medium-risk refactor slices, starting with store claims and Store MCP.

Why this order:

- Provider selection and Capability Gateway decisions affect privacy/security
  assumptions that downstream refactors should not obscure.
- Memory API and raw daemon protocol decisions define which public surfaces
  refactors must preserve.
- GraphQL schema and asset packaging decisions change generated/client-facing
  artifacts and should not be bundled with internal module splits.
- Medium-risk refactors are easier and safer once product/security/API
  boundaries are explicit.

## Exclusions

Explicitly excluded low-risk deletion slices:

- Memory-pipeline wrappers and staged helpers.
- Store schema cruft and alias module.
- Small runtime cleanup.
- GraphQL private cruft.
- Fabricated daemon output cleanup.
- Provider-core unused exports and enum pieces.
- MCP unused schema/test helper.
- Provider-adapter unused wrappers.
- CLI GraphQL facade.
- Frontend unused helpers/barrel.
- Core misc unused helpers.
- Desktop subscription serialization test cleanup.

Explicitly excluded keep/no-action findings:

- `memory-persistence`
- `store-claims-003`
- `DR-005`
- `GQL-002`
- `DC-001`, `DC-002`
- `MP-003`, `MP-004`, `MP-005`, `MP-006`
- `memory-003`, `memory-004`, `memory-006`, `memory-007`, `memory-008`
- `provider-core-006`
- `provider-adapters-009`, `provider-adapters-010`
- `desktop-cruft-004`
