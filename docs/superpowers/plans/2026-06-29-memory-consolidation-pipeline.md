# Memory Consolidation Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Noema's future-write memory consolidation pipeline so LLMs can propose graph-shaped claims and predicate proposals while Noema validates, matches, consolidates, and persists memory truth.

**Architecture:** Add a pure `memory_consolidation` module for proposal, canonicalization, predicate proposal, and decision types. Extend the SurrealDB store with predicate proposal inspection, bounded claim matching, and relation persistence, then wire the daemon memory write path through the pipeline while preserving exact fingerprint reinforcement as the repository safety net.

**Tech Stack:** Rust 2024, Tokio, serde/serde_json, async-graphql, embedded SurrealDB v3, existing Noema daemon/runtime tests.

---

## Scope

This plan implements all four approved slices in one cohesive attempt:

1. internal proposal/canonicalization/consolidation types,
2. LLM canonicalization and predicate proposal persistence,
3. bounded match search and deterministic consolidation rules,
4. conservative LLM consolidation decisions and basic inspection surfaces.

Existing local database repair is out of scope. The implementation may add or
rewrite pre-V1 schema definitions directly.

## File Structure

- Create `crates/noema-core/src/memory_consolidation.rs`
  - Pure types, strict JSON parsers, prompt builders, deterministic validation helpers, and unit tests.
- Modify `crates/noema-core/src/lib.rs`
  - Export the new pure module and public types needed by store/runtime/tests.
- Modify `crates/noema-core/src/store.rs`
  - Export predicate proposal, match, and consolidation persistence types.
- Modify `crates/noema-core/src/store/ontology.rs`
  - Expand predicate records and add predicate proposal records/candidates.
- Modify `crates/noema-core/src/store/schema.rs`
  - Add `related_to` relation and any stricter predicate proposal fields needed by the repository.
- Modify `crates/noema-core/src/store/claims.rs`
  - Add bounded consolidation match search and decision persistence helpers.
- Modify `crates/noema-core/src/store/tests/claims.rs`
  - Add repository tests for match search, predicate proposals, relation persistence, and review-gated outcomes.
- Modify `crates/noema-core/src/daemon/memory_pipeline.rs`
  - Convert explicit and provider proposals into `MemoryWriteProposal`, call canonicalization/consolidation, and preserve deterministic fallback behavior.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Use the pipeline from both explicit and provider proposal paths, call provider-backed canonicalizer/comparator when needed, and emit richer outcome metadata.
- Modify `crates/noema-core/src/daemon/tests.rs`
  - Add runtime tests for provider "loves" canonicalization, predicate proposals, reinforcement, conflict/review, and related outcomes.
- Modify `crates/noema-core/src/graphql/types.rs`
  - Add GraphQL predicate proposal inspection types.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Add `memoryPredicateProposals` and `memoryPredicateProposal` queries plus schema tests.
- Modify `crates/noema-cli/src/inspection.rs`
  - Add `noema memory predicate-proposals` and `noema memory predicate-proposal <id>` inspection commands.
- Modify `crates/noema-cli/src/main.rs`
  - Wire the new CLI subcommands through existing GraphQL memory inspection.
- Modify docs:
  - `docs/context/current.md`
  - `docs/frontend/current-contract.md`

## Task 1: Add Pure Consolidation Types And Parsers

**Files:**
- Create: `crates/noema-core/src/memory_consolidation.rs`
- Modify: `crates/noema-core/src/lib.rs`

- [ ] **Step 1: Write failing parser and validation tests**

Add this test module to the new `crates/noema-core/src/memory_consolidation.rs` file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Sensitivity;

    #[test]
    fn parses_promoted_predicate_canonicalization() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:planes", "entity_type": "concept", "canonical_name": "planes"},
            "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
            "fact": "Kevin likes planes.",
            "sensitivity": "normal",
            "status": "active",
            "confidence": 0.94,
            "retrieval_hints": {"keywords": ["planes"], "summary": "Kevin likes planes."},
            "rationale": "The user directly said they love planes."
          }]
        }"#;

        let parsed = parse_canonicalization_response(json).expect("canonicalization");
        assert_eq!(parsed.candidates.len(), 1);
        let candidate = &parsed.candidates[0];
        assert_eq!(candidate.subject.entity_id, "human:local");
        assert_eq!(candidate.object.entity_id, "concept:planes");
        assert_eq!(candidate.predicate, PredicateResolution::PromotedPredicate { predicate_id: "likes".to_string() });
        assert_eq!(candidate.fact, "Kevin likes planes.");
        assert_eq!(candidate.sensitivity, Sensitivity::Normal);
        assert_eq!(candidate.status, CanonicalClaimStatus::Active);
    }

    #[test]
    fn parses_predicate_proposal_canonicalization() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:model_aircraft", "entity_type": "concept", "canonical_name": "model aircraft"},
            "predicate": {
              "kind": "predicate_proposal",
              "proposal": {
                "label": "collects",
                "description": "The subject collects the object.",
                "allowed_subject_types": ["human", "person"],
                "allowed_object_types": ["concept", "other"],
                "allowed_use_modes": ["answer", "personalize"],
                "default_sensitivity": "normal",
                "conflict_policy": "allow_many",
                "review_policy": "auto_candidate",
                "inverse_behavior": "none",
                "inverse_predicate_id": null,
                "proactivity_default": 1,
                "merge_hints": {"strategy": "object_identity"},
                "synonym_hints": ["gathers", "keeps a collection of"],
                "extraction_hints": {"examples": ["I collect model aircraft"]},
                "rationale": "No promoted predicate represents collecting."
              }
            },
            "fact": "Kevin collects model aircraft.",
            "sensitivity": "normal",
            "status": "candidate",
            "confidence": 0.88,
            "retrieval_hints": {"keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
            "rationale": "The source states a durable collecting relationship."
          }]
        }"#;

        let parsed = parse_canonicalization_response(json).expect("canonicalization");
        let PredicateResolution::PredicateProposal { proposal } = &parsed.candidates[0].predicate else {
            panic!("expected predicate proposal");
        };
        assert_eq!(proposal.label, "collects");
        assert_eq!(proposal.allowed_subject_types, vec!["human", "person"]);
        assert_eq!(parsed.candidates[0].status, CanonicalClaimStatus::Candidate);
    }

    #[test]
    fn rejects_invalid_canonicalization_confidence() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:planes", "entity_type": "concept", "canonical_name": "planes"},
            "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
            "fact": "Kevin likes planes.",
            "sensitivity": "normal",
            "status": "active",
            "confidence": 1.7,
            "retrieval_hints": {},
            "rationale": "bad confidence"
          }]
        }"#;

        let error = parse_canonicalization_response(json).expect_err("invalid confidence");
        assert!(error.to_string().contains("confidence must be between 0.0 and 1.0"));
    }

    #[test]
    fn parses_consolidation_decision() {
        let json = r#"{
          "decision": "reinforce",
          "existing_claim_id": "claim:abc",
          "confidence": 0.92,
          "rationale": "same preference"
        }"#;

        let decision = parse_consolidation_decision(json).expect("decision");
        assert_eq!(decision.decision, ConsolidationDecisionKind::Reinforce);
        assert_eq!(decision.existing_claim_id.as_deref(), Some("claim:abc"));
        assert_eq!(decision.confidence, 0.92);
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p noema-core memory_consolidation --no-fail-fast
```

Expected: FAIL because `memory_consolidation` is not exported and the parser/types do not exist.

- [ ] **Step 3: Implement pure types and parsers**

Create `crates/noema-core/src/memory_consolidation.rs` with this implementation:

```rust
//! Pure future-write memory consolidation types, prompts, and parsers.

