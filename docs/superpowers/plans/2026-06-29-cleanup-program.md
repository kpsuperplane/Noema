# Cleanup Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor Noema's highest-friction frontend, API, runtime, store, provider, and CLI files into focused modules while preserving behavior.

**Architecture:** Execute the approved "Stabilize The Rails First" cleanup program in small, reviewable units. Start with Noema-owned frontend component boundaries, then split GraphQL/web transport, daemon runtime, graph-memory store, provider parsers, and CLI protocol surfaces.

**Tech Stack:** Rust, Tokio, async-graphql, SurrealDB Rust SDK, React, TypeScript, Apollo Client, Vite, Bun, Base UI/shadcn-style primitives.

---

## Source Spec

- Design: `docs/superpowers/specs/2026-06-29-cleanup-program-design.md`

## Global Execution Rules

- Start each task with `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- Keep commits small and scoped to one task.
- Move code before rewriting it.
- Do not change product behavior unless the task explicitly says so.
- Keep bundled/generated web assets out of refactor commits unless validation intentionally regenerates them.
- For frontend tasks, respect the rule that Noema-owned React components live one component per file.
- For backend tasks, keep public crate re-exports stable unless the task explicitly updates consumers.

## Global Validation Commands

Frontend validation from `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

Rust validation from repo root:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Before each commit:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

---

### Task 1: Document Frontend Component Organization

**Files:**
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/context/current.md`
- Reference: `docs/superpowers/specs/2026-06-29-cleanup-program-design.md`

- [ ] **Step 1: Inspect existing frontend docs**

Run:

```bash
sed -n '1,260p' docs/frontend/current-contract.md
sed -n '1,240p' docs/context/current.md
```

Expected: Current frontend contract and durable context mention shadcn/ui and frontend validation, but not the one-component-per-file rule.

- [ ] **Step 2: Add the component organization rule**

Update `docs/frontend/current-contract.md` with a short section that states:

```markdown
## Frontend Code Organization

Noema-owned React product components should live one component per file.
Component folders may contain pure `.ts` helpers, shared type files, and nearby
tests. Design-system or shadcn-style compound primitive wrappers under
`components/ui` may remain grouped when they mirror an upstream primitive API,
but new Noema-owned product components should follow the one-component-per-file
rule.
```

- [ ] **Step 3: Update durable context**

Add one concise settled decision to `docs/context/current.md` near the existing web UI bullets:

```markdown
- Noema-owned React product components should live one component per file. Pure
  helper/model logic belongs in `.ts` files, and `components/ui` primitive
  wrappers may remain grouped when they mirror upstream compound APIs.
