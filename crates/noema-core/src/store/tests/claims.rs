use serde_json::{Value, json};
use surrealdb::types::SurrealValue;

use super::test_store;
use crate::{
    ActorRef, ClaimStatus, ClaimWriteOutcome, ConversationItemKind, ConversationItemStatus,
    EntityCandidate, EntityType, EvidenceAuthority, EvidenceCandidate, NewClaimCandidate,
    NewConversation, NewConversationItem, NewConversationTurn, NoemaStore, StoreError,
    memory::{ClaimRetrievalRequest, Sensitivity, UseMode},
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
    assert_eq!(summary.write_outcome, ClaimWriteOutcome::Created);
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
    assert_eq!(first.write_outcome, ClaimWriteOutcome::Created);
    assert_eq!(second.write_outcome, ClaimWriteOutcome::Reinforced);
    assert_eq!(second.evidence_count, 2);
    assert_eq!(claim_count(&store).await, 1);
}

#[tokio::test]
async fn fallback_note_punctuation_variants_reinforce_one_claim() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Garage code is 1234!").await;
    let second_item = create_source_item(&store, "Garage code is 1234.").await;

    let first = store
        .create_or_reinforce_claim(note_claim(
            first_item.item_id,
            "claim_object:has_note:note:v1:placeholder".to_string(),
            "Garage code is 1234",
            "Garage code is 1234!",
        ))
        .await
        .expect("first note claim");
    let second = store
        .create_or_reinforce_claim(note_claim(
            second_item.item_id,
            "claim_object:has_note:note:v1:placeholder".to_string(),
            "Garage code is 1234",
            "Garage code is 1234.",
        ))
        .await
        .expect("reinforced note claim");

    assert_eq!(first.claim_id, second.claim_id);
    assert_eq!(second.write_outcome, ClaimWriteOutcome::Reinforced);
    assert_eq!(second.evidence_count, 2);
    assert_eq!(claim_count(&store).await, 1);
}

#[tokio::test]
async fn confirmed_reinforcement_promotes_candidate_claim() {
    let store = test_store().await;
    let provider_item = create_source_item(&store, "I like trains.").await;
    let explicit_item = create_source_item(&store, "remember: I like trains.").await;

    let mut provider_candidate = train_claim(provider_item.item_id);
    provider_candidate.status = ClaimStatus::Candidate;
    provider_candidate.sensitivity = Sensitivity::Public;
    provider_candidate.confidence = Some(0.42);
    provider_candidate.evidence.authority = EvidenceAuthority::AgentInference;
    let candidate = store
        .create_or_reinforce_claim(provider_candidate)
        .await
        .expect("candidate claim");

    assert_eq!(candidate.status, ClaimStatus::Candidate);
    assert!(
        store
            .retrieve_claims(&personalize_request(), "trains", 8)
            .await
            .expect("candidate retrieval")
            .included
            .is_empty()
    );

    let mut explicit_candidate = train_claim(explicit_item.item_id);
    explicit_candidate.status = ClaimStatus::Confirmed;
    explicit_candidate.confidence = Some(0.95);
    let confirmed = store
        .create_or_reinforce_claim(explicit_candidate)
        .await
        .expect("confirmed reinforcement");

    assert_eq!(candidate.claim_id, confirmed.claim_id);
    assert_eq!(confirmed.write_outcome, ClaimWriteOutcome::Reinforced);
    assert_eq!(confirmed.status, ClaimStatus::Confirmed);
    assert_eq!(confirmed.sensitivity, Sensitivity::Normal);
    assert_eq!(confirmed.evidence_count, 2);
    assert_eq!(
        claim_confidence(&store, &confirmed.claim_id).await,
        Some(0.95)
    );

    let result = store
        .retrieve_claims(&personalize_request(), "trains", 8)
        .await
        .expect("confirmed retrieval");
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, confirmed.claim_id);
}

