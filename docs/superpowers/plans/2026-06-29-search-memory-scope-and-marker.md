# Search Memory Scope And Marker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `search_memory` reliably answer broad and topical questions about the user's saved memories by adding validated concrete `scope_ids`, while rendering chat memory markers as truthful saved or updated fact previews.

**Architecture:** Extend the daemon tool contract with `scope_ids`, validate those IDs against trusted turn context, and call a scoped store retrieval API that applies policy gates, optional scope filtering, and optional query narrowing as AND semantics. Add canonical claim outcome metadata to memory-write activities so the React transcript can render `Memory saved: <fact>` without an extra read.

**Tech Stack:** Rust, Tokio, serde/serde_json, SurrealDB embedded store, existing Noema daemon runtime, React 19, TypeScript, Bun, existing transcript component tests.

---

## Scope Check

This plan implements the approved design in `docs/superpowers/specs/2026-06-29-search-memory-scope-and-marker-design.md`.

In scope:

- `search_memory` accepts `scope_ids: Vec<String>`.
- Empty `query` is accepted only when `scope_ids` is non-empty.
- The daemon validates agent-supplied scope IDs against active trusted IDs for the turn.
- Query and scope use AND semantics.
- Store retrieval remains deterministic and policy-gated.
- Runtime prompt guidance exposes concrete active retrieval IDs and teaches scoped empty queries.
- Memory persistence activities include persisted claim outcome previews.
- The transcript marker renders `Memory saved: User likes planes` or `Memory updated: ...` when safe metadata is present.

Out of scope:

- Automatic pre-turn memory injection.
- Backend branching on English phrases such as "what memories do you have of me".
- Agent-invented aliases such as `current_human`.
- Vector search, LLM retrieval ranking, or full memory-management UI.
- Backwards compatibility migrations for pre-V1 transcript metadata.

## Implementation Rules

- Preserve unrelated dirty changes, especially `crates/noema-core/web/src/components/ui/attachment.tsx`.
- Use the existing `human:local`, `conversation:<id>`, and project scope IDs produced by `project_scope_from_cwd`.
- Keep `retrieve_claims` available for existing query-only callers by delegating to the new scoped API.
- Treat invalid scope IDs as failed tool calls, not broadened searches.
- Keep old activity metadata keys such as `claim_ids`, `created_claim_count`, `reinforced_claim_count`, and `failed_proposal_count`; add `claim_outcomes` alongside them.
- Commit after each task if implementing directly.

## Task 1: Store Scoped Claim Retrieval

**Files:**

- Modify: `crates/noema-core/src/store/retrieval.rs`
- Test: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Add failing store tests for scoped empty queries and AND semantics**

Add these tests after `retrieval_limit_zero_returns_empty_result` in `crates/noema-core/src/store/tests/claims.rs`:

```rust
#[tokio::test]
async fn scoped_empty_query_returns_active_claims_for_scope() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes planes.").await;
    let alex_item = create_source_item(&store, "Alex likes planes.").await;
    let train_summary = store
        .create_or_reinforce_claim({
            let mut claim = train_claim(train_item.item_id);
            claim.object = EntityCandidate::concept("planes", "planes");
            claim.fact = "Kevin likes planes.".to_string();
            claim.retrieval_hints = json!({ "keywords": ["planes", "aviation"] });
            claim
        })
        .await
        .expect("create local human claim");
    store
        .create_or_reinforce_claim(person_train_claim(
            "person:alex",
            "Alex",
            alex_item.item_id,
            "Alex likes planes.",
        ))
        .await
        .expect("create other person claim");

    let result = store
        .retrieve_claims_scoped(
            &personalize_request(),
            "",
            &["human:local".to_string()],
            8,
        )
        .await
        .expect("retrieve scoped memories");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, train_summary.claim_id);
    assert_eq!(result.included[0].fact, "Kevin likes planes.");
}

#[tokio::test]
async fn scoped_query_narrows_inside_requested_scope() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let coffee_item = create_source_item(&store, "Kevin prefers coffee.").await;
    let train_summary = store
        .create_or_reinforce_claim(train_claim(train_item.item_id))
        .await
        .expect("create train claim");
    store
        .create_or_reinforce_claim({
            let mut claim = train_claim(coffee_item.item_id);
            claim.object = EntityCandidate::concept("coffee", "coffee");
            claim.predicate_id = "prefers".to_string();
            claim.fact = "Kevin prefers coffee.".to_string();
            claim.retrieval_hints = json!({ "keywords": ["coffee"] });
            claim
        })
        .await
        .expect("create coffee claim");

    let result = store
        .retrieve_claims_scoped(
            &personalize_request(),
            "trains",
            &["human:local".to_string()],
            8,
        )
        .await
        .expect("retrieve scoped topical memories");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, train_summary.claim_id);
}

#[tokio::test]
async fn query_only_retrieval_still_uses_existing_api() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    let summary = store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create claim");

    let result = store
        .retrieve_claims(&personalize_request(), "trains", 8)
        .await
        .expect("retrieve query-only memories");

    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, summary.claim_id);
}
```

