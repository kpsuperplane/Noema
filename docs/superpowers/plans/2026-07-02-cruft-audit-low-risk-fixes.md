# Cruft Audit Low-Risk Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the verified low-risk cruft slices from `docs/cruft-audit-learnings.md` without making product, security, API, or packaging decisions.

**Architecture:** Treat each verified slice as an independent cleanup unit with one commit per slice. Delete only code that Phase 2 classified as uncalled, test-only, or behavior-preserving duplication; keep all decision-dependent findings out of this plan. Preserve existing runtime behavior by relying on compile errors, focused unit tests, and final workspace validation.

**Tech Stack:** Rust workspace (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`), SurrealDB-backed store tests, Tauri desktop Rust tests, React/TypeScript frontend (`bun run lint`, `bun run build`).

---

## Source Inputs

- Audit execution plan: `docs/cruft-audit-plan.md`
- Durable audit findings and verifier classifications: `docs/cruft-audit-learnings.md`
- Project brief: `docs/project.md`
- Current context: `docs/context/current.md`

## Scope Check

This plan covers only the "Low-risk deletion slices ready to implement" from `docs/cruft-audit-learnings.md`. The final synthesis also lists behavior-preserving refactors and product/security/API decision points; those are intentionally excluded from implementation here.

Excluded examples:

- Public memory API removal: `memory-001`, `memory-002`, `ccd-001`
- Capability policy/quarantine/owner extraction: `core-misc-001`, `core-misc-002`, `core-misc-003`
- Provider selection/privacy semantics: `DR-003`, `DR-004`
- Raw daemon protocol/public API: `DC-004`, `DC-005`, `DC-006`, `DC-007`
- Embedded generated web assets packaging: `ccd-005`

## Execution Rules

- Start execution with `git status --short --branch`.
- Preserve unrelated dirty worktree changes.
- Do not modify, unset, bypass, or work around `CARGO_BUILD_RUSTC_WRAPPER` or the configured `sccache` wrapper.
- Run unit tests only. Do not run smoke tests or fixture tests.
- Do not use browser tools for frontend validation.
- Commit after each task using the commit message in the task.

## File Structure

- `crates/noema-core/src/daemon/memory_pipeline.rs`: remove legacy memory candidate wrappers and staged helpers.
- `crates/noema-core/src/daemon/tests.rs`: remove wrapper-only daemon tests and imports.
- `crates/noema-core/src/store/schema.rs`: remove unused tool/quarantine tables and retired evidence source object types.
- `crates/noema-core/src/store.rs`: stop exporting the unused `store::objects` alias module.
- `crates/noema-core/src/store/objects.rs`: delete the unused alias module.
- `crates/noema-core/src/store/tests/mcp.rs`: remove schema-only tests for deleted tables.
- `crates/noema-core/src/store/tests/claims.rs`: add evidence source object type regression coverage.
- `crates/noema-core/src/daemon/runtime/actor.rs`: remove cached tool snapshot storage.
- `crates/noema-core/src/daemon/runtime/turn.rs`: render tools once per turn and remove snapshot hashing.
- `crates/noema-core/src/daemon/runtime/handle.rs`: inline provider kind getter.
- `crates/noema-core/src/graphql.rs`: remove unused `types` module declaration.
- `crates/noema-core/src/graphql/types.rs`: delete unused re-export module.
- `crates/noema-core/src/graphql/memory.rs`: consolidate duplicate private node-id mapping helpers.
- `crates/noema-core/src/daemon/memory_tool.rs`: remove fabricated `context_packet_id`.
- `crates/noema-core/src/daemon/agent_name_tool.rs`: remove stale file-level dead-code allowance.
- `crates/noema-core/src/provider/accounts.rs`: remove unused provider account parser exports.
- `crates/noema-core/src/provider/model_catalog.rs`: remove unused metadata extras and `profiles_metadata`.
- `crates/noema-core/src/provider/contract.rs`: remove unused `ProviderError::UnsupportedFeature`.
- `crates/noema-core/src/provider/adapters/codex_oauth.rs`: remove the match arm for the deleted provider error.
- `crates/noema-core/src/lib.rs`: remove deleted re-exports.
- `crates/noema-core/src/mcp.rs`: remove unused `McpToolSchema`.
- `crates/noema-core/src/mcp/client.rs`: remove unused fake transport helper.
- `crates/noema-core/src/provider/adapters/sse.rs`: remove unused `sse_events`.
- `crates/noema-core/src/provider/adapters/responses.rs`: remove unused `send_stream`.
- `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`: remove unused one-shot `generate`.
- `crates/noema-cli/src/graphql_client.rs`: delete compatibility facade.
- `crates/noema-cli/src/main.rs`: remove `mod graphql_client`.
- `crates/noema-cli/src/commands/chat.rs`, `crates/noema-cli/src/commands/memory.rs`, `crates/noema-cli/src/inspection.rs`: import from `crate::graphql` directly.
- `crates/noema-core/web/src/App.tsx`: remove unused route helper.
- `crates/noema-core/web/src/routes.ts`: remove unused settings fallback helper.
- `crates/noema-core/web/src/components/transcript/index.ts`: delete unused barrel.
- `crates/noema-core/src/config.rs`: remove unused `Config::load_codex`.
- `crates/noema-core/src/runtime_host.rs`: remove unused test runtime-host constructor.
- `crates/noema-desktop/src/graphql_ipc.rs`: replace ineffective field-access test with serialized wire-key test.

### Task 1: Memory-Pipeline Legacy Wrappers

**Files:**
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Confirm the wrapper-only references**

Run:

```bash
rg -n "explicit_memory_claim_candidate|provider_memory_claim_candidate|infer_chat_memory_type|title_from_memory_content" crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/tests.rs
```

Expected: definitions in `memory_pipeline.rs` and test references in `daemon/tests.rs`; no runtime references.

- [ ] **Step 2: Remove wrapper imports from daemon tests**

Change the import block in `crates/noema-core/src/daemon/tests.rs` from:

```rust
memory_pipeline::{
    ConversationMemoryContext, explicit_memory_claim_candidate, explicit_memory_content,
    infer_chat_sensitivity, provider_memory_claim_candidate, provider_memory_write_proposal,
},
```

to:

```rust
memory_pipeline::{
    ConversationMemoryContext, explicit_memory_content, infer_chat_sensitivity,
    provider_memory_write_proposal,
},
```

- [ ] **Step 3: Delete wrapper-only tests**

Delete these test functions from `crates/noema-core/src/daemon/tests.rs`:

```rust
explicit_claim_candidate_falls_back_for_punctuation_only_relation_objects
explicit_claim_candidate_normalizes_relation_object_whitespace
explicit_claim_candidate_canonicalizes_dislikes
explicit_and_provider_dislikes_share_claim_shape
provider_note_fallback_uses_content_for_object_identity
note_fallback_object_id_uses_opaque_content_fingerprint
third_party_same_name_provider_subject_stays_non_local
named_same_name_provider_subject_stays_non_local
literal_local_human_provider_subject_maps_to_local
provider_subject_uses_first_subject_without_local_participant_override
provider_empty_retrieval_hints_fall_back_to_deterministic_hints
```

Keep tests that call `provider_memory_write_proposal`, `explicit_memory_content`, or `infer_chat_sensitivity` directly.

- [ ] **Step 4: Delete legacy wrappers and staged helpers**

Delete these functions and their dead-code attributes from `crates/noema-core/src/daemon/memory_pipeline.rs`:

```rust
explicit_memory_claim_candidate
provider_memory_claim_candidate
infer_chat_memory_type
title_from_memory_content
```

Do not delete `explicit_memory_write_proposal`, `provider_memory_write_proposal`, `infer_chat_sensitivity`, `provider_subject_entity`, or subject/entity helper functions used by `provider_memory_write_proposal`.

- [ ] **Step 5: Verify no deleted symbols remain**

Run:

```bash
rg -n "explicit_memory_claim_candidate|provider_memory_claim_candidate|infer_chat_memory_type|title_from_memory_content" crates/noema-core/src/daemon
```

Expected: no output.

- [ ] **Step 6: Run focused Rust validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast deterministic_sensitivity_classifier_fails_closed_for_common_secrets
cargo test -p noema-core --no-fail-fast provider_memory_write
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/tests.rs
git commit -m "refactor: remove legacy memory candidate wrappers"
```

### Task 2: Store Schema Cruft And Aliases

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store.rs`
- Delete: `crates/noema-core/src/store/objects.rs`
- Modify: `crates/noema-core/src/store/tests/mcp.rs`
- Modify: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Add a regression test for retired evidence source object types**

Add this test to `crates/noema-core/src/store/tests/claims.rs` near the other evidence relation tests:

```rust
#[tokio::test]
async fn evidence_source_object_type_rejects_retired_object_types() {
    let store = test_store().await;

    for (table, authority) in [
        ("supported_by", "explicit_human_statement"),
        ("corrected_by", "human_correction"),
        ("contradicted_by", "agent_inference"),
    ] {
        for object_type in ["memory_item", "relationship", "context_packet"] {
            let query = format!(
                r#"
                CREATE type::record('{table}', string::concat('legacy_', $object_type)) SET
                  relation_id = string::concat('relation:', $object_type),
                  claim_id = 'claim:legacy-object-type',
                  source_kind = 'object',
                  source_item_id = NONE,
                  source_object_type = $object_type,
                  source_object_id = string::concat($object_type, ':legacy'),
                  authority = $authority,
                  excerpt = NONE,
                  observed_at = NONE,
                  created_by = 'agent:primary',
                  metadata = {{}};
                "#
            );

            let error = store
                .db()
                .query(query)
                .bind(("object_type", object_type))
                .bind(("authority", authority))
                .await
                .expect("legacy source object type query")
                .check()
                .expect_err("retired source object type should be rejected");

            assert!(
                error.to_string().contains(object_type),
                "unexpected error for {table} / {object_type}: {error}"
            );
        }
    }
}
```

- [ ] **Step 2: Run the new test and verify it fails before the schema edit**

Run:

```bash
cargo test -p noema-core evidence_source_object_type_rejects_retired_object_types -- --exact
```

Expected: FAIL because the current schema still accepts `memory_item`, `relationship`, and `context_packet`.

- [ ] **Step 3: Remove unused tool/quarantine tables from the schema**

Delete these schema blocks from `crates/noema-core/src/store/schema.rs`:

```sql
DEFINE TABLE IF NOT EXISTS tool_invocations SCHEMAFULL;
DEFINE FIELD OVERWRITE tool_invocation_id ON TABLE tool_invocations TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE tool_invocations TYPE option<string>;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE tool_invocations TYPE option<string>;
DEFINE FIELD OVERWRITE conversation_id ON TABLE tool_invocations TYPE option<string>;
DEFINE FIELD OVERWRITE turn_id ON TABLE tool_invocations TYPE option<string>;
DEFINE FIELD OVERWRITE requesting_actor_id ON TABLE tool_invocations TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE status ON TABLE tool_invocations TYPE string ASSERT $value INSIDE ['proposed', 'denied', 'approval_required', 'invoked', 'completed', 'failed'];
DEFINE FIELD OVERWRITE policy_decision ON TABLE tool_invocations TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE proposal_payload ON TABLE tool_invocations TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE result_summary ON TABLE tool_invocations TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE tool_invocations TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE tool_invocations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS tool_invocations_invocation_id ON TABLE tool_invocations COLUMNS tool_invocation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS tool_invocations_mcp_tool_id ON TABLE tool_invocations COLUMNS mcp_tool_id;
DEFINE INDEX IF NOT EXISTS tool_invocations_conversation_turn ON TABLE tool_invocations COLUMNS conversation_id, turn_id;

DEFINE TABLE IF NOT EXISTS quarantined_tool_results SCHEMAFULL;
DEFINE FIELD OVERWRITE quarantine_id ON TABLE quarantined_tool_results TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE tool_invocation_id ON TABLE quarantined_tool_results TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE validation_status ON TABLE quarantined_tool_results TYPE string ASSERT $value INSIDE ['pending', 'valid', 'invalid'];
DEFINE FIELD OVERWRITE owner_trust ON TABLE quarantined_tool_results TYPE string ASSERT $value INSIDE ['trusted', 'untrusted', 'mixed', 'unresolved'];
DEFINE FIELD OVERWRITE release_status ON TABLE quarantined_tool_results TYPE string ASSERT $value INSIDE ['quarantined', 'released', 'released_sanitized', 'denied'];
DEFINE FIELD OVERWRITE owner_evidence ON TABLE quarantined_tool_results TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE scan_summary ON TABLE quarantined_tool_results TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE raw_result_ref ON TABLE quarantined_tool_results TYPE option<string>;
DEFINE FIELD OVERWRITE released_payload ON TABLE quarantined_tool_results TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE quarantined_tool_results TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE quarantined_tool_results TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS quarantined_tool_results_quarantine_id ON TABLE quarantined_tool_results COLUMNS quarantine_id UNIQUE;
DEFINE INDEX IF NOT EXISTS quarantined_tool_results_invocation_id ON TABLE quarantined_tool_results COLUMNS tool_invocation_id;
```

- [ ] **Step 4: Remove seed rows and schema-only tests for deleted tables**

In `crates/noema-core/src/store/tests/mcp.rs`, delete the setup `CREATE` statements for:

```sql
type::record('tool_invocations', 'local_test_invocation')
type::record('quarantined_tool_results', 'local_test_result')
```

Also delete these test functions:

```rust
tool_invocation_schema_rejects_invalid_status
quarantined_tool_result_schema_rejects_invalid_status
```

Keep `approval_requests` rows and tests. The `approval_requests.tool_invocation_id` field is a string reference and does not require the deleted table.

- [ ] **Step 5: Remove retired evidence object values**

In `crates/noema-core/src/store/schema.rs`, replace all three evidence object-type assertions with this value list:

```sql
$value INSIDE ['human', 'agent', 'tool', 'conversation', 'conversation_turn', 'conversation_item', 'entity']
```

Apply that replacement to `supported_by`, `corrected_by`, and `contradicted_by`.

- [ ] **Step 6: Delete the unused store object alias module**

In `crates/noema-core/src/store.rs`, remove:

```rust
pub mod objects;
```

Delete the file:

```bash
rm crates/noema-core/src/store/objects.rs
```

- [ ] **Step 7: Verify removed schema names are gone from source**

Run:

```bash
rg -n "tool_invocations|quarantined_tool_results|memory_item|relationship|context_packet|pub mod objects|store::objects|crate::store::objects" crates/noema-core/src/store.rs crates/noema-core/src/store crates/noema-core/src/store/tests
```

Expected: no references to deleted tables, deleted store alias module, or retired evidence object values. References to unrelated product docs are not part of this command.

- [ ] **Step 8: Run focused store validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core evidence_source_object_type_rejects_retired_object_types -- --exact
cargo test -p noema-core --no-fail-fast store::tests::mcp
cargo test -p noema-core --no-fail-fast store::tests::claims
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 9: Commit**

Run:

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store.rs crates/noema-core/src/store/tests/mcp.rs crates/noema-core/src/store/tests/claims.rs
git add -u crates/noema-core/src/store/objects.rs
git commit -m "refactor: prune unused store schema cruft"
```

### Task 3: Small Runtime Cleanup

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/actor.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/handle.rs`

- [ ] **Step 1: Remove cached tool snapshot state from runtime actor types**

In `crates/noema-core/src/daemon/runtime/actor.rs`, change:

```rust
#[derive(Debug, Clone)]
pub(super) struct ActiveConversation {
    pub(super) provider_kind: String,
    pub(super) model: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) next_turn_index: u64,
    pub(super) tool_snapshot: Option<CachedToolSnapshot>,
}