```

- [ ] **Step 4: Validate doc-only changes**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 5: Commit**

Run:

```bash
git add docs/frontend/current-contract.md docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "docs: document frontend component boundaries"
```

Expected: one docs-only commit.

---

### Task 2: Split Transcript Model Helpers From Rendering

**Files:**
- Modify: `crates/noema-core/web/src/components/Transcript.tsx`
- Create: `crates/noema-core/web/src/components/transcript/renderModel.ts`
- Create: `crates/noema-core/web/src/components/transcript/markerModel.ts`
- Create: `crates/noema-core/web/src/components/transcript/scrollModel.ts`
- Modify: `crates/noema-core/web/src/components/Transcript.test.ts`

- [ ] **Step 1: Inspect transcript exports and tests**

Run:

```bash
rg -n "export function|function |type |const " crates/noema-core/web/src/components/Transcript.tsx
sed -n '1,260p' crates/noema-core/web/src/components/Transcript.test.ts
```

Expected: `Transcript.tsx` contains rendering components plus render grouping, marker metadata, and scroll/animation helpers.

- [ ] **Step 2: Create `renderModel.ts`**

Move these pure items from `Transcript.tsx` into `components/transcript/renderModel.ts`:

- render entry types that do not depend on JSX,
- `renderableTranscriptEntries`,
- `shouldAnchorTranscriptEntry`,
- `shouldShowTypingIndicator`,
- `groupTranscriptMarkers`,
- `sameTurn`,
- `transcriptGroupSource`,
- transcript render id/fingerprint helpers,
- lane helpers that do not render JSX.

Export only the helpers used by `Transcript.tsx` or tests.

- [ ] **Step 3: Create `markerModel.ts`**

Move these pure items into `components/transcript/markerModel.ts`:

- memory outcome metadata parsing,
- memory marker label logic,
- memory detail item construction,
- tool marker label/tone/pending helpers,
- metadata formatting helpers used by marker components.

Keep the existing `memoryCardsFromStructuredItem` import from `../../memoryCards`.

- [ ] **Step 4: Create `scrollModel.ts`**

Move these pure items into `components/transcript/scrollModel.ts`:

- scroll key helpers,
- arrival scroll key helpers,
- initial seen-message helpers,
- `isScrolledToBottom`.

- [ ] **Step 5: Update `Transcript.tsx` imports**

Update `Transcript.tsx` so it imports pure helpers from:

```ts
import {
  renderableTranscriptEntries,
  renderedEntryMessageId,
  renderedTranscriptLane,
  shouldAnchorRenderedEntry,
  shouldAnimateRenderedEntryArrivalForSeen,
  shouldAnimateRenderedEntryTextForSeen,
  shouldCompactMarkerClusterSpacing,
  shouldRevealRenderedEntryAfterArrival,
  transcriptArrivalScrollKey,
  transcriptScrollKey
} from "./transcript/renderModel";
```

Adjust names to match the final exported helper set. Do not change runtime behavior.

- [ ] **Step 6: Update transcript tests**

Update imports in `Transcript.test.ts` so tests import pure helpers from the new `.ts` files where appropriate. Keep React-rendering tests pointed at `Transcript.tsx`.

- [ ] **Step 7: Run focused frontend tests**

Run from `crates/noema-core/web`:

```bash
bun test src/components/Transcript.test.ts
```

Expected: transcript tests pass.

- [ ] **Step 8: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run lint
bun run build
```

Expected: lint and build pass.

- [ ] **Step 9: Commit**

Run:

```bash
git add crates/noema-core/web/src/components/Transcript.tsx crates/noema-core/web/src/components/Transcript.test.ts crates/noema-core/web/src/components/transcript
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(web): split transcript model helpers"
```

Expected: commit only transcript helper extraction files.

---

### Task 3: Split Transcript Components Into One File Each

**Files:**
- Modify: `crates/noema-core/web/src/components/Transcript.tsx`
- Create: `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- Create: `crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx`
- Create: `crates/noema-core/web/src/components/transcript/RenderedTranscriptEntryFrame.tsx`
- Create: `crates/noema-core/web/src/components/transcript/TranscriptRow.tsx`
- Create: `crates/noema-core/web/src/components/transcript/Message.tsx`
- Create: `crates/noema-core/web/src/components/transcript/TypingMessage.tsx`
- Create: `crates/noema-core/web/src/components/transcript/ActivityRow.tsx`
- Create: `crates/noema-core/web/src/components/transcript/StructuredCard.tsx`
- Create: `crates/noema-core/web/src/components/transcript/MemoryMarker.tsx`
- Create: `crates/noema-core/web/src/components/transcript/MemoryDetailAttachment.tsx`
- Create: `crates/noema-core/web/src/components/transcript/MemoryDetailList.tsx`
- Create: `crates/noema-core/web/src/components/transcript/MemoryDetailRow.tsx`
- Create: `crates/noema-core/web/src/components/transcript/ToolMarker.tsx`
- Create: `crates/noema-core/web/src/components/transcript/ToolDetailAttachment.tsx`
- Create: `crates/noema-core/web/src/components/transcript/ToolDetailRow.tsx`
- Create: `crates/noema-core/web/src/components/transcript/ErrorNotice.tsx`
- Create: `crates/noema-core/web/src/components/transcript/index.ts`

- [ ] **Step 1: Convert old `Transcript.tsx` to a compatibility export**

After moving the `Transcript` implementation to `components/transcript/Transcript.tsx`, leave the old file as:

```ts
export { Transcript } from "./transcript/Transcript";
export type { MemoryDetailItem } from "./transcript/markerModel";
export { memoryDetailItems, shouldAnchorTranscriptEntry, shouldAnimateMessageText } from "./transcript/renderModel";
```

Adjust the exact export list to match existing tests and imports.

- [ ] **Step 2: Move one component per file**

Move each JSX component from the old file into its matching new file. Keep props interfaces in the same file as the component unless shared by multiple components.

- [ ] **Step 3: Add `components/transcript/index.ts`**

Export only the public surface:

```ts
export { Transcript } from "./Transcript";
export type { MemoryDetailItem } from "./markerModel";
```

- [ ] **Step 4: Update imports**

Run:

```bash
rg -n "components/Transcript|./Transcript" crates/noema-core/web/src
```

Update imports only where needed. Existing imports from `./components/Transcript` may continue through the compatibility export.

- [ ] **Step 5: Run focused tests**

Run from `crates/noema-core/web`:

```bash
bun test src/components/Transcript.test.ts
```

Expected: transcript tests pass.

- [ ] **Step 6: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run lint
bun run build
```