- [ ] **Step 2: Run the failing store tests**

Run:

```bash
cargo test -p noema-core scoped_empty_query_returns_active_claims_for_scope
cargo test -p noema-core scoped_query_narrows_inside_requested_scope
cargo test -p noema-core query_only_retrieval_still_uses_existing_api
```

Expected: FAIL because `NoemaStore::retrieve_claims_scoped` does not exist.

- [ ] **Step 3: Add the scoped retrieval API**

In `crates/noema-core/src/store/retrieval.rs`, add `HashMap` and `HashSet` imports:

```rust
use std::collections::{HashMap, HashSet};
```

Replace the body of `retrieve_claims` with a delegation:

```rust
pub async fn retrieve_claims(
    &self,
    request: &ClaimRetrievalRequest,
    query_text: &str,
    limit: usize,
) -> Result<ClaimRetrievalResult, StoreError> {
    self.retrieve_claims_scoped(request, query_text, &[], limit)
        .await
}
```

Add the new method next to `retrieve_claims`:

```rust
pub async fn retrieve_claims_scoped(
    &self,
    request: &ClaimRetrievalRequest,
    query_text: &str,
    scope_ids: &[String],
    limit: usize,
) -> Result<ClaimRetrievalResult, StoreError> {
    if limit == 0 {
        return Ok(ClaimRetrievalResult::default());
    }

    let mut response = self
        .db
        .query(
            r#"
            SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact, status, sensitivity, retrieval_hints
            FROM claims
            WHERE status IN ['active', 'confirmed']
            ORDER BY claim_id ASC;
            "#,
        )
        .await?;
    let rows: Vec<ClaimRetrievalRow> = response.take(0)?;
    let entity_names = self.retrieval_entity_names(&rows).await?;
    let query = QueryTerms::from_query(query_text);
    let scope_set = scope_ids.iter().cloned().collect::<HashSet<_>>();
    let mut result = ClaimRetrievalResult::default();
    let mut included = Vec::new();

    for row in rows {
        if !scope_matches_claim(&row, &scope_set) {
            continue;
        }

        let predicate = self.predicate_policy(&row.predicate_id).await?;
        let Some(match_score) = deterministic_match_score(&row, &predicate, &entity_names, &query) else {
            continue;
        };
        let object_entity_id = row.object_entity_id.clone().ok_or_else(|| {
            StoreError::Schema(format!("claim missing object_entity_id: {}", row.claim_id))
        })?;
        let policy_claim = PolicyClaim {
            claim_id: row.claim_id.clone(),
            subject_entity_id: row.subject_entity_id.clone(),
            object_entity_id,
            predicate_allowed_use_modes: predicate
                .allowed_use_modes
                .iter()
                .map(|mode| parse_use_mode(mode))
                .collect::<Result<Vec<_>, _>>()?,
            status: parse_claim_status(&row.status)?,
            sensitivity: parse_sensitivity(&row.sensitivity)?,
        };

        if claim_policy_allows(&policy_claim, request).is_ok() {
            included.push(RetrievedClaim {
                claim_id: row.claim_id,
                fact: row.fact,
                predicate_id: row.predicate_id,
                rank_score: match_score,
            });
        } else {
            result.redacted_omission_count += 1;
        }
    }

    included.sort_by(|left, right| {
        right
            .rank_score
            .cmp(&left.rank_score)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    included.truncate(limit);
    result.included = included;

    Ok(result)
}
```

- [ ] **Step 4: Add retrieval helpers for scope, predicate labels, entity names, and tokenized query matching**

Extend `PredicatePolicyRow`:

```rust
struct PredicatePolicyRow {
    predicate_id: String,
    label: String,
    allowed_use_modes: Vec<String>,
}
```

Update `predicate_policy` query:

```sql
SELECT predicate_id, label, allowed_use_modes
FROM predicates
WHERE predicate_id = $predicate_id
LIMIT 1;
```