#[derive(Debug, Clone)]
pub(super) struct CachedToolSnapshot {
    pub(super) hash: u64,
    pub(super) rendered_tools: String,
}
```

to:

```rust
#[derive(Debug, Clone)]
pub(super) struct ActiveConversation {
    pub(super) provider_kind: String,
    pub(super) model: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) next_turn_index: u64,
}
```

- [ ] **Step 2: Remove `tool_snapshot: None` initializers**

In `crates/noema-core/src/daemon/runtime/turn.rs`, remove every initializer field:

```rust
tool_snapshot: None,
```

from `ActiveConversation` construction.

- [ ] **Step 3: Render tools once per turn**

In `crates/noema-core/src/daemon/runtime/turn.rs`, replace:

```rust
let _tool_snapshot = self.refresh_tool_snapshot(&conversation_id).await?;
self.update_conversation_agent_status(
    &conversation_id,
    PersistedAgentStatus::InputReceived,
    &item_tx,
)
.await?;
let tool_snapshot = self.refresh_tool_snapshot(&conversation_id).await?;
```

with:

```rust
let rendered_tools = self.render_available_tools().await?;
self.update_conversation_agent_status(
    &conversation_id,
    PersistedAgentStatus::InputReceived,
    &item_tx,
)
.await?;
```

Then replace every use of:

```rust
&tool_snapshot.rendered_tools
tool_snapshot.rendered_tools.clone()
```

with:

```rust
&rendered_tools
rendered_tools.clone()
```

- [ ] **Step 4: Delete snapshot helper and hashing**

In `crates/noema-core/src/daemon/runtime/turn.rs`, remove the `CachedToolSnapshot` import, the `refresh_tool_snapshot` method, and this function:

```rust
fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
```

If `std::hash::{Hash, Hasher}` or `std::collections::hash_map::DefaultHasher` becomes unused, remove those imports.

- [ ] **Step 5: Inline provider kind getter**

In `crates/noema-core/src/daemon/runtime/handle.rs`, replace:

```rust
fn configured_provider_kind(&self) -> &str {
    &self.default_provider_kind
}