#[tokio::test]
async fn reinforcement_does_not_resurrect_non_live_claim_statuses() {
    for existing_status in [
        ClaimStatus::Archived,
        ClaimStatus::Superseded,
        ClaimStatus::Disputed,
    ] {
        for incoming_status in [ClaimStatus::Active, ClaimStatus::Confirmed] {
            let store = test_store().await;
            let first_item = create_source_item(&store, "Kevin likes trains.").await;
            let second_item = create_source_item(&store, "Still true: Kevin likes trains.").await;

            let existing = store
                .create_or_reinforce_claim(train_claim(first_item.item_id))
                .await
                .expect("initial claim");
            set_claim_status(&store, &existing.claim_id, existing_status).await;

            let mut reinforcement = train_claim(second_item.item_id);
            reinforcement.status = incoming_status;
            reinforcement.evidence.authority = EvidenceAuthority::RepeatedObservation;
            reinforcement.evidence.excerpt = Some("Still true: Kevin likes trains.".to_string());
            let reinforced = store
                .create_or_reinforce_claim(reinforcement)
                .await
                .expect("reinforce non-live claim");

            assert_eq!(reinforced.claim_id, existing.claim_id);
            assert_eq!(reinforced.write_outcome, ClaimWriteOutcome::Reinforced);
            assert_eq!(reinforced.status, existing_status);
            assert_eq!(reinforced.evidence_count, 2);
        }
    }
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
    assert_eq!(replacement.write_outcome, ClaimWriteOutcome::Created);

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
async fn retrieval_term_ranking_keeps_content_matches_over_hint_only_matches() {
    let store = test_store().await;
    let hint_item = create_source_item(&store, "Kevin likes locomotives.").await;
    let fact_item = create_source_item(&store, "Kevin likes night trains.").await;
    let hint_terms = (0..25)
        .map(|index| format!("hint{index}"))
        .collect::<Vec<_>>();
    let mut hint_only = train_claim(hint_item.item_id);
    hint_only.fact = "Kevin likes locomotives.".to_string();
    hint_only.object = EntityCandidate::concept("locomotives", "locomotives");
    hint_only.retrieval_hints = json!({ "keywords": hint_terms.clone() });
    let hint_summary = store
        .create_or_reinforce_claim(hint_only)
        .await
        .expect("create hint-only claim");
    let mut fact = train_claim(fact_item.item_id);
    fact.fact = "Kevin likes night trains.".to_string();
    let fact_summary = store
        .create_or_reinforce_claim(fact)
        .await
        .expect("create content-match claim");

    let query = format!("night {}", hint_terms.join(" "));
    let result = store
        .retrieve_claims(&personalize_request(), &query, 1)
        .await
        .expect("retrieve claims");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, fact_summary.claim_id);
    assert_ne!(result.included[0].claim_id, hint_summary.claim_id);
}

#[tokio::test]
async fn retrieval_short_terms_match_tokens_not_substrings() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create train claim");

    let result = store
        .retrieve_claims(&personalize_request(), "ai", 8)
        .await
        .expect("retrieve claims");

    assert_eq!(result.redacted_omission_count, 0);
    assert!(result.included.is_empty());
}

#[tokio::test]
async fn retrieval_long_content_token_query_cannot_outrank_exact_content_match() {
    let store = test_store().await;
    let exact_item = create_source_item(&store, "Kevin likes exact ranked terms.").await;
    let token_item = create_source_item(&store, "Kevin likes scattered ranked terms.").await;
    let terms = (0..40)
        .map(|index| format!("topic{index}"))
        .collect::<Vec<_>>();
    let query = terms.join(" ");
    let exact_summary = store
        .create_or_reinforce_claim({
            let mut claim = train_claim(exact_item.item_id);
            claim.object = EntityCandidate::concept("exact-ranked-terms", "exact ranked terms");
            claim.fact = format!("Kevin remembers {query}.");
            claim
        })
        .await
        .expect("create exact content claim");
    store
        .create_or_reinforce_claim({
            let mut claim = train_claim(token_item.item_id);
            claim.object =
                EntityCandidate::concept("scattered-ranked-terms", "scattered ranked terms");
            claim.fact = format!(
                "Kevin remembers scattered terms: {}.",
                terms.into_iter().rev().collect::<Vec<_>>().join(" ")
            );
            claim
        })
        .await
        .expect("create content-token claim");

    let result = store
        .retrieve_claims(&personalize_request(), &query, 1)
        .await
        .expect("retrieve claims");

    assert_eq!(result.redacted_omission_count, 0);
    assert_eq!(result.included.len(), 1);
    assert_eq!(result.included[0].claim_id, exact_summary.claim_id);
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
        .retrieve_claims_scoped(&personalize_request(), "", &["human:local".to_string()], 8)
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

#[tokio::test]
async fn list_claims_filters_status_predicate_query_and_clamps_limit() {
    let store = test_store().await;
    let trains_item = create_source_item(&store, "Kevin likes trains.").await;
    let coffee_item = create_source_item(&store, "Kevin prefers coffee.").await;
    let mut trains = train_claim(trains_item.item_id);
    trains.status = ClaimStatus::Confirmed;
    let trains_summary = store
        .create_or_reinforce_claim(trains)
        .await
        .expect("create train claim");
    let mut coffee = train_claim(coffee_item.item_id);
    coffee.object = EntityCandidate::concept("coffee", "coffee");
    coffee.predicate_id = "prefers".to_string();
    coffee.fact = "Kevin prefers coffee.".to_string();
    coffee.status = ClaimStatus::Candidate;
    store
        .create_or_reinforce_claim(coffee)
        .await
        .expect("create coffee claim");

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("trains".to_string()),
            status: Some(ClaimStatus::Confirmed),
            predicate_id: Some("likes".to_string()),
            limit: Some(999),
        })
        .await
        .expect("list claims");

    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].claim_id, trains_summary.claim_id);
    assert_eq!(claims[0].predicate_label, "likes");
    assert_eq!(claims[0].subject_entity_name, "Local human");
    assert_eq!(claims[0].object_entity_name.as_deref(), Some("trains"));
    assert_eq!(claims[0].object_entity_type.as_deref(), Some("concept"));
    assert_eq!(claims[0].evidence_count, 1);
    assert!(claims[0].created_at.contains('T'));
    assert!(claims[0].updated_at.contains('T'));
}

