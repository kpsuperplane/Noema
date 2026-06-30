# One-Call Memory Write Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse provider ordinary-chat memory persistence from two post-proposal model calls into one post-proposal model call before graph memory is updated.

**Architecture:** Keep memory extraction as-is: the assistant turn still emits draft memory proposals. After a proposal validates locally, Noema performs a broad bounded local claim search, then sends one provider prompt containing the write proposal, promoted predicate catalog, and candidate existing claims; the provider returns both the canonical claim and consolidation decision in one strict JSON response. Noema validates all promoted predicates and referenced claim IDs before writing graph memory.

**Tech Stack:** Rust, Tokio, serde/serde_json, SurrealDB embedded store, existing Noema daemon runtime tests.

---

## Scope

This plan changes only the backend memory write pipeline after a provider has already proposed memory. It does not change the initial assistant response format, web rendering, GraphQL schema, CLI commands, or explicit `remember:` writes.

## File Structure

- Modify `crates/noema-core/src/memory/consolidation.rs`
  - Add a unified one-call provider response type.
  - Add parser/validator for the unified response.
  - Add a prompt builder that includes the memory proposal, promoted predicates, and bounded existing claim candidates.
  - Keep existing canonicalization and consolidation types until all callers/tests are migrated.

- Modify `crates/noema-core/src/store/claims/model.rs`
  - Add a pre-canonicalization memory write match request type for broad bounded candidate search.

- Modify `crates/noema-core/src/store/claims/consolidation.rs`
  - Add `NoemaStore::find_memory_write_matches`.
  - Reuse `ConsolidationMatch` as the returned projection so runtime code can validate decisions with existing helpers.

- Modify `crates/noema-core/src/store/tests/claims.rs`
  - Add store tests proving broad matching returns bounded live same-subject candidates by query terms and excludes sensitive claims above the proposal sensitivity.

- Modify `crates/noema-core/src/daemon/runtime/memory_writes.rs`
  - Replace the `canonicalize_memory_write` + optional `build_consolidation_prompt` provider call chain with one `resolve_memory_write` call.
  - Persist decisions with the existing `persist_consolidation_decision` path.
  - Preserve deterministic fallback behavior when the provider returns no decisions.

- Modify `crates/noema-core/src/daemon/tests.rs`
  - Add fake-provider request logging.
  - Add a regression test that an ordinary memory reinforcement uses exactly one post-proposal provider resolution call and zero old comparator calls.
  - Update fake provider response routing for the new resolver prompt.

## Task 1: Unified Memory Write Resolution Contract

**Files:**
- Modify: `crates/noema-core/src/memory/consolidation.rs`

- [ ] **Step 1: Write failing parser and prompt tests**

Add these types to the existing `#[cfg(test)] mod tests` section after the helper named `canonicalization_json`:

```rust
fn memory_write_resolution_json(
    candidate: CanonicalClaimCandidate,
    decision: ConsolidationDecisionKind,
    existing_claim_id: Option<&str>,
) -> String {
    serde_json::json!({
        "decisions": [{
            "candidate": candidate,
            "decision": decision,
            "existing_claim_id": existing_claim_id,
            "confidence": 0.91,
            "rationale": "The candidate should be persisted with this consolidation action."
        }]
    })
    .to_string()
}
```

Add these tests in the same test module:

```rust
#[test]
fn memory_write_resolution_prompt_combines_proposal_predicates_and_existing_claims() {
    let promoted_predicates = serde_json::json!([{
        "predicate_id": "likes",
        "label": "likes",
        "allowed_subject_types": ["human"],
        "allowed_object_types": ["concept"],
        "merge_hints": {"equivalent_phrases": ["adore", "love"]}
    }]);
    let existing = serde_json::json!([{
        "claim_id": "claim:likes-trains",
        "subject_entity_id": "human:local",
        "object_entity_id": "concept:trains",
        "predicate_id": "likes",
        "fact": "Kevin likes trains.",
        "status": "active",
        "sensitivity": "normal",
        "confidence": 0.88
    }]);

    let prompt = build_memory_write_resolution_prompt(
        &proposal_fixture(),
        &promoted_predicates,
        &existing,
    );

    assert!(prompt.contains("You are Noema's memory write resolver."));
    assert!(prompt.contains("Return strict JSON only."));
    assert!(prompt.contains(r#""decisions": ["#));
    assert!(prompt.contains(r#""candidate""#));
    assert!(prompt.contains(r#""decision": "create""#));
    assert!(prompt.contains("Use reinforce, supersede, dispute, or relate only with an existing_claim_id from existing_claim_candidates."));
    assert!(prompt.contains("Promoted predicate catalog JSON:"));
    assert!(prompt.contains("\"predicate_id\": \"likes\""));
    assert!(prompt.contains("Existing claim candidates JSON:"));
    assert!(prompt.contains("\"claim_id\": \"claim:likes-trains\""));
    assert!(prompt.contains("Input memory write proposal JSON:"));
    assert!(prompt.contains("\"raw_text\": \"I adore trains.\""));
}

#[test]
fn parses_memory_write_resolution_response() {
    let json = memory_write_resolution_json(
        candidate_fixture(),
        ConsolidationDecisionKind::Reinforce,
        Some("claim:likes-trains"),
    );

    let parsed = parse_memory_write_resolution_response(&json).expect("resolution");

    assert_eq!(parsed.decisions.len(), 1);
    assert_eq!(parsed.decisions[0].candidate.fact, "Kevin likes trains.");
    assert_eq!(parsed.decisions[0].decision.decision, ConsolidationDecisionKind::Reinforce);
    assert_eq!(
        parsed.decisions[0].decision.existing_claim_id.as_deref(),
        Some("claim:likes-trains")
    );
}

#[test]
fn memory_write_resolution_allows_empty_decisions_for_deterministic_fallback() {
    let parsed = parse_memory_write_resolution_response(r#"{"decisions":[]}"#)
        .expect("empty decisions are allowed");

    assert!(parsed.decisions.is_empty());
}

#[test]
fn memory_write_resolution_rejects_reinforce_without_existing_claim_id() {
    let json = memory_write_resolution_json(
        candidate_fixture(),
        ConsolidationDecisionKind::Reinforce,
        None,
    );

    let error = parse_memory_write_resolution_response(&json).expect_err("missing claim id");

    assert!(
        error
            .to_string()
            .contains("existing_claim_id is required for this decision"),
        "{error}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core memory::consolidation::tests::memory_write_resolution -- --nocapture
```