pub(crate) fn provider_kind(&self) -> &str {
    self.configured_provider_kind()
}
```

with:

```rust
pub(crate) fn provider_kind(&self) -> &str {
    &self.default_provider_kind
}
```

- [ ] **Step 6: Verify deleted runtime symbols are gone**

Run:

```bash
rg -n "CachedToolSnapshot|tool_snapshot|stable_hash|configured_provider_kind" crates/noema-core/src/daemon/runtime
```

Expected: no output.

- [ ] **Step 7: Run focused runtime validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast primary_agent_runtime_preference_supplies_turn_model
cargo test -p noema-core --no-fail-fast primary_agent_runtime_preference_selects_provider_without_restart
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/actor.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/runtime/handle.rs
git commit -m "refactor: simplify runtime tool rendering state"
```

### Task 4: GraphQL Private Cruft

**Files:**
- Modify: `crates/noema-core/src/graphql.rs`
- Delete: `crates/noema-core/src/graphql/types.rs`
- Modify: `crates/noema-core/src/graphql/memory.rs`

- [ ] **Step 1: Remove unused GraphQL types module**

In `crates/noema-core/src/graphql.rs`, delete:

```rust
mod types;
```

Delete the file:

```bash
rm crates/noema-core/src/graphql/types.rs
```