#[tokio::test]
async fn memory_graph_defaults_to_current_statuses_and_reports_truncation() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let coffee_item = create_source_item(&store, "Kevin prefers coffee.").await;
    let tea_item = create_source_item(&store, "Kevin prefers tea.").await;
    let notebook_item = create_source_item(&store, "Kevin keeps a Noema notebook.").await;
    let archived_item = create_source_item(&store, "Kevin liked old buses.").await;

    let mut train = train_claim(train_item.item_id);
    train.status = ClaimStatus::Candidate;
    store
        .create_or_reinforce_claim(train)
        .await
        .expect("create candidate claim");

    let mut coffee = train_claim(coffee_item.item_id);
    coffee.object = EntityCandidate::concept("coffee", "coffee");
    coffee.predicate_id = "prefers".to_string();
    coffee.fact = "Kevin prefers coffee.".to_string();
    coffee.status = ClaimStatus::Active;
    store
        .create_or_reinforce_claim(coffee)
        .await
        .expect("create active claim");

    let mut tea = train_claim(tea_item.item_id);
    tea.object = EntityCandidate::concept("tea", "tea");
    tea.predicate_id = "prefers".to_string();
    tea.fact = "Kevin prefers tea.".to_string();
    tea.status = ClaimStatus::Confirmed;
    store
        .create_or_reinforce_claim(tea)
        .await
        .expect("create confirmed claim");

    let mut notebook = train_claim(notebook_item.item_id);
    notebook.object = EntityCandidate::concept("noema-notebook", "Noema notebook");
    notebook.predicate_id = "has_note".to_string();
    notebook.fact = "Kevin keeps a Noema notebook.".to_string();
    notebook.status = ClaimStatus::Active;
    store
        .create_or_reinforce_claim(notebook)
        .await
        .expect("create second active claim");

    let mut archived = train_claim(archived_item.item_id);
    archived.object = EntityCandidate::concept("old-buses", "old buses");
    archived.fact = "Kevin liked old buses.".to_string();
    archived.status = ClaimStatus::Archived;
    store
        .create_or_reinforce_claim(archived)
        .await
        .expect("create archived claim");

    let full_graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: None,
            statuses: None,
            predicate_id: None,
            sensitivity: None,
            limit: Some(10),
        })
        .await
        .expect("full memory graph");

    assert_eq!(full_graph.edges.len(), 4);
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Candidate)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Active)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .any(|edge| edge.status == ClaimStatus::Confirmed)
    );
    assert!(
        full_graph
            .edges
            .iter()
            .all(|edge| edge.status != ClaimStatus::Archived)
    );

    let graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: None,
            statuses: None,
            predicate_id: None,
            sensitivity: None,
            limit: Some(3),
        })
        .await
        .expect("memory graph");

    assert_eq!(graph.summary.limit, 3);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(graph.summary.returned_claim_count, 3);
    assert_eq!(graph.summary.returned_node_count, graph.nodes.len() as i64);
    assert!(graph.summary.truncated);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.entity_id == "human:local" && node.claim_count >= 1)
    );
}

