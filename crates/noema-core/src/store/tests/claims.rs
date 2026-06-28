use serde_json::{Value, json};

use super::test_store;
use crate::{
    ActorRef, ClaimStatus, ConversationItemKind, ConversationItemStatus, EntityCandidate,
    EntityType, EvidenceAuthority, EvidenceCandidate, NewClaimCandidate, NewConversationItem,
    NewConversationTurn, NoemaStore, StoreError,
    memory::{ClaimRetrievalRequest, Sensitivity, UseMode},
    memory_persistence::NewConversation,
};

#[tokio::test]
async fn known_predicate_claim_gets_evidence() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;

    let summary = store
        .create_or_reinforce_claim(NewClaimCandidate {
            subject: EntityCandidate::local_human(),
            object: EntityCandidate::concept("trains", "trains"),
            predicate_id: "likes".to_string(),
            fact: "Kevin likes trains.".to_string(),
            sensitivity: Sensitivity::Normal,
            status: ClaimStatus::Active,
            confidence: Some(0.9),
            evidence: EvidenceCandidate {
                source_item_id: source_item.item_id,
                authority: EvidenceAuthority::ExplicitHumanStatement,
                excerpt: Some("Kevin likes trains.".to_string()),
            },
            retrieval_hints: json!({ "keywords": ["trains"] }),
            metadata: json!({}),
        })
        .await
        .expect("create claim");

    assert_eq!(summary.predicate_id, "likes");
    assert_eq!(summary.status, ClaimStatus::Active);
    assert_eq!(summary.sensitivity, Sensitivity::Normal);
    assert_eq!(summary.evidence_count, 1);
}

#[tokio::test]
async fn reinforcing_existing_claim_adds_evidence() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Kevin likes trains.").await;
    let second_item = create_source_item(&store, "Still true: Kevin likes trains.").await;

    let first = store
        .create_or_reinforce_claim(train_claim(first_item.item_id))
        .await
        .expect("first claim");
    let mut second_candidate = train_claim(second_item.item_id);
    second_candidate.fact = "  kevin likes trains.  ".to_string();
    second_candidate.evidence.authority = EvidenceAuthority::RepeatedObservation;
    second_candidate.evidence.excerpt = Some("Still true: Kevin likes trains.".to_string());
    second_candidate.confidence = Some(0.8);
    let second = store
        .create_or_reinforce_claim(second_candidate)
        .await
        .expect("reinforce claim");

    assert_eq!(first.claim_id, second.claim_id);
    assert_eq!(second.evidence_count, 2);
    assert_eq!(claim_count(&store).await, 1);
}

#[tokio::test]
async fn concurrent_same_fingerprint_writes_reinforce_one_claim() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Kevin likes trains.").await;
    let second_item = create_source_item(&store, "Kevin really likes trains.").await;
    let first_store = store.clone();
    let second_store = store.clone();

    let (first, second) = tokio::join!(
        async move {
            first_store
                .create_or_reinforce_claim(train_claim(first_item.item_id))
                .await
        },
        async move {
            second_store
                .create_or_reinforce_claim(train_claim(second_item.item_id))
                .await
        }
    );
    let first = first.expect("first concurrent write");
    let second = second.expect("second concurrent write");

    assert_eq!(first.claim_id, second.claim_id);
    assert_eq!(claim_count(&store).await, 1);
    assert_eq!(support_count(&store, &first.claim_id).await, 2);
}