- [ ] **Step 2: Consolidate duplicate memory graph node mapping helpers**

In `crates/noema-core/src/graphql/memory.rs`, replace:

```rust
fn mapped_memory_graph_node_id(
    node_id_map: &HashMap<String, String>,
    store_node_id: &str,
) -> String {
    node_id_map
        .get(store_node_id)
        .expect("memory graph node must exist in response node map")
        .clone()
}

fn mapped_memory_graph_edge_endpoint_id(
    node_id_map: &HashMap<String, String>,
    store_node_id: &str,
) -> String {
    node_id_map
        .get(store_node_id)
        .expect("memory graph edge endpoint must exist in returned nodes")
        .clone()
}
```

with:

```rust
fn mapped_memory_graph_node_id(
    node_id_map: &HashMap<String, String>,
    store_node_id: &str,
) -> String {
    node_id_map
        .get(store_node_id)
        .expect("memory graph node must exist in returned node map")
        .clone()
}
```

- [ ] **Step 3: Update memory graph edge call sites**

In `GraphqlMemoryGraphEdge::from_store_edge`, replace:

```rust
source_node_id: mapped_memory_graph_edge_endpoint_id(node_id_map, &edge.source_node_id),
target_node_id: mapped_memory_graph_edge_endpoint_id(node_id_map, &edge.target_node_id),
```

with:

```rust
source_node_id: mapped_memory_graph_node_id(node_id_map, &edge.source_node_id),
target_node_id: mapped_memory_graph_node_id(node_id_map, &edge.target_node_id),
```

- [ ] **Step 4: Verify removed GraphQL symbols are gone**

Run:

```bash
rg -n "mod types|mapped_memory_graph_edge_endpoint_id|graphql::types|crate::graphql::types" crates/noema-core/src/graphql.rs crates/noema-core/src/graphql crates/noema-core/src
```

Expected: no output.