Add these helpers below the row structs:

```rust
#[derive(Debug, Deserialize, SurrealValue)]
struct RetrievalEntityNameRow {
    entity_id: String,
    canonical_name: String,
}

struct QueryTerms {
    raw: String,
    terms: Vec<String>,
}

impl QueryTerms {
    fn from_query(query: &str) -> Self {
        let raw = query.trim().to_lowercase();
        let terms = raw
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .map(str::trim)
            .filter(|term| term.len() > 1)
            .map(ToString::to_string)
            .collect();
        Self { raw, terms }
    }

    fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

fn scope_matches_claim(row: &ClaimRetrievalRow, scope_ids: &HashSet<String>) -> bool {
    if scope_ids.is_empty() {
        return true;
    }
    scope_ids.contains(&row.subject_entity_id)
        || row
            .object_entity_id
            .as_ref()
            .is_some_and(|object_entity_id| scope_ids.contains(object_entity_id))
}

fn deterministic_match_score(
    row: &ClaimRetrievalRow,
    predicate: &PredicatePolicyRow,
    entity_names: &HashMap<String, String>,
    query: &QueryTerms,
) -> Option<i64> {
    if query.is_empty() {
        return Some(50);
    }

    let mut haystack = vec![
        row.fact.as_str(),
        row.predicate_id.as_str(),
        predicate.label.as_str(),
    ];
    if let Some(subject_name) = entity_names.get(&row.subject_entity_id) {
        haystack.push(subject_name.as_str());
    }
    if let Some(object_entity_id) = row.object_entity_id.as_ref() {
        if let Some(object_name) = entity_names.get(object_entity_id) {
            haystack.push(object_name.as_str());
        }
    }
    let hints = row.retrieval_hints.to_string();
    haystack.push(hints.as_str());
    let joined = haystack.join(" ").to_lowercase();

    if joined.contains(&query.raw) {
        return Some(100);
    }

    let matched_terms = query
        .terms
        .iter()
        .filter(|term| joined.contains(term.as_str()))
        .count();
    (matched_terms > 0).then_some(50 + i64::try_from(matched_terms).unwrap_or(0))
}
```

Add the entity-name preload method inside the `impl NoemaStore` block:

```rust
async fn retrieval_entity_names(
    &self,
    rows: &[ClaimRetrievalRow],
) -> Result<HashMap<String, String>, StoreError> {
    let mut entity_ids = rows
        .iter()
        .flat_map(|row| {
            row.object_entity_id
                .iter()
                .chain(std::iter::once(&row.subject_entity_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    entity_ids.sort();
    entity_ids.dedup();
    if entity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut response = self
        .db
        .query("SELECT entity_id, canonical_name FROM entities WHERE entity_id IN $entity_ids;")
        .bind(("entity_ids", entity_ids))
        .await?;
    let rows: Vec<RetrievalEntityNameRow> = response.take(0)?;
    Ok(rows
        .into_iter()
        .map(|row| (row.entity_id, row.canonical_name))
        .collect())
}
```

- [ ] **Step 5: Run store validation and commit**

Run:

```bash
cargo test -p noema-core scoped_empty_query_returns_active_claims_for_scope
cargo test -p noema-core scoped_query_narrows_inside_requested_scope
cargo test -p noema-core query_only_retrieval_still_uses_existing_api
cargo test -p noema-core retrieval_respects_use_mode_predicate_policy
git status --short --branch
git diff --check
git add crates/noema-core/src/store/retrieval.rs crates/noema-core/src/store/tests/claims.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add scoped memory claim retrieval"
```

Expected: targeted tests PASS; only the two store files are staged; unrelated `attachment.tsx` remains unstaged.

## Task 2: Extend `search_memory` Tool Contract And Prompt Guidance

**Files:**