Expected: lint and build pass without behavior changes.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/noema-core/web/src/components/Transcript.tsx crates/noema-core/web/src/components/transcript crates/noema-core/web/src/components/Transcript.test.ts
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(web): split transcript components"
```

Expected: transcript component split commit.

---

### Task 4: Split Remaining Noema-Owned Frontend Components

**Files:**
- Modify: `crates/noema-core/web/src/components/Onboarding.tsx`
- Create: `crates/noema-core/web/src/components/onboarding/Onboarding.tsx`
- Create: `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx`
- Create: `crates/noema-core/web/src/components/onboarding/ProviderStatus.tsx`
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Create: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Create: `crates/noema-core/web/src/components/shell/ShellAttentionItem.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`
- Create: `crates/noema-core/web/src/components/memory/MemoryEntityNode.tsx`
- Create: `crates/noema-core/web/src/components/memory/MemoryClaimEdge.tsx`
- Create: `crates/noema-core/web/src/components/memory/MemoryGraphFlow.tsx`
- Modify tests near these components.

- [ ] **Step 1: Split onboarding components**

Move `AuthAttempt` and `ProviderStatus` into their own files under `components/onboarding/`. Keep `components/Onboarding.tsx` as a compatibility export:

```ts
export { Onboarding, PROVIDER_AUTH_POLL_INTERVAL_MS } from "./onboarding/Onboarding";
```

- [ ] **Step 2: Split app shell components**

Move `ShellSidebar` and `ShellAttentionItem` into their own files under `components/shell/`. Keep `AppShell` in `AppShell.tsx`.

- [ ] **Step 3: Split memory graph canvas components**

Move `MemoryEntityNode`, `MemoryClaimEdge`, and `MemoryGraphFlow` into separate files under `components/memory/`. Keep `MemoryGraphCanvas` in `MemoryGraphCanvas.tsx`.

- [ ] **Step 4: Run focused tests**

Run from `crates/noema-core/web`:

```bash
bun test src/components/Onboarding.test.ts src/components/shell/AppShell.test.ts src/memoryGraphLayout.test.ts
```

Expected: focused tests pass.

- [ ] **Step 5: Run frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run lint
bun run build
```

Expected: lint and build pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/web/src/components
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(web): split product components"
```

Expected: only Noema-owned product component split files are committed.

---

### Task 5: Split GraphQL Domains

**Files:**
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/types.rs`
- Create: `crates/noema-core/src/graphql/local_status.rs`
- Create: `crates/noema-core/src/graphql/onboarding.rs`
- Create: `crates/noema-core/src/graphql/chat.rs`
- Create: `crates/noema-core/src/graphql/memory.rs`
- Create: `crates/noema-core/src/graphql/errors.rs`