- [ ] **Step 5: Run focused GraphQL validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast memory_graph_query
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/graphql.rs crates/noema-core/src/graphql/memory.rs
git add -u crates/noema-core/src/graphql/types.rs
git commit -m "refactor: remove unused graphql cruft"
```

### Task 5: Daemon-Core Fabricated Output Cleanup

**Files:**
- Modify: `crates/noema-core/src/daemon/memory_tool.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/src/daemon/agent_name_tool.rs`

- [ ] **Step 1: Remove fabricated context packet output**

In `crates/noema-core/src/daemon/memory_tool.rs`, delete:

```rust
let context_packet_id = context_packet_id(context, call_id);
```

Then remove this JSON field from the result JSON object returned by
`execute_search_memory_inner`:

```rust
"context_packet_id": context_packet_id,
```

- [ ] **Step 2: Delete context packet helper code and unit test**

In `crates/noema-core/src/daemon/memory_tool.rs`, delete:

```rust
fn context_packet_id(context: &MemoryToolRuntimeContext, call_id: Option<&str>) -> String
fn sanitize_context_packet_fragment(value: &str) -> String
```

Also delete the test:

```rust
context_packet_id_uses_sanitized_per_call_discriminator
```

- [ ] **Step 3: Update daemon test payload assertion**

In `crates/noema-core/src/daemon/tests.rs`, replace:

```rust
assert!(payload["context_packet_id"].as_str().is_some());
```

with:

```rust
assert!(payload.get("context_packet_id").is_none());
```

- [ ] **Step 4: Remove stale dead-code allowance from agent name tool**

In `crates/noema-core/src/daemon/agent_name_tool.rs`, delete the first line:

```rust
#![allow(dead_code)]
```

- [ ] **Step 5: Verify deleted symbols are gone**

Run:

```bash
rg -n "context_packet_id|sanitize_context_packet_fragment|allow\\(dead_code\\)" crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/agent_name_tool.rs crates/noema-core/src/daemon/tests.rs
```

Expected: no output.

- [ ] **Step 6: Run focused daemon validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast search_memory
cargo test -p noema-core --no-fail-fast update_own_name
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/daemon/agent_name_tool.rs
git commit -m "refactor: remove fabricated memory context packet id"
```

### Task 6: Provider-Core Unused Exports And Enum Pieces

**Files:**
- Modify: `crates/noema-core/src/provider/accounts.rs`
- Modify: `crates/noema-core/src/provider/model_catalog.rs`
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_oauth.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Remove unused provider account parser exports**

In `crates/noema-core/src/provider/accounts.rs`, delete:

```rust
/// Parse a stored provider auth method.
///
/// # Errors
///
/// Returns [`MemoryPersistenceError::InvalidEnum`] when the value is outside
/// the provider auth method vocabulary.
pub fn parse_auth_method(value: &str) -> Result<ProviderAuthMethod, MemoryPersistenceError> {
    match value {
        "oauth_device_code" => Ok(ProviderAuthMethod::OauthDeviceCode),
        "secret_input" => Ok(ProviderAuthMethod::SecretInput),
        "external_manual" => Ok(ProviderAuthMethod::ExternalManual),
        "none" => Ok(ProviderAuthMethod::None),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_auth_method",
            value: other.to_string(),
        }),
    }
}

/// Parse a stored provider account status.
///
/// # Errors
///
/// Returns [`MemoryPersistenceError::InvalidEnum`] when the value is outside
/// the provider account status vocabulary.
pub fn parse_account_status(value: &str) -> Result<ProviderAccountStatus, MemoryPersistenceError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_account_status",
            value: other.to_string(),
        }),
    }
}
```

If `MemoryPersistenceError` becomes unused in this file, remove that import.

- [ ] **Step 2: Remove parser re-exports**

In `crates/noema-core/src/lib.rs`, replace:

```rust
pub use provider::accounts::{
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod, parse_account_status,
    parse_auth_method,
};
```

with:

```rust
pub use provider::accounts::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
```

- [ ] **Step 3: Remove unused model metadata extras**

In `crates/noema-core/src/provider/model_catalog.rs`, remove this insertion block from `profile_value_from_model`:

```rust
if let Some(default_reasoning_effort) = string_field(model, &["default_reasoning_level"]) {
    object.insert(
        "default_reasoning_effort".to_string(),
        Value::String(default_reasoning_effort.to_string()),
    );
}
if let Some(input_modalities) = model.get("input_modalities").and_then(Value::as_array) {
    object.insert(
        "input_modalities".to_string(),
        Value::Array(input_modalities.clone()),
    );
}
```

Delete the unused helper:

```rust
/// Build provider account metadata for tests and seeded local providers.
#[must_use]
pub fn profiles_metadata(profiles: &[(&str, &str)]) -> Value {
    json!({
        "profiles": profiles
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect::<Vec<_>>()
    })
}
```

- [ ] **Step 4: Update model catalog test assertions**

In `extracts_visible_profiles_from_codex_model_list`, remove:

```rust
assert_eq!(profiles[0]["default_reasoning_effort"], "medium");
assert_eq!(profiles[0]["input_modalities"], json!(["text", "image"]));
```

Keep the assertions for `id`, `label`, and visible-profile filtering.

- [ ] **Step 5: Remove `ProviderError::UnsupportedFeature`**

In `crates/noema-core/src/provider/contract.rs`, delete the enum variant:

```rust
/// The provider does not support a requested option.
#[error("unsupported provider feature: {feature}")]
UnsupportedFeature {
    /// Unsupported feature name.
    feature: String,
},
```