#[tokio::test]
async fn deleted_claim_fingerprint_can_be_reused_by_new_claim() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Kevin likes trains.").await;
    let second_item = create_source_item(&store, "Kevin still likes trains.").await;

    let deleted = store
        .create_or_reinforce_claim(train_claim(first_item.item_id))
        .await
        .expect("initial claim");
    store
        .db()
        .query(
            r#"
            UPDATE claims SET
              status = 'deleted',
              updated_at = time::now()
            WHERE claim_id = $claim_id;
            "#,
        )
        .bind(("claim_id", deleted.claim_id.clone()))
        .await
        .expect("delete claim")
        .check()
        .expect("deleted claim update should succeed");

    let mut replacement_candidate = train_claim(second_item.item_id);
    replacement_candidate.evidence.authority = EvidenceAuthority::RepeatedObservation;
    replacement_candidate.evidence.excerpt = Some("Kevin still likes trains.".to_string());
    let replacement = store
        .create_or_reinforce_claim(replacement_candidate)
        .await
        .expect("replacement claim should be created");

    assert_ne!(deleted.claim_id, replacement.claim_id);
    assert_eq!(replacement.status, ClaimStatus::Active);
    assert_eq!(replacement.evidence_count, 1);

    let rows = matching_train_claims(&store).await;
    let active_rows = rows
        .iter()
        .filter(|row| row.status != "deleted")
        .collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert_eq!(active_rows.len(), 1);
    assert_eq!(active_rows[0].claim_id, replacement.claim_id);
    assert_eq!(active_rows[0].evidence_count, 1);
    assert_eq!(
        rows.iter()
            .find(|row| row.claim_id == deleted.claim_id)
            .expect("deleted claim row")
            .evidence_count,
        1
    );
}

#[tokio::test]
async fn unknown_predicate_claim_is_rejected() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin studies semaphore signals.").await;

    let error = store
        .create_or_reinforce_claim(NewClaimCandidate {
            subject: EntityCandidate::local_human(),
            object: EntityCandidate::concept("semaphore", "semaphore"),
            predicate_id: "studies".to_string(),
            fact: "Kevin studies semaphore signals.".to_string(),
            sensitivity: Sensitivity::Normal,
            status: ClaimStatus::Active,
            confidence: Some(0.7),
            evidence: EvidenceCandidate {
                source_item_id: source_item.item_id,
                authority: EvidenceAuthority::ExplicitHumanStatement,
                excerpt: Some("Kevin studies semaphore signals.".to_string()),
            },
            retrieval_hints: json!({}),
            metadata: json!({}),
        })
        .await
        .expect_err("unknown predicate should be rejected");

    assert!(
        error.to_string().contains("predicate"),
        "unexpected error: {error}"
    );
    assert_eq!(claim_count(&store).await, 0);
}

#[tokio::test]
async fn missing_source_item_claim_is_rejected() {
    let store = test_store().await;

    let error = store
        .create_or_reinforce_claim(train_claim("item:missing".to_string()))
        .await
        .expect_err("missing source item should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemNotFound { item_id } if item_id == "item:missing"
    ));
    assert_eq!(claim_count(&store).await, 0);
}

#[tokio::test]
async fn deleted_source_item_claim_is_rejected() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    store
        .db()
        .query(
            r#"
            UPDATE conversation_items SET
              deleted_at = 'test-deleted',
              updated_at = time::now()
            WHERE item_id = $item_id;
            "#,
        )
        .bind(("item_id", source_item.item_id.clone()))
        .await
        .expect("delete source item")
        .check()
        .expect("source item delete should succeed");

    let error = store
        .create_or_reinforce_claim(train_claim(source_item.item_id.clone()))
        .await
        .expect_err("deleted source item should be rejected");

    assert!(matches!(
        error,
        StoreError::ConversationItemNotFound { item_id } if item_id == source_item.item_id
    ));
    assert_eq!(claim_count(&store).await, 0);
}

#[tokio::test]
async fn colliding_record_fragment_entity_ids_remain_distinct() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Alex likes trains.").await;
    let second_item = create_source_item(&store, "Blair likes trains.").await;

    store
        .create_or_reinforce_claim(person_train_claim(
            "person:a-b",
            "Alex",
            first_item.item_id,
            "Alex likes trains.",
        ))
        .await
        .expect("first person claim");
    store
        .create_or_reinforce_claim(person_train_claim(
            "person:a_b",
            "Blair",
            second_item.item_id,
            "Blair likes trains.",
        ))
        .await
        .expect("second person claim");

    let rows = entities_by_id(&store, &["person:a-b", "person:a_b"]).await;

    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| { row.entity_id == "person:a-b" && row.canonical_name == "Alex" })
    );
    assert!(
        rows.iter()
            .any(|row| { row.entity_id == "person:a_b" && row.canonical_name == "Blair" })
    );
}