use crate::memory::Sensitivity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryWriteProposal {
    pub source_kind: MemoryWriteSourceKind,
    pub source_item_id: String,
    pub source_actor_id: String,
    pub source_excerpt: String,
    pub owner_object_type: String,
    pub owner_object_id: String,
    pub raw_text: String,
    pub memory_type: String,
    pub sensitivity: Sensitivity,
    pub risk_flags: Vec<String>,
    pub retrieval_hints: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWriteSourceKind {
    ExplicitRemember,
    OrdinaryChat,
    DocumentImport,
    ToolOutput,
    TaskOutput,
    SystemSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalizationResponse {
    pub candidates: Vec<CanonicalClaimCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalClaimCandidate {
    pub subject: CanonicalEntity,
    pub object: CanonicalEntity,
    pub predicate: PredicateResolution,
    pub fact: String,
    pub sensitivity: Sensitivity,
    pub status: CanonicalClaimStatus,
    pub confidence: f64,
    #[serde(default)]
    pub retrieval_hints: Value,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalEntity {
    pub entity_id: String,
    pub entity_type: String,
    pub canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PredicateResolution {
    PromotedPredicate { predicate_id: String },
    PredicateProposal { proposal: ProposedPredicate },
    FallbackNote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedPredicate {
    pub label: String,
    pub description: String,
    pub allowed_subject_types: Vec<String>,
    pub allowed_object_types: Vec<String>,
    pub allowed_use_modes: Vec<String>,
    pub default_sensitivity: Sensitivity,
    pub conflict_policy: String,
    pub review_policy: String,
    pub inverse_behavior: String,
    pub inverse_predicate_id: Option<String>,
    pub proactivity_default: i64,
    #[serde(default)]
    pub merge_hints: Value,
    #[serde(default)]
    pub synonym_hints: Vec<String>,
    #[serde(default)]
    pub extraction_hints: Value,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalClaimStatus {
    Candidate,
    Active,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationDecision {
    pub decision: ConsolidationDecisionKind,
    pub existing_claim_id: Option<String>,
    pub confidence: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationDecisionKind {
    Create,
    Reinforce,
    Supersede,
    Dispute,
    Relate,
    NeedsReview,
}

#[derive(Debug, Error)]
pub enum MemoryConsolidationError {
    #[error("invalid consolidation JSON: {source}")]
    InvalidJson {
        #[from]
        source: serde_json::Error,
    },
    #[error("invalid canonicalization response: {0}")]
    InvalidCanonicalization(&'static str),
    #[error("invalid consolidation decision: {0}")]
    InvalidDecision(&'static str),
}

pub fn parse_canonicalization_response(
    text: &str,
) -> Result<CanonicalizationResponse, MemoryConsolidationError> {
    let mut response: CanonicalizationResponse = serde_json::from_str(text.trim())?;
    for candidate in &mut response.candidates {
        candidate.fact = candidate.fact.trim().to_string();
        candidate.rationale = candidate.rationale.trim().to_string();
        if candidate.fact.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "fact must not be empty",
            ));
        }
        if !candidate.confidence.is_finite() || !(0.0..=1.0).contains(&candidate.confidence) {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "confidence must be between 0.0 and 1.0",
            ));
        }
        if candidate.rationale.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "rationale must not be empty",
            ));
        }
    }
    Ok(response)
}

pub fn parse_consolidation_decision(
    text: &str,
) -> Result<ConsolidationDecision, MemoryConsolidationError> {
    let mut decision: ConsolidationDecision = serde_json::from_str(text.trim())?;
    decision.rationale = decision.rationale.trim().to_string();
    if !decision.confidence.is_finite() || !(0.0..=1.0).contains(&decision.confidence) {
        return Err(MemoryConsolidationError::InvalidDecision(
            "confidence must be between 0.0 and 1.0",
        ));
    }
    if !matches!(decision.decision, ConsolidationDecisionKind::Create | ConsolidationDecisionKind::NeedsReview)
        && decision.existing_claim_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(MemoryConsolidationError::InvalidDecision(
            "existing_claim_id is required for this decision",
        ));
    }
    if decision.rationale.is_empty() {
        return Err(MemoryConsolidationError::InvalidDecision(
            "rationale must not be empty",
        ));
    }
    Ok(decision)
}

pub fn build_claim_canonicalization_prompt(
    proposal: &MemoryWriteProposal,
    promoted_predicates_json: &Value,
) -> String {
    format!(
        r#"You are Noema's memory claim canonicalizer.

Return strict JSON only. Do not include Markdown, comments, or prose.

Map the proposal to promoted predicates when one clearly fits. If none fits,
return a predicate_proposal. Use fallback_note only for genuinely unstructured
notes that do not express a durable relationship.

Promoted predicate catalog JSON:
{promoted_predicates_json}

Input JSON payload:
{}"#,
        serde_json::to_string_pretty(proposal).expect("serialize proposal")
    )
}

pub fn build_consolidation_prompt(candidate: &CanonicalClaimCandidate, existing: &Value) -> String {
    let payload = serde_json::json!({
        "candidate": candidate,
        "existing_memories": existing,
    });
    format!(
        r#"You are Noema's memory consolidation comparator.

Return strict JSON only with fields decision, existing_claim_id, confidence, and rationale.
Use create, reinforce, supersede, dispute, relate, or needs_review.
Prefer needs_review when uncertain.

Input JSON payload:
{}"#,
        serde_json::to_string_pretty(&payload).expect("serialize comparator payload")
    )
}
```

Modify `crates/noema-core/src/lib.rs`:

```rust
/// Future-write memory consolidation types and prompts.
pub mod memory_consolidation;

pub use memory_consolidation::{
    CanonicalClaimCandidate, CanonicalClaimStatus, CanonicalEntity,
    ConsolidationDecision, ConsolidationDecisionKind, MemoryConsolidationError,
    MemoryWriteProposal, MemoryWriteSourceKind, PredicateResolution, ProposedPredicate,
    build_claim_canonicalization_prompt, build_consolidation_prompt,
    parse_canonicalization_response, parse_consolidation_decision,
};
```

- [ ] **Step 4: Run the tests**

Run:

```bash
cargo test -p noema-core memory_consolidation --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/memory_consolidation.rs crates/noema-core/src/lib.rs
git commit -m "feat: add memory consolidation types"
```

## Task 2: Add Predicate Proposal Repository Support

**Files:**
- Modify: `crates/noema-core/src/store/ontology.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Write failing predicate proposal store tests**

Append these tests to `crates/noema-core/src/store/tests/claims.rs`:

```rust
#[tokio::test]
async fn predicate_proposal_can_be_created_and_listed() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "I collect model aircraft.").await;

    let proposal = store
        .create_predicate_proposal(crate::store::PredicateProposalCandidate {
            label: "collects".to_string(),
            description: "The subject collects the object.".to_string(),
            proposed_predicate: json!({
                "allowed_subject_types": ["human", "person"],
                "allowed_object_types": ["concept", "other"],
                "allowed_use_modes": ["answer", "personalize"],
                "default_sensitivity": "normal",
                "conflict_policy": "allow_many",
                "review_policy": "auto_candidate",
                "inverse_behavior": "none",
                "inverse_predicate_id": null,
                "proactivity_default": 1,
                "merge_hints": {"strategy": "object_identity"},
                "synonym_hints": ["keeps a collection of"],
                "extraction_hints": {"examples": ["I collect model aircraft"]},
                "rationale": "No promoted predicate represents collecting."
            }),
            source_item_id: Some(source_item.item_id.clone()),
            proposed_claim: json!({
                "fact": "Kevin collects model aircraft.",
                "subject_entity_id": "human:local",
                "object_entity_id": "concept:model_aircraft"
            }),
        })
        .await
        .expect("create predicate proposal");

    assert!(proposal.proposal_id.starts_with("predicate_proposal:"));
    assert_eq!(proposal.status, "candidate");
    assert_eq!(proposal.source_item_id.as_deref(), Some(source_item.item_id.as_str()));

    let proposals = store
        .list_predicate_proposals(crate::store::PredicateProposalFilter {
            status: Some("candidate".to_string()),
            limit: Some(10),
        })
        .await
        .expect("list predicate proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].proposal_id, proposal.proposal_id);

    let detail = store
        .get_predicate_proposal(&proposal.proposal_id)
        .await
        .expect("get proposal")
        .expect("proposal exists");
    assert_eq!(detail.label, "collects");
    assert_eq!(detail.proposed_claim["fact"], "Kevin collects model aircraft.");
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p noema-core predicate_proposal_can_be_created_and_listed --no-fail-fast
```

Expected: FAIL because the repository types and methods do not exist.

- [ ] **Step 3: Add predicate proposal structs**

Modify `crates/noema-core/src/store/ontology.rs`:

```rust
use serde_json::Value;
use surrealdb::types::Datetime;

#[derive(Debug, Clone, PartialEq)]
pub struct PredicateProposalCandidate {
    pub label: String,
    pub description: String,
    pub proposed_predicate: Value,
    pub source_item_id: Option<String>,
    pub proposed_claim: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PredicateProposalFilter {
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PredicateProposalRecord {
    pub proposal_id: String,
    pub label: String,
    pub description: String,
    pub proposed_predicate: Value,
    pub proposed_claim: Value,
    pub status: String,
    pub source_item_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
struct PredicateProposalRow {
    proposal_id: String,
    label: String,
    description: String,
    proposed_predicate: Value,
    status: String,
    source_item_id: Option<String>,
    proposed_claim: Option<Value>,
    created_at: Datetime,
    updated_at: Datetime,
}
```

Extend the existing `PredicateRecord` in the same file to include the fields the canonicalizer needs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, SurrealValue)]
pub struct PredicateRecord {
    pub predicate_id: String,
    pub label: String,
    pub description: String,
    pub allowed_subject_types: Vec<String>,
    pub allowed_object_types: Vec<String>,
    pub allowed_use_modes: Vec<String>,
    pub default_sensitivity: String,
    pub conflict_policy: String,
    pub review_policy: String,
    pub inverse_behavior: String,
    pub inverse_predicate_id: Option<String>,
    pub proactivity_default: i64,
    pub synonym_hints: Vec<String>,
}
```

- [ ] **Step 4: Add repository methods**

In `crates/noema-core/src/store/ontology.rs`, add an `impl NoemaStore` block:

```rust
use super::{ids::{allocate_id, record_fragment}, NoemaStore, StoreError};