- [ ] **Step 1: Inspect current GraphQL boundaries**

Run:

```bash
rg -n "async fn|pub struct|pub enum|impl From|fn parse_" crates/noema-core/src/graphql/schema.rs crates/noema-core/src/graphql/types.rs
```

Expected: local status, onboarding, memory, chat, and subscription types are co-located.

- [ ] **Step 2: Extract memory GraphQL types and resolvers**

Move memory claim, predicate proposal, and memory graph GraphQL types plus related conversion helpers into `graphql/memory.rs`. Move query resolver helper functions for memory into that file.

- [ ] **Step 3: Extract chat GraphQL types and resolvers**

Move transcript item types, conversation started/turn input/turn accepted types, and chat mutation helpers into `graphql/chat.rs`.

- [ ] **Step 4: Extract onboarding/local status modules**

Move local status types/resolver helpers into `graphql/local_status.rs`. Move onboarding and provider-auth attempt GraphQL types/helpers into `graphql/onboarding.rs`.

- [ ] **Step 5: Keep root schema thin**

Keep `QueryRoot`, `MutationRoot`, and `SubscriptionRoot` in `schema.rs`, but delegate domain work to functions from the new modules.

- [ ] **Step 6: Run Rust formatting and GraphQL tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core graphql --no-fail-fast
```

Expected: formatting passes and GraphQL-focused tests pass.

- [ ] **Step 7: Run frontend type generation**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
```

Expected: generated schema/types are unchanged except for formatting or ordering caused by the same schema.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/noema-core/src/graphql.rs crates/noema-core/src/graphql crates/noema-core/web/src/generated
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(core): split graphql domains"
```

Expected: GraphQL domain split commit. If generated files did not change, they should not appear in the staged diff.

---

### Task 6: Split Daemon Web Transport Modules

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Create: `crates/noema-core/src/daemon/web/http.rs`
- Create: `crates/noema-core/src/daemon/web/assets.rs`
- Create: `crates/noema-core/src/daemon/web/origin.rs`
- Create: `crates/noema-core/src/daemon/web/provider_auth.rs`
- Create: `crates/noema-core/src/daemon/web/graphql_ws.rs`
- Create: `crates/noema-core/src/daemon/web/replay.rs`

- [x] **Step 1: Extract HTTP request/response code**

Move `HttpRequest`, `HttpRequestError`, `content_length`, `normalized_path`, `write_response`, `write_json`, and `write_json_error` into `web/http.rs`.

- [x] **Step 2: Extract assets**

Move `EmbeddedAsset`, `embedded_asset`, `is_spa_entry_path`, and `asset_body` into `web/assets.rs`.

- [x] **Step 3: Extract origin checks**

Move `validate_json_post_request`, `validate_mutation_request`, `origin_matches_host`, `origin_authority`, `local_authorities_match`, `split_authority`, and `is_local_host` into `web/origin.rs`.

- [x] **Step 4: Extract provider auth web orchestration**

Move provider-auth web traits and helpers into `web/provider_auth.rs`, preserving the existing public functions used by GraphQL:

- `start_provider_auth_attempt_view`
- `persist_provider_account_status_from_attempt`
- `reconcile_onboarding_provider_account`
- `is_user_onboarded_for_chat`

- [x] **Step 5: Extract GraphQL WebSocket handling**

Move `upgrade_graphql_websocket`, `handle_graphql_websocket`, WebSocket frame structs, frame read/write helpers, and protocol constants into `web/graphql_ws.rs`.

- [x] **Step 6: Extract replay conversion**

Move `visible_conversation_replay`, `ConversationReplayItem`, `web_conversation_item_from_record`, and replay payload structs into `web/replay.rs`.

- [x] **Step 7: Keep `mod.rs` as route dispatch**

Leave `WebState`, `bind_listener`, `handle_connection`, and `handle_graphql_http` in `mod.rs`, importing helpers from child modules.

- [x] **Step 8: Run focused daemon web tests**

Run:

```bash
cargo test -p noema-core daemon::web --no-fail-fast
```

Expected: daemon web tests pass. If socket permissions fail in sandbox, rerun with the required permissions and report it.

- [x] **Step 9: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/web
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(core): split daemon web modules"
```

