# Memory Consolidation Dedupe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent repeated explicit or extracted memories from creating duplicate memory rows, and route semantic repeats/conflicts through auditable consolidation outcomes.

**Architecture:** Add deterministic dedupe fingerprints inside memory persistence, then add a daemon-side consolidation module that turns memory candidates into `created`, `reused`, `reinforced`, or `conflict` outcomes. Exact dedupe is transactional and database-backed; semantic comparison is bounded to public/normal plausible matches and uses the existing provider `generate` interface with strict JSON.

**Tech Stack:** Rust, Tokio, SQLx/Postgres, ring digest, serde/serde_json, existing Noema daemon runtime and memory persistence modules.

---

## File Structure

- Modify `crates/noema-core/src/memory_persistence.rs`: register and export a new persistence dedupe module.
- Create `crates/noema-core/src/memory_persistence/dedupe.rs`: canonical memory candidate fingerprint normalization and SHA-256 formatting.
- Modify `crates/noema-core/src/memory_persistence/postgres_schema.rs`: add `memory_dedupe_fingerprint` and a partial unique index for non-deleted memory rows.
- Modify `crates/noema-core/src/memory_persistence/models.rs`: add `dedupe_fingerprint` to `MemorySummary` so callers can inspect exact reuse.
- Modify `crates/noema-core/src/memory_persistence/repository.rs`: read fingerprint values into `MemorySummary`.
- Modify `crates/noema-core/src/memory_persistence/provenance.rs`: make `append_memory_candidate` compute the fingerprint, reuse exact matches, insert the fingerprint, add support provenance, and expose a reinforcement helper.
- Modify `crates/noema-core/src/memory_persistence/postgres_tests.rs`: add exact fingerprint and append idempotency tests.
- Create `crates/noema-core/src/daemon/memory_consolidation.rs`: bounded match search, semantic comparator prompt/parse logic, and consolidation outcome types.
- Modify `crates/noema-core/src/daemon.rs`: register the new daemon consolidation module.
- Modify `crates/noema-core/src/daemon/runtime.rs`: route explicit remembers, provider proposals, and ordinary-chat extraction through consolidation outcomes.
- Modify `crates/noema-core/src/daemon/tests.rs`: add runtime coverage for exact repeats, semantic repeats, conflicts, and transcript payload outcomes.
- Modify `docs/context/current.md`: summarize the settled implementation detail after the code lands.

## Task 1: Add Canonical Fingerprint Computation

**Files:**
- Create: `crates/noema-core/src/memory_persistence/dedupe.rs`
- Modify: `crates/noema-core/src/memory_persistence.rs`
- Test: `crates/noema-core/src/memory_persistence/dedupe.rs`

- [ ] **Step 1: Create the failing fingerprint tests**

Create `crates/noema-core/src/memory_persistence/dedupe.rs` with the module skeleton and tests first:

```rust
use ring::digest;

use super::{
    helpers::{participant_role_to_db, sensitivity_to_db, subject_role_to_db},
    models::NewMemoryCandidate,
};

pub(crate) fn memory_candidate_dedupe_fingerprint(_candidate: &NewMemoryCandidate) -> String {
    String::new()
}

fn canonical_content(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .to_ascii_lowercase()
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::{
        memory::{ParticipantRole, Sensitivity, SubjectRole},
        memory_persistence::{
            ActorRef, MemoryExtractionMethod, MemoryType, NewMemoryCandidate,
            NewMemoryParticipant, NewMemorySubject, ObjectRef, ObjectType,
        },
    };

    use super::{canonical_content, memory_candidate_dedupe_fingerprint};

    fn candidate(content: &str) -> NewMemoryCandidate {
        let mut candidate = NewMemoryCandidate::confirmed_note(
            ObjectRef::new(ObjectType::Conversation, "conversation_1").expect("owner"),
            content,
            ActorRef::agent("agent:primary"),
            ObjectRef::conversation_item("item_1"),
        );
        candidate.memory_type = MemoryType::Preference;
        candidate.sensitivity = Sensitivity::Normal;
        candidate.extraction_method = MemoryExtractionMethod::LlmExtracted;
        candidate.participants = vec![
            NewMemoryParticipant::new(ActorRef::human("human:local"), ParticipantRole::HumanInScope),
            NewMemoryParticipant::new(ActorRef::agent("agent:primary"), ParticipantRole::AgentInScope),
        ];
        let mut subject =
            NewMemorySubject::new("human:local", "human", "Local human", SubjectRole::About);
        subject.linked_object = Some(ObjectRef::human("human:local"));
        candidate.subjects = vec![subject];
        candidate
    }

    #[test]
    fn canonical_content_normalizes_low_risk_surface_changes() {
        assert_eq!(canonical_content("  I   Like ICE CREAM!  "), "i like ice cream");
    }

    #[test]
    fn fingerprint_ignores_source_title_confidence_and_metadata() {
        let mut first = candidate("I like ice cream.");
        first.title = Some("Ice cream".to_string());
        first.confidence = Some(0.95);
        first.metadata = serde_json::json!({"turn_id": "turn_1"});

        let mut second = candidate("  I LIKE   ICE CREAM ");
        second.title = Some("Dessert preference".to_string());
        second.confidence = Some(0.72);
        second.metadata = serde_json::json!({"turn_id": "turn_2"});
        second.source = Some(crate::memory_persistence::ObjectProvenanceSource {
            source: ObjectRef::conversation_item("item_2"),
            evidence_excerpt: Some("I LIKE ICE CREAM".to_string()),
        });

        assert_eq!(
            memory_candidate_dedupe_fingerprint(&first),
            memory_candidate_dedupe_fingerprint(&second)
        );
    }

    #[test]
    fn fingerprint_changes_when_subject_changes() {
        let first = candidate("I like ice cream.");
        let mut second = candidate("I like ice cream.");
        second.subjects = vec![NewMemorySubject::new(
            "person:casey",
            "person",
            "Casey",
            SubjectRole::About,
        )];

        assert_ne!(
            memory_candidate_dedupe_fingerprint(&first),
            memory_candidate_dedupe_fingerprint(&second)
        );
    }
}
```

- [ ] **Step 2: Register the module so the failing tests compile far enough**

Modify `crates/noema-core/src/memory_persistence.rs`:

```rust
mod context_packets;
mod conversations;
mod dedupe;
mod error;
mod helpers;
mod ids;
pub(crate) mod models;
mod objects;
mod postgres_context_graph;
mod postgres_helpers;
mod postgres_schema;
#[cfg(test)]
mod postgres_tests;
mod provenance;
mod provider_accounts;
mod queries;
mod repository;

pub(crate) use dedupe::memory_candidate_dedupe_fingerprint;
```

- [ ] **Step 3: Run the fingerprint tests to verify failure**

Run:

```bash
cargo test -p noema-core memory_persistence::dedupe --no-fail-fast
```

Expected: FAIL because `memory_candidate_dedupe_fingerprint` returns an empty string and does not distinguish subject changes.

- [ ] **Step 4: Implement fingerprint computation**

Replace `memory_candidate_dedupe_fingerprint` in `crates/noema-core/src/memory_persistence/dedupe.rs`:

```rust
pub(crate) fn memory_candidate_dedupe_fingerprint(candidate: &NewMemoryCandidate) -> String {
    let mut participants = candidate
        .participants
        .iter()
        .map(|participant| {
            format!(
                "{}:{}",
                participant.participant.actor_id,
                participant_role_to_db(participant.role)
            )
        })
        .collect::<Vec<_>>();
    participants.sort();

    let mut subjects = candidate
        .subjects
        .iter()
        .map(|subject| {
            format!(
                "{}:{}:{}",
                subject.entity_id,
                subject.entity_type,
                subject_role_to_db(subject.role)
            )
        })
        .collect::<Vec<_>>();
    subjects.sort();

    let canonical = serde_json::json!({
        "owner": {
            "object_type": candidate.owner.object_type.as_str(),
            "object_id": candidate.owner.object_id.as_str(),
        },
        "memory_type": candidate.memory_type.as_str(),
        "content": canonical_content(&candidate.content),
        "sensitivity": sensitivity_to_db(candidate.sensitivity),
        "participants": participants,
        "subjects": subjects,
    });
    let bytes = serde_json::to_vec(&canonical).expect("canonical dedupe JSON is serializable");
    let digest = digest::digest(&digest::SHA256, &bytes);
    format!("sha256:{}", hex_lower(digest.as_ref()))
}
```

- [ ] **Step 5: Run the fingerprint tests to verify pass**

Run:

```bash
cargo test -p noema-core memory_persistence::dedupe --no-fail-fast
```

Expected: PASS for the three dedupe unit tests.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add crates/noema-core/src/memory_persistence.rs crates/noema-core/src/memory_persistence/dedupe.rs
git commit -m "Add memory dedupe fingerprints"
```

## Task 2: Store Fingerprints And Make Exact Appends Idempotent

**Files:**
- Modify: `crates/noema-core/src/memory_persistence/postgres_schema.rs`
- Modify: `crates/noema-core/src/memory_persistence/models.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Test: `crates/noema-core/src/memory_persistence/postgres_tests.rs`

- [ ] **Step 1: Write the failing exact append test**

Add this test near `append_memory_candidate_records_source_conversation_and_edges` in `crates/noema-core/src/memory_persistence/postgres_tests.rs`:

```rust
#[tokio::test]
async fn append_memory_candidate_reuses_exact_dedupe_fingerprint() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(
            Some("test-model".to_string()),
            Some("/tmp/noema".to_string()),
        ))
        .await
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");
    let first_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I like ice cream.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first item");
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I LIKE   ICE CREAM".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("second item");

    let mut first = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(ObjectType::Conversation, conversation.conversation_id.as_str())
            .expect("owner"),
        "I like ice cream.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(first_item.item_id.as_str()),
    );
    first.memory_type = MemoryType::Preference;
    first.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
        ParticipantRole::HumanInScope,
    )];
    let mut subject =
        NewMemorySubject::new("human:local", "human", "Local human", SubjectRole::About);
    subject.linked_object = Some(ObjectRef::human("human:local"));
    first.subjects = vec![subject.clone()];

    let mut second = first.clone();
    second.content = "I LIKE   ICE CREAM".to_string();
    second.source = Some(ObjectProvenanceSource {
        source: ObjectRef::conversation_item(second_item.item_id.as_str()),
        evidence_excerpt: Some("I LIKE   ICE CREAM".to_string()),
    });

    let first_summary = repo
        .append_memory_candidate(first)
        .await
        .expect("first memory");
    let second_summary = repo
        .append_memory_candidate(second)
        .await
        .expect("second memory");

    assert_eq!(first_summary.id, second_summary.id);
    assert_eq!(
        first_summary.dedupe_fingerprint,
        second_summary.dedupe_fingerprint
    );

    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);

    let edge_count = sqlx::query_scalar::<_, i64>(
        r"
        SELECT COUNT(*)
        FROM object_provenance_edges
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
          AND source_object_type = 'conversation_item'
        ",
    )
    .bind(first_summary.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("edge count");
    assert_eq!(edge_count, 2);
}
```

- [ ] **Step 2: Run the exact append test to verify failure**

Run:

```bash
cargo test -p noema-core append_memory_candidate_reuses_exact_dedupe_fingerprint --no-fail-fast
```

Expected: FAIL because `MemorySummary` has no `dedupe_fingerprint` field and appends still allocate fresh memory ids.

- [ ] **Step 3: Add fingerprint storage to the Postgres bootstrap schema**

In `crates/noema-core/src/memory_persistence/postgres_schema.rs`, add this column to `memory_items` after `retrieval_hints`:

```sql
  memory_dedupe_fingerprint TEXT,
```

Then add this block after the `CREATE TABLE IF NOT EXISTS memory_items` statement:

```sql
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_name = 'memory_items'
      AND column_name = 'memory_dedupe_fingerprint'
  ) THEN
    ALTER TABLE memory_items ADD COLUMN memory_dedupe_fingerprint TEXT;
  END IF;
END
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_items_live_dedupe_fingerprint
ON memory_items(memory_dedupe_fingerprint)
WHERE memory_dedupe_fingerprint IS NOT NULL
  AND deleted_at IS NULL
  AND status != 'deleted';
```

- [ ] **Step 4: Add the fingerprint field to summaries**

Modify `MemorySummary` in `crates/noema-core/src/memory_persistence/models.rs`:

```rust
pub struct MemorySummary {
    pub id: MemoryId,
    pub status: MemoryStatus,
    pub memory_type: MemoryType,
    pub home_scope_id: ScopeId,
    pub owner_object_type: String,
    pub owner_object_id: String,
    pub sensitivity: Sensitivity,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub dedupe_fingerprint: Option<String>,
    pub source_object_type: Option<String>,
    pub source_object_id: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub conversation_id: Option<String>,
}
```

- [ ] **Step 5: Read fingerprints in repository summaries**

Modify `MemorySummaryRow` and the summary conversion in `crates/noema-core/src/memory_persistence/repository.rs`:

```rust
struct MemorySummaryRow {
    memory_id: String,
    status: String,
    memory_type: String,
    owner_object_type: String,
    owner_object_id: String,
    sensitivity: String,
    title: String,
    content: String,
    created_at: String,
    dedupe_fingerprint: Option<String>,
    source_object_type: Option<String>,
    source_object_id: Option<String>,
    conversation_id: Option<String>,
}
```

Set the field in `postgres_row_to_memory_summary`:

```rust
dedupe_fingerprint: row.dedupe_fingerprint,
```

Update tuple query shapes in `list_recent_memories` and `get_memory` from 12 fields to 13 fields by adding `Option<String>` before source fields, and bind `dedupe_fingerprint` into `MemorySummaryRow`.

- [ ] **Step 6: Update summary SQL queries**

Modify `crates/noema-core/src/memory_persistence/queries.rs` so `POSTGRES_RECENT_MEMORY_SQL` and `POSTGRES_MEMORY_SUMMARY_BY_ID_SQL` select `m.memory_dedupe_fingerprint` after `m.created_at::text`:

```sql
       m.created_at::text,
       m.memory_dedupe_fingerprint,
       source.source_object_type,
```

- [ ] **Step 7: Make append exact-idempotent and record support provenance**

Modify imports in `crates/noema-core/src/memory_persistence/provenance.rs`:

```rust
    memory_candidate_dedupe_fingerprint,
```

Inside `append_memory_candidate`, compute the fingerprint before beginning the insert:

```rust
let dedupe_fingerprint = memory_candidate_dedupe_fingerprint(&candidate);
```

After opening the transaction and before allocating `memory_id`, add:

```rust
if let Some(existing) =
    existing_memory_summary_for_dedupe_fingerprint_tx(&mut tx, dedupe_fingerprint.as_str()).await?
{
    if let Some(source) = &candidate.source {
        let target = ObjectRef::new(ObjectType::MemoryItem, existing.id.clone())?;
        insert_postgres_object_provenance_edge_tx(
            &mut tx,
            &NewObjectProvenanceEdge {
                target,
                source: source.source.clone(),
                relation: "supports".to_string(),
                evidence_excerpt: source.evidence_excerpt.clone(),
                created_by: candidate.created_by.clone(),
                metadata: json!({"dedupe": "exact_fingerprint"}),
            },
        )
        .await?;
    }
    let event_id = allocate_postgres_id(&mut *tx, "evt").await?;
    sqlx::query(
        r"
        INSERT INTO object_events (
          event_id,
          event_type,
          actor_id,
          target_object_type,
          target_object_id,
          reason,
          details
        )
        VALUES ($1, 'memory_reused', $2, 'memory_item', $3, $4, $5)
        ",
    )
    .bind(event_id.as_str())
    .bind(candidate.created_by.actor_id.as_str())
    .bind(existing.id.as_str())
    .bind("exact_dedupe_fingerprint")
    .bind(json_value(json!({"dedupe_fingerprint": dedupe_fingerprint})))
    .execute(&mut *tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;
    tx.commit().await.map_err(MemoryPersistenceError::Database)?;
    return Ok(existing);
}
```

Add `memory_dedupe_fingerprint` to the insert column list and values:

```sql
              retrieval_hints,
              memory_dedupe_fingerprint,
              status,
```

```sql
              $9, $10, $11, $12, $13, $14, $15::timestamptz, $16
```

Bind it after retrieval hints and renumber the following bind positions:

```rust
.bind(dedupe_fingerprint.as_str())
```

Set the summary field:

```rust
dedupe_fingerprint: Some(dedupe_fingerprint),
```

- [ ] **Step 8: Add the transaction helper for exact lookup**

Add this helper near `postgres_conversation_id_for_summary` in `crates/noema-core/src/memory_persistence/provenance.rs`:

```rust
async fn existing_memory_summary_for_dedupe_fingerprint_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    dedupe_fingerprint: &str,
) -> Result<Option<MemorySummary>, MemoryPersistenceError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
        ),
    >(
        r"
        SELECT
          memory_id,
          status,
          memory_type,
          owner_object_type,
          owner_object_id,
          sensitivity,
          title,
          content,
          created_at::text,
          memory_dedupe_fingerprint
        FROM memory_items
        WHERE memory_dedupe_fingerprint = $1
          AND deleted_at IS NULL
          AND status IN ('candidate', 'active', 'confirmed')
        ORDER BY created_at ASC
        LIMIT 1
        ",
    )
    .bind(dedupe_fingerprint)
    .fetch_optional(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    row.map(
        |(
            memory_id,
            status,
            memory_type,
            owner_object_type,
            owner_object_id,
            sensitivity,
            title,
            content,
            created_at,
            dedupe_fingerprint,
        )| {
            Ok(MemorySummary {
                id: memory_id,
                status: crate::memory_persistence::helpers::parse_memory_status(&status)?,
                memory_type: crate::memory_persistence::helpers::parse_memory_type(&memory_type)?,
                home_scope_id: crate::memory_persistence::helpers::object_ref_key(
                    &owner_object_type,
                    &owner_object_id,
                ),
                owner_object_type,
                owner_object_id,
                sensitivity: crate::memory_persistence::helpers::parse_sensitivity(&sensitivity)?,
                title,
                content,
                created_at,
                dedupe_fingerprint,
                source_object_type: None,
                source_object_id: None,
                source_type: None,
                source_id: None,
                conversation_id: None,
            })
        },
    )
    .transpose()
}
```

- [ ] **Step 9: Run the exact append test**

Run:

```bash
cargo test -p noema-core append_memory_candidate_reuses_exact_dedupe_fingerprint --no-fail-fast
```