- Modify: `crates/noema-core/src/daemon/memory_tool.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing memory tool unit tests**

In `crates/noema-core/src/daemon/memory_tool.rs`, add tests in the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn parses_empty_query_when_scope_ids_are_present() {
    let payload = json!({
        "arguments": {
            "scope_ids": ["human:local"],
            "query": "",
            "purpose": "answer_human_question"
        }
    });

    let arguments = parse_arguments(&payload).expect("parse scoped empty query");

    assert_eq!(arguments.query, "");
    assert_eq!(arguments.scope_ids, vec!["human:local"]);
}

#[test]
fn rejects_empty_query_without_scope_ids() {
    let payload = json!({
        "arguments": {
            "query": "   "
        }
    });

    let error = parse_arguments(&payload).expect_err("empty unscoped query rejected");

    assert_eq!(
        safe_error_message(&error),
        "query is required unless scope_ids is non-empty"
    );
}

#[test]
fn validates_scope_ids_against_trusted_active_ids() {
    let context = MemoryToolRuntimeContext {
        conversation_id: "conv_123".to_string(),
        turn_id: "turn_456".to_string(),
        turn_index: 7,
        call_site_id: "output_0".to_string(),
        cwd: Some("/Users/kpsuperplane/Documents/Projects/Noema".to_string()),
        user_input: "What memories do you have of me?".to_string(),
    };
    let arguments = SearchMemoryArguments {
        query: "".to_string(),
        scope_ids: vec!["human:local".to_string()],
        purpose: Some("answer_human_question".to_string()),
        limit: None,
    };

    validate_scope_ids(&context, &arguments).expect("trusted scope id");
}

#[test]
fn rejects_out_of_context_scope_ids() {
    let context = MemoryToolRuntimeContext {
        conversation_id: "conv_123".to_string(),
        turn_id: "turn_456".to_string(),
        turn_index: 7,
        call_site_id: "output_0".to_string(),
        cwd: None,
        user_input: "What memories do you have of me?".to_string(),
    };
    let arguments = SearchMemoryArguments {
        query: "".to_string(),
        scope_ids: vec!["project:other".to_string()],
        purpose: Some("answer_human_question".to_string()),
        limit: None,
    };

    let error = validate_scope_ids(&context, &arguments).expect_err("invalid scope id");

    assert_eq!(
        safe_error_message(&error),
        "unsupported scope_id: project:other"
    );
}
```

- [ ] **Step 2: Run failing tool tests**

Run:

```bash
cargo test -p noema-core parses_empty_query_when_scope_ids_are_present
cargo test -p noema-core rejects_empty_query_without_scope_ids
cargo test -p noema-core validates_scope_ids_against_trusted_active_ids
cargo test -p noema-core rejects_out_of_context_scope_ids
```

Expected: FAIL because `scope_ids`, `validate_scope_ids`, and the new error message do not exist.

- [ ] **Step 3: Implement parser and trusted scope validation**

Update `SearchMemoryArguments`:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchMemoryArguments {
    query: String,
    #[serde(default)]
    scope_ids: Vec<String>,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}