- [ ] **Step 6: Remove deleted variant from exhaustive matches**

In `crates/noema-core/src/daemon/runtime/turn.rs`, remove this match arm:

```rust
| ProviderError::UnsupportedFeature { .. }
```

In `crates/noema-core/src/provider/adapters/codex_oauth.rs`, replace:

```rust
ProviderError::ProtocolError { message, .. }
| ProviderError::PartialResponse { message, .. }
| ProviderError::UnsupportedFeature { feature: message } => message,
```

with:

```rust
ProviderError::ProtocolError { message, .. }
| ProviderError::PartialResponse { message, .. } => message,
```

- [ ] **Step 7: Verify deleted provider symbols are gone**

Run:

```bash
rg -n "parse_auth_method|parse_account_status|profiles_metadata|default_reasoning_effort|input_modalities|UnsupportedFeature" crates/noema-core/src/provider crates/noema-core/src/lib.rs crates/noema-core/src/daemon
```

Expected: no source references to deleted functions, metadata keys, or enum variant. Model-list fixture input may still contain provider-supplied fields only if the test intentionally proves they are ignored.

- [ ] **Step 8: Run focused provider validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast extracts_visible_profiles_from_codex_model_list
cargo test -p noema-core --no-fail-fast codex_catalog_refresh_with_tokens_marks_unknown_account_authenticated
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 9: Commit**

Run:

```bash
git add crates/noema-core/src/provider/accounts.rs crates/noema-core/src/provider/model_catalog.rs crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider/adapters/codex_oauth.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/lib.rs
git commit -m "refactor: remove unused provider core surface"
```

### Task 7: MCP Unused Schema And Test Helper

**Files:**
- Modify: `crates/noema-core/src/mcp.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/mcp/client.rs`

- [ ] **Step 1: Remove unused MCP schema type**

In `crates/noema-core/src/mcp.rs`, delete:

```rust
/// MCP tool schema metadata captured during discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolSchema {
    /// MCP input schema used for argument validation.
    pub input_schema: Value,
    /// Optional MCP output schema used for result validation.
    pub output_schema: Option<Value>,
    /// MCP tool annotations captured as non-authoritative setup hints.
    pub annotations: Value,
}
```

If `serde_json::Value` becomes unused in `mcp.rs`, remove that import only after checking the rest of the file.

- [ ] **Step 2: Remove root re-export**

In `crates/noema-core/src/lib.rs`, remove `McpToolSchema` from:

```rust
pub use mcp::{
    McpCalibrationStatus, McpToolSchema, McpTransportKind, McpTrustClassification, OwnerExtractor,
    OwnerExtractorSource, TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};
```

The result should be:

```rust
pub use mcp::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, OwnerExtractor,
    OwnerExtractorSource, TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};
```

- [ ] **Step 3: Remove unused fake transport helper if present**

In `crates/noema-core/src/mcp/client.rs`, delete this inherent test helper
inside the test-only `impl FakeMcpTransport`:

```rust
#[allow(dead_code)]
async fn call_tool(&self) {
    tokio::time::sleep(Duration::from_millis(1)).await;
    self.state.call_count.fetch_add(1, Ordering::SeqCst);
}
```

Delete that inherent helper only. Keep the `McpTransport for FakeMcpTransport` trait implementation.

- [ ] **Step 4: Verify deleted MCP symbols are gone**

Run:

```bash
rg -n "McpToolSchema|allow\\(dead_code\\).*call_tool|fn call_tool\\(" crates/noema-core/src/mcp.rs crates/noema-core/src/mcp/client.rs crates/noema-core/src/lib.rs
```

Expected: no `McpToolSchema`; only trait-method `call_tool` definitions/usages that are part of live transport code.

- [ ] **Step 5: Run focused MCP validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast mcp
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/mcp.rs crates/noema-core/src/lib.rs crates/noema-core/src/mcp/client.rs
git commit -m "refactor: remove unused mcp schema cruft"
```

### Task 8: Provider-Adapter Unused Wrappers

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/sse.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`

- [ ] **Step 1: Remove unused SSE event wrapper**

In `crates/noema-core/src/provider/adapters/sse.rs`, delete:

```rust
#[allow(dead_code)]
pub(crate) fn sse_events(text: &str) -> impl Iterator<Item = SseEvent> + '_ {
    text.split("\n\n").filter_map(|chunk| {
        let event = parse_sse_event(chunk);
        (event.event.is_some() || event.data.is_some()).then_some(event)
    })
}
```

Keep `parse_sse_event_bytes` and `next_sse_event_boundary`.

- [ ] **Step 2: Remove unused Responses transport wrapper**

In `crates/noema-core/src/provider/adapters/responses.rs`, delete:

```rust
/// Send one streaming Responses request and collect the terminal response.
///
/// # Errors
///
/// Returns [`ProviderError`] for HTTP transport failures, API errors, or
/// malformed Server-Sent Events.
pub async fn send_stream<T>(
    &self,
    bearer_token: &str,
    body: T,
    extra_headers: HeaderMap,
    diagnostics: ResponsesDiagnosticContext,
) -> Result<ResponsesResponse, ProviderError>
where
    T: Serialize,
{
    self.send_streaming(bearer_token, body, extra_headers, diagnostics, &mut |_| {})
        .await
}
```