Expected: PASS.

- [ ] **Step 10: Run focused persistence tests**

Run:

```bash
cargo test -p noema-core memory_persistence::postgres_tests --no-fail-fast
```

Expected: PASS, unless the local Postgres test database is unavailable. If unavailable, report the database setup error and run the focused non-Postgres dedupe unit tests from Task 1.

- [ ] **Step 11: Commit Task 2**

Run:

```bash
git add crates/noema-core/src/memory_persistence/postgres_schema.rs crates/noema-core/src/memory_persistence/models.rs crates/noema-core/src/memory_persistence/repository.rs crates/noema-core/src/memory_persistence/queries.rs crates/noema-core/src/memory_persistence/provenance.rs crates/noema-core/src/memory_persistence/postgres_tests.rs
git commit -m "Make exact memory appends idempotent"
```

## Task 3: Add Consolidation Outcome Types And Bounded Match Search

**Files:**
- Create: `crates/noema-core/src/daemon/memory_consolidation.rs`
- Modify: `crates/noema-core/src/daemon.rs`
- Modify: `crates/noema-core/src/memory_persistence/repository.rs`
- Test: `crates/noema-core/src/daemon/memory_consolidation.rs`

- [ ] **Step 1: Write pure outcome and parser tests**

Create `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
use crate::{
    memory::Sensitivity,
    memory_persistence::{MemorySummary, NewMemoryCandidate, PostgresMemoryRepository},
    provider::{GenerateRequest, ProviderError},
};
use serde::Deserialize;
use serde_json::json;

use super::runtime::RuntimeModelProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MemoryConsolidationOutcome {
    Created { memory_id: String },
    Reused { memory_id: String, reason: &'static str },
    Reinforced { memory_id: String, reason: &'static str },
    Conflict { memory_id: String, conflicting_memory_id: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SemanticConsolidationDecision {
    Create,
    Reuse { existing_memory_id: String },
    Reinforce { existing_memory_id: String },
    Conflict { existing_memory_id: String, rationale: String },
}

#[derive(Debug, Deserialize)]
struct SemanticDecisionJson {
    decision: String,
    existing_memory_id: Option<String>,
    confidence: f64,
    rationale: String,
}

pub(super) fn parse_semantic_decision(
    text: &str,
) -> Result<SemanticConsolidationDecision, String> {
    let parsed: SemanticDecisionJson =
        serde_json::from_str(text.trim()).map_err(|error| format!("invalid JSON: {error}"))?;
    if !(0.0..=1.0).contains(&parsed.confidence) {
        return Err("confidence must be between 0.0 and 1.0".to_string());
    }
    match parsed.decision.as_str() {
        "create" => Ok(SemanticConsolidationDecision::Create),
        "reuse" => Ok(SemanticConsolidationDecision::Reuse {
            existing_memory_id: required_existing_memory_id(&parsed)?,
        }),
        "reinforce" => Ok(SemanticConsolidationDecision::Reinforce {
            existing_memory_id: required_existing_memory_id(&parsed)?,
        }),
        "conflict" => Ok(SemanticConsolidationDecision::Conflict {
            existing_memory_id: required_existing_memory_id(&parsed)?,
            rationale: parsed.rationale,
        }),
        other => Err(format!("unsupported semantic decision: {other}")),
    }
}

fn required_existing_memory_id(parsed: &SemanticDecisionJson) -> Result<String, String> {
    parsed
        .existing_memory_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| "existing_memory_id is required".to_string())
}

#[cfg(test)]
mod tests {
    use super::{SemanticConsolidationDecision, parse_semantic_decision};

    #[test]
    fn parse_semantic_reinforce_decision() {
        let parsed = parse_semantic_decision(
            r#"{"decision":"reinforce","existing_memory_id":"mem_1","confidence":0.91,"rationale":"same preference"}"#,
        )
        .expect("decision");

        assert_eq!(
            parsed,
            SemanticConsolidationDecision::Reinforce {
                existing_memory_id: "mem_1".to_string()
            }
        );
    }

    #[test]
    fn parse_semantic_decision_requires_existing_memory_for_reuse() {
        let error = parse_semantic_decision(
            r#"{"decision":"reuse","existing_memory_id":null,"confidence":0.91,"rationale":"same preference"}"#,
        )
        .expect_err("missing id");

        assert_eq!(error, "existing_memory_id is required");
    }
}
```

- [ ] **Step 2: Register the module**

Modify `crates/noema-core/src/daemon.rs`:

```rust
mod memory_consolidation;
mod memory_pipeline;
mod memory_tool;
mod protocol;
mod runtime;
```

- [ ] **Step 3: Run parser tests to verify pass**

Run:

```bash
cargo test -p noema-core daemon::memory_consolidation --no-fail-fast
```

Expected: PASS for parser-only tests.

- [ ] **Step 4: Add repository match search method**

Add this public method to `impl PostgresMemoryRepository` in `crates/noema-core/src/memory_persistence/repository.rs`:

```rust
pub async fn find_memory_consolidation_matches(
    &self,
    candidate: &NewMemoryCandidate,
    limit: u32,
) -> Result<Vec<MemorySummary>, MemoryPersistenceError> {
    let limit = limit.clamp(1, 12);
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r"
        SELECT DISTINCT
          m.memory_id,
          m.status,
          m.memory_type,
          m.owner_object_type,
          m.owner_object_id,
          m.sensitivity,
          m.title,
          m.content,
          m.created_at::text,
          m.memory_dedupe_fingerprint,
          source.source_object_type,
          source.source_object_id,
          source_conversation.conversation_id
        FROM memory_items m
        LEFT JOIN LATERAL (
          SELECT source_object_type, source_object_id
          FROM object_provenance_edges
          WHERE target_object_type = 'memory_item'
            AND target_object_id = m.memory_id
            AND deleted_at IS NULL
          ORDER BY created_at ASC
          LIMIT 1
        ) source ON true
        LEFT JOIN conversation_items source_conversation
          ON source.source_object_type = 'conversation_item'
         AND source.source_object_id = source_conversation.item_id
        LEFT JOIN memory_participants mp ON mp.memory_id = m.memory_id
        WHERE m.deleted_at IS NULL
          AND m.status IN ('candidate', 'active', 'confirmed')
          AND m.memory_type = $1
          AND (
            (m.owner_object_type = $2 AND m.owner_object_id = $3)
            OR mp.participant_actor_id = ANY($4)
          )
        ORDER BY
          CASE
            WHEN m.search_vector @@ plainto_tsquery('simple', $5) THEN 0
            WHEN lower(m.content) LIKE '%' || lower($5) || '%' THEN 1
            ELSE 2
          END,
          m.created_at DESC
        LIMIT $6
        ",
    )
    .bind(candidate.memory_type.as_str())
    .bind(candidate.owner.object_type.as_str())
    .bind(candidate.owner.object_id.as_str())
    .bind(
        candidate
            .participants
            .iter()
            .map(|participant| participant.participant.actor_id.clone())
            .collect::<Vec<_>>(),
    )
    .bind(candidate.content.as_str())
    .bind(i64::from(limit))
    .fetch_all(&self.pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    rows.into_iter()
        .map(
            |(
                memory_id,
                status,
                memory_type,
                owner_object_type,
                owner_object_id,
                sensitivity,
                title,
                content,
                created_at,
                dedupe_fingerprint,
                source_object_type,
                source_object_id,
                conversation_id,
            )| {
                postgres_row_to_memory_summary(MemorySummaryRow {
                    memory_id,
                    status,
                    memory_type,
                    owner_object_type,
                    owner_object_id,
                    sensitivity,
                    title,
                    content,
                    created_at,
                    dedupe_fingerprint,
                    source_object_type,
                    source_object_id,
                    conversation_id,
                })
            },
        )
        .collect()
}
```

Add `NewMemoryCandidate` to the repository module imports from `super`.

- [ ] **Step 5: Add semantic eligibility helper**

Append this to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
pub(super) fn semantic_consolidation_allowed(candidate: &NewMemoryCandidate) -> bool {
    matches!(candidate.sensitivity, Sensitivity::Public | Sensitivity::Normal)
}
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p noema-core daemon::memory_consolidation --no-fail-fast
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
git add crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/memory_consolidation.rs crates/noema-core/src/memory_persistence/repository.rs
git commit -m "Add memory consolidation match search"
```

## Task 4: Implement Semantic Comparator And Reinforcement Persistence

**Files:**
- Modify: `crates/noema-core/src/daemon/memory_consolidation.rs`
- Modify: `crates/noema-core/src/memory_persistence/provenance.rs`
- Test: `crates/noema-core/src/daemon/memory_consolidation.rs`

- [ ] **Step 1: Add prompt builder tests**

Add tests to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
#[cfg(test)]
mod prompt_tests {
    use crate::{
        memory::{MemoryStatus, Sensitivity},
        memory_persistence::{MemorySummary, MemoryType},
    };

    use super::build_semantic_consolidation_prompt;

    fn summary(id: &str, content: &str) -> MemorySummary {
        MemorySummary {
            id: id.to_string(),
            status: MemoryStatus::Active,
            memory_type: MemoryType::Preference,
            home_scope_id: "human:human:local".to_string(),
            owner_object_type: "human".to_string(),
            owner_object_id: "human:local".to_string(),
            sensitivity: Sensitivity::Normal,
            title: content.to_string(),
            content: content.to_string(),
            created_at: "2026-06-28 00:00:00+00".to_string(),
            dedupe_fingerprint: Some("sha256:abc".to_string()),
            source_object_type: None,
            source_object_id: None,
            source_type: None,
            source_id: None,
            conversation_id: None,
        }
    }

    #[test]
    fn semantic_prompt_contains_strict_decision_contract() {
        let prompt = build_semantic_consolidation_prompt(
            "Kevin likes ice cream.",
            &[summary("mem_1", "Kevin enjoys ice cream.")],
        );

        assert!(prompt.contains("\"decision\":\"create|reuse|reinforce|conflict\""));
        assert!(prompt.contains("mem_1"));
        assert!(prompt.contains("Kevin likes ice cream."));
    }
}
```

- [ ] **Step 2: Implement semantic prompt builder**