Expected: daemon web module split commit.

---

### Task 7: Split Daemon Runtime Boundaries

**Files:**
- Modify: `crates/noema-core/src/daemon.rs`
- Replace file with module tree: `crates/noema-core/src/daemon/runtime.rs`
- Create: `crates/noema-core/src/daemon/runtime/handle.rs`
- Create: `crates/noema-core/src/daemon/runtime/actor.rs`
- Create: `crates/noema-core/src/daemon/runtime/turn.rs`
- Create: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Create: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Create: `crates/noema-core/src/daemon/runtime/memory_writes.rs`

- [x] **Step 1: Convert runtime file into a module root**

Move current `runtime.rs` contents into child modules, leaving `runtime.rs` as:

```rust
mod actor;
mod handle;
mod local_tools;
mod memory_writes;
mod transcript_persistence;
mod turn;

pub(crate) use handle::CodexRuntimeHandle;
```

Add additional `pub(super)` or `pub(crate)` exports only where needed.

- [x] **Step 2: Move handle and command types**

Move `RuntimeModelProvider`, `CodexRuntimeHandle`, `apply_provider_account_home`, and `CodexRuntimeCommand` into `runtime/handle.rs`.

- [x] **Step 3: Move actor state**

Move `CodexRuntimeActor`, `ActiveConversation`, and command dispatch loop into `runtime/actor.rs`.

- [x] **Step 4: Move turn orchestration**

Move `start_conversation`, `start_primary_conversation`, onboarding turn handling, and `turn` orchestration into `runtime/turn.rs` as impl blocks for `CodexRuntimeActor`.

- [x] **Step 5: Move transcript persistence**

Move `send_conversation_item`, `send_transient_turn_item`, stream event helpers, `persist_provider_response_output_item`, `persist_provider_action_output_item`, `persist_provider_action_output`, `persist_and_send_turn_item`, and `persist_turn_item` into `runtime/transcript_persistence.rs`.

- [x] **Step 6: Move local tool handling**

Move `LocalToolResult`, `execute_local_tools`, `agent_identity_after_local_tools`, and local tool result conversion helpers into `runtime/local_tools.rs`.

- [x] **Step 7: Move memory write orchestration**

Move canonicalization, provider memory proposal persistence, consolidation decision persistence, explicit memory claim persistence, memory activity summary helpers, and `PersistedMemoryOutcome` into `runtime/memory_writes.rs`.

- [x] **Step 8: Remove or implement `MemoryExtractionWorker`**

If no caller uses it for real work, remove `MemoryExtractionWorkerHandle`, `MemoryExtractionWorkerCommand`, and `MemoryExtractionWorker`, and remove the actor field. Runtime shutdown should remain graceful without it.

- [x] **Step 9: Run daemon runtime tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core daemon --no-fail-fast
```

Expected: daemon tests pass or report sandbox socket permission distinction.

- [x] **Step 10: Commit**

Run:

```bash
git add crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/runtime
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(core): split daemon runtime"
```

Expected: runtime module split commit.

---

### Task 8: Split Graph Claim Store Modules

**Files:**
- Modify: `crates/noema-core/src/store.rs`
- Replace file with module tree: `crates/noema-core/src/store/claims.rs`
- Create: `crates/noema-core/src/store/claims/model.rs`
- Create: `crates/noema-core/src/store/claims/write.rs`
- Create: `crates/noema-core/src/store/claims/consolidation.rs`
- Create: `crates/noema-core/src/store/claims/inspection.rs`
- Create: `crates/noema-core/src/store/claims/graph.rs`
- Create: `crates/noema-core/src/store/claims/rows.rs`
- Create: `crates/noema-core/src/store/claims/labels.rs`
- Modify tests under `crates/noema-core/src/store/tests`

- [ ] **Step 1: Convert `claims.rs` into module root** *(in progress)*

Create `store/claims/` and leave `store/claims.rs` as:

```rust
mod consolidation;
mod graph;
mod inspection;
mod labels;
mod model;
mod rows;
mod write;