Keep `send_streaming`.

- [ ] **Step 3: Remove unused Foundation bridge one-shot generate wrapper**

In `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs`, delete:

```rust
/// Generate assistant text through a bridge-backed session.
///
/// # Errors
///
/// Returns [`FoundationBridgeError`] when session creation or generation
/// fails.
pub async fn generate(
    &mut self,
    conversation_id: String,
    model_profile: String,
    instructions: Option<String>,
    input: String,
    max_output_tokens: Option<u32>,
    on_delta: &mut (dyn FnMut(String) + Send),
) -> Result<String, FoundationBridgeError> {
    let session_id = self
        .create_session(conversation_id, model_profile, instructions)
        .await?;
    self.generate_in_session(session_id, input, max_output_tokens, on_delta)
        .await
}
```

Keep `create_session` and `generate_in_session`.

- [ ] **Step 4: Verify deleted wrappers are gone**

Run:

```bash
rg -n "fn sse_events|send_stream\\(|pub async fn generate\\(" crates/noema-core/src/provider/adapters
```

Expected: no output for these wrappers. `send_streaming` and `generate_in_session` remain.

- [ ] **Step 5: Run focused provider-adapter validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast sse
cargo test -p noema-core --no-fail-fast responses
cargo test -p noema-core --no-fail-fast foundation
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/provider/adapters/sse.rs crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/provider/adapters/foundation_bridge_process.rs
git commit -m "refactor: remove unused provider adapter wrappers"
```

### Task 9: CLI GraphQL Facade

**Files:**
- Delete: `crates/noema-cli/src/graphql_client.rs`
- Modify: `crates/noema-cli/src/main.rs`
- Modify: `crates/noema-cli/src/commands/chat.rs`
- Modify: `crates/noema-cli/src/commands/memory.rs`
- Modify: `crates/noema-cli/src/inspection.rs`

- [ ] **Step 1: Rewrite CLI imports to use `crate::graphql` directly**

In `crates/noema-cli/src/commands/chat.rs`, replace:

```rust
graphql_client::{
    start_primary_conversation, stream_conversation_turn, validate_graphql_base_url,
},
```

with:

```rust
graphql::{start_primary_conversation, stream_conversation_turn, validate_graphql_base_url},
```

In `crates/noema-cli/src/commands/memory.rs`, replace:

```rust
graphql_client::validate_graphql_base_url,
```

with:

```rust
graphql::validate_graphql_base_url,
```

In `crates/noema-cli/src/inspection.rs`, replace:

```rust
graphql_client::{self, GraphqlRequest},
```

with:

```rust
graphql::{self, GraphqlRequest},
```

Then replace each `graphql_client::execute` call with `graphql::execute`.

- [ ] **Step 2: Remove module declaration and facade file**

In `crates/noema-cli/src/main.rs`, delete:

```rust
mod graphql_client;
```

Delete the facade file:

```bash
rm crates/noema-cli/src/graphql_client.rs
```

- [ ] **Step 3: Verify no facade references remain**

Run:

```bash
rg -n "graphql_client|mod graphql_client|crate::graphql_client" crates/noema-cli/src
```

Expected: no output, except test names containing the phrase if those names are still useful and compile.

- [ ] **Step 4: Run focused CLI validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-cli --no-fail-fast graphql
cargo test -p noema-cli --no-fail-fast inspection
cargo check -p noema-cli
```

Expected: all commands pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noema-cli/src/main.rs crates/noema-cli/src/commands/chat.rs crates/noema-cli/src/commands/memory.rs crates/noema-cli/src/inspection.rs
git add -u crates/noema-cli/src/graphql_client.rs
git commit -m "refactor: remove cli graphql compatibility facade"
```

### Task 10: Frontend Unused Helpers And Barrel

**Files:**
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/routes.ts`
- Delete: `crates/noema-core/web/src/components/transcript/index.ts`

- [ ] **Step 1: Remove unused app-shell route helper**

In `crates/noema-core/web/src/App.tsx`, delete:

```ts
export function shouldRouteThroughAppShell({
  route,
  onboarded
}: {
  route: AppRoute;
  onboarded: boolean;
}) {
  return onboarded && Boolean(route);
}
```

- [ ] **Step 2: Remove unused settings fallback helper**

In `crates/noema-core/web/src/routes.ts`, delete:

```ts
export function settingsFallbackRoute(route: NonSettingsAppRoute | null): NonSettingsAppRoute {
  return route ?? { kind: "chat" };
}
```

- [ ] **Step 3: Delete unused transcript barrel**

Run:

```bash
rm crates/noema-core/web/src/components/transcript/index.ts
```

Do not delete `crates/noema-core/web/src/components/Transcript.tsx` or files under `crates/noema-core/web/src/components/transcript/`.

- [ ] **Step 4: Verify deleted frontend exports are gone**

Run:

```bash
rg -n "shouldRouteThroughAppShell|settingsFallbackRoute|components/transcript" crates/noema-core/web/src
```

Expected: no deleted helper references. Existing imports from `components/Transcript` may remain.

- [ ] **Step 5: Run focused frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both commands pass. Do not use browser inspection for this cleanup.

- [ ] **Step 6: Run Rust check if frontend type generation touched Rust-exported schema**

Run:

```bash
git status --short
cargo check -p noema-core
```