#[tokio::test]
async fn memory_graph_filters_query_predicate_and_sensitivity() {
    let store = test_store().await;
    let train_item = create_source_item(&store, "Kevin likes trains.").await;
    let private_item = create_source_item(&store, "Garage code is 1234.").await;

    store
        .create_or_reinforce_claim(train_claim(train_item.item_id))
        .await
        .expect("create train claim");

    let mut private_note = train_claim(private_item.item_id);
    private_note.object = EntityCandidate::concept("garage-code", "Garage code");
    private_note.predicate_id = "has_note".to_string();
    private_note.fact = "Garage code is 1234.".to_string();
    private_note.sensitivity = Sensitivity::Private;
    private_note.status = ClaimStatus::Confirmed;
    store
        .create_or_reinforce_claim(private_note)
        .await
        .expect("create private claim");

    let graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: Some("has_note".to_string()),
            statuses: Some(vec![ClaimStatus::Confirmed]),
            predicate_id: Some("has_note".to_string()),
            sensitivity: Some(Sensitivity::Private),
            limit: Some(150),
        })
        .await
        .expect("memory graph");

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].predicate_id, "has_note");
    assert_eq!(graph.edges[0].sensitivity, Sensitivity::Private);
    assert_eq!(graph.edges[0].fact, "Garage code is 1234.");
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.max_sensitivity == Sensitivity::Private)
    );
}

#[tokio::test]
async fn memory_graph_query_does_not_match_non_public_content() {
    let store = test_store().await;
    let private_item = create_source_item(&store, "Garage code is 1234.").await;

    let mut private_note = train_claim(private_item.item_id);
    private_note.object = EntityCandidate::concept("garage-code", "Garage code");
    private_note.predicate_id = "has_note".to_string();
    private_note.fact = "Garage code is 1234.".to_string();
    private_note.sensitivity = Sensitivity::Private;
    private_note.status = ClaimStatus::Confirmed;
    store
        .create_or_reinforce_claim(private_note)
        .await
        .expect("create private claim");

    let fact_graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: Some("1234".to_string()),
            statuses: Some(vec![ClaimStatus::Confirmed]),
            predicate_id: None,
            sensitivity: None,
            limit: Some(10),
        })
        .await
        .expect("private fact query should not error");
    assert!(fact_graph.edges.is_empty());

    let entity_graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: Some("garage".to_string()),
            statuses: Some(vec![ClaimStatus::Confirmed]),
            predicate_id: None,
            sensitivity: None,
            limit: Some(10),
        })
        .await
        .expect("private entity query should not error");
    assert!(entity_graph.edges.is_empty());
}

#[tokio::test]
async fn memory_graph_query_uses_bounded_candidate_window() {
    let store = test_store().await;
    let old_item = create_source_item(&store, "Kevin likes old trains.").await;
    let mid_item = create_source_item(&store, "Kevin likes mid-century trains.").await;
    let new_item = create_source_item(&store, "Kevin likes new trains.").await;

    let mut old = train_claim(old_item.item_id);
    old.object = EntityCandidate::concept("old-trains", "old trains");
    old.fact = "Kevin likes old trains.".to_string();
    let old_summary = store
        .create_or_reinforce_claim(old)
        .await
        .expect("create older matching claim");

    let mut mid = train_claim(mid_item.item_id);
    mid.object = EntityCandidate::concept("mid-century-trains", "mid-century trains");
    mid.fact = "Kevin likes mid-century trains.".to_string();
    store
        .create_or_reinforce_claim(mid)
        .await
        .expect("create middle matching claim");

    let mut new = train_claim(new_item.item_id);
    new.object = EntityCandidate::concept("new-trains", "new trains");
    new.fact = "Kevin likes new trains.".to_string();
    store
        .create_or_reinforce_claim(new)
        .await
        .expect("create newest matching claim");

    store
        .db
        .query(
            r#"
            UPDATE claims SET
              object_entity_id = 'concept:missing',
              created_at = <datetime>'2020-01-01T00:00:00Z',
              updated_at = <datetime>'2020-01-01T00:00:00Z'
            WHERE claim_id = $claim_id;
            "#,
        )
        .bind(("claim_id", old_summary.claim_id))
        .await
        .expect("corrupt older claim object reference")
        .check()
        .expect("corrupt older claim object reference check");

    let graph = store
        .memory_graph(crate::MemoryGraphFilter {
            query: Some("not-in-any-visible-row".to_string()),
            statuses: Some(vec![ClaimStatus::Active]),
            predicate_id: None,
            sensitivity: None,
            limit: Some(1),
        })
        .await
        .expect("bounded query should not inspect older corrupt rows");

    assert!(graph.edges.is_empty());
    assert!(graph.summary.truncated);
}