```

Update `parse_arguments`:

```rust
if arguments.query.trim().is_empty() && arguments.scope_ids.is_empty() {
    return Err(MemoryToolError::InvalidArguments(
        "query is required unless scope_ids is non-empty".to_string(),
    ));
}
```

Add helpers:

```rust
fn trusted_active_scope_ids(context: &MemoryToolRuntimeContext) -> Vec<String> {
    let mut ids = vec![
        "human:local".to_string(),
        format!("conversation:{}", context.conversation_id),
    ];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        ids.push(project_scope);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn validate_scope_ids(
    context: &MemoryToolRuntimeContext,
    arguments: &SearchMemoryArguments,
) -> Result<(), MemoryToolError> {
    let trusted = trusted_active_scope_ids(context)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    for scope_id in &arguments.scope_ids {
        if !trusted.contains(scope_id) {
            return Err(MemoryToolError::InvalidArguments(format!(
                "unsupported scope_id: {scope_id}"
            )));
        }
    }
    Ok(())
}
```

Call `validate_scope_ids(context, &arguments)?;` in `execute_search_memory_inner` after parsing and before `build_request`.

- [ ] **Step 4: Call scoped store retrieval and echo scope metadata**

Replace the retrieval call:

```rust
let retrieval = store
    .retrieve_claims_scoped(
        &request,
        arguments.query.trim(),
        &arguments.scope_ids,
        arguments.limit(),
    )
    .await?;
```

Add `scope_ids` to the successful tool payload:

```rust
Ok(json!({
    "memories": memories,
    "omissions": omissions,
    "context_packet_id": context_packet_id,
    "scope_ids": arguments.scope_ids,
}))
```

- [ ] **Step 5: Add prompt guidance test in runtime**

In `crates/noema-core/src/daemon/runtime.rs`, add a local test module near the existing `build_structured_turn_system_prompt_includes_transcript_context` test:

```rust
#[test]
fn structured_turn_prompt_exposes_active_retrieval_ids_and_scope_guidance() {
    let prompt = build_structured_turn_system_prompt(
        "conv_123",
        4,
        Some("/Users/kpsuperplane/Documents/Projects/Noema"),
        "",
    );

    assert!(prompt.contains("Active retrieval IDs:"));
    assert!(prompt.contains("- human:local"));
    assert!(prompt.contains("- conversation:conv_123"));
    assert!(prompt.contains("\"scope_ids\":[\"human:local\"],\"query\":\"\""));
    assert!(prompt.contains("Never invent scope IDs"));
}
```

Then update `build_structured_turn_system_prompt` to include:

```text
Active retrieval IDs:
- human:local
- conversation:{conversation_id}
- {project_hint when not none}

Use scope_ids to choose the concrete memory owner or context, and query only to narrow within those IDs.
For broad questions about what Noema remembers about the user, call search_memory with "scope_ids":["human:local"] and "query":"".
For topical questions about the user, keep "scope_ids":["human:local"] and use a concise topic query such as "aviation" or "planes".
Never invent scope IDs. Use only IDs listed in Active retrieval IDs or returned by prior Noema tools.
Do not tell the user Noema has no memories unless the scoped tool result is empty for the scope actually being discussed.
```

Update the tool call shape in the prompt to:

```json
{"kind":"tool_call","id":"call_memory_1","name":"search_memory","payload":{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}
```

- [ ] **Step 6: Add a runtime continuation test for broad profile retrieval**

In `crates/noema-core/src/daemon/tests.rs`:

1. Add enum variant `SearchMemoryProfileContinuation`.
2. In `FakeCodexProvider::generate_response`, add a branch:

```rust
FakeCodexScenario::SearchMemoryProfileContinuation => {
    if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
        vec![
            GenerateOutputItem::AssistantText {
                text: "I remember that you like planes.".to_string(),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ]
    } else if input.contains("What memories do you have of me?") {
        vec![
            GenerateOutputItem::AssistantText {
                text: "Searching memory.".to_string(),
            },
            search_memory_tool_call(
                "call_profile",
                json!({"arguments": {
                    "scope_ids": ["human:local"],
                    "query": "",
                    "purpose": "answer_human_question",
                    "limit": 8
                }}),
            ),
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ]
    } else {
        assistant_with_no_memories("fake answer")
    }
}
```

3. Add a test that seeds an explicit memory, asks "What memories do you have of me?", and asserts the persisted local tool result contains `"Kevin likes planes."` with `success: true`.

- [ ] **Step 7: Run daemon validation and commit**

Run:

```bash
cargo test -p noema-core parses_empty_query_when_scope_ids_are_present
cargo test -p noema-core rejects_empty_query_without_scope_ids
cargo test -p noema-core validates_scope_ids_against_trusted_active_ids
cargo test -p noema-core rejects_out_of_context_scope_ids
cargo test -p noema-core structured_turn_prompt_exposes_active_retrieval_ids_and_scope_guidance
cargo test -p noema-core search_memory_profile_continuation_uses_scoped_empty_query
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: support scoped memory search"
```

Expected: targeted daemon tests PASS; only daemon files are staged; unrelated `attachment.tsx` remains unstaged.

## Task 3: Add Canonical Claim Outcome Metadata To Memory Writes

**Files:**

- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing tests for provider and explicit claim outcomes**

Extend `runtime_actor_persists_provider_memory_proposals_as_graph_claims` by finding the completed `memory_extraction` activity metadata and asserting:

```rust
let activity_metadata = items
    .iter()
    .find_map(|item| match item {
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "memory_extraction" && title == "Memory saved" => Some(metadata),
        _ => None,
    })
    .expect("provider memory activity metadata");
assert_eq!(
    activity_metadata["claim_outcomes"][0]["fact_preview"],
    json!("Kevin prefers same-call memory proposals.")
);
assert_eq!(
    activity_metadata["claim_outcomes"][0]["outcome"],
    json!("created")
);
assert_eq!(
    activity_metadata["claim_outcomes"][0]["sensitivity"],
    json!("normal")
);
```

Add a new runtime test named `explicit_memory_saved_activity_includes_claim_outcome` near `repeated_explicit_memory_reinforces_one_claim`:

```rust
#[tokio::test]
async fn explicit_memory_saved_activity_includes_claim_outcome() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");

    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "remember: I like planes.".to_string(),
    )
    .await
    .expect("turn");

    let metadata = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction" && title == "Explicit memory saved" => {
                Some(metadata)
            }
            _ => None,
        })
        .expect("explicit memory activity metadata");

    assert_eq!(
        metadata["claim_outcomes"][0]["fact_preview"],
        json!("Kevin likes planes.")
    );
    assert_eq!(metadata["claim_outcomes"][0]["outcome"], json!("created"));
    assert_eq!(metadata["claim_outcomes"][0]["sensitivity"], json!("normal"));
    handle.shutdown().await;
}
```

The new test should fail before implementation because `claim_outcomes` is absent.

Also extend the existing reinforced explicit memory test to assert reinforced metadata:

```rust
assert_eq!(
    claim_activity["claim_outcomes"][0]["outcome"],
    json!("reinforced")
);
assert_eq!(
    claim_activity["claim_outcomes"][0]["fact_preview"],
    json!("Kevin likes ice cream.")
);
```

- [ ] **Step 2: Run failing outcome tests**

Run:

```bash
cargo test -p noema-core runtime_actor_persists_provider_memory_proposals_as_graph_claims
cargo test -p noema-core explicit_memory_saved_activity_includes_claim_outcome
```

Expected: FAIL because `claim_outcomes` does not exist in activity metadata.

- [ ] **Step 3: Add outcome serialization helpers**

In `crates/noema-core/src/daemon/runtime.rs`, add helpers near `provider_memory_claim_activity`:

```rust
fn claim_outcome_json(summary: &crate::ClaimSummary) -> Value {
    json!({
        "claim_id": summary.claim_id,
        "outcome": claim_write_outcome_label(summary.write_outcome),
        "fact_preview": fact_preview(&summary.fact),
        "sensitivity": sensitivity_label(summary.sensitivity),
    })
}