impl NoemaStore {
    pub async fn predicate_catalog(&self) -> Result<Vec<PredicateRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT predicate_id, label, description, allowed_subject_types,
                  allowed_object_types, allowed_use_modes, default_sensitivity,
                  conflict_policy, review_policy, inverse_behavior,
                  inverse_predicate_id, proactivity_default, synonym_hints
                FROM predicates
                ORDER BY predicate_id ASC;
                "#,
            )
            .await?;
        let rows: Vec<PredicateRecord> = response.take(0)?;
        Ok(rows)
    }

    pub async fn create_predicate_proposal(
        &self,
        candidate: PredicateProposalCandidate,
    ) -> Result<PredicateProposalRecord, StoreError> {
        if let Some(source_item_id) = candidate.source_item_id.as_deref() {
            self.require_source_item(source_item_id).await?;
        }
        let proposal_id = allocate_id("predicate_proposal");
        let mut proposed_predicate = candidate.proposed_predicate;
        if let Some(object) = proposed_predicate.as_object_mut() {
            object.insert("proposed_claim".to_string(), candidate.proposed_claim.clone());
        }
        self.db
            .query(
                r#"
                CREATE type::record('predicate_proposals', $record_id) SET
                  proposal_id = $proposal_id,
                  label = $label,
                  description = $description,
                  proposed_predicate = $proposed_predicate,
                  status = 'candidate',
                  source_item_id = $source_item_id,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&proposal_id)))
            .bind(("proposal_id", proposal_id.clone()))
            .bind(("label", candidate.label))
            .bind(("description", candidate.description))
            .bind(("proposed_predicate", proposed_predicate))
            .bind(("source_item_id", candidate.source_item_id))
            .await?
            .check()?;

        self.get_predicate_proposal(&proposal_id)
            .await?
            .ok_or_else(|| StoreError::Schema(format!("missing predicate proposal after write: {proposal_id}")))
    }

    pub async fn list_predicate_proposals(
        &self,
        filter: PredicateProposalFilter,
    ) -> Result<Vec<PredicateProposalRecord>, StoreError> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let mut sql = String::from(
            r#"
            SELECT proposal_id, label, description, proposed_predicate, status,
              source_item_id, created_at, updated_at
            FROM predicate_proposals
            "#,
        );
        if filter.status.is_some() {
            sql.push_str("WHERE status = $status\n");
        }
        sql.push_str("ORDER BY created_at DESC, proposal_id ASC LIMIT $limit;");
        let mut statement = self.db.query(sql).bind(("limit", limit));
        if let Some(status) = filter.status {
            statement = statement.bind(("status", status));
        }
        let mut response = statement.await?;
        let rows: Vec<PredicateProposalRow> = response.take(0)?;
        rows.into_iter().map(predicate_proposal_record).collect()
    }

    pub async fn get_predicate_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<PredicateProposalRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT proposal_id, label, description, proposed_predicate, status,
                  source_item_id, created_at, updated_at
                FROM predicate_proposals
                WHERE proposal_id = $proposal_id
                LIMIT 1;
                "#,
            )
            .bind(("proposal_id", proposal_id.to_string()))
            .await?;
        let rows: Vec<PredicateProposalRow> = response.take(0)?;
        rows.into_iter().next().map(predicate_proposal_record).transpose()
    }
}