#[tokio::test]
async fn entity_upsert_preserves_existing_aliases_and_metadata() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    store
        .db()
        .query(
            r#"
            CREATE entities SET
              entity_id = 'concept:trains',
              entity_type = 'concept',
              canonical_name = 'rail transport',
              aliases = ['railways'],
              metadata = { source: 'manual' },
              updated_at = time::now();
            "#,
        )
        .await
        .expect("create enriched entity")
        .check()
        .expect("enriched entity should insert");

    store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("claim with existing enriched entity");
    let rows = entities_by_id(&store, &["concept:trains"]).await;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].aliases, vec!["railways".to_string()]);
    assert_eq!(rows[0].metadata, json!({ "source": "manual" }));
}

#[tokio::test]
async fn retrieval_includes_normal_active_claim_for_allowed_use_mode() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    let summary = store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create claim");

    let result = store
        .retrieve_claims(&personalize_request(), "trains", 8)
        .await
        .expect("retrieve claims");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, summary.claim_id);
    assert_eq!(result.included[0].fact, "Kevin likes trains.");
    assert_eq!(result.included[0].predicate_id, "likes");
    assert_eq!(result.included[0].rank_score, 100);
}

#[tokio::test]
async fn retrieval_redacts_policy_denied_sensitive_claim() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes night trains.").await;
    let mut candidate = train_claim(source_item.item_id);
    candidate.fact = "Kevin likes night trains.".to_string();
    candidate.sensitivity = Sensitivity::Sensitive;
    candidate.retrieval_hints = json!({ "keywords": ["night trains"] });
    store
        .create_or_reinforce_claim(candidate)
        .await
        .expect("create sensitive claim");

    let mut request = personalize_request();
    request.sensitivity_ceiling = Sensitivity::Sensitive;
    let result = store
        .retrieve_claims(&request, "night trains", 8)
        .await
        .expect("retrieve claims");

    assert!(result.included.is_empty());
    assert_eq!(result.redacted_omission_count, 1);
}

#[tokio::test]
async fn retrieval_respects_use_mode_predicate_policy() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create claim");

    let mut request = personalize_request();
    request.use_mode = UseMode::Act;
    let result = store
        .retrieve_claims(&request, "trains", 8)
        .await
        .expect("retrieve claims");

    assert!(result.included.is_empty());
    assert_eq!(result.redacted_omission_count, 1);
}

#[tokio::test]
async fn retrieval_limit_keeps_fact_match_over_earlier_hint_only_match() {
    let store = test_store().await;
    let hint_item = create_source_item(&store, "Kevin likes locomotives.").await;
    let fact_item = create_source_item(&store, "Kevin likes trains.").await;
    let mut hint_only = train_claim(hint_item.item_id);
    hint_only.fact = "Kevin likes locomotives.".to_string();
    hint_only.object = EntityCandidate::concept("locomotives", "locomotives");
    hint_only.retrieval_hints = json!({ "keywords": ["trains"] });
    let hint_summary = store
        .create_or_reinforce_claim(hint_only)
        .await
        .expect("create hint-only claim");
    let fact_summary = store
        .create_or_reinforce_claim(train_claim(fact_item.item_id))
        .await
        .expect("create fact-match claim");

    let result = store
        .retrieve_claims(&personalize_request(), "trains", 1)
        .await
        .expect("retrieve claims");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, fact_summary.claim_id);
    assert_ne!(result.included[0].claim_id, hint_summary.claim_id);
    assert_eq!(result.included[0].rank_score, 100);
}

#[tokio::test]
async fn retrieval_limit_zero_returns_empty_result() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create claim");

    let result = store
        .retrieve_claims(&personalize_request(), "trains", 0)
        .await
        .expect("retrieve claims");

    assert!(result.included.is_empty());
    assert_eq!(result.redacted_omission_count, 0);
}

async fn create_source_item(store: &NoemaStore, text: &str) -> crate::ConversationItemRecord {
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let turn = store
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({ "turn_index": 1 }),
        })
        .await
        .expect("turn");
    store
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id,
            turn_id: Some(turn.turn_id),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some(text.to_string()),
            payload_json: json!({}),
            metadata: json!({}),
        })
        .await
        .expect("source item")
}