#[tokio::test]
async fn memory_graph_errors_on_unresolved_object_entity_reference() {
    let store = test_store().await;
    let source_item = create_source_item(&store, "Kevin likes trains.").await;
    let summary = store
        .create_or_reinforce_claim(train_claim(source_item.item_id))
        .await
        .expect("create claim");

    store
        .db
        .query(
            r#"
            UPDATE claims SET object_entity_id = 'concept:missing'
            WHERE claim_id = $claim_id;
            "#,
        )
        .bind(("claim_id", summary.claim_id))
        .await
        .expect("corrupt claim object reference")
        .check()
        .expect("corrupt claim object reference check");

    let error = store
        .memory_graph(crate::MemoryGraphFilter {
            query: None,
            statuses: None,
            predicate_id: None,
            sensitivity: None,
            limit: Some(10),
        })
        .await
        .expect_err("corrupt graph reference should fail closed");

    assert!(matches!(error, StoreError::Schema(message) if message.contains("object entity")));
}

#[tokio::test]
async fn claim_detail_includes_support_evidence_and_unknown_claim_is_none() {
    let store = test_store().await;
    let first_item = create_source_item(&store, "Kevin likes trains.").await;
    let second_item = create_source_item(&store, "Still true: Kevin likes trains.").await;
    let first = store
        .create_or_reinforce_claim(train_claim(first_item.item_id.clone()))
        .await
        .expect("first claim");
    let mut second = train_claim(second_item.item_id.clone());
    second.evidence.authority = EvidenceAuthority::RepeatedObservation;
    second.evidence.excerpt = Some("Still true: Kevin likes trains.".to_string());
    store
        .create_or_reinforce_claim(second)
        .await
        .expect("reinforce claim");

    let detail = store
        .get_claim_detail(&first.claim_id)
        .await
        .expect("get detail")
        .expect("claim detail");

    assert_eq!(detail.claim.claim_id, first.claim_id);
    assert_eq!(detail.claim.evidence_count, 2);
    assert_eq!(detail.evidence.len(), 2);
    assert!(
        detail
            .evidence
            .iter()
            .all(|evidence| evidence.evidence_id.is_some())
    );
    assert!(
        detail
            .evidence
            .iter()
            .any(|evidence| evidence.source_item_id.as_deref() == Some(&first_item.item_id))
    );
    assert!(
        detail
            .evidence
            .iter()
            .any(|evidence| evidence.authority == "repeated_observation")
    );

    let missing = store
        .get_claim_detail("claim:missing")
        .await
        .expect("missing detail");
    assert!(missing.is_none());
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

fn note_claim(
    source_item_id: String,
    object_entity_id: String,
    object_name: &str,
    fact: &str,
) -> NewClaimCandidate {
    NewClaimCandidate {
        subject: EntityCandidate::local_human(),
        object: EntityCandidate::concept(&object_entity_id, object_name),
        predicate_id: "has_note".to_string(),
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
    #[derive(Debug, serde::Deserialize, SurrealValue)]
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
    #[derive(Debug, serde::Deserialize, SurrealValue)]
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

async fn claim_confidence(store: &NoemaStore, claim_id: &str) -> Option<f64> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct ConfidenceRow {
        confidence: Option<f64>,
    }

    let mut response = store
        .db()
        .query(
            r#"
            SELECT confidence
            FROM claims
            WHERE claim_id = $claim_id
            LIMIT 1;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .await
        .expect("select claim confidence");
    let rows: Vec<ConfidenceRow> = response.take(0).expect("confidence rows");
    rows.into_iter().next().and_then(|row| row.confidence)
}

async fn set_claim_status(store: &NoemaStore, claim_id: &str, status: ClaimStatus) {
    store
        .db()
        .query(
            r#"
            UPDATE claims SET
              status = $status,
              updated_at = time::now()
            WHERE claim_id = $claim_id;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .bind(("status", claim_status_str(status).to_string()))
        .await
        .expect("update claim status")
        .check()
        .expect("claim status update should succeed");
}

const fn claim_status_str(status: ClaimStatus) -> &'static str {
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

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct ClaimRow {
    claim_id: String,
    status: String,
    evidence_count: i64,
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct EntityRow {
    entity_id: String,
    canonical_name: String,
    aliases: Vec<String>,
    metadata: Value,
}