fn predicate_proposal_record(
    row: PredicateProposalRow,
) -> Result<PredicateProposalRecord, StoreError> {
    let proposed_claim = row
        .proposed_predicate
        .get("proposed_claim")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(PredicateProposalRecord {
        proposal_id: row.proposal_id,
        label: row.label,
        description: row.description,
        proposed_predicate: row.proposed_predicate,
        proposed_claim,
        status: row.status,
        source_item_id: row.source_item_id,
        created_at: super::claims::format_datetime(row.created_at),
        updated_at: super::claims::format_datetime(row.updated_at),
    })
}
```

In `crates/noema-core/src/store/claims.rs`, change the existing `format_datetime`
helper signature to:

```rust
pub(super) fn format_datetime(value: Datetime) -> String {
    value.to_raw()
}
```

In `crates/noema-core/src/store/claims.rs`, also change the existing
`require_source_item` helper signature so `ontology.rs` can validate predicate
proposal source items:

```rust
pub(super) async fn require_source_item(&self, item_id: &str) -> Result<(), StoreError> {
```

- [ ] **Step 5: Export store types**

Modify `crates/noema-core/src/store.rs`:

```rust
pub use ontology::{
    EntityCandidate, EntityType, PredicateProposalCandidate, PredicateProposalFilter,
    PredicateProposalRecord, PredicateRecord,
};
```

Modify `crates/noema-core/src/lib.rs`:

```rust
pub use store::{
    PredicateProposalCandidate, PredicateProposalFilter, PredicateProposalRecord,
};
```

- [ ] **Step 6: Run the store test**

Run:

```bash
cargo test -p noema-core predicate_proposal_can_be_created_and_listed --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/store/ontology.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests/claims.rs
git commit -m "feat: persist predicate proposals"
```

## Task 3: Add Bounded Match Search And Relation Persistence

**Files:**
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/claims.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/store/tests/claims.rs`

- [ ] **Step 1: Write failing match and relation tests**

Append these tests to `crates/noema-core/src/store/tests/claims.rs`:

```rust
#[tokio::test]
async fn bounded_consolidation_match_search_finds_same_subject_predicate_claims() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    let summary = store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("seed claim");

    let matches = store
        .find_consolidation_matches(crate::store::ConsolidationMatchRequest {
            subject_entity_id: "human:local".to_string(),
            predicate_id: "likes".to_string(),
            object_entity_id: Some("concept:claim_object_likes_trains".to_string()),
            query_terms: vec!["trains".to_string()],
            sensitivity: Sensitivity::Normal,
            limit: 12,
        })
        .await
        .expect("matches");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].claim_id, summary.claim_id);
    assert_eq!(matches[0].fact, "Kevin likes trains.");
}

#[tokio::test]
async fn related_claim_relation_can_be_persisted() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let aviation_item = create_source_item(&store, "Kevin likes commercial aviation.").await;
    let train = store
        .create_or_reinforce_claim(train_claim(train_item.item_id))
        .await
        .expect("train claim");
    let mut aviation = train_claim(aviation_item.item_id);
    aviation.object = EntityCandidate::concept(
        "claim_object:likes:commercial aviation",
        "commercial aviation",
    );
    aviation.fact = "Kevin likes commercial aviation.".to_string();
    let aviation = store
        .create_or_reinforce_claim(aviation)
        .await
        .expect("aviation claim");

    store
        .relate_claims(crate::store::RelatedClaimCandidate {
            claim_id: aviation.claim_id.clone(),
            related_claim_id: train.claim_id.clone(),
            relation_kind: "related_preference".to_string(),
            rationale: "Both claims describe aviation-related preferences.".to_string(),
        })
        .await
        .expect("relate claims");

    let relations = store
        .related_claims(&aviation.claim_id)
        .await
        .expect("relations");
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].related_claim_id, train.claim_id);
    assert_eq!(relations[0].relation_kind, "related_preference");
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p noema-core bounded_consolidation_match_search_finds_same_subject_predicate_claims related_claim_relation_can_be_persisted --no-fail-fast
```

Expected: FAIL because match and relation APIs do not exist.

- [ ] **Step 3: Add schema for related claims**

Modify `crates/noema-core/src/store/schema.rs` after the `derived_from` table:

```surql
DEFINE TABLE IF NOT EXISTS related_to SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE related_to TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE related_to TYPE string;
DEFINE FIELD IF NOT EXISTS related_claim_id ON TABLE related_to TYPE string;
DEFINE FIELD IF NOT EXISTS relation_kind ON TABLE related_to TYPE string;
DEFINE FIELD IF NOT EXISTS rationale ON TABLE related_to TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE related_to TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE related_to TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS related_to_relation_id ON TABLE related_to COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS related_to_claim_pair ON TABLE related_to COLUMNS claim_id, related_claim_id, relation_kind UNIQUE;
```

- [ ] **Step 4: Add match and relation structs**

Add these types to `crates/noema-core/src/store/claims.rs` near the read-model structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationMatchRequest {
    pub subject_entity_id: String,
    pub predicate_id: String,
    pub object_entity_id: Option<String>,
    pub query_terms: Vec<String>,
    pub sensitivity: Sensitivity,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidationMatch {
    pub claim_id: String,
    pub subject_entity_id: String,
    pub object_entity_id: Option<String>,
    pub predicate_id: String,
    pub fact: String,
    pub status: ClaimStatus,
    pub sensitivity: Sensitivity,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedClaimCandidate {
    pub claim_id: String,
    pub related_claim_id: String,
    pub relation_kind: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedClaimRecord {
    pub relation_id: String,
    pub claim_id: String,
    pub related_claim_id: String,
    pub relation_kind: String,
    pub rationale: String,
    pub created_at: String,
}
```

- [ ] **Step 5: Implement match and relation methods**

Add methods to `impl NoemaStore` in `crates/noema-core/src/store/claims.rs`:

```rust
pub async fn find_consolidation_matches(
    &self,
    request: ConsolidationMatchRequest,
) -> Result<Vec<ConsolidationMatch>, StoreError> {
    let limit = request.limit.clamp(1, 20);
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
              status, sensitivity, confidence
            FROM claims
            WHERE subject_entity_id = $subject_entity_id
              AND predicate_id IN $predicate_ids
              AND status IN ['candidate', 'active', 'confirmed']
              AND sensitivity IN $allowed_sensitivities
            ORDER BY updated_at DESC, claim_id ASC
            LIMIT $limit;
            "#,
        )
        .bind(("subject_entity_id", request.subject_entity_id))
        .bind(("predicate_ids", compatible_match_predicates(&request.predicate_id)))
        .bind(("allowed_sensitivities", allowed_match_sensitivities(request.sensitivity)))
        .bind(("limit", limit))
        .await?;
    let rows: Vec<ConsolidationMatchRow> = response.take(0)?;
    rows.into_iter()
        .filter(|row| {
            if let Some(object_entity_id) = request.object_entity_id.as_deref()
                && row.object_entity_id.as_deref() == Some(object_entity_id)
            {
                return true;
            }
            query_terms.is_empty()
                || query_terms
                    .iter()
                    .any(|term| row.fact.to_ascii_lowercase().contains(term))
        })
        .map(consolidation_match)
        .collect()
}

pub async fn relate_claims(
    &self,
    candidate: RelatedClaimCandidate,
) -> Result<RelatedClaimRecord, StoreError> {
    let relation_id = allocate_id("related_claim");
    self.db
        .query(
            r#"
            CREATE type::record('related_to', $record_id) SET
              relation_id = $relation_id,
              claim_id = $claim_id,
              related_claim_id = $related_claim_id,
              relation_kind = $relation_kind,
              rationale = $rationale,
              metadata = {};
            "#,
        )
        .bind(("record_id", record_fragment(&relation_id)))
        .bind(("relation_id", relation_id.clone()))
        .bind(("claim_id", candidate.claim_id))
        .bind(("related_claim_id", candidate.related_claim_id))
        .bind(("relation_kind", candidate.relation_kind))
        .bind(("rationale", candidate.rationale))
        .await?
        .check()?;
    self.related_claim_by_relation_id(&relation_id)
        .await?
        .ok_or_else(|| StoreError::Schema(format!("missing related claim relation after write: {relation_id}")))
}

pub async fn related_claims(
    &self,
    claim_id: &str,
) -> Result<Vec<RelatedClaimRecord>, StoreError> {
    let mut response = self
        .db
        .query(
            r#"
            SELECT relation_id, claim_id, related_claim_id, relation_kind, rationale, created_at
            FROM related_to
            WHERE claim_id = $claim_id
            ORDER BY created_at ASC, relation_id ASC;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .await?;
    let rows: Vec<RelatedClaimRow> = response.take(0)?;
    rows.into_iter().map(related_claim_record).collect()
}
```

Add private helpers:

```rust
#[derive(Debug, Deserialize, SurrealValue)]
struct ConsolidationMatchRow {
    claim_id: String,
    subject_entity_id: String,
    object_entity_id: Option<String>,
    predicate_id: String,
    fact: String,
    status: String,
    sensitivity: String,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct RelatedClaimRow {
    relation_id: String,
    claim_id: String,
    related_claim_id: String,
    relation_kind: String,
    rationale: String,
    created_at: Datetime,
}

fn allowed_match_sensitivities(sensitivity: Sensitivity) -> Vec<String> {
    match sensitivity {
        Sensitivity::Public => vec!["public".to_string()],
        Sensitivity::Normal => vec!["public".to_string(), "normal".to_string()],
        Sensitivity::Private | Sensitivity::Sensitive | Sensitivity::Secret => {
            vec![sensitivity_to_store(sensitivity).to_string()]
        }
    }
}

fn compatible_match_predicates(predicate_id: &str) -> Vec<String> {
    match predicate_id {
        "likes" => vec!["likes".to_string(), "dislikes".to_string()],
        "dislikes" => vec!["dislikes".to_string(), "likes".to_string()],
        other => vec![other.to_string()],
    }
}

fn consolidation_match(row: ConsolidationMatchRow) -> Result<ConsolidationMatch, StoreError> {
    Ok(ConsolidationMatch {
        claim_id: row.claim_id,
        subject_entity_id: row.subject_entity_id,
        object_entity_id: row.object_entity_id,
        predicate_id: row.predicate_id,
        fact: row.fact,
        status: ClaimStatus::parse(&row.status)?,
        sensitivity: parse_sensitivity(&row.sensitivity)?,
        confidence: row.confidence,
    })
}

fn related_claim_record(row: RelatedClaimRow) -> Result<RelatedClaimRecord, StoreError> {
    Ok(RelatedClaimRecord {
        relation_id: row.relation_id,
        claim_id: row.claim_id,
        related_claim_id: row.related_claim_id,
        relation_kind: row.relation_kind,
        rationale: row.rationale,
        created_at: format_datetime(row.created_at),
    })
}
```

Add `related_claim_by_relation_id` as a private query mirroring `related_claims` with `WHERE relation_id = $relation_id LIMIT 1`.

- [ ] **Step 6: Export store types**

Modify `crates/noema-core/src/store.rs` and `crates/noema-core/src/lib.rs` to export:

```rust
ConsolidationMatch, ConsolidationMatchRequest, RelatedClaimCandidate, RelatedClaimRecord,
```

- [ ] **Step 7: Run the tests**

Run:

```bash
cargo test -p noema-core bounded_consolidation_match_search_finds_same_subject_predicate_claims related_claim_relation_can_be_persisted --no-fail-fast
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/store/schema.rs crates/noema-core/src/store/claims.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/tests/claims.rs
git commit -m "feat: add memory consolidation match support"
```

## Task 4: Route Existing Writes Through The Pipeline With Deterministic Canonicalization

**Files:**
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write failing runtime test for provider loves canonicalization**

Add this test to `crates/noema-core/src/daemon/tests.rs`:

```rust
#[tokio::test]
async fn provider_user_loves_planes_canonicalizes_to_likes_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I love planes.".to_string(),
    )
    .await
    .expect("turn");

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
                && metadata["created_claim_count"] == 1
        )
    }));

    let claims = store
        .retrieve_claims(&answer_claim_request(), "planes", 8)
        .await
        .expect("retrieve planes claim");
    assert_eq!(claims.included.len(), 1, "expected one claim: {claims:?}");
    assert_eq!(claims.included[0].predicate_id, "likes");
    assert_eq!(claims.included[0].fact, "Kevin likes planes.");
    handle.shutdown().await;
}
```

Extend `memory_extraction_output` in `crates/noema-core/src/daemon/tests.rs`:

```rust
if input.contains("I love planes.") {
    return vec![
        GenerateOutputItem::AssistantText {
            text: "fake answer".to_string(),
        },
        GenerateOutputItem::MemoryProposals {
            proposals: vec![proposal(json!({
                "content": "The user loves planes.",
                "memory_type": "preference",
                "title": "Plane preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "The user loves planes."},
                "risk_flags": [],
                "evidence_excerpt": "I love planes."
            }))],
        },
    ];
}
```

- [ ] **Step 2: Run the failing runtime test**

Run:

```bash
cargo test -p noema-core provider_user_loves_planes_canonicalizes_to_likes_claim --no-fail-fast
```

Expected: FAIL because the provider parser still falls back to `has_note`.

- [ ] **Step 3: Add proposal conversion helpers**

In `crates/noema-core/src/daemon/memory_pipeline.rs`, replace direct candidate builder usage with helper functions that produce `MemoryWriteProposal` first:

```rust
pub(super) fn explicit_memory_write_proposal(
    content: &str,
    context: &ConversationMemoryContext,
) -> MemoryWriteProposal {
    MemoryWriteProposal {
        source_kind: MemoryWriteSourceKind::ExplicitRemember,
        source_item_id: context.user_item_id.clone(),
        source_actor_id: "human:local".to_string(),
        source_excerpt: content.to_string(),
        owner_object_type: "human".to_string(),
        owner_object_id: "human:local".to_string(),
        raw_text: content.to_string(),
        memory_type: "note".to_string(),
        sensitivity: infer_chat_sensitivity(content),
        risk_flags: Vec::new(),
        retrieval_hints: json!({}),
        metadata: json!({
            "trigger": "explicit_remember",
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
        }),
    }
}

