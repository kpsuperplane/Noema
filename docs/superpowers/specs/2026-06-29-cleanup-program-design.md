# Cleanup Program Design

## Purpose

Noema is moving quickly, and several high-churn areas have grown into broad
files that mix product behavior, transport plumbing, rendering, persistence,
and policy-shaped helpers. The cleanup program should create stable ownership
boundaries without changing behavior for its own sake.

This is a phased roadmap, not a single mega-refactor. Each phase should leave
the repository shippable, validated, and easier to work in than before.

## Strategy

Use the **Stabilize The Rails First** approach:

1. Make frontend, API, and web-server boundaries legible.
2. Then split daemon runtime and graph-memory store internals.
3. Finish with provider streaming and CLI boundary cleanup.

This order reduces future merge pressure before the deepest backend refactors.
It also creates clearer consumers before moving runtime and store code.

## Program Rules

- Preserve behavior unless a phase explicitly scopes a behavior change.
- Keep work in small commits, each with one clear ownership boundary.
- Preserve unrelated dirty worktree changes.
- Move tests with the code they cover.
- Keep generated and bundled artifacts out of refactor commits unless project
  validation regenerates them and the commit intentionally includes them.
- Treat 750 source lines as an inspection threshold. If a source file remains
  above that threshold after a phase, document why or split it further.
- Prefer moving code before rewriting code.
- Do not add migrations or compatibility layers for pre-V1 schema changes
  unless explicitly requested.
- Do not use direct text, prefix, or English phrase matching as the authority
  for semantic user intent. Cleanup work may preserve existing behavior
  temporarily, but should not further entrench phrase matching.

## Frontend Organization Rule

React components should live one component per file by default.

For Noema-owned product components, this is an acceptance criterion, not a
style preference. A component folder may contain:

- one file per React component,
- pure helper/model files in `.ts`,
- type files when shared types would otherwise create import cycles,
- tests near the behavior they cover.

Existing design-system or shadcn-style compound primitive wrappers under
`components/ui` may remain temporarily when they mirror an upstream primitive
API, but any new Noema-owned product component should follow the one-component
per-file rule. Splitting `components/ui` wrappers can be scheduled separately
if they become difficult to maintain.

## Phase 1: Frontend Component Boundaries

### Scope

Split Noema-owned frontend components into one component per file, starting
with the highest-churn and largest files:

- `crates/noema-core/web/src/components/Transcript.tsx`
- `crates/noema-core/web/src/components/Onboarding.tsx`
- `crates/noema-core/web/src/components/shell/AppShell.tsx`
- `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`

`Transcript.tsx` should become a folder such as
`components/transcript/`, with separate component files for:

- `Transcript`
- `TranscriptBottomFollower`
- `RenderedTranscriptEntryFrame`
- `TranscriptRow`
- `Message`
- `TypingMessage`
- `ActivityRow`
- `StructuredCard`
- `MemoryMarker`
- `ToolMarker`
- detail attachment and detail row components
- `ErrorNotice`

Pure grouping, animation-decision, marker-model, metadata, and detail-item
helpers should move into `.ts` files in the same folder.

### Non-Goals

- Do not redesign chat visuals.
- Do not change transcript behavior.
- Do not change GraphQL operations or generated types.

### Acceptance

- Noema-owned product component files contain one React component each.
- Shared helper logic is in non-component `.ts` files.
- Existing transcript tests still cover grouping, animation decisions, memory
  markers, and tool markers after the split.
- Frontend validation passes.

## Phase 2: GraphQL And Web Server Rails

### Scope

Split GraphQL by product domain while preserving the existing schema:

- `graphql/local_status.rs`
- `graphql/onboarding.rs`
- `graphql/chat.rs`
- `graphql/memory.rs`
- `graphql/subscriptions.rs`
- shared parsing/error helpers where needed

Split `daemon/web` into modules with clear ownership:

- `http.rs` for request parsing and response writing,
- `assets.rs` for static asset routing,
- `origin.rs` for same-origin validation,
- `provider_auth.rs` for provider-auth web orchestration and status
  reconciliation,
- `graphql_ws.rs` for `graphql-transport-ws` handling and WebSocket frames,
- `replay.rs` for converting stored conversation items to web replay items,
- `mod.rs` for `WebState`, listener binding, and route dispatch.

### Non-Goals

- Do not rename GraphQL fields.
- Do not change route paths.
- Do not change WebSocket protocol behavior.
- Do not change onboarding policy.

### Acceptance

- GraphQL resolver files are domain-oriented.
- `daemon/web/mod.rs` no longer owns raw HTTP, WebSocket frame parsing,
  provider-auth reconciliation, asset serving, and replay conversion directly.
- Existing GraphQL and web-server tests pass.

## Phase 3: Daemon Runtime Boundaries

### Scope

Split `daemon/runtime.rs` around stable behavior seams:

- `runtime/handle.rs` for public runtime handle and command messages,
- `runtime/actor.rs` for actor command dispatch and conversation map state,
- `runtime/turn.rs` for turn lifecycle orchestration,
- `runtime/transcript_persistence.rs` for durable item creation and stream
  fanout,
- `runtime/local_tools.rs` for local tool dispatch/result conversion,
- `runtime/memory_writes.rs` for canonicalization, consolidation, and memory
  persistence orchestration.

Prompt assembly that does not belong inside turn orchestration should move to
the existing `daemon/prompts.rs` module or focused child modules under
`daemon/prompts/`.

Remove or implement the current `MemoryExtractionWorker` stub. It should not
remain as a spawned actor that only waits for shutdown and ignores its provider
and store inputs.

### Non-Goals

- Do not change provider request behavior.
- Do not change memory policy.
- Do not change transcript item schemas.

### Acceptance

- The actor file owns coordination, not all turn internals.
- Turn orchestration can be read without reading memory consolidation details.
- Memory write orchestration can be tested independently from runtime command
  dispatch.
- The worker stub is gone or has real responsibility.

## Phase 4: Graph Memory Store Boundaries

### Scope

Split graph-claim store implementation into focused modules:

- `store/claims/model.rs` for public claim and graph projection types,
- `store/claims/write.rs` for claim create, reinforce, supersede, and relation
  writes,
- `store/claims/consolidation.rs` for bounded consolidation matching,
- `store/claims/inspection.rs` for claim list/detail read models,
- `store/claims/graph.rs` for memory graph projection,
- `store/claims/rows.rs` for SurrealDB row DTOs,
- `store/claims/labels.rs` or shared helpers for enum parsing and labels.

Keep the external `crate::store` re-exports stable while moving internals.

### Non-Goals

- Do not redesign the SurrealDB schema.
- Do not change claim IDs, fingerprints, or evidence relation semantics.
- Do not change redaction policy.

### Acceptance

- Claim write code, inspection read models, and graph projection are separable.
- Tests are organized around the split responsibilities.
- Existing graph-memory unit tests pass.

## Phase 5: Provider And CLI Boundary Cleanup

### Scope

Extract provider streaming helpers:

- Responses SSE accumulation/parsing moves out of `providers/responses.rs`.
- Codex structured JSON delta extraction moves out of
  `providers/codex_responses.rs`.

Split CLI responsibilities:

- command dispatch remains thin in `main.rs`,
- chat command behavior moves to `commands/chat.rs`,
- memory/context inspection commands move under `commands/` or an
  `inspection/` module tree,
- GraphQL HTTP and WebSocket client code split under `graphql/`,
- transcript event decoding has its own focused module.

### Non-Goals

- Do not add providers.
- Do not redesign CLI UX.
- Do not change output format except where tests already define exact output.

### Acceptance

- Provider parser modules have focused tests.
- CLI command dispatch is thin.
- CLI GraphQL transport and transcript decoding can be tested independently.

## Validation

For frontend phases:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

For Rust phases:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Use targeted tests first during each phase, then run the broader validation
before committing the finished unit. If daemon/OpenAI provider tests fail in
the sandbox because of local socket permissions, rerun the same command with
the required socket permissions and report the distinction.

Before committing or pushing any implementation unit:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

## Milestone Boundaries

Each phase should be split into implementation units that can be reviewed and
committed independently. Suggested first units:

1. Document frontend component organization expectations.
2. Split transcript helpers from transcript components.
3. Split transcript components into one file each.
4. Split onboarding, app shell, and memory graph canvas components.
5. Split GraphQL memory/chat domains.
6. Split daemon web replay, WebSocket, and provider-auth modules.
7. Split daemon runtime turn orchestration from memory write orchestration.
8. Split graph-claim store write, inspection, and graph projection modules.
9. Extract provider streaming parsers.
10. Split CLI GraphQL transport and command modules.

## Risks And Mitigations

- **Risk: behavior drift during mechanical moves.** Mitigate by moving code
  first, keeping tests close, and avoiding opportunistic rewrites.
- **Risk: import cycles after frontend splits.** Mitigate by placing shared
  types and helper functions in `.ts` files before moving components.
- **Risk: generated web assets muddy diffs.** Mitigate by excluding generated
  or bundled artifacts unless a phase explicitly requires them.
- **Risk: backend visibility changes create awkward public APIs.** Mitigate by
  preferring `pub(super)` and module-local helpers, with stable re-exports only
  at established crate boundaries.
- **Risk: cleanup hides semantic intent-matching debt.** Mitigate by flagging
  text-prefix intent parsing as temporary and not expanding it during refactor
  phases.

## Out Of Scope

- New product surfaces.
- UI redesign.
- GraphQL schema redesign.
- Store schema redesign.
- Migration compatibility layers.
- Provider feature expansion.
- CLI UX redesign.