fn train_claim(source_item_id: String) -> NewClaimCandidate {
    NewClaimCandidate {
        subject: EntityCandidate::local_human(),
        object: EntityCandidate::concept("trains", "trains"),
        predicate_id: "likes".to_string(),
        fact: "Kevin likes trains.".to_string(),
        sensitivity: Sensitivity::Normal,
        status: ClaimStatus::Active,
        confidence: Some(0.9),
        evidence: EvidenceCandidate {
            source_item_id,
            authority: EvidenceAuthority::ExplicitHumanStatement,
            excerpt: Some("Kevin likes trains.".to_string()),
        },
        retrieval_hints: json!({}),
        metadata: json!({}),
    }
}

fn personalize_request() -> ClaimRetrievalRequest {
    ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids: Vec::new(),
        use_mode: UseMode::Personalize,
        explicit_memory_request: false,
        sensitivity_ceiling: Sensitivity::Normal,
        approved_secret_access: false,
    }
}

fn person_train_claim(
    subject_entity_id: &str,
    subject_name: &str,
    source_item_id: String,
    fact: &str,
) -> NewClaimCandidate {
    NewClaimCandidate {
        subject: EntityCandidate {
            entity_id: subject_entity_id.to_string(),
            entity_type: EntityType::Person,
            canonical_name: subject_name.to_string(),
        },
        object: EntityCandidate::concept("trains", "trains"),
        predicate_id: "likes".to_string(),
        fact: fact.to_string(),
        sensitivity: Sensitivity::Normal,
        status: ClaimStatus::Active,
        confidence: Some(0.9),
        evidence: EvidenceCandidate {
            source_item_id,
            authority: EvidenceAuthority::ExplicitHumanStatement,
            excerpt: Some(fact.to_string()),
        },
        retrieval_hints: json!({}),
        metadata: json!({}),
    }
}

async fn claim_count(store: &NoemaStore) -> i64 {
    #[derive(Debug, serde::Deserialize)]
    struct CountRow {
        count: i64,
    }

    let mut response = store
        .db()
        .query("SELECT count() AS count FROM claims GROUP ALL;")
        .await
        .expect("count claims");
    let rows: Vec<CountRow> = response.take(0).expect("claim count rows");
    rows.first().map_or(0, |row| row.count)
}

async fn support_count(store: &NoemaStore, claim_id: &str) -> i64 {
    #[derive(Debug, serde::Deserialize)]
    struct CountRow {
        count: i64,
    }

    let mut response = store
        .db()
        .query(
            r#"
            SELECT count() AS count
            FROM supported_by
            WHERE claim_id = $claim_id
            GROUP ALL;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .await
        .expect("count support rows");
    let rows: Vec<CountRow> = response.take(0).expect("support count rows");
    rows.first().map_or(0, |row| row.count)
}

async fn matching_train_claims(store: &NoemaStore) -> Vec<ClaimRow> {
    let mut response = store
        .db()
        .query(
            r#"
            SELECT claim_id, status, count(SELECT * FROM supported_by WHERE claim_id = $parent.claim_id) AS evidence_count
            FROM claims
            WHERE subject_entity_id = 'human:local'
              AND object_entity_id = 'concept:trains'
              AND predicate_id = 'likes'
              AND fact = 'Kevin likes trains.'
            ORDER BY status ASC;
            "#,
        )
        .await
        .expect("select matching claims");
    response.take(0).expect("matching claim rows")
}

async fn entities_by_id(store: &NoemaStore, entity_ids: &[&str]) -> Vec<EntityRow> {
    let mut response = store
        .db()
        .query(
            r#"
            SELECT entity_id, canonical_name, aliases, metadata
            FROM entities
            WHERE entity_id IN $entity_ids
            ORDER BY entity_id ASC;
            "#,
        )
        .bind((
            "entity_ids",
            entity_ids
                .iter()
                .map(|entity_id| (*entity_id).to_string())
                .collect::<Vec<_>>(),
        ))
        .await
        .expect("select entities");
    response.take(0).expect("entity rows")
}

#[derive(Debug, serde::Deserialize)]
struct ClaimRow {
    claim_id: String,
    status: String,
    evidence_count: i64,
}

#[derive(Debug, serde::Deserialize)]
struct EntityRow {
    entity_id: String,
    canonical_name: String,
    aliases: Vec<String>,
    metadata: Value,
}