pub use model::{
    ClaimStatus, ClaimSummary, ClaimWriteOutcome, ConsolidationMatch,
    ConsolidationMatchRequest, EvidenceAuthority, EvidenceCandidate,
    MemoryClaimDetail, MemoryClaimEvidence, MemoryClaimFilter,
    MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewClaimCandidate,
    RelatedClaimCandidate, RelatedClaimRecord, SupersedeClaimCandidate,
};
```

Adjust exported names to the final moved types.

- [ ] **Step 2: Move public types into `model.rs`**

Move public claim status, candidate, summary, inspection, consolidation, relation, and graph projection structs/enums into `model.rs`.

- [ ] **Step 3: Move row DTOs into `rows.rs`**

Move SurrealDB `Deserialize` row structs into `rows.rs`. Keep fields crate-visible only where another child module needs them.

- [ ] **Step 4: Move label and parsing helpers**

Move sensitivity parsing, status parsing, label helpers, datetime formatting, and entity record id helpers into `labels.rs` if shared by multiple child modules.

- [ ] **Step 5: Move write path**

Move claim create/reinforce/supersede/relation write methods and write helpers into `write.rs`.

- [ ] **Step 6: Move consolidation matching**

Move `find_consolidation_matches` and relevance/match helper functions into `consolidation.rs`.

- [ ] **Step 7: Move inspection read models**

Move `list_claims`, `get_claim_detail`, and inspection lookup helpers into `inspection.rs`.

- [ ] **Step 8: Move graph projection**

Move `memory_graph` and graph node/edge construction helpers into `graph.rs`.

- [ ] **Step 9: Run graph store tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core store::tests::claims --no-fail-fast
```

Expected: graph claim tests pass.

- [ ] **Step 10: Commit**

Run:

```bash
git add crates/noema-core/src/store.rs crates/noema-core/src/store/claims.rs crates/noema-core/src/store/claims crates/noema-core/src/store/tests
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(core): split graph claim store"
```

Expected: graph claim store split commit.

---

### Task 9: Extract Provider Streaming Parsers

**Files:**
- Modify: `crates/noema-core/src/providers/mod.rs`
- Modify: `crates/noema-core/src/providers/responses.rs`
- Modify: `crates/noema-core/src/providers/codex_responses.rs`
- Create: `crates/noema-core/src/providers/sse.rs`
- Create: `crates/noema-core/src/providers/noema_response_stream.rs`

- [ ] **Step 1: Extract SSE accumulator**

Move `SseAccumulator`, `SseEvent`, `parse_sse_event`, `parse_sse_event_bytes`, `sse_events`, `next_sse_event_boundary`, `collect_terminal_response_metadata`, and stream error formatting into `providers/sse.rs`.

- [ ] **Step 2: Keep Responses transport API stable**

Update `ResponsesTransport::send_streaming` to use `providers::sse::SseAccumulator` with the same callback behavior.

- [ ] **Step 3: Extract Noema response JSON stream parser**

Move `NoemaAssistantTextDeltaExtractor`, `JsonContext`, `JsonObjectContext`, `OutputItemState`, `JsonStringReader`, and related enums into `providers/noema_response_stream.rs`.

- [ ] **Step 4: Keep Codex provider API stable**

Update `CodexResponsesProvider` to instantiate the extracted stream parser without changing `ModelProvider` behavior.