Expected: generated frontend asset changes may appear if `bun run build` writes assets; inspect them and keep only expected build outputs if this repo currently tracks them. `cargo check -p noema-core` passes.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/routes.ts
git add -u crates/noema-core/web/src/components/transcript/index.ts
git status --short
git commit -m "refactor: remove unused frontend helpers"
```

### Task 11: Core Misc Unused Helpers

**Files:**
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/runtime_host.rs`

- [ ] **Step 1: Remove unused Codex-only config loader**

In `crates/noema-core/src/config.rs`, delete:

```rust
/// Load the Codex provider configuration without requiring `OpenAI` credentials.
///
/// # Errors
///
/// Returns [`ConfigError`] when path resolution fails, the config file is
/// missing or invalid, environment values are invalid, or Codex-specific
/// values fail validation.
pub fn load_codex(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
) -> Result<CodexProviderConfig, ConfigError> {
    let raw = load_raw_config(path_override, cli)?;
    raw.resolve_codex_config()
}
```

If `CodexProviderConfig` or `PathBuf` imports become unused, remove only the unused imports reported by the compiler.

- [ ] **Step 2: Remove unused test runtime-host constructor**

In `crates/noema-core/src/runtime_host.rs`, delete:

```rust
/// Construct a host around already-open test state.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn for_tests_with_store_and_runtime(
    store: NoemaStore,
    runtime: CodexRuntimeHandle,
) -> Self {
    Self {
        runtime,
        store,
        provider_auth: ProviderAuthManager::new(),
        mcp_oauth: McpOAuthSetupManager::new(),
        system_errors: SystemErrorLogger::from_paths(
            &NoemaPaths::from_process_env().expect("test paths"),
        ),
        paths: NoemaPaths::from_process_env().expect("test paths"),
        subscriptions: crate::graphql::ConversationSubscriptionRegistry::default(),
    }
}
```

- [ ] **Step 3: Verify deleted helpers are gone**

Run:

```bash
rg -n "load_codex|for_tests_with_store_and_runtime" crates/noema-core/src
```

Expected: no output.

- [ ] **Step 4: Run focused validation**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-core --no-fail-fast config
cargo test -p noema-core --no-fail-fast runtime_host
cargo check -p noema-core
```

Expected: all commands pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/noema-core/src/config.rs crates/noema-core/src/runtime_host.rs
git commit -m "refactor: remove unused core helpers"
```

### Task 12: Desktop Subscription Serialization Test

**Files:**
- Modify: `crates/noema-desktop/src/graphql_ipc.rs`

- [ ] **Step 1: Replace ineffective field-access test**

In `crates/noema-desktop/src/graphql_ipc.rs`, replace:

```rust
#[test]
fn subscription_event_payload_keeps_subscription_id() {
    let payload = SubscriptionEventPayload {
        subscription_id: "sub_1".to_string(),
        response: serde_json::json!({"data": {"ok": true}}),
    };

    assert_eq!(payload.subscription_id, "sub_1");
    assert_eq!(payload.response["data"]["ok"], true);
}
```

with:

```rust
#[test]
fn subscription_event_payload_serializes_frontend_wire_key() {
    let payload = SubscriptionEventPayload {
        subscription_id: "sub_1".to_string(),
        response: serde_json::json!({"data": {"ok": true}}),
    };

    let serialized = serde_json::to_value(&payload).expect("serialize subscription event payload");

    assert_eq!(serialized["subscriptionId"], "sub_1");
    assert!(serialized.get("subscription_id").is_none());
    assert_eq!(serialized["response"]["data"]["ok"], true);
}
```

- [ ] **Step 2: Run the desktop test**

Run:

```bash
cargo fmt --all --check
cargo test -p noema-desktop subscription_event_payload_serializes_frontend_wire_key -- --exact
cargo check -p noema-desktop
```

Expected: all commands pass.

- [ ] **Step 3: Commit**

Run:

```bash
git add crates/noema-desktop/src/graphql_ipc.rs
git commit -m "test: verify desktop graphql subscription wire key"
```

### Task 13: Whole-Plan Validation

**Files:**
- No source edits unless validation reveals a compile or test issue introduced by an earlier task.

- [ ] **Step 1: Check final worktree state**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: branch is ahead by the task commits, with no unstaged source changes unless generated frontend build artifacts were intentionally committed in Task 10. `git diff --check` prints no output.

- [ ] **Step 2: Run full Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands pass. If a test fails because a Noema daemon/OpenAI provider test cannot bind a local socket, rerun the same test command with the necessary socket permissions and report the distinction.

- [ ] **Step 3: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both commands pass.

- [ ] **Step 4: Inspect committed scope**

Run:

```bash
git log --oneline --max-count=15
git status --short --branch
```

Expected: one commit per task, no unexpected dirty files.

## Self-Review

- Spec coverage: all 12 low-risk deletion slices from `docs/cruft-audit-learnings.md` are represented as Tasks 1 through 12. Behavior-preserving refactor slices and product/security/API decisions are explicitly out of scope.
- Placeholder scan: no open-ended implementation markers are present. Each code-changing task names exact files, symbols, snippets, commands, expected results, and commit messages.
- Type consistency: names in later tasks match existing source names inspected during planning: `rendered_tools`, `ProviderError`, `SubscriptionEventPayload`, `GraphqlRequest`, `McpToolSchema`, and `GatewayToolProposal` are used consistently with the current codebase.