pub(super) fn provider_memory_write_proposal(
    validated: &ValidatedMemoryProposal,
    context: &ConversationMemoryContext,
    proposal_index: usize,
    trigger: &str,
) -> MemoryWriteProposal {
    let proposal = &validated.proposal;
    let evidence_source = evidence_source_for_excerpt(&proposal.evidence_excerpt, context);
    MemoryWriteProposal {
        source_kind: MemoryWriteSourceKind::OrdinaryChat,
        source_item_id: evidence_source.source_item_id,
        source_actor_id: match evidence_source.source {
            ProviderEvidenceSource::User => "human:local",
            ProviderEvidenceSource::Assistant => "agent:primary",
        }
        .to_string(),
        source_excerpt: proposal.evidence_excerpt.clone(),
        owner_object_type: "human".to_string(),
        owner_object_id: "human:local".to_string(),
        raw_text: proposal.content.clone(),
        memory_type: memory_type_label(proposal.memory_type).to_string(),
        sensitivity: proposal.sensitivity,
        risk_flags: proposal.risk_flags.iter().map(|flag| format!("{flag:?}")).collect(),
        retrieval_hints: serde_json::to_value(&proposal.retrieval_hints).unwrap_or_else(|_| json!({})),
        metadata: json!({
            "trigger": trigger,
            "source": "provider_structured_output",
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
            "proposal_index": proposal_index,
            "title": proposal.title,
            "cwd_project_hint": project_scope_from_cwd(context.cwd.as_deref()),
        }),
    }
}
```

- [ ] **Step 4: Add deterministic canonicalization adapter**

In `crates/noema-core/src/daemon/memory_pipeline.rs`, add:

```rust
pub(super) fn deterministic_canonical_claim(
    proposal: &MemoryWriteProposal,
    status: ClaimStatus,
    confidence: Option<f64>,
    authority: EvidenceAuthority,
) -> NewClaimCandidate {
    let parsed = parse_provider_claim(&proposal.raw_text, None);
    NewClaimCandidate {
        subject: EntityCandidate::local_human(),
        object: claim_object_entity(parsed.predicate_id, &parsed.object_phrase),
        predicate_id: parsed.predicate_id.to_string(),
        fact: parsed.fact.clone(),
        sensitivity: proposal.sensitivity,
        status,
        confidence,
        evidence: EvidenceCandidate {
            source_item_id: proposal.source_item_id.clone(),
            authority,
            excerpt: Some(proposal.source_excerpt.clone()),
        },
        retrieval_hints: if proposal.retrieval_hints.is_object() {
            proposal.retrieval_hints.clone()
        } else {
            json!({
                "keywords": [parsed.object_phrase],
                "summary": parsed.fact,
            })
        },
        metadata: proposal.metadata.clone(),
    }
}
```

Update the `parse_provider_claim` likes prefix list to include these variants:

```rust
"local human loves ",
"the user loves ",
"user loves ",
"current human loves ",
"current user loves ",
```

Update dislikes and prefers lists with matching `local human`, `the user`, `user`, and `current user` forms for consistency.

Make the existing status conversion helper visible to `runtime.rs`:

```rust
pub(super) fn claim_status_from_memory_status(status: MemoryStatus) -> ClaimStatus {
```

- [ ] **Step 5: Wire runtime to use proposal helpers**

Modify `persist_provider_memory_proposals` in `crates/noema-core/src/daemon/runtime.rs`:

```rust
let write_proposal = provider_memory_write_proposal(
    &proposal.proposal,
    &proposal.context,
    proposal_index,
    "ordinary_chat",
);
let candidate = deterministic_canonical_claim(
    &write_proposal,
    claim_status_from_memory_status(proposal.proposal.status),
    Some(f64::from(proposal.proposal.proposal.confidence)),
    EvidenceAuthority::AgentInference,
);
```

Modify `persist_explicit_memory_claim`:

```rust
let write_proposal = explicit_memory_write_proposal(content, context);
let candidate = deterministic_canonical_claim(
    &write_proposal,
    ClaimStatus::Confirmed,
    Some(1.0),
    EvidenceAuthority::ExplicitHumanStatement,
);
```

Keep the existing `provider_memory_claim_candidate` and `explicit_memory_claim_candidate` tests compiling by either rewriting those functions as wrappers around the new helpers or updating tests to use the new helper path.

- [ ] **Step 6: Run targeted runtime tests**

Run:

```bash
cargo test -p noema-core provider_user_loves_planes_canonicalizes_to_likes_claim provider_first_person_memory_reinforces_explicit_canonical_claim repeated_explicit_memory_reinforces_one_claim --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: route memory writes through proposals"
```

## Task 5: Add LLM Canonicalization And Predicate Proposal Persistence

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`

- [ ] **Step 1: Write failing tests for unknown predicate proposals**

Add this runtime test:

```rust
#[tokio::test]
async fn unknown_memory_relationship_creates_predicate_proposal() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I collect model aircraft.".to_string(),
    )
    .await
    .expect("turn");

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
                && title == "Memory needs review"
                && metadata["predicate_proposal_count"] == 1
        )
    }));

    let proposals = store
        .list_predicate_proposals(crate::store::PredicateProposalFilter {
            status: Some("candidate".to_string()),
            limit: Some(10),
        })
        .await
        .expect("predicate proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].label, "collects");

    let claims = store
        .retrieve_claims(&answer_claim_request(), "model aircraft", 8)
        .await
        .expect("retrieve claims");
    assert!(claims.included.is_empty(), "unpromoted predicate should not retrieve: {claims:?}");
    handle.shutdown().await;
}
```

Extend `memory_extraction_output`:

```rust
if input.contains("I collect model aircraft.") && !input.contains("memory claim canonicalizer") {
    return vec![
        GenerateOutputItem::AssistantText {
            text: "fake answer".to_string(),
        },
        GenerateOutputItem::MemoryProposals {
            proposals: vec![proposal(json!({
                "content": "Kevin collects model aircraft.",
                "memory_type": "preference",
                "title": "Model aircraft collection",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["hobbies"], "keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                "risk_flags": [],
                "evidence_excerpt": "I collect model aircraft."
            }))],
        },
    ];
}
```

Add a canonicalizer branch:

```rust
if input.contains("Noema's memory claim canonicalizer") {
    let response = if input.contains("Kevin collects model aircraft.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:model_aircraft", "entity_type": "concept", "canonical_name": "model aircraft"},
                "predicate": {
                    "kind": "predicate_proposal",
                    "proposal": {
                        "label": "collects",
                        "description": "The subject collects the object.",
                        "allowed_subject_types": ["human", "person"],
                        "allowed_object_types": ["concept", "other"],
                        "allowed_use_modes": ["answer", "personalize"],
                        "default_sensitivity": "normal",
                        "conflict_policy": "allow_many",
                        "review_policy": "auto_candidate",
                        "inverse_behavior": "none",
                        "inverse_predicate_id": null,
                        "proactivity_default": 1,
                        "merge_hints": {"strategy": "object_identity"},
                        "synonym_hints": ["keeps a collection of"],
                        "extraction_hints": {"examples": ["I collect model aircraft"]},
                        "rationale": "No promoted predicate represents collecting."
                    }
                },
                "fact": "Kevin collects model aircraft.",
                "sensitivity": "normal",
                "status": "candidate",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                "rationale": "The source states a durable collecting relationship."
            }]
        })
    } else if input.contains("Kevin enjoys ice cream desserts.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:claim_object_likes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
                "fact": "Kevin likes ice cream.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                "rationale": "The dessert statement restates the durable ice cream preference."
            }]
        })
    } else if input.contains("Kevin hates ice cream.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:claim_object_dislikes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "dislikes"},
                "fact": "Kevin dislikes ice cream.",
                "sensitivity": "normal",
                "status": "candidate",
                "confidence": 0.91,
                "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin dislikes ice cream."},
                "rationale": "The source directly states a dislike that may conflict with an existing like."
            }]
        })
    } else {
        json!({"candidates": []})
    };
    return vec![GenerateOutputItem::AssistantText {
        text: serde_json::to_string(&response).expect("canonicalizer json"),
    }];
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p noema-core unknown_memory_relationship_creates_predicate_proposal --no-fail-fast
```

Expected: FAIL because runtime does not call canonicalization or persist predicate proposals.

- [ ] **Step 3: Add provider canonicalization helper**

In `crates/noema-core/src/daemon/runtime.rs`, add:

```rust
async fn canonicalize_memory_write(
    &self,
    proposal: &MemoryWriteProposal,
) -> Result<Vec<CanonicalClaimCandidate>, DaemonError> {
    let predicates = self.store.predicate_catalog().await?;
    let catalog_json = serde_json::to_value(predicates)
        .map_err(|error| DaemonError::Protocol(format!("predicate catalog serialization failed: {error}")))?;
    let prompt = build_claim_canonicalization_prompt(proposal, &catalog_json);
    let mut ignored_events = |_| {};
    let response = self
        .provider
        .generate_streaming(GenerateRequest::text(prompt), &mut ignored_events)
        .await
        .map_err(DaemonError::Provider)?;
    let parsed = parse_canonicalization_response(&response.assistant_text())
        .map_err(|error| DaemonError::Protocol(format!("memory canonicalization failed: {error}")))?;
    Ok(parsed.candidates)
}
```

Import the types and functions from `crate::memory_consolidation`.

- [ ] **Step 4: Convert canonical candidates to store candidates**

In `crates/noema-core/src/daemon/memory_pipeline.rs`, add:

```rust
pub(super) fn new_claim_from_canonical(
    candidate: &CanonicalClaimCandidate,
    proposal: &MemoryWriteProposal,
    authority: EvidenceAuthority,
) -> Option<NewClaimCandidate> {
    let PredicateResolution::PromotedPredicate { predicate_id } = &candidate.predicate else {
        return None;
    };
    Some(NewClaimCandidate {
        subject: EntityCandidate {
            entity_id: candidate.subject.entity_id.clone(),
            entity_type: entity_type_from_canonical(&candidate.subject.entity_type),
            canonical_name: candidate.subject.canonical_name.clone(),
        },
        object: EntityCandidate {
            entity_id: candidate.object.entity_id.clone(),
            entity_type: entity_type_from_canonical(&candidate.object.entity_type),
            canonical_name: candidate.object.canonical_name.clone(),
        },
        predicate_id: predicate_id.clone(),
        fact: candidate.fact.clone(),
        sensitivity: candidate.sensitivity,
        status: claim_status_from_canonical(candidate.status),
        confidence: Some(candidate.confidence),
        evidence: EvidenceCandidate {
            source_item_id: proposal.source_item_id.clone(),
            authority,
            excerpt: Some(proposal.source_excerpt.clone()),
        },
        retrieval_hints: candidate.retrieval_hints.clone(),
        metadata: proposal.metadata.clone(),
    })
}

pub(super) fn predicate_proposal_candidate_from_canonical(
    candidate: &CanonicalClaimCandidate,
    proposal: &MemoryWriteProposal,
) -> Option<PredicateProposalCandidate> {
    let PredicateResolution::PredicateProposal { proposal: predicate } = &candidate.predicate else {
        return None;
    };
    Some(PredicateProposalCandidate {
        label: predicate.label.clone(),
        description: predicate.description.clone(),
        proposed_predicate: serde_json::to_value(predicate).expect("predicate proposal value"),
        source_item_id: Some(proposal.source_item_id.clone()),
        proposed_claim: json!({
            "subject": candidate.subject,
            "object": candidate.object,
            "fact": candidate.fact,
            "sensitivity": canonical_sensitivity_label(candidate.sensitivity),
            "status": "candidate",
            "confidence": candidate.confidence,
            "retrieval_hints": candidate.retrieval_hints,
            "rationale": candidate.rationale,
        }),
    })
}
```

Add helper functions `entity_type_from_canonical`,
`claim_status_from_canonical`, and `canonical_sensitivity_label` beside
existing status helpers:

```rust
fn entity_type_from_canonical(entity_type: &str) -> EntityType {
    match entity_type {
        "human" => EntityType::Human,
        "agent" => EntityType::Agent,
        "person" => EntityType::Person,
        "organization" => EntityType::Organization,
        "project" => EntityType::Project,
        "workspace" => EntityType::Workspace,
        "conversation" => EntityType::Conversation,
        "document" => EntityType::Document,
        "tool" => EntityType::Tool,
        "place" => EntityType::Place,
        "task" => EntityType::Task,
        "goal" => EntityType::Goal,
        "concept" => EntityType::Concept,
        _ => EntityType::Other,
    }
}

fn claim_status_from_canonical(status: CanonicalClaimStatus) -> ClaimStatus {
    match status {
        CanonicalClaimStatus::Candidate => ClaimStatus::Candidate,
        CanonicalClaimStatus::Active => ClaimStatus::Active,
        CanonicalClaimStatus::Confirmed => ClaimStatus::Confirmed,
    }
}

fn canonical_sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}
```

- [ ] **Step 5: Persist predicate proposals from runtime**

In `persist_provider_memory_proposals`, after building `write_proposal`, call `canonicalize_memory_write`. For each canonical candidate:

```rust
match &canonical.predicate {
    PredicateResolution::PromotedPredicate { .. } => {
        if let Some(candidate) = new_claim_from_canonical(
            &canonical,
            &write_proposal,
            EvidenceAuthority::AgentInference,
        ) {
            match self.store.create_or_reinforce_claim(candidate).await {
                Ok(summary) => {
                    match summary.write_outcome {
                        ClaimWriteOutcome::Created => created_claim_count += 1,
                        ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                    }
                    claim_outcomes.push(claim_outcome_json(&summary));
                    claim_ids.push(summary.claim_id);
                }
                Err(error) => failed_proposals.push(json!({
                    "proposal_index": proposal_index,
                    "error": error.to_string(),
                })),
            }
        }
    }
    PredicateResolution::PredicateProposal { .. } => {
        if let Some(candidate) = predicate_proposal_candidate_from_canonical(
            &canonical,
            &write_proposal,
        ) {
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
                Err(error) => failed_proposals.push(json!({
                    "proposal_index": proposal_index,
                    "error": error.to_string(),
                })),
            }
        }
    }
    PredicateResolution::FallbackNote => {
        let candidate = deterministic_canonical_claim(
            &write_proposal,
            claim_status_from_memory_status(proposal.proposal.status),
            Some(f64::from(proposal.proposal.proposal.confidence)),
            EvidenceAuthority::AgentInference,
        );
        match self.store.create_or_reinforce_claim(candidate).await {
            Ok(summary) => {
                match summary.write_outcome {
                    ClaimWriteOutcome::Created => created_claim_count += 1,
                    ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                }
                claim_outcomes.push(claim_outcome_json(&summary));
                claim_ids.push(summary.claim_id);
            }
            Err(error) => failed_proposals.push(json!({
                "proposal_index": proposal_index,
                "error": error.to_string(),
            })),
        }
    }
}
```

Use the title `Memory needs review` when at least one predicate proposal was stored and no active claim was saved. Include `predicate_proposal_count` in metadata.

If `canonicalize_memory_write` returns an empty candidate list, use
`deterministic_canonical_claim` so existing promoted and fallback writes continue
to function while prompt coverage grows.

- [ ] **Step 6: Run targeted tests**

Run:

```bash
cargo test -p noema-core unknown_memory_relationship_creates_predicate_proposal provider_user_loves_planes_canonicalizes_to_likes_claim --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: canonicalize memory writes with predicates"
```

## Task 6: Add Consolidation Decisions For Promoted Claims

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/memory_pipeline.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write failing reinforcement and dispute tests**

Add these tests to `crates/noema-core/src/daemon/tests.rs`:

```rust
#[tokio::test]
async fn semantic_repeat_reinforces_existing_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(&handle, conversation_id.clone(), "I like ice cream.".to_string())
        .await
        .expect("seed turn");
    let items = collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("repeat turn");

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

    let claims = store
        .retrieve_claims(&answer_claim_request(), "ice cream", 8)
        .await
        .expect("retrieve ice cream");
    assert_eq!(claims.included.len(), 1, "expected one reinforced claim: {claims:?}");
    handle.shutdown().await;
}

#[tokio::test]
async fn contradiction_becomes_reviewable_dispute() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(&handle, conversation_id.clone(), "I like ice cream.".to_string())
        .await
        .expect("seed turn");
    let items = collect_turn(&handle, conversation_id, "I hate ice cream.".to_string())
        .await
        .expect("conflict turn");

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
                && title == "Memory needs review"
                && metadata["disputed_claim_count"] == 1
        )
    }));

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Disputed),
            predicate_id: None,
            limit: Some(10),
        })
        .await
        .expect("disputed claims");
    assert_eq!(claims.len(), 1);
    handle.shutdown().await;
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p noema-core semantic_repeat_reinforces_existing_claim contradiction_becomes_reviewable_dispute --no-fail-fast
```

Expected: FAIL because semantic comparison is not wired.

- [ ] **Step 3: Add runtime consolidation helper**

In `crates/noema-core/src/daemon/runtime.rs`, add:

```rust
async fn consolidate_promoted_claim(
    &self,
    candidate: NewClaimCandidate,
    canonical: &CanonicalClaimCandidate,
) -> Result<PersistedMemoryOutcome, DaemonError> {
    let query_terms = canonical
        .retrieval_hints
        .get("keywords")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let matches = self
        .store
        .find_consolidation_matches(ConsolidationMatchRequest {
            subject_entity_id: candidate.subject.entity_id.clone(),
            predicate_id: candidate.predicate_id.clone(),
            object_entity_id: Some(candidate.object.entity_id.clone()),
            query_terms,
            sensitivity: candidate.sensitivity,
            limit: 12,
        })
        .await?;

    if matches.is_empty() {
        let summary = self.store.create_or_reinforce_claim(candidate).await?;
        return Ok(PersistedMemoryOutcome::from_claim_summary(summary));
    }

    let existing_json = serde_json::to_value(
        matches
            .iter()
            .map(|item| json!({
                "memory_id": item.claim_id,
                "claim_id": item.claim_id,
                "fact": item.fact,
                "predicate_id": item.predicate_id,
                "status": claim_status_label(item.status),
                "sensitivity": sensitivity_label(item.sensitivity),
            }))
            .collect::<Vec<_>>(),
    )
    .map_err(|error| DaemonError::Protocol(format!("match serialization failed: {error}")))?;
    let prompt = build_consolidation_prompt(canonical, &existing_json);
    let mut ignored_events = |_| {};
    let response = self
        .provider
        .generate_streaming(GenerateRequest::text(prompt), &mut ignored_events)
        .await
        .map_err(DaemonError::Provider)?;
    let decision = parse_consolidation_decision(&response.assistant_text())
        .map_err(|error| DaemonError::Protocol(format!("memory consolidation failed: {error}")))?;
    self.persist_consolidation_decision(candidate, decision).await
}
```

Add this small runtime helper near `sensitivity_label`:

```rust
fn claim_status_label(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Candidate => "candidate",
        ClaimStatus::Active => "active",
        ClaimStatus::Confirmed => "confirmed",
        ClaimStatus::Disputed => "disputed",
        ClaimStatus::Superseded => "superseded",
        ClaimStatus::Archived => "archived",
        ClaimStatus::Deleted => "deleted",
    }
}
```

- [ ] **Step 4: Add persistence for decision outcomes**

Add a runtime-local enum near `claim_outcome_json`:

```rust
#[derive(Debug, Clone)]
struct PersistedMemoryOutcome {
    claim_id: Option<String>,
    outcome: &'static str,
    fact_preview: String,
    sensitivity: String,
}

impl PersistedMemoryOutcome {
    fn from_claim_summary(summary: crate::ClaimSummary) -> Self {
        Self {
            claim_id: Some(summary.claim_id),
            outcome: claim_write_outcome_label(summary.write_outcome),
            fact_preview: fact_preview(&summary.fact),
            sensitivity: sensitivity_label(summary.sensitivity).to_string(),
        }
    }
}
```

Add `persist_consolidation_decision`:

```rust
async fn persist_consolidation_decision(
    &self,
    mut candidate: NewClaimCandidate,
    decision: ConsolidationDecision,
) -> Result<PersistedMemoryOutcome, DaemonError> {
    match decision.decision {
        ConsolidationDecisionKind::Create => {
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            Ok(PersistedMemoryOutcome::from_claim_summary(summary))
        }
        ConsolidationDecisionKind::Reinforce => {
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            Ok(PersistedMemoryOutcome::from_claim_summary(summary))
        }
        ConsolidationDecisionKind::Dispute => {
            candidate.status = ClaimStatus::Disputed;
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            Ok(PersistedMemoryOutcome {
                claim_id: Some(summary.claim_id),
                outcome: "disputed",
                fact_preview: fact_preview(&summary.fact),
                sensitivity: sensitivity_label(summary.sensitivity).to_string(),
            })
        }
        ConsolidationDecisionKind::Relate => {
            let related_claim_id = decision.existing_claim_id.clone().ok_or_else(|| {
                DaemonError::Protocol("relate decision missing existing claim id".to_string())
            })?;
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            self.store
                .relate_claims(RelatedClaimCandidate {
                    claim_id: summary.claim_id.clone(),
                    related_claim_id,
                    relation_kind: "semantic_related".to_string(),
                    rationale: decision.rationale,
                })
                .await?;
            Ok(PersistedMemoryOutcome {
                claim_id: Some(summary.claim_id),
                outcome: "related",
                fact_preview: fact_preview(&summary.fact),
                sensitivity: sensitivity_label(summary.sensitivity).to_string(),
            })
        }
        ConsolidationDecisionKind::Supersede | ConsolidationDecisionKind::NeedsReview => {
            candidate.status = ClaimStatus::Candidate;
            let summary = self.store.create_or_reinforce_claim(candidate).await?;
            Ok(PersistedMemoryOutcome {
                claim_id: Some(summary.claim_id),
                outcome: "needs_review",
                fact_preview: fact_preview(&summary.fact),
                sensitivity: sensitivity_label(summary.sensitivity).to_string(),
            })
        }
    }
}
```

This first implementation records disputes as disputed claims and review needs
as candidate claims. `relate_claims` covers the approved relation case in this
plan; `supersedes` already exists in the schema and can be wired by changing the
`Supersede` match arm to insert that relation if product behavior requires
automatic supersession.

- [ ] **Step 5: Replace direct claim summary counters with outcome counters**

When processing promoted canonical claims, call `consolidate_promoted_claim`. Increment counts based on `PersistedMemoryOutcome.outcome`:

```rust
match outcome.outcome {
    "created" => created_claim_count += 1,
    "reinforced" => reinforced_claim_count += 1,
    "disputed" => disputed_claim_count += 1,
    "related" => related_claim_count += 1,
    "needs_review" => needs_review_claim_count += 1,
    _ => {}
}
claim_outcomes.push(json!({
    "claim_id": outcome.claim_id,
    "outcome": outcome.outcome,
    "fact_preview": outcome.fact_preview,
    "sensitivity": outcome.sensitivity,
}));
```

Use `Memory needs review` when `saved_claim_count == 0` and any review/dispute count is positive. Include `disputed_claim_count`, `related_claim_count`, and `needs_review_claim_count` in metadata.

- [ ] **Step 6: Run targeted tests**

Run:

```bash
cargo test -p noema-core semantic_repeat_reinforces_existing_claim contradiction_becomes_reviewable_dispute provider_first_person_memory_reinforces_explicit_canonical_claim --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/memory_pipeline.rs crates/noema-core/src/daemon/tests.rs
git commit -m "feat: consolidate promoted memory claims"
```

## Task 7: Add Predicate Proposal GraphQL And CLI Inspection

**Files:**
- Modify: `crates/noema-core/src/graphql/types.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-cli/src/inspection.rs`
- Modify: `crates/noema-cli/src/main.rs`
- Modify: `crates/noema-core/web/src/generated/schema.graphql`

- [ ] **Step 1: Write failing GraphQL schema and query tests**

In `crates/noema-core/src/graphql/schema.rs`, extend `schema_sdl_exposes_initial_noema_fields`:

```rust
assert!(sdl.contains("memoryPredicateProposals"));
assert!(sdl.contains("memoryPredicateProposal"));
assert!(sdl.contains("type GraphqlPredicateProposal"));
```

Add this async test:

```rust
#[tokio::test]
async fn predicate_proposal_query_returns_seeded_candidate() {
    use crate::{ActorRef, ConversationItemKind, ConversationItemStatus, NewConversation,
        NewConversationItem, NewConversationTurn, store::tests::test_store};

    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .await
        .expect("turn");
    let item = store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id,
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I collect model aircraft.".to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("source item");
    let proposal = store
        .create_predicate_proposal(crate::PredicateProposalCandidate {
            label: "collects".to_string(),
            description: "The subject collects the object.".to_string(),
            proposed_predicate: json!({"allowed_subject_types": ["human"]}),
            source_item_id: Some(item.item_id),
            proposed_claim: json!({"fact": "Kevin collects model aircraft."}),
        })
        .await
        .expect("proposal");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(async_graphql::Request::new(format!(
            r#"
            {{
              memoryPredicateProposal(proposalId: "{}") {{
                proposalId
                label
                status
                proposedClaim
              }}
              memoryPredicateProposals(limit: 10) {{
                proposalId
                label
              }}
            }}
            "#,
            proposal.proposal_id
        )))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    assert_eq!(data["memoryPredicateProposal"]["label"], "collects");
    assert_eq!(data["memoryPredicateProposals"][0]["proposalId"], proposal.proposal_id);
}
```

- [ ] **Step 2: Run failing GraphQL tests**

Run:

```bash
cargo test -p noema-core predicate_proposal_query_returns_seeded_candidate schema_sdl_exposes_initial_noema_fields --no-fail-fast
```

Expected: FAIL because GraphQL types/queries do not exist.

- [ ] **Step 3: Add GraphQL types**

In `crates/noema-core/src/graphql/types.rs`, add:

```rust
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlPredicateProposal {
    pub proposal_id: String,
    pub label: String,
    pub description: String,
    pub proposed_predicate: Json<Value>,
    pub proposed_claim: Json<Value>,
    pub status: String,
    pub source_item_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<crate::PredicateProposalRecord> for GraphqlPredicateProposal {
    fn from(record: crate::PredicateProposalRecord) -> Self {
        Self {
            proposal_id: record.proposal_id,
            label: record.label,
            description: record.description,
            proposed_predicate: Json(record.proposed_predicate),
            proposed_claim: Json(record.proposed_claim),
            status: record.status,
            source_item_id: record.source_item_id,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}
```

- [ ] **Step 4: Add GraphQL queries**

In `crates/noema-core/src/graphql/schema.rs`, import `GraphqlPredicateProposal` and add to `QueryRoot`:

```rust
async fn memory_predicate_proposals(
    &self,
    ctx: &Context<'_>,
    status: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<GraphqlPredicateProposal>> {
    let limit = match limit {
        Some(value) if value < 1 => {
            return Err(async_graphql::Error::new(
                "memoryPredicateProposals limit must be at least 1",
            ));
        }
        Some(value) => Some(usize::try_from(value).map_err(|_| {
            async_graphql::Error::new("memoryPredicateProposals limit is too large")
        })?),
        None => None,
    };
    let state = ctx.data_unchecked::<GraphqlState>();
    let proposals = state
        .store()?
        .list_predicate_proposals(crate::PredicateProposalFilter { status, limit })
        .await
        .map_err(graphql_error)?;
    Ok(proposals.into_iter().map(Into::into).collect())
}

async fn memory_predicate_proposal(
    &self,
    ctx: &Context<'_>,
    proposal_id: String,
) -> Result<Option<GraphqlPredicateProposal>> {
    let state = ctx.data_unchecked::<GraphqlState>();
    let proposal = state
        .store()?
        .get_predicate_proposal(&proposal_id)
        .await
        .map_err(graphql_error)?;
    Ok(proposal.map(Into::into))
}
```

- [ ] **Step 5: Add CLI subcommands**

In `crates/noema-cli/src/inspection.rs`, extend `MemoryCommand`:

```rust
#[command(about = "List predicate proposals awaiting review.")]
PredicateProposals {
    #[arg(long, default_value_t = 20, help = "Maximum proposals to show.")]
    limit: u32,
    #[arg(long, help = "Only show proposals with this status.")]
    status: Option<String>,
},
#[command(about = "Show one predicate proposal.")]
PredicateProposal {
    #[arg(value_name = "PROPOSAL_ID")]
    proposal_id: String,
},
```

Add GraphQL query strings:

```rust
const PREDICATE_PROPOSALS_QUERY: &str = r#"
query CliPredicateProposals($status: String, $limit: Int) {
  memoryPredicateProposals(status: $status, limit: $limit) {
    proposalId
    label
    description
    status
    sourceItemId
    createdAt
    updatedAt
  }
}
"#;

const PREDICATE_PROPOSAL_QUERY: &str = r#"
query CliPredicateProposal($proposalId: String!) {
  memoryPredicateProposal(proposalId: $proposalId) {
    proposalId
    label
    description
    proposedPredicate
    proposedClaim
    status
    sourceItemId
    createdAt
    updatedAt
  }
}
"#;
```

Add response structs and `write_predicate_proposal_list` / `write_predicate_proposal_detail` functions that mirror the existing memory claim list/detail style.

- [ ] **Step 6: Regenerate web GraphQL schema**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated schema/types update with `memoryPredicateProposals` and `memoryPredicateProposal`.

- [ ] **Step 7: Run GraphQL and CLI tests**

Run:

```bash
cargo test -p noema-core predicate_proposal_query_returns_seeded_candidate schema_sdl_exposes_initial_noema_fields --no-fail-fast
cargo test -p noema-cli parses_memory_list_subcommand parses_memory_show_subcommand --no-fail-fast
```

Expected: PASS. Add parser tests for the new CLI subcommands if the clap test section is straightforward to extend.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/graphql/types.rs crates/noema-core/src/graphql/schema.rs crates/noema-cli/src/inspection.rs crates/noema-cli/src/main.rs crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "feat: inspect predicate proposals"
```

## Task 8: Full Validation And Documentation

**Files:**
- Modify: `docs/context/current.md`
- Modify: `docs/frontend/current-contract.md`
- Review: all files changed in Tasks 1-7

- [ ] **Step 1: Update durable context**

In `docs/context/current.md`, replace the open-loop line for the consolidation pipeline with a settled-decision note:

```markdown
- The long-term future-write memory consolidation pipeline has landed:
  all explicit and provider memory writes use a shared proposal/canonicalization
  path, promoted predicates are validated before active claims, unknown
  predicates create review-gated predicate proposals, bounded match search
  supports conservative consolidation, and GraphQL/CLI expose predicate proposal
  inspection.
```

- [ ] **Step 2: Update frontend/API contract**

In `docs/frontend/current-contract.md`, add the new memory inspection contract near the existing memory read model section:

```markdown
| Predicate proposal inspection | Current Rust-backed | GraphQL `memoryPredicateProposals` and `memoryPredicateProposal`; CLI mirrors these for owner/admin inspection. Proposals are review-gated and do not unlock ordinary retrieval until promoted or merged. |
```

- [ ] **Step 3: Run Rust formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, inspect the diff, and rerun `cargo fmt --all --check`.

- [ ] **Step 4: Run Rust check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 5: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. Run unit tests only; do not run smoke or fixture tests.

- [ ] **Step 7: Run frontend validation if generated GraphQL changed**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 8: Run final ship checks**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: clean unstaged diff except intentionally staged final docs. `git diff --check` must pass.

- [ ] **Step 9: Commit final docs and generated outputs**

```bash
git add docs/context/current.md docs/frontend/current-contract.md crates/noema-core/web/src/generated/schema.graphql crates/noema-core/web/src/generated/graphql.ts
git commit -m "docs: record memory consolidation pipeline"
```

## Final Verification Checklist

- [ ] All explicit and provider writes route through `MemoryWriteProposal`.
- [ ] Provider "The user loves planes" creates a `likes` claim, not `has_note`.
- [ ] Unknown predicates create `predicate_proposals` and do not enter ordinary retrieval.
- [ ] Repeated equivalent promoted claims reinforce existing claims.
- [ ] Contradictions become disputed or review-gated instead of overwriting active truth.
- [ ] Related non-identical claims can be linked without collapsing truth.
- [ ] Predicate proposal inspection works through GraphQL and CLI.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo check --workspace` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace --no-fail-fast` passes.
- [ ] Web generated GraphQL files are current.