Expected: compile failure naming missing `build_memory_write_resolution_prompt`, `parse_memory_write_resolution_response`, and response types.

- [ ] **Step 3: Add unified response types and parser**

In `crates/noema-core/src/memory/consolidation.rs`, after `pub struct ConsolidationDecision`, insert:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict provider response resolving memory writes in one model call.
pub struct MemoryWriteResolutionResponse {
    /// Canonical claim candidates paired with their consolidation decisions.
    pub decisions: Vec<ResolvedMemoryWriteDecision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One canonical memory candidate and the decision for how to persist it.
pub struct ResolvedMemoryWriteDecision {
    /// Canonical graph claim candidate.
    pub candidate: CanonicalClaimCandidate,
    /// Consolidation action to apply.
    pub decision: ConsolidationDecisionKind,
    /// Existing claim id for decisions that operate on a stored claim.
    pub existing_claim_id: Option<String>,
    /// Provider confidence between 0.0 and 1.0.
    pub confidence: f64,
    /// Provider rationale for the resolution.
    pub rationale: String,
}

impl ResolvedMemoryWriteDecision {
    /// Convert the inline decision fields into the existing decision type.
    #[must_use]
    pub fn consolidation_decision(&self) -> ConsolidationDecision {
        ConsolidationDecision {
            decision: self.decision,
            existing_claim_id: self.existing_claim_id.clone(),
            confidence: self.confidence,
            rationale: self.rationale.clone(),
        }
    }
}
```

After `parse_consolidation_decision`, add:

```rust
/// Parse and validate strict memory write resolution JSON.
///
/// # Errors
///
/// Returns an error when the text is not valid strict JSON for memory write
/// resolution, or when deterministic validation rejects a candidate or
/// decision.
pub fn parse_memory_write_resolution_response(
    text: &str,
) -> Result<MemoryWriteResolutionResponse, MemoryConsolidationError> {
    let mut response: MemoryWriteResolutionResponse = serde_json::from_str(text.trim())?;
    for item in &mut response.decisions {
        validate_entity(&mut item.candidate.subject, "subject")?;
        validate_entity(&mut item.candidate.object, "object")?;
        validate_predicate_resolution(&mut item.candidate.predicate)?;
        item.candidate.fact = item.candidate.fact.trim().to_string();
        item.candidate.rationale = item.candidate.rationale.trim().to_string();
        item.rationale = item.rationale.trim().to_string();
        if item.candidate.fact.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "fact must not be empty",
            ));
        }
        if !item.candidate.confidence.is_finite()
            || !(0.0..=1.0).contains(&item.candidate.confidence)
        {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "confidence must be between 0.0 and 1.0",
            ));
        }
        if item.candidate.rationale.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "rationale must not be empty",
            ));
        }
        if !item.confidence.is_finite() || !(0.0..=1.0).contains(&item.confidence) {
            return Err(MemoryConsolidationError::InvalidDecision(
                "confidence must be between 0.0 and 1.0",
            ));
        }
        if !matches!(
            item.decision,
            ConsolidationDecisionKind::Create | ConsolidationDecisionKind::NeedsReview
        ) && item
            .existing_claim_id
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return Err(MemoryConsolidationError::InvalidDecision(
                "existing_claim_id is required for this decision",
            ));
        }
        if item.rationale.is_empty() {
            return Err(MemoryConsolidationError::InvalidDecision(
                "rationale must not be empty",
            ));
        }
        item.existing_claim_id = item
            .existing_claim_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
    }
    Ok(response)
}
```

- [ ] **Step 4: Add unified prompt builder**

After `build_consolidation_prompt`, add:

```rust
/// Build the provider prompt for canonicalizing and consolidating one memory write.
#[must_use]
pub fn build_memory_write_resolution_prompt(
    proposal: &MemoryWriteProposal,
    promoted_predicates_json: &Value,
    existing_claim_candidates_json: &Value,
) -> String {
    let promoted_predicates =
        serde_json::to_string_pretty(promoted_predicates_json).expect("serialize predicates");
    let existing_claim_candidates = serde_json::to_string_pretty(existing_claim_candidates_json)
        .expect("serialize existing claim candidates");
    format!(
        r#"You are Noema's memory write resolver.

Return strict JSON only. Do not include Markdown, comments, or prose.

Required output JSON shape:
{{
  "decisions": [
    {{
      "candidate": {{
        "subject": {{
          "entity_id": "human:local",
          "entity_type": "human",
          "canonical_name": "Local human"
        }},
        "object": {{
          "entity_id": "concept:stable_object_id",
          "entity_type": "concept",
          "canonical_name": "stable object name"
        }},
        "predicate": {{
          "kind": "promoted_predicate",
          "predicate_id": "exact_catalog_predicate_id"
        }},
        "fact": "Local human has a durable relationship to stable object.",
        "sensitivity": "normal",
        "status": "active",
        "confidence": 0.9,
        "retrieval_hints": {{
          "keywords": ["stable object"],
          "summary": "Local human has a durable relationship to stable object."
        }},
        "rationale": "The source states a durable relationship."
      }},
      "decision": "create",
      "existing_claim_id": null,
      "confidence": 0.9,
      "rationale": "No existing claim candidate represents the same memory."
    }}
  ]
}}

The decisions array may be empty only when the input proposal should not become
a memory. Every non-empty item must include candidate, decision,
existing_claim_id, confidence, and rationale.

Candidate rules:
- Candidate fields are exactly subject, object, predicate, fact, sensitivity,
  status, confidence, retrieval_hints, and rationale.
- Entity fields are exactly entity_id, entity_type, and canonical_name.
- Sensitivity must be one of public, normal, private, sensitive, or secret.
- Status must be active, confirmed, or candidate. Use candidate when the memory
  needs review. Use active for memories that can be saved immediately.
- Use {{"kind": "promoted_predicate", "predicate_id": "exact_catalog_predicate_id"}} only when the
  predicate_id is copied exactly from the promoted predicate catalog below.
- Use {{"kind": "predicate_proposal", "proposal": {{"label": "collects"}}}} when no catalog
  predicate clearly fits. A predicate proposal must include label, description,
  allowed_subject_types, allowed_object_types, allowed_use_modes,
  default_sensitivity, conflict_policy, review_policy, inverse_behavior,
  inverse_predicate_id, proactivity_default, merge_hints, synonym_hints,
  extraction_hints, and rationale.
- Use {{"kind": "fallback_note"}} only for genuinely unstructured notes that do
  not express a durable relationship.

Decision rules:
- decision must be one of create, reinforce, supersede, dispute, relate, or
  needs_review.
- Use create when none of the existing claim candidates represents the same
  memory.
- Use reinforce when an existing claim candidate means the same thing and the
  new source adds support.
- Use dispute when the new memory conflicts with an existing claim candidate.
- Use supersede when the new memory should replace an existing claim candidate.
- Use relate only when the candidate is meaningfully related but not equivalent
  or conflicting.
- Use needs_review when uncertain.
- Use reinforce, supersede, dispute, or relate only with an existing_claim_id
  from existing_claim_candidates. Never invent an existing_claim_id.
- Use existing_claim_id null for create and needs_review.

Promoted predicate catalog JSON:
{promoted_predicates}

Existing claim candidates JSON:
{existing_claim_candidates}

Input memory write proposal JSON:
{}"#,
        serde_json::to_string_pretty(proposal).expect("serialize proposal")
    )
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test -p noema-core memory::consolidation::tests::memory_write_resolution -- --nocapture
```

Expected: all four new `memory_write_resolution_*` tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/memory/consolidation.rs
git commit -m "feat: add one-call memory write resolution contract"
```

Expected: commit succeeds with only `memory/consolidation.rs` staged.

## Task 2: Broad Pre-Canonicalization Claim Search

**Files:**
- Modify: `crates/noema-core/src/store/claims/model.rs`
- Modify: `crates/noema-core/src/store/claims/consolidation.rs`
- Test: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Write failing store tests**

In `crates/noema-core/src/store/tests/claims.rs`, add this test near the existing consolidation match search tests:

```rust
#[tokio::test]
async fn memory_write_match_search_finds_bounded_live_same_subject_claims_by_terms() {
    let store = test_store().await;
    let first_item = seed_source_item(&store, "conversation:match-terms", "first")
        .await
        .expect("first source item");
    let second_item = seed_source_item(&store, "conversation:match-terms", "second")
        .await
        .expect("second source item");

    let mut train = train_claim(first_item.item_id);
    train.fact = "Kevin likes trains.".to_string();
    train.retrieval_hints = serde_json::json!({"keywords": ["trains"], "summary": "Kevin likes trains."});
    store
        .create_or_reinforce_claim(train)
        .await
        .expect("train claim");

    let mut planes = train_claim(second_item.item_id);
    planes.object.entity_id = "concept:planes".to_string();
    planes.object.canonical_name = "planes".to_string();
    planes.fact = "Kevin likes planes.".to_string();
    planes.retrieval_hints = serde_json::json!({"keywords": ["planes"], "summary": "Kevin likes planes."});
    store
        .create_or_reinforce_claim(planes)
        .await
        .expect("plane claim");

    let matches = store
        .find_memory_write_matches(crate::store::MemoryWriteMatchRequest {
            subject_entity_ids: vec!["human:local".to_string()],
            query_terms: vec!["trains".to_string()],
            sensitivity: crate::Sensitivity::Normal,
            limit: 8,
        })
        .await
        .expect("memory write matches");

    assert_eq!(matches.len(), 1, "expected only train match: {matches:?}");
    assert_eq!(matches[0].fact, "Kevin likes trains.");
    assert_eq!(matches[0].predicate_id, "likes");
}

#[tokio::test]
async fn memory_write_match_search_excludes_claims_above_proposal_sensitivity() {
    let store = test_store().await;
    let source_item = seed_source_item(&store, "conversation:match-sensitivity", "secret")
        .await
        .expect("source item");
    let mut secret = train_claim(source_item.item_id);
    secret.fact = "Kevin has a train vault code.".to_string();
    secret.sensitivity = crate::Sensitivity::Secret;
    secret.retrieval_hints = serde_json::json!({"keywords": ["train", "vault"], "summary": "Kevin has a train vault code."});
    store
        .create_or_reinforce_claim(secret)
        .await
        .expect("secret claim");

    let matches = store
        .find_memory_write_matches(crate::store::MemoryWriteMatchRequest {
            subject_entity_ids: vec!["human:local".to_string()],
            query_terms: vec!["train".to_string()],
            sensitivity: crate::Sensitivity::Normal,
            limit: 8,
        })
        .await
        .expect("memory write matches");

    assert!(matches.is_empty(), "normal proposal must not see secret claims: {matches:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core store::tests::claims::memory_write_match_search -- --nocapture
```

Expected: compile failure naming missing `MemoryWriteMatchRequest` and `find_memory_write_matches`.

- [ ] **Step 3: Add the request type**

In `crates/noema-core/src/store/claims/model.rs`, after `pub struct ConsolidationMatchRequest`, insert:

```rust
/// Request for broad bounded existing-claim candidates before provider canonicalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWriteMatchRequest {
    /// Candidate subject entity ids inferred from trusted proposal/runtime state.
    pub subject_entity_ids: Vec<String>,
    /// Query terms collected from proposal content, evidence, and retrieval hints.
    pub query_terms: Vec<String>,
    /// Proposal sensitivity; candidates above this tier are hidden.
    pub sensitivity: Sensitivity,
    /// Requested result limit, clamped by the store.
    pub limit: usize,
}
```

Ensure the type is re-exported through the existing store module exports if `crate::store::MemoryWriteMatchRequest` does not resolve. The re-export should be in the same file that already re-exports `ConsolidationMatchRequest`.

- [ ] **Step 4: Add the store search method**

In `crates/noema-core/src/store/claims/consolidation.rs`, extend the model import:

```rust
model::{ClaimStatus, ConsolidationMatch, ConsolidationMatchRequest, MemoryWriteMatchRequest},
```

Add this method inside `impl NoemaStore`, after `find_consolidation_matches`:

```rust
/// Find broad bounded existing claim candidates before provider canonicalization.
///
/// # Errors
///
/// Returns [`StoreError`] when stored enum data is invalid or the embedded
/// store read fails.
pub async fn find_memory_write_matches(
    &self,
    request: MemoryWriteMatchRequest,
) -> Result<Vec<ConsolidationMatch>, StoreError> {
    let limit = request.limit.clamp(1, 20);
    let subject_entity_ids = request
        .subject_entity_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if subject_entity_ids.is_empty() {
        return Ok(Vec::new());
    }

    let query_terms = request
        .query_terms
        .iter()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    let mut response = self
        .db
        .query(
            r#"
            SELECT claim_id, subject_entity_id, object_entity_id, predicate_id, fact,
              status, sensitivity, confidence, updated_at
            FROM claims
            WHERE subject_entity_id IN $subject_entity_ids
              AND status IN ['candidate', 'active', 'confirmed']
              AND sensitivity IN $allowed_sensitivities
            ORDER BY updated_at DESC, claim_id ASC
            LIMIT $candidate_limit;
            "#,
        )
        .bind(("subject_entity_ids", subject_entity_ids))
        .bind((
            "allowed_sensitivities",
            allowed_match_sensitivities(request.sensitivity),
        ))
        .bind(("candidate_limit", limit))
        .await?;
    let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
    rows.into_iter()
        .filter(|row| memory_write_match_is_relevant(row, &query_terms))
        .map(consolidation_match)
        .take(limit)
        .collect()
}
```

Add this helper after `consolidation_match_is_relevant`:

```rust
fn memory_write_match_is_relevant(row: &ConsolidationMatchRow, query_terms: &[String]) -> bool {
    if query_terms.is_empty() {
        return true;
    }
    let fact = row.fact.to_ascii_lowercase();
    query_terms.iter().any(|term| fact.contains(term))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test -p noema-core store::tests::claims::memory_write_match_search -- --nocapture
```

Expected: both new store tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/store/claims/model.rs crates/noema-core/src/store/claims/consolidation.rs crates/noema-core/src/store/tests/claims.rs
git commit -m "feat: add broad memory write match search"
```

Expected: commit succeeds with only store files staged.

## Task 3: Runtime One-Call Memory Resolution

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/memory_writes.rs`

- [ ] **Step 1: Add runtime imports for unified resolution**

In `crates/noema-core/src/daemon/runtime/memory_writes.rs`, replace the `memory::consolidation` import block with:

```rust
memory::consolidation::{
    CanonicalClaimCandidate, ConsolidationDecision, ConsolidationDecisionKind,
    MemoryConsolidationError, MemoryWriteProposal, PredicateResolution,
    ResolvedMemoryWriteDecision, build_memory_write_resolution_prompt,
    parse_memory_write_resolution_response,
},
```

Replace the current multi-item `store` import block with:

```rust
store::{
    ClaimStatus, ConsolidationMatch, MemoryWriteMatchRequest, NewClaimCandidate,
    RelatedClaimCandidate, SupersedeClaimCandidate,
},
```

Expected: code does not compile yet because the new imports are not used and old prompt functions are still referenced.

- [ ] **Step 2: Replace canonicalization with one-call resolution helper**

Delete the existing `async fn canonicalize_memory_write` method.

Insert this method in the same `impl CodexRuntimeActor` block where `canonicalize_memory_write` was:

```rust
async fn resolve_memory_write(
    &self,
    proposal: &MemoryWriteProposal,
) -> Result<(Vec<ResolvedMemoryWriteDecision>, Vec<ConsolidationMatch>), DaemonError> {
    let predicates = self.store.predicate_catalog().await?;
    let catalog_json = serde_json::to_value(&predicates).map_err(|error| {
        DaemonError::Protocol(format!("predicate catalog serialization failed: {error}"))
    })?;
    let matches = self
        .store
        .find_memory_write_matches(MemoryWriteMatchRequest {
            subject_entity_ids: memory_write_match_subjects(proposal),
            query_terms: memory_write_match_terms(proposal),
            sensitivity: proposal.sensitivity,
            limit: 12,
        })
        .await?;
    let existing_json = serde_json::to_value(
        matches
            .iter()
            .map(|item| {
                json!({
                    "claim_id": item.claim_id,
                    "subject_entity_id": item.subject_entity_id,
                    "object_entity_id": item.object_entity_id,
                    "predicate_id": item.predicate_id,
                    "fact": item.fact,
                    "status": claim_status_label(item.status),
                    "sensitivity": sensitivity_label(item.sensitivity),
                    "confidence": item.confidence,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|error| DaemonError::Protocol(format!("match serialization failed: {error}")))?;
    let prompt = build_memory_write_resolution_prompt(proposal, &catalog_json, &existing_json);
    let mut ignored_events = |_| {};
    let response = self
        .provider
        .generate_streaming(GenerateRequest::text(prompt), &mut ignored_events)
        .await
        .map_err(DaemonError::Provider)?;
    let parsed = parse_memory_write_resolution_response(&response.assistant_text()).map_err(
        |error| match error {
            MemoryConsolidationError::InvalidJson { .. }
            | MemoryConsolidationError::InvalidCanonicalization(_)
            | MemoryConsolidationError::InvalidDecision(_) => {
                DaemonError::Protocol(format!("memory write resolution failed: {error}"))
            }
        },
    )?;
    for item in &parsed.decisions {
        if let PredicateResolution::PromotedPredicate { predicate_id } = &item.candidate.predicate
            && !predicates
                .iter()
                .any(|predicate| predicate.predicate_id == *predicate_id)
        {
            return Err(DaemonError::Protocol(format!(
                "memory write resolution failed: unknown promoted predicate_id {predicate_id}"
            )));
        }
        if let Some(existing_claim_id) = item.existing_claim_id.as_deref() {
            validate_consolidation_decision_target(
                decision_kind_label(item.decision),
                existing_claim_id,
                &matches,
            )?;
        }
    }
    Ok((parsed.decisions, matches))
}
```

- [ ] **Step 3: Add proposal subject and term helpers**

After `claim_write_outcome_label`, add:

```rust
fn memory_write_match_subjects(proposal: &MemoryWriteProposal) -> Vec<String> {
    let mut subjects = Vec::new();
    if proposal.owner_object_id.starts_with("human:") {
        subjects.push(proposal.owner_object_id.clone());
    }
    if let Some(subject_id) = proposal.metadata["canonical_subject"]["entity_id"].as_str()
        && subject_id.starts_with("human:")
    {
        subjects.push(subject_id.to_string());
    }
    subjects.sort();
    subjects.dedup();
    subjects
}

fn memory_write_match_terms(proposal: &MemoryWriteProposal) -> Vec<String> {
    let mut terms = Vec::new();
    push_memory_write_term(&mut terms, &proposal.raw_text);
    push_memory_write_term(&mut terms, &proposal.source_excerpt);
    if let Some(summary) = proposal.retrieval_hints["summary"].as_str() {
        push_memory_write_term(&mut terms, summary);
    }
    if let Some(keywords) = proposal.retrieval_hints["keywords"].as_array() {
        for keyword in keywords {
            if let Some(keyword) = keyword.as_str() {
                push_memory_write_term(&mut terms, keyword);
            }
        }
    }
    terms.sort();
    terms.dedup();
    terms
}

fn push_memory_write_term(terms: &mut Vec<String>, value: &str) {
    let normalized = value
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if normalized.len() >= 3 {
        terms.push(normalized);
    }
}

const fn decision_kind_label(decision: ConsolidationDecisionKind) -> &'static str {
    match decision {
        ConsolidationDecisionKind::Create => "create",
        ConsolidationDecisionKind::Reinforce => "reinforce",
        ConsolidationDecisionKind::Supersede => "supersede",
        ConsolidationDecisionKind::Dispute => "dispute",
        ConsolidationDecisionKind::Relate => "relate",
        ConsolidationDecisionKind::NeedsReview => "needs_review",
    }
}
```

- [ ] **Step 4: Replace the loop body that calls canonicalize + consolidate**

Inside `persist_provider_memory_proposals`, find the block that starts with:

```rust
let canonical_candidates = match self.canonicalize_memory_write(&write_proposal).await {
```

and ends immediately before the existing deterministic fallback block:

```rust
if canonical_candidates_empty {
```

Replace that entire block with:

```rust
let (resolved_decisions, allowed_matches) = match self.resolve_memory_write(&write_proposal).await
{
    Ok(result) => result,
    Err(error) => {
        failed_proposals.push(json!({
            "proposal_index": proposal_index,
            "error": error.to_string(),
        }));
        continue;
    }
};
let canonical_candidates_empty = resolved_decisions.is_empty();
for resolved in resolved_decisions {
    let canonical = resolved.candidate;
    match &canonical.predicate {
        PredicateResolution::PromotedPredicate { .. } => {
            let candidate = match new_claim_from_canonical(
                &canonical,
                &write_proposal,
                crate::EvidenceAuthority::AgentInference,
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                    continue;
                }
            };
            let decision = resolved.consolidation_decision();
            match self
                .persist_consolidation_decision(candidate, decision, &allowed_matches)
                .await
            {
                Ok(outcome) => {
                    match outcome.outcome {
                        "created" => created_claim_count += 1,
                        "reinforced" => reinforced_claim_count += 1,
                        "disputed" => disputed_claim_count += 1,
                        "related" => related_claim_count += 1,
                        "superseded" => superseded_claim_count += 1,
                        "needs_review" => needs_review_claim_count += 1,
                        _ => {}
                    }
                    if saved_claim_counts_as_active(outcome.status) {
                        active_saved_claim_count += 1;
                    }
                    claim_outcomes.push(json!({
                        "claim_id": outcome.claim_id,
                        "outcome": outcome.outcome,
                        "fact_preview": outcome.fact_preview,
                        "sensitivity": outcome.sensitivity,
                    }));
                    if let Some(claim_id) = outcome.claim_id {
                        claim_ids.push(claim_id);
                    }
                }
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                }
            }
        }
        PredicateResolution::PredicateProposal { .. } => {
            let Some(candidate) =
                predicate_proposal_candidate_from_canonical(&canonical, &write_proposal)
            else {
                continue;
            };
            match self.store.create_predicate_proposal(candidate).await {
                Ok(record) => {
                    predicate_proposal_count += 1;
                    claim_outcomes.push(json!({
                        "outcome": "needs_review",
                        "predicate_proposal_id": record.proposal_id,
                        "fact_preview": fact_preview(&canonical.fact),
                        "sensitivity": sensitivity_label(canonical.sensitivity),
                    }));
                }
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                }
            }
        }
        PredicateResolution::FallbackNote => {
            let candidate = deterministic_canonical_claim(
                &write_proposal,
                claim_status_from_memory_status(proposal.proposal.status),
                Some(f64::from(proposal.proposal.proposal.confidence)),
                crate::EvidenceAuthority::AgentInference,
            );
            match self.store.create_or_reinforce_claim(candidate).await {
                Ok(summary) => {
                    match summary.write_outcome {
                        ClaimWriteOutcome::Created => created_claim_count += 1,
                        ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                    }
                    if saved_claim_counts_as_active(summary.status) {
                        active_saved_claim_count += 1;
                    }
                    claim_outcomes.push(claim_outcome_json(&summary));
                    claim_ids.push(summary.claim_id);
                }
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                }
            }
        }
    }
}
```

Keep the existing `if canonical_candidates_empty {` deterministic fallback block immediately after the new loop.

- [ ] **Step 5: Delete the old comparator provider call path**

Delete the entire `async fn consolidate_promoted_claim` method. Keep `persist_consolidation_decision` and `validate_consolidation_decision_target`.

Expected: `build_consolidation_prompt`, `parse_consolidation_decision`, and `ConsolidationMatchRequest` should no longer be imported by `memory_writes.rs`.

- [ ] **Step 6: Run a compile check**

Run:

```bash
cargo check -p noema-core
```

Expected: `noema-core` checks successfully.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/memory_writes.rs
git commit -m "feat: resolve provider memory writes in one call"
```

Expected: commit succeeds with only `memory_writes.rs` staged.

## Task 4: Fake Provider and Runtime Regression Tests

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add request logging to the fake provider**

Modify `FakeCodexProvider`:

```rust
struct FakeCodexProvider {
    scenario: FakeCodexScenario,
    invalid_consolidation_target_id: Arc<Mutex<Option<String>>>,
    request_log: Arc<Mutex<Vec<String>>>,
}
```

Update constructors:

```rust
fn new(scenario: FakeCodexScenario) -> Self {
    Self {
        scenario,
        invalid_consolidation_target_id: Arc::new(Mutex::new(None)),
        request_log: Arc::new(Mutex::new(Vec::new())),
    }
}

fn with_invalid_consolidation_target(
    invalid_consolidation_target_id: Arc<Mutex<Option<String>>>,
) -> Self {
    Self {
        scenario: FakeCodexScenario::MemoryExtraction,
        invalid_consolidation_target_id,
        request_log: Arc::new(Mutex::new(Vec::new())),
    }
}

fn request_log(&self) -> Arc<Mutex<Vec<String>>> {
    self.request_log.clone()
}
```

At the start of `fn generate_response`, after extracting `input`, add:

```rust
self.request_log
    .lock()
    .expect("request log lock")
    .push(input.clone());
```

Add helpers near `assistant_item_id_for_text`:

```rust
fn clear_request_log(log: &Arc<Mutex<Vec<String>>>) {
    log.lock().expect("request log lock").clear();
}

fn request_log_count_containing(log: &Arc<Mutex<Vec<String>>>, needle: &str) -> usize {
    log.lock()
        .expect("request log lock")
        .iter()
        .filter(|request| request.contains(needle))
        .count()
}
```

- [ ] **Step 2: Route the new resolver prompt in the fake provider**

In `FakeCodexProvider::generate_response`, before the existing canonicalizer branch:

```rust
if input.contains("Noema's memory write resolver") {
    let text = memory_write_resolution_response_text(&input, self.scenario);
    return Ok(GenerateResponse {
        output: vec![GenerateOutputItem::AssistantText { text }],
        provider: "codex".to_string(),
        model,
        response_id: Some("fake-response".to_string()),
        usage: None,
    });
}
```

Add this helper near `canonicalization_response_text`:

```rust
fn memory_write_resolution_response_text(input: &str, scenario: FakeCodexScenario) -> String {
    let canonicalization = canonicalization_response_text(input, scenario);
    let parsed: crate::memory::consolidation::CanonicalizationResponse =
        serde_json::from_str(&canonicalization).expect("fake canonicalization parses");
    let decisions = parsed
        .candidates
        .into_iter()
        .map(|candidate| {
            let existing_claim_id = first_memory_id_from_resolution_prompt(input);
            let decision = if let Some(existing_claim_id) = existing_claim_id.as_deref() {
                if candidate.fact.contains("dislikes ice cream") {
                    serde_json::json!({
                        "candidate": candidate,
                        "decision": "dispute",
                        "existing_claim_id": existing_claim_id,
                        "confidence": 0.93,
                        "rationale": "opposite ice cream preference"
                    })
                } else {
                    serde_json::json!({
                        "candidate": candidate,
                        "decision": "reinforce",
                        "existing_claim_id": existing_claim_id,
                        "confidence": 0.92,
                        "rationale": "same memory as an existing candidate"
                    })
                }
            } else {
                serde_json::json!({
                    "candidate": candidate,
                    "decision": "create",
                    "existing_claim_id": null,
                    "confidence": 0.9,
                    "rationale": "no existing candidate matched"
                })
            };
            decision
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "decisions": decisions }).to_string()
}

fn first_memory_id_from_resolution_prompt(input: &str) -> Option<String> {
    let payload_start = input.find("Existing claim candidates JSON:")?;
    let payload = &input[payload_start..];
    first_memory_id_from_consolidation_prompt(payload)
}
```

- [ ] **Step 3: Write the one-call runtime regression test**

Add this test near `provider_first_person_memory_reinforces_explicit_canonical_claim`:

```rust
#[tokio::test]
async fn provider_memory_reinforcement_uses_one_resolution_call_after_proposal() {
    let provider = fake_codex_provider_with_memory_extraction();
    let request_log = provider.request_log();
    let (handle, store) = test_runtime_handle_with_store(provider).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "remember this: I like ice cream.".to_string(),
    )
    .await
    .expect("seed explicit memory");
    clear_request_log(&request_log);

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("reinforcement turn");

    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["reinforced_claim_count"] == 1
        )
    }));
    assert_eq!(
        request_log_count_containing(&request_log, "Noema's memory write resolver"),
        1,
        "one post-proposal resolver call should handle canonicalization and consolidation"
    );
    assert_eq!(
        request_log_count_containing(&request_log, "Noema's memory claim canonicalizer"),
        0,
        "old canonicalizer call should not run"
    );
    assert_eq!(
        request_log_count_containing(&request_log, "Noema's memory consolidation comparator"),
        0,
        "old comparator call should not run"
    );

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Active),
            predicate_id: Some("likes".to_string()),
            limit: Some(10),
        })
        .await
        .expect("ice cream claims");
    assert_eq!(claims.len(), 1, "reinforcement should not create a duplicate: {claims:?}");
    assert_eq!(claims[0].evidence_count, 2);
    handle.shutdown().await;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p noema-core provider_memory_reinforcement_uses_one_resolution_call_after_proposal -- --nocapture
