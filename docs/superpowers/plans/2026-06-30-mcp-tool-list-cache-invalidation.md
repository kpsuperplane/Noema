# MCP Tool List Cache Invalidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make already-open chats pick up any changed final model-visible tool list on the next user turn, while avoiding unnecessary recomputation when the tool list has not changed.

**Architecture:** Keep a runtime-scoped cache of the finalized tool list that is actually passed to the provider. Compute a stable hash for that final list, compare it at turn start, and invalidate the cache from any write path that can change the final list. Built-in tools and MCP tools share the same cache boundary because the model only sees one final list.

**Tech Stack:** Rust, SurrealDB-backed Noema store, daemon runtime actor state, GraphQL/MCP settings mutation paths, existing provider request construction.

---

### Task 1: Add runtime cache state for the final tool list

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`

- [ ] **Step 1: Write the failing test**

Add a daemon runtime test in `crates/noema-core/src/daemon/tests.rs` that starts one conversation, captures the current final tool list hash, mutates the tool set through a store-backed path, and asserts the next user turn observes a different cache hash before provider setup proceeds.

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p noema-core daemon::tests::mcp_tool_list_refreshes_on_next_turn -- --nocapture`
Expected: FAIL because the runtime has no tool-list cache state yet.

- [ ] **Step 3: Add the minimal runtime cache structure**

Add a small runtime-only cache type alongside `ActiveConversation`, for example:

```rust
#[derive(Debug, Clone)]
struct CachedToolList {
    hash: String,
    final_tools: Vec<serde_json::Value>,
}
```

Store it on `ActiveConversation` or a sibling runtime map keyed by conversation id, whichever keeps the ownership boundary smallest and simplest for turn setup.

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -p noema-core daemon::tests::mcp_tool_list_refreshes_on_next_turn -- --nocapture`
Expected: PASS after the runtime can retain and refresh the cached list.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: cache final tool list at runtime"
```

### Task 2: Compute and compare a stable hash for the final model-visible tool list

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Test: `crates/noema-core/src/provider/contract.rs` unit tests or nearby daemon tests

- [ ] **Step 1: Write the failing test**

Add a unit test that builds two final tool lists with identical tool content and asserts they produce the same hash, and a third list with one model-visible difference that produces a different hash.

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p noema-core provider::contract::tests::final_tool_list_hash_changes_only_when_model_visible_tool_shape_changes`
Expected: FAIL because no stable hash helper exists yet.

- [ ] **Step 3: Add the hash helper**

Add a canonical hash helper for the final tool list, using a stable serialization of the exact model-visible tool payload. Keep the helper narrowly scoped to provider-facing tool data so privacy-only metadata does not affect the cache key.

```rust
pub fn hash_final_tool_list(final_tools: &[serde_json::Value]) -> String
```

- [ ] **Step 4: Wire the turn path to compare hashes**

In the turn setup path, compare the current hash against the cached hash before building the provider request. Rebuild the cache only when the hash differs.

- [ ] **Step 5: Run the focused test to verify it passes**

Run: `cargo test -p noema-core provider::contract::tests::final_tool_list_hash_changes_only_when_model_visible_tool_shape_changes`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/provider/contract.rs crates/noema-core/src/daemon/runtime/turn.rs
git commit -m "feat: hash final provider tool list"
```

### Task 3: Invalidate the cache from every tool-list-changing write path

**Files:**
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/store/mcp.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs` if built-in tool registration is sourced there
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs` if built-in tool registration is sourced there

- [ ] **Step 1: Write the failing test**

Add focused tests around the MCP mutation paths that currently persist servers, refresh discovery, or save calibration, and assert each path marks the tool-list cache dirty for the affected conversation/runtime.

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p noema-core graphql::schema::tests::mcp_tools_query_and_delete_mutation_use_persisted_setup_state`
Expected: FAIL until the mutation paths can signal cache invalidation.

- [ ] **Step 3: Add cache dirtiness hooks**

Introduce a small invalidation API on the runtime or daemon state, for example:

```rust
pub(crate) fn mark_tool_list_dirty(&self, conversation_id: &str)
```

Call it from every write path that can change the final model-visible tool list, including:

- MCP server create/delete
- discovery refresh that changes tool membership
- calibration save that changes tool availability
- built-in tool registry refresh if built-ins are loaded dynamically

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -p noema-core graphql::schema::tests::mcp_tools_query_and_delete_mutation_use_persisted_setup_state`
Expected: PASS once the write paths invalidate the cache.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/store/mcp.rs crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/codex_responses.rs
git commit -m "feat: invalidate tool list cache on writes"
```

### Task 4: Verify turn-start reuse and next-turn refresh behavior end to end

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/prompts.rs` only if the turn request needs an explicit tool snapshot handoff

- [ ] **Step 1: Write the failing test**

Add an end-to-end daemon test that:

1. starts an existing conversation
2. records the tool list cache hash on the first turn
3. mutates the tool list through a write path
4. sends the next user turn
5. asserts the provider setup uses the refreshed final tool list immediately

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p noema-core daemon::tests::existing_conversation_picks_up_tool_changes_on_next_turn -- --nocapture`
Expected: FAIL before the turn path is fully wired to reuse and refresh the cached snapshot.

- [ ] **Step 3: Finish the turn integration**

Make the turn setup path reuse the cached final tool list when the hash matches and rebuild it when the hash changes, then hand the final list to provider request construction for that turn.

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -p noema-core daemon::tests::existing_conversation_picks_up_tool_changes_on_next_turn -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the relevant workspace validation**

Run:

```bash
git status --short --branch
git diff --check
cargo test -p noema-core daemon::tests::mcp_tool_list_refreshes_on_next_turn -- --nocapture
cargo test -p noema-core daemon::tests::existing_conversation_picks_up_tool_changes_on_next_turn -- --nocapture
```

Expected: clean diff check and passing targeted daemon tests.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/daemon/tests.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/prompts.rs
git commit -m "feat: refresh tool list cache on next turn"
```

### Task 5: Update durable context after the change lands

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Write the update**

Add one settled decision line noting that the daemon now caches the final model-visible tool list in runtime memory, hashes it, and invalidates it on any tool-list-changing write so existing chats pick up new tools on the next turn.

- [ ] **Step 2: Run a quick self-check**

Confirm the context update matches the implemented behavior and does not mention any internal cache names that were not actually introduced.

- [ ] **Step 3: Commit**

```bash
git add docs/context/current.md
git commit -m "docs: note tool list cache invalidation behavior"
```