- [ ] **Step 5: Run provider tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core providers --no-fail-fast
```

Expected: provider tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/providers
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(core): extract provider stream parsers"
```

Expected: provider parser extraction commit.

---

### Task 10: Split CLI Command And GraphQL Client Boundaries

**Files:**
- Modify: `crates/noema-cli/src/main.rs`
- Create: `crates/noema-cli/src/commands.rs`
- Create: `crates/noema-cli/src/commands/chat.rs`
- Create: `crates/noema-cli/src/commands/config.rs`
- Create: `crates/noema-cli/src/commands/start.rs`
- Create: `crates/noema-cli/src/commands/memory.rs`
- Modify or replace: `crates/noema-cli/src/graphql_client.rs`
- Create: `crates/noema-cli/src/graphql/mod.rs`
- Create: `crates/noema-cli/src/graphql/http.rs`
- Create: `crates/noema-cli/src/graphql/ws.rs`
- Create: `crates/noema-cli/src/graphql/transcript.rs`
- Modify: `crates/noema-cli/src/inspection.rs`
- Modify tests under `crates/noema-cli/src`

- [ ] **Step 1: Move command implementations**

Move `run_start`, `run_config`, `run_chat`, `run_memory_graphql`, `run_interactive_chat`, `print_chat_turn_graphql`, and one-shot provider code into focused command modules.

- [ ] **Step 2: Keep `main.rs` thin**

`main.rs` should retain argument definitions, `CliError`, `main`, `run`, and top-level dispatch only.

- [ ] **Step 3: Split GraphQL HTTP transport**

Move request/response structs and `execute` into `graphql/http.rs`.

- [ ] **Step 4: Split GraphQL WebSocket transport**

Move WebSocket URL construction, connection setup, subscription readiness, JSON write/read helpers, and subscription stream setup into `graphql/ws.rs`.

- [ ] **Step 5: Split transcript event decoding**

Move `graphql_turn_event`, event matching, transcript item decoding, and activity status parsing into `graphql/transcript.rs`.

- [ ] **Step 6: Preserve CLI public behavior**

Run existing CLI argument tests and inspection tests without changing expected output unless tests explicitly require path updates.

- [ ] **Step 7: Run CLI tests**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-cli --no-fail-fast
```

Expected: CLI tests pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/noema-cli/src
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor(cli): split command and graphql boundaries"
```

Expected: CLI boundary split commit.

---

### Task 11: Final Whole-Program Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`
- Optional modify: `docs/frontend/current-contract.md`

- [ ] **Step 1: Run full frontend validation**

Run from `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

Expected: all commands pass.

- [ ] **Step 2: Run full Rust validation**

Run from repo root:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands pass, except socket-permission failures should be rerun with appropriate permissions and reported.

- [ ] **Step 3: Record durable context**

Update `docs/context/current.md` with concise bullets summarizing the completed cleanup boundaries:

```markdown
- Frontend Noema-owned product components now follow the one-component-per-file
  rule, with transcript render/model helpers split from React components.
- GraphQL, daemon web transport, daemon runtime, graph-claim store, provider
  streaming parsers, and CLI command/GraphQL client code are split into focused
  modules while preserving existing behavior.
```

Only include bullets for phases actually completed.

- [ ] **Step 4: Commit context update**

Run:

```bash
git add docs/context/current.md docs/frontend/current-contract.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "docs: record cleanup program completion"
```

Expected: docs-only context commit.

---

## Execution Recommendation

Use subagent-driven development for implementation:

- one subagent per task,
- no two subagents editing the same module tree at the same time,
- main agent reviews each returned diff,
- main agent runs validation and owns commits.

Recommended first execution batch:

1. Task 1 inline or one small subagent.
2. Task 2 as a dedicated frontend transcript subagent.
3. Task 3 only after Task 2 is reviewed and committed.

Do not run backend split tasks in parallel with frontend transcript work unless they touch disjoint files and validation time is acceptable.