```

Expected: the test passes and would fail on the old two-call path because it would log canonicalizer/comparator requests instead of a resolver request.

- [ ] **Step 5: Run nearby provider memory tests**

Run:

```bash
cargo test -p noema-core provider_memory -- --test-threads=1
```

Expected: all provider memory tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/noema-core/src/daemon/tests.rs
git commit -m "test: assert one-call provider memory resolution"
```

Expected: commit succeeds with only daemon tests staged.

## Task 5: Remove Old Runtime Dependency and Keep Compatibility Tests Focused

**Files:**
- Modify: `crates/noema-core/src/memory/consolidation.rs`
- Modify: `crates/noema-core/src/daemon/runtime/memory_writes.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Search for old two-call runtime references**

Run:

```bash
rg -n "build_claim_canonicalization_prompt|build_consolidation_prompt|parse_consolidation_decision|canonicalize_memory_write|consolidate_promoted_claim|Noema's memory consolidation comparator|Noema's memory claim canonicalizer" crates/noema-core/src
```

Expected: references remain only in pure parser/prompt tests and fake-provider compatibility helpers, not in `crates/noema-core/src/daemon/runtime/memory_writes.rs`.

- [ ] **Step 2: Delete old runtime-only fake provider branches if unused**

If the search shows old prompt strings only exist for tests that still intentionally exercise pure parser builders, keep those tests. If `FakeCodexProvider::generate_response` still contains:

```rust
if input.contains("Noema's memory claim canonicalizer") {
```

delete that entire `if` branch through its matching closing brace only after confirming no runtime test needs it:

```bash
cargo test -p noema-core provider_memory -- --test-threads=1
```

Expected: if the branch is removed correctly, provider memory tests still pass because runtime uses `Noema's memory write resolver`.

- [ ] **Step 3: Run full Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit cleanup**

Run:

```bash
git add crates/noema-core/src/memory/consolidation.rs crates/noema-core/src/daemon/runtime/memory_writes.rs crates/noema-core/src/daemon/tests.rs
git commit -m "chore: remove two-call memory resolution leftovers"
```

Expected: commit succeeds. If Step 2 found no cleanup was needed, skip this commit and record that no cleanup commit was created.

## Task 6: Update Durable Context

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Add a settled-decision bullet**

In `docs/context/current.md`, under the existing memory consolidation bullets, add:

```markdown
- Provider ordinary-chat memory writes resolve proposal canonicalization and
  consolidation in one post-proposal provider call. Noema performs bounded local
  candidate lookup before the resolver call, validates promoted predicates and
  existing claim targets against trusted store data, and then writes graph
  claims deterministically.
```

- [ ] **Step 2: Run docs diff check**

Run:

```bash
git diff -- docs/context/current.md
```

Expected: diff contains exactly the new settled-decision bullet.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/context/current.md
git commit -m "docs: record one-call memory write resolution"
```

Expected: commit succeeds with only `docs/context/current.md` staged.

## Final Verification

- [ ] **Step 1: Run ship checklist status**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: status shows a clean worktree except branch-ahead count; diff check prints no output.

- [ ] **Step 2: Run focused regression tests**

Run:

```bash
cargo test -p noema-core memory_write_resolution -- --nocapture
cargo test -p noema-core memory_write_match_search -- --nocapture
cargo test -p noema-core provider_memory_reinforcement_uses_one_resolution_call_after_proposal -- --nocapture
cargo test -p noema-core provider_memory -- --test-threads=1
```

Expected: all commands pass.

- [ ] **Step 3: Run full Rust validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands exit 0.

- [ ] **Step 4: Inspect final history**

Run:

```bash
git log --oneline -6
git status --short --branch
```

Expected: recent commits include the task commits above; status is clean except the branch being ahead of `origin/main`.

## Self-Review

Spec coverage:
- One post-proposal provider call is implemented by Task 1 and Task 3.
- Broad local candidate search before the one call is implemented by Task 2.
- Validation of promoted predicates and existing claim target allowlist is implemented by Task 3.
- Runtime regression coverage proving the old second call is gone is implemented by Task 4.
- Durable project context is updated by Task 6.

Placeholder scan:
- No unchecked placeholders remain in the executable steps. All code snippets use concrete type and function names introduced in earlier tasks.

Type consistency:
- `MemoryWriteResolutionResponse`, `ResolvedMemoryWriteDecision`, `build_memory_write_resolution_prompt`, and `parse_memory_write_resolution_response` are introduced in Task 1 and used consistently in Tasks 3 and 4.
- `MemoryWriteMatchRequest` is introduced in Task 2 and used consistently in Task 3.