Add this function to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
pub(super) fn build_semantic_consolidation_prompt(
    proposal_content: &str,
    matches: &[MemorySummary],
) -> String {
    let existing = matches
        .iter()
        .map(|memory| {
            json!({
                "memory_id": memory.id,
                "memory_type": memory.memory_type.as_str(),
                "owner": {
                    "object_type": memory.owner_object_type,
                    "object_id": memory.owner_object_id,
                },
                "sensitivity": format!("{:?}", memory.sensitivity).to_ascii_lowercase(),
                "content": memory.content,
            })
        })
        .collect::<Vec<_>>();

    format!(
        r#"You are Noema's memory consolidation comparator.

Return strict JSON only. Do not include Markdown, prose, or comments.

Choose one decision:
- create: the proposal is distinct from every existing memory.
- reuse: an existing memory fully covers the proposal and no new support is useful.
- reinforce: the proposal restates existing truth and should add supporting provenance.
- conflict: the proposal and an existing memory cannot both be true.

Return exactly this shape:
{{"decision":"create|reuse|reinforce|conflict","existing_memory_id":null,"confidence":0.0,"rationale":"short reason"}}

Rules:
- Use reuse, reinforce, or conflict only when the existing memory has the same subject and owner.
- Prefer create when uncertain.
- Prefer reinforce for paraphrases of the same preference or fact.
- Prefer conflict for direct contradiction.

Proposal:
{proposal_content}

Existing memories:
{existing}"#
    )
}
```

- [ ] **Step 3: Add repository reinforcement helper**

Add this method to `impl PostgresMemoryRepository` in `crates/noema-core/src/memory_persistence/provenance.rs`:

```rust
pub async fn reinforce_memory_with_candidate(
    &self,
    memory_id: &str,
    candidate: &NewMemoryCandidate,
    reason: &str,
) -> Result<MemorySummary, MemoryPersistenceError> {
    validate_postgres_memory_candidate_refs(self.pool(), candidate).await?;
    let mut tx = self
        .pool()
        .begin()
        .await
        .map_err(MemoryPersistenceError::Database)?;

    let Some(existing) = existing_memory_summary_by_id_tx(&mut tx, memory_id).await? else {
        return Err(MemoryPersistenceError::MemoryNotFound {
            memory_id: memory_id.to_string(),
        });
    };

    if let Some(source) = &candidate.source {
        let target = ObjectRef::new(ObjectType::MemoryItem, memory_id.to_string())?;
        insert_postgres_object_provenance_edge_tx(
            &mut tx,
            &NewObjectProvenanceEdge {
                target,
                source: source.source.clone(),
                relation: "supports".to_string(),
                evidence_excerpt: source.evidence_excerpt.clone(),
                created_by: candidate.created_by.clone(),
                metadata: json!({"reason": reason}),
            },
        )
        .await?;
    }

    let event_id = allocate_postgres_id(&mut *tx, "evt").await?;
    sqlx::query(
        r"
        INSERT INTO object_events (
          event_id,
          event_type,
          actor_id,
          target_object_type,
          target_object_id,
          reason,
          details
        )
        VALUES ($1, 'memory_reinforced', $2, 'memory_item', $3, $4, $5)
        ",
    )
    .bind(event_id.as_str())
    .bind(candidate.created_by.actor_id.as_str())
    .bind(memory_id)
    .bind(reason)
    .bind(json_value(json!({"candidate_content": candidate.content})))
    .execute(&mut *tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    tx.commit().await.map_err(MemoryPersistenceError::Database)?;
    Ok(existing)
}
```

Add `existing_memory_summary_by_id_tx` by adapting `existing_memory_summary_for_dedupe_fingerprint_tx` to filter by `memory_id = $1` and return a `MemorySummary`.

- [ ] **Step 4: Add comparator orchestration**

Add this function to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
pub(super) async fn semantic_consolidation_decision(
    provider: &dyn RuntimeModelProvider,
    candidate: &NewMemoryCandidate,
    matches: &[MemorySummary],
) -> Result<SemanticConsolidationDecision, String> {
    if matches.is_empty() || !semantic_consolidation_allowed(candidate) {
        return Ok(SemanticConsolidationDecision::Create);
    }

    let prompt = build_semantic_consolidation_prompt(&candidate.content, matches);
    let response = provider
        .generate(GenerateRequest::text(prompt))
        .await
        .map_err(|error: ProviderError| format!("semantic consolidation model failed: {error}"))?;
    parse_semantic_decision(&response.assistant_text())
}
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p noema-core daemon::memory_consolidation --no-fail-fast
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add crates/noema-core/src/daemon/memory_consolidation.rs crates/noema-core/src/memory_persistence/provenance.rs
git commit -m "Add semantic memory consolidation"
```

## Task 5: Route Runtime Memory Writes Through Consolidation Outcomes

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/memory_consolidation.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add runtime exact duplicate test**

Add this test to `crates/noema-core/src/daemon/tests.rs`:

```rust
#[tokio::test]
async fn runtime_actor_reuses_repeated_explicit_memory() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(fake_codex_provider(), database.url.clone()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "remember: I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    collect_turn(
        &handle,
        conversation_id,
        "remember: I LIKE   ICE CREAM".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "I like ice cream.");
}
```

- [ ] **Step 2: Add provider semantic repeat test**

Extend `memory_extraction_output` in `crates/noema-core/src/daemon/tests.rs` so ordinary-chat extraction can seed the first ice-cream memory:

```rust
    if input.contains("ordinary-chat memory proposal extractor") {
        let proposal = if input.contains("I like ice cream.") {
            proposal(json!({
                "content": "Kevin likes ice cream.",
                "memory_type": "preference",
                "title": "Ice cream preference",
                "confidence": 0.94,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                "risk_flags": [],
                "evidence_excerpt": "I like ice cream."
            }))
        } else if input.contains("Alice prefers decaf.") {
            proposal(json!({
                "content": "Alice prefers decaf.",
                "memory_type": "preference",
                "title": "Alice decaf preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "Alice", "role": "about"}],
                "retrieval_hints": {"topics": ["people"], "keywords": ["Alice", "decaf"], "summary": "Alice prefers decaf."},
                "risk_flags": [],
                "evidence_excerpt": "Alice prefers decaf."
            }))
        } else {
            proposal(json!({
                "content": "Kevin prefers automatic memory extraction in chat.",
                "memory_type": "preference",
                "title": "Automatic memory extraction preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["memory"], "keywords": ["automatic memory extraction", "chat"], "summary": "Kevin prefers automatic memory extraction in chat."},
                "risk_flags": [],
                "evidence_excerpt": "I prefer automatic memory extraction in chat."
            }))
        };
        return vec![GenerateOutputItem::AssistantText {
            text: serde_json::to_string(&json!({"proposals": [proposal]})).expect("extractor json"),
        }];
    }
```

Then add a same-call provider proposal branch for the paraphrased second turn:

```rust
    if input.contains("Ice cream is one of my favorite desserts.") {
        let proposal = serde_json::json!({
            "content": "Kevin considers ice cream one of his favorite desserts.",
            "memory_type": "preference",
            "title": "Ice cream preference",
            "confidence": 0.94,
            "sensitivity": "normal",
            "subjects": [{"id": "human:local", "kind": "human", "name": "current human", "role": "about"}],
            "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream", "dessert"], "summary": "Kevin likes ice cream."},
            "risk_flags": [],
            "evidence_excerpt": "Ice cream is one of my favorite desserts."
        });
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: serde_json::from_value(serde_json::json!([proposal]))
                    .expect("proposal"),
            },
        ];
    }
```

Add a helper and fake semantic comparator branch in `crates/noema-core/src/daemon/tests.rs`:

```rust
fn first_memory_id_from_consolidation_prompt(input: &str) -> String {
    let marker = r#""memory_id":""#;
    let Some(start) = input.find(marker).map(|index| index + marker.len()) else {
        return "mem_missing".to_string();
    };
    let rest = &input[start..];
    let Some(end) = rest.find('"') else {
        return "mem_missing".to_string();
    };
    rest[..end].to_string()
}
```

```rust
    if input.contains("Noema's memory consolidation comparator") {
        let existing_memory_id = first_memory_id_from_consolidation_prompt(input);
        return vec![GenerateOutputItem::AssistantText {
            text: format!(
                r#"{{"decision":"reinforce","existing_memory_id":"{existing_memory_id}","confidence":0.91,"rationale":"same ice cream preference"}}"#
            ),
        }];
    }
```

Then add the test:

```rust
#[tokio::test]
async fn runtime_actor_reinforces_semantic_memory_repeat() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(
        fake_codex_provider_with_memory_extraction(),
        database.url.clone(),
    )
    .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);

    let event_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM object_events WHERE event_type = 'memory_reinforced'",
    )
    .fetch_one(repo.pool())
    .await
    .expect("event count");
    assert_eq!(event_count, 1);
}
```

- [ ] **Step 3: Run runtime tests to verify failure**

Run:

```bash
cargo test -p noema-core runtime_actor_reuses_repeated_explicit_memory runtime_actor_reinforces_semantic_memory_repeat --no-fail-fast
```

Expected: FAIL because runtime still uses `append_memory_candidate` directly and does not return outcome payloads.

- [ ] **Step 4: Add consolidation execution function**

Add to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
pub(super) async fn consolidate_memory_candidate(
    repository: &PostgresMemoryRepository,
    provider: &dyn RuntimeModelProvider,
    candidate: NewMemoryCandidate,
) -> Result<MemoryConsolidationOutcome, String> {
    let matches = repository
        .find_memory_consolidation_matches(&candidate, 12)
        .await
        .map_err(|error| format!("failed to search memory consolidation matches: {error}"))?;

    let exact_fingerprint = crate::memory_persistence::memory_candidate_dedupe_fingerprint(&candidate);
    if let Some(existing) = matches
        .iter()
        .find(|memory| memory.dedupe_fingerprint.as_deref() == Some(exact_fingerprint.as_str()))
    {
        let memory = repository
            .reinforce_memory_with_candidate(&existing.id, &candidate, "exact_repeat")
            .await
            .map_err(|error| format!("failed to reinforce exact memory repeat: {error}"))?;
        return Ok(MemoryConsolidationOutcome::Reinforced {
            memory_id: memory.id,
            reason: "exact_repeat",
        });
    }

    let decision = semantic_consolidation_decision(provider, &candidate, &matches)
        .await
        .unwrap_or(SemanticConsolidationDecision::Create);

    match decision {
        SemanticConsolidationDecision::Create => {
            let memory = repository
                .append_memory_candidate(candidate)
                .await
                .map_err(|error| format!("failed to persist memory: {error}"))?;
            Ok(MemoryConsolidationOutcome::Created {
                memory_id: memory.id,
            })
        }
        SemanticConsolidationDecision::Reuse { existing_memory_id } => {
            Ok(MemoryConsolidationOutcome::Reused {
                memory_id: existing_memory_id,
                reason: "semantic_reuse",
            })
        }
        SemanticConsolidationDecision::Reinforce { existing_memory_id } => {
            let memory = repository
                .reinforce_memory_with_candidate(&existing_memory_id, &candidate, "semantic_repeat")
                .await
                .map_err(|error| format!("failed to reinforce memory: {error}"))?;
            Ok(MemoryConsolidationOutcome::Reinforced {
                memory_id: memory.id,
                reason: "semantic_repeat",
            })
        }
        SemanticConsolidationDecision::Conflict {
            existing_memory_id,
            rationale,
        } => {
            let mut disputed = candidate;
            disputed.status = crate::memory::MemoryStatus::Disputed;
            disputed.metadata = json!({
                "semantic_consolidation": "conflict",
                "conflicting_memory_id": existing_memory_id,
                "rationale": rationale,
            });
            let memory = repository
                .append_memory_candidate(disputed)
                .await
                .map_err(|error| format!("failed to persist conflict memory: {error}"))?;
            Ok(MemoryConsolidationOutcome::Conflict {
                memory_id: memory.id,
                conflicting_memory_id: existing_memory_id,
                reason: rationale,
            })
        }
    }
}
```

- [ ] **Step 5: Use consolidation in ordinary-chat worker**

Modify `persist_validated_memory_proposals` in `crates/noema-core/src/daemon/runtime.rs` to accept the provider:

```rust
async fn persist_validated_memory_proposals(
    memory_repository: &PostgresMemoryRepository,
    provider: &dyn RuntimeModelProvider,
    context: &ConversationMemoryContext,
    proposals: Vec<ValidatedMemoryProposal>,
    trigger: &str,
) -> Result<Vec<MemoryConsolidationOutcome>, String> {
```

Import:

```rust
    memory_consolidation::{MemoryConsolidationOutcome, consolidate_memory_candidate},
```

Inside the loop, replace `append_memory_candidate` with:

```rust
let outcome = consolidate_memory_candidate(memory_repository, provider, candidate).await?;
outcomes.push(outcome);
```

Update caller sites to pass `self.provider.as_ref()`.

- [ ] **Step 6: Use exact append outcome for explicit remembers**

In `persist_chat_memory_candidate`, keep using repository append for now because exact dedupe is already database-backed. Rename the local variable from `memory` to `summary` and return `Some(summary.id)`. This preserves the existing explicit memory API while exact repeats reuse the same id:

```rust
let summary = self
    .memory_repository
    .append_memory_candidate(candidate)
    .await?;
Ok(Some(summary.id))
```

- [ ] **Step 7: Run focused runtime tests**

Run:

```bash
cargo test -p noema-core runtime_actor_reuses_repeated_explicit_memory runtime_actor_reinforces_semantic_memory_repeat --no-fail-fast
```

Expected: PASS.

- [ ] **Step 8: Commit Task 5**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/memory_consolidation.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Route memory writes through consolidation"
```

## Task 6: Add Conflict Outcomes And Transcript Payloads

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/memory_consolidation.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add conflict runtime test**

Add a fake provider branch in `memory_extraction_output`:

```rust
    if input.contains("I hate ice cream.") {
        let proposal = serde_json::json!({
            "content": "Kevin hates ice cream.",
            "memory_type": "preference",
            "title": "Ice cream dislike",
            "confidence": 0.94,
            "sensitivity": "normal",
            "subjects": [{"id": "human:local", "kind": "human", "name": "current human", "role": "about"}],
            "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin hates ice cream."},
            "risk_flags": ["conflict"],
            "evidence_excerpt": "I hate ice cream."
        });
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: serde_json::from_value(serde_json::json!([proposal]))
                    .expect("proposal"),
            },
        ];
    }
```

Adjust the semantic comparator branch so it returns `conflict` when the prompt contains `Kevin hates ice cream.`:

```rust
    if input.contains("Noema's memory consolidation comparator") {
        let existing_memory_id = first_memory_id_from_consolidation_prompt(input);
        let text = if input.contains("Kevin hates ice cream.") {
            format!(
                r#"{{"decision":"conflict","existing_memory_id":"{existing_memory_id}","confidence":0.93,"rationale":"opposite ice cream preference"}}"#
            )
        } else {
            format!(
                r#"{{"decision":"reinforce","existing_memory_id":"{existing_memory_id}","confidence":0.91,"rationale":"same ice cream preference"}}"#
            )
        };
        return vec![GenerateOutputItem::AssistantText {
            text,
        }];
    }
```

Add the test:

```rust
#[tokio::test]
async fn runtime_actor_creates_disputed_memory_for_semantic_conflict() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(
        fake_codex_provider_with_memory_extraction(),
        database.url.clone(),
    )
    .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    collect_turn(
        &handle,
        conversation_id,
        "I hate ice cream.".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 2);
    assert!(
        memories
            .iter()
            .any(|memory| memory.status == crate::memory::MemoryStatus::Disputed)
    );
}
```

- [ ] **Step 2: Add transcript outcome payload helper**

Add to `crates/noema-core/src/daemon/memory_consolidation.rs`:

```rust
pub(super) fn consolidation_outcome_json(
    outcome: &MemoryConsolidationOutcome,
    proposal_content: &str,
) -> serde_json::Value {
    match outcome {
        MemoryConsolidationOutcome::Created { memory_id } => json!({
            "outcome": "created",
            "memory_id": memory_id,
            "proposal_content": proposal_content,
            "reason": "new_memory"
        }),
        MemoryConsolidationOutcome::Reused { memory_id, reason } => json!({
            "outcome": "reused",
            "memory_id": memory_id,
            "proposal_content": proposal_content,
            "reason": reason
        }),
        MemoryConsolidationOutcome::Reinforced { memory_id, reason } => json!({
            "outcome": "reinforced",
            "memory_id": memory_id,
            "proposal_content": proposal_content,
            "reason": reason
        }),
        MemoryConsolidationOutcome::Conflict {
            memory_id,
            conflicting_memory_id,
            reason,
        } => json!({
            "outcome": "conflict",
            "memory_id": memory_id,
            "conflicting_memory_id": conflicting_memory_id,
            "proposal_content": proposal_content,
            "reason": reason
        }),
    }
}
```

- [ ] **Step 3: Update provider memory proposal card payloads**

In `persist_provider_memory_proposals`, replace `created_memory_ids` with outcome-derived values:

```rust
let created_only_ids = outcomes
    .iter()
    .filter_map(|outcome| match outcome {
        MemoryConsolidationOutcome::Created { memory_id } => Some(memory_id.clone()),
        _ => None,
    })
    .collect::<Vec<_>>();
let memory_outcomes = outcomes
    .iter()
    .zip(card_proposals.iter())
    .map(|(outcome, proposal)| consolidation_outcome_json(outcome, &proposal.proposal.content))
    .collect::<Vec<_>>();
```

Use this payload:

```rust
payload: json!({
    "turn_index": turn_index,
    "source": "provider_structured_output",
    "created_memory_ids": created_only_ids,
    "memory_outcomes": memory_outcomes,
    "proposals": card_proposals,
}),
```

Keep `created_memory_ids` for compatibility with the current web card rendering, but treat `memory_outcomes` as the new canonical field.

- [ ] **Step 4: Update activity summaries**

Add a helper in `runtime.rs`:

```rust
fn memory_outcome_summary(outcomes: &[MemoryConsolidationOutcome]) -> String {
    let created = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, MemoryConsolidationOutcome::Created { .. }))
        .count();
    let reused = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, MemoryConsolidationOutcome::Reused { .. }))
        .count();
    let reinforced = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, MemoryConsolidationOutcome::Reinforced { .. }))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, MemoryConsolidationOutcome::Conflict { .. }))
        .count();
    format!(
        "created {created}, reused {reused}, reinforced {reinforced}, conflicts {conflicts}"
    )
}
```

Use it in the completed memory extraction activity summary.

- [ ] **Step 5: Run focused runtime tests**

Run:

```bash
cargo test -p noema-core runtime_actor_creates_disputed_memory_for_semantic_conflict runtime_actor_persists_provider_structured_memory_proposals_as_activity --no-fail-fast
```

Expected: PASS after updating existing provider proposal assertions to accept `memory_outcomes`.

- [ ] **Step 6: Commit Task 6**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/memory_consolidation.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Report memory consolidation outcomes"
```

## Task 7: Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Add this bullet under `## Settled Decisions` in `docs/context/current.md`:

```markdown
- Memory writes run through consolidation before creating rows: exact
  fingerprints prevent duplicate appends, normal/public semantic repeats can
  reuse or reinforce existing memory, and conflicts create reviewable disputed
  state instead of silently overwriting truth.
```

- [ ] **Step 2: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 3: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p noema-core memory_persistence::dedupe append_memory_candidate_reuses_exact_dedupe_fingerprint runtime_actor_reuses_repeated_explicit_memory runtime_actor_reinforces_semantic_memory_repeat runtime_actor_creates_disputed_memory_for_semantic_conflict --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Run broader unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. If daemon or provider tests fail because sandboxed local sockets are denied, rerun the same command with socket permissions and report the difference.

- [ ] **Step 6: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 7: Run ship diff checks**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected: `git diff --check` PASS. The staged diff commands should either show no staged changes or only the intentional context update if Step 8 has not committed yet.

- [ ] **Step 8: Commit context update**

Run:

```bash
git add docs/context/current.md
git commit -m "Document memory consolidation context"
```

## Final Review Checklist

- Exact duplicates reuse one `memory_items` row.
- Repeated exact evidence adds support provenance or reuse events without copying truth.
- Normal/public semantic repeats reinforce or reuse existing memory.
- Sensitive/secret memories skip broad semantic comparison.
- Semantic comparator receives at most 12 plausible matches.
- Conflicts create disputed/reviewable state.
- Provider proposals and ordinary-chat extraction share consolidation outcomes.
- Transcript payloads include `memory_outcomes`.
- Existing unrelated dirty daemon/web transcript work remains preserved unless the executor explicitly owns it.