fn claim_write_outcome_label(outcome: ClaimWriteOutcome) -> &'static str {
    match outcome {
        ClaimWriteOutcome::Created => "created",
        ClaimWriteOutcome::Reinforced => "reinforced",
    }
}

fn fact_preview(fact: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;
    let trimmed = fact.trim();
    if trimmed.chars().count() <= MAX_PREVIEW_CHARS {
        return trimmed.to_string();
    }
    let preview = trimmed.chars().take(MAX_PREVIEW_CHARS - 3).collect::<String>();
    format!("{preview}...")
}
```

- [ ] **Step 4: Include outcomes in provider proposal metadata**

In `persist_provider_memory_proposals`, introduce:

```rust
let mut claim_outcomes = Vec::with_capacity(proposal_count);
```

When a write succeeds:

```rust
claim_outcomes.push(claim_outcome_json(&summary));
claim_ids.push(summary.claim_id);
```

Add the metadata field:

```rust
"claim_outcomes": claim_outcomes,
```

- [ ] **Step 5: Include outcomes in explicit memory metadata**

In `persist_explicit_memory_claim`, add:

```rust
let claim_outcome = claim_outcome_json(&summary);
```

Then include:

```rust
"claim_outcomes": [claim_outcome],
"created_claim_count": if summary.write_outcome == ClaimWriteOutcome::Created { 1 } else { 0 },
"reinforced_claim_count": if summary.write_outcome == ClaimWriteOutcome::Reinforced { 1 } else { 0 },
"failed_proposal_count": 0,
```

- [ ] **Step 6: Run backend validation and commit**

Run:

```bash
cargo test -p noema-core runtime_actor_persists_provider_memory_proposals_as_graph_claims
cargo test -p noema-core explicit_memory_saved_activity_includes_claim_outcome
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: expose memory claim outcome previews"
```

Expected: outcome tests PASS; only runtime and daemon tests are staged; unrelated `attachment.tsx` remains unstaged.

## Task 4: Render Specific Memory Marker Labels

**Files:**

- Modify: `crates/noema-core/web/src/components/Transcript.tsx`
- Test: `crates/noema-core/web/src/components/Transcript.test.ts`

- [ ] **Step 1: Add failing transcript tests**

In `crates/noema-core/web/src/components/Transcript.test.ts`, add tests that use `memoryExtractionEntry` with override metadata:

```ts
test("renders a saved memory marker with a single fact preview", () => {
  const entries: TranscriptEntry[] = [
    memoryExtractionEntry("activity-done", "item-done", "COMPLETED", "Memory saved", {
      claim_outcomes: [
        {
          claim_id: "claim:planes",
          outcome: "created",
          fact_preview: "User likes planes",
          sensitivity: "normal"
        }
      ],
      created_claim_count: 1,
      reinforced_claim_count: 0,
      failed_proposal_count: 0
    })
  ];

  const { getByText } = render(<Transcript entries={entries} pendingTurn={false} />);

  assert.ok(getByText("Memory saved: User likes planes"));
});

test("renders an updated memory marker with a reinforced fact preview", () => {
  const entries: TranscriptEntry[] = [
    memoryExtractionEntry("activity-done", "item-done", "COMPLETED", "Memory updated", {
      claim_outcomes: [
        {
          claim_id: "claim:planes",
          outcome: "reinforced",
          fact_preview: "User likes planes",
          sensitivity: "normal"
        }
      ],
      created_claim_count: 0,
      reinforced_claim_count: 1,
      failed_proposal_count: 0
    })
  ];

  const { getByText } = render(<Transcript entries={entries} pendingTurn={false} />);

  assert.ok(getByText("Memory updated: User likes planes"));
});

test("renders count marker for multiple memory outcomes", () => {
  const entries: TranscriptEntry[] = [
    memoryExtractionEntry("activity-done", "item-done", "COMPLETED", "Memory saved", {
      claim_outcomes: [
        { claim_id: "claim:planes", outcome: "created", fact_preview: "User likes planes" },
        { claim_id: "claim:trains", outcome: "created", fact_preview: "User likes trains" }
      ],
      created_claim_count: 2,
      reinforced_claim_count: 0,
      failed_proposal_count: 0
    })
  ];

  const { getByText } = render(<Transcript entries={entries} pendingTurn={false} />);

  assert.ok(getByText("Memory saved: 2 memories"));
});

test("renders partial failure marker for memory outcomes", () => {
  const entries: TranscriptEntry[] = [
    memoryExtractionEntry("activity-done", "item-done", "COMPLETED", "Memory saved", {
      claim_outcomes: [
        { claim_id: "claim:planes", outcome: "created", fact_preview: "User likes planes" }
      ],
      created_claim_count: 1,
      reinforced_claim_count: 0,
      failed_proposal_count: 1
    })
  ];

  const { getByText } = render(<Transcript entries={entries} pendingTurn={false} />);

  assert.ok(getByText("Memory saved: 1 memory; 1 failed"));
});
```

Change `memoryExtractionEntry` to accept an optional metadata override:

```ts
function memoryExtractionEntry(
  id: string,
  itemId: string,
  status: "STARTED" | "COMPLETED",
  title: string,
  metadata: Record<string, unknown> = { turn_index: 1, proposal_count: 1 }
): TranscriptEntry {
  ...
  metadata
  ...
}
```

- [ ] **Step 2: Run failing transcript tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
```

Expected: FAIL because the marker still renders generic `Memory updated`.

- [ ] **Step 3: Add metadata parsing helpers**

In `crates/noema-core/web/src/components/Transcript.tsx`, add:

```ts
type MemoryClaimOutcome = {
  claimId: string;
  outcome: "created" | "reinforced";
  factPreview?: string;
  sensitivity?: string;
};

function memoryClaimOutcomes(metadata: unknown): MemoryClaimOutcome[] {
  if (!metadata || typeof metadata !== "object") {
    return [];
  }
  const outcomes = (metadata as { claim_outcomes?: unknown }).claim_outcomes;
  if (!Array.isArray(outcomes)) {
    return [];
  }
  return outcomes.flatMap((outcome): MemoryClaimOutcome[] => {
    if (!outcome || typeof outcome !== "object") {
      return [];
    }
    const record = outcome as Record<string, unknown>;
    const claimId = typeof record.claim_id === "string" ? record.claim_id : "";
    const rawOutcome = record.outcome;
    if (!claimId || (rawOutcome !== "created" && rawOutcome !== "reinforced")) {
      return [];
    }
    return [
      {
        claimId,
        outcome: rawOutcome,
        factPreview: typeof record.fact_preview === "string" ? record.fact_preview : undefined,
        sensitivity: typeof record.sensitivity === "string" ? record.sensitivity : undefined
      }
    ];
  });
}

function metadataCount(metadata: unknown, key: string): number {
  if (!metadata || typeof metadata !== "object") {
    return 0;
  }
  const value = (metadata as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0;
}
```

- [ ] **Step 4: Render outcome-based marker labels**

Add:

```ts
function memoryMarkerLabel(extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>): string {
  if (!extraction) {
    return "Memory updated";
  }
  if (extraction.status === "FAILED") {
    return "Memory update failed";
  }
  if (extraction.status === "STARTED") {
    return "Memory proposed";
  }

  const outcomes = memoryClaimOutcomes(extraction.metadata);
  const failedCount = metadataCount(extraction.metadata, "failed_proposal_count");
  if (outcomes.length === 0) {
    return failedCount > 0 ? "Memory update failed" : "Memory updated";
  }
  if (outcomes.length === 1 && failedCount === 0) {
    const outcome = outcomes[0];
    const verb = outcome.outcome === "reinforced" ? "Memory updated" : "Memory saved";
    return outcome.factPreview ? `${verb}: ${outcome.factPreview}` : verb;
  }

  const createdCount = metadataCount(extraction.metadata, "created_claim_count");
  const reinforcedCount = metadataCount(extraction.metadata, "reinforced_claim_count");
  const savedCount = createdCount + reinforcedCount || outcomes.length;
  const noun = savedCount === 1 ? "memory" : "memories";
  const prefix = createdCount > 0 ? "Memory saved" : "Memory updated";
  const failureSuffix = failedCount > 0 ? `; ${failedCount} failed` : "";
  return `${prefix}: ${savedCount} ${noun}${failureSuffix}`;
}
```

Then replace:

```ts
const label = failed ? "Memory update failed" : started ? "Memory proposed" : "Memory updated";
```

with:

```ts
const label = memoryMarkerLabel(extraction);
```

- [ ] **Step 5: Populate expanded details from claim outcomes when there is no proposal card**

Add:

```ts
function memoryCardsFromClaimOutcomes(
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>
): MemoryCardData[] {
  return memoryClaimOutcomes(extraction?.metadata).map((outcome) => ({
    id: outcome.claimId,
    title: outcome.factPreview ?? outcome.claimId,
    content: outcome.factPreview ?? outcome.claimId,
    status: outcome.outcome,
    sensitivity: outcome.sensitivity
  }));
}
```

Update `MemoryMarker`:

```ts
const memories = proposal ? memoryCardsFromStructuredItem(proposal) ?? [] : memoryCardsFromClaimOutcomes(extraction);
```

- [ ] **Step 6: Run frontend validation and commit**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
bun run lint
bun run build
cd ../../..
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/Transcript.tsx crates/noema-core/web/src/components/Transcript.test.ts
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: show saved memory fact previews"
```

Expected: Transcript tests, lint, and build PASS; only transcript files are staged; unrelated `attachment.tsx` remains unstaged unless the user separately asks to include it.

## Task 5: Full Validation And Durable Context

**Files:**

- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Add a concise settled decision bullet to `docs/context/current.md`:

```markdown
- The local `search_memory` tool supports validated concrete `scope_ids`.
  Empty `query` is allowed only for scoped reads, and `query` narrows within
  scope rather than broadening it. Memory write activities expose canonical
  claim outcome previews so chat markers can name the saved or reinforced fact.
```

- [ ] **Step 2: Run complete validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cd crates/noema-core/web
bun run gen:types
bun test src/components/Transcript.test.ts
bun run lint
bun run build
cd ../../..
git status --short --branch
git diff --check
```

Expected: all commands PASS. If a Noema daemon or provider test fails only because the sandbox blocks local Unix/TCP sockets, rerun the same failing Rust test command with socket permissions and report that distinction.

- [ ] **Step 3: Commit context update**

Run:

```bash
git add docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "docs: record scoped memory search behavior"
```

Expected: only `docs/context/current.md` is staged.

- [ ] **Step 4: Final implementation report**

Report:

- The scoped search contract and validation behavior.
- The store retrieval semantics: policy AND scope AND optional query.
- The marker rendering behavior and fallback behavior.
- The exact validation commands run.
- Any remaining untracked or unstaged files, including the pre-existing `attachment.tsx` dirty file if still present.

## Adversarial Review Checklist

Before considering implementation complete, verify:

- Empty `query` without `scope_ids` fails before store retrieval.
- Unknown `scope_ids` fail and never fall back to broad retrieval.
- `scope_ids` are concrete IDs, not aliases such as `current_human`.
- `query` and `scope_ids` are ANDed in store retrieval.
- Policy redactions remain successful retrieval results with generic omission counts.
- The prompt teaches the agent to use `scope_ids: ["human:local"]` and `query: ""` for broad user-memory questions.
- `fact_preview` comes from persisted `ClaimSummary.fact`, not raw model proposal content.
- Transcript markers still render sensible labels for legacy metadata without `claim_outcomes`.
- No unrelated dirty files are staged.
