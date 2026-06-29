use super::*;

#[test]
fn graph_claim_list_redacts_non_public_facts() {
    let claim = GraphqlMemoryClaim {
        claim_id: "claim_sensitive".to_string(),
        fact: "[redacted; use memoryClaim(claimId) for detail]".to_string(),
        fact_redacted: true,
        predicate_id: "has_note".to_string(),
        predicate_label: "has note".to_string(),
        subject_entity_id: "human:local".to_string(),
        subject_entity_name: "Kevin".to_string(),
        subject_entity_type: "human".to_string(),
        object_entity_id: Some("note:garage".to_string()),
        object_entity_name: Some("garage code".to_string()),
        object_entity_type: Some("concept".to_string()),
        status: "confirmed".to_string(),
        sensitivity: "secret".to_string(),
        confidence: Some(0.9),
        evidence_count: 1,
        created_at: "2026-06-28T12:00:00Z".to_string(),
        updated_at: "2026-06-28T12:00:00Z".to_string(),
    };
    let mut output = Vec::new();

    write_memory_claim_list(&mut output, &[claim]).expect("write list");
    let output = String::from_utf8(output).expect("utf8");

    assert!(output.contains("claim_sensitive"));
    assert!(output.contains("secret"));
    assert!(output.contains("has note"));
    assert!(output.contains("[redacted; use memoryClaim(claimId) for detail]"));
    assert!(!output.contains("Garage code is 1234"));
}

#[test]
fn graph_claim_detail_includes_unredacted_evidence() {
    let detail = GraphqlMemoryClaimDetail {
        claim_id: "claim_1".to_string(),
        fact: "Kevin likes trains.".to_string(),
        predicate_id: "likes".to_string(),
        predicate_label: "likes".to_string(),
        subject_entity_id: "human:local".to_string(),
        subject_entity_name: "Kevin".to_string(),
        subject_entity_type: "human".to_string(),
        object_entity_id: Some("concept:trains".to_string()),
        object_entity_name: Some("trains".to_string()),
        object_entity_type: Some("concept".to_string()),
        status: "confirmed".to_string(),
        sensitivity: "normal".to_string(),
        confidence: Some(0.9),
        evidence_count: 1,
        created_at: "2026-06-28T12:00:00Z".to_string(),
        updated_at: "2026-06-28T12:00:00Z".to_string(),
        evidence: vec![GraphqlMemoryClaimEvidence {
            evidence_id: Some("evidence_1".to_string()),
            source_item_id: Some("item_1".to_string()),
            authority: "explicit_human_statement".to_string(),
            excerpt: Some("Kevin likes trains.".to_string()),
            observed_at: Some("2026-06-28T12:00:00Z".to_string()),
            created_at: "2026-06-28T12:00:00Z".to_string(),
        }],
    };
    let mut output = Vec::new();

    write_memory_claim_detail(&mut output, &detail).expect("write detail");
    let output = String::from_utf8(output).expect("utf8");

    assert!(output.contains("ID: claim_1"));
    assert!(output.contains("Fact: Kevin likes trains."));
    assert!(output.contains("Evidence"));
    assert!(output.contains("item_1"));
    assert!(output.contains("explicit_human_statement"));
    assert!(output.contains("Kevin likes trains."));
}

#[test]
fn predicate_proposal_list_shows_compact_columns() {
    let proposal = GraphqlPredicateProposalSummary {
        proposal_id: "predicate_proposal_1".to_string(),
        label: "collects".to_string(),
        description: "The subject collects the object.".to_string(),
        status: "candidate".to_string(),
        source_item_id: Some("item_1".to_string()),
        created_at: "2026-06-29T12:00:00Z".to_string(),
        updated_at: "2026-06-29T12:00:00Z".to_string(),
    };
    let mut output = Vec::new();

    write_predicate_proposal_list(&mut output, &[proposal]).expect("write list");
    let output = String::from_utf8(output).expect("utf8");

    assert!(output.contains("predicate_proposal_1"));
    assert!(output.contains("candidate"));
    assert!(output.contains("collects"));
    assert!(output.contains("The subject collects the object."));
}

#[test]
fn predicate_proposal_list_query_omits_full_json_fields() {
    assert!(!PREDICATE_PROPOSALS_QUERY.contains("proposedPredicate"));
    assert!(!PREDICATE_PROPOSALS_QUERY.contains("proposedClaim"));
    assert!(PREDICATE_PROPOSAL_QUERY.contains("proposedPredicate"));
    assert!(PREDICATE_PROPOSAL_QUERY.contains("proposedClaim"));
}

#[test]
fn predicate_proposal_detail_includes_pretty_json() {
    let proposal = GraphqlPredicateProposal {
        proposal_id: "predicate_proposal_1".to_string(),
        label: "collects".to_string(),
        description: "The subject collects the object.".to_string(),
        proposed_predicate: serde_json::json!({
            "label": "collects",
            "allowed_use_modes": ["answer", "personalize"]
        }),
        proposed_claim: serde_json::json!({
            "fact": "Kevin collects model trains.",
            "subject": "human:local"
        }),
        status: "candidate".to_string(),
        source_item_id: Some("item_1".to_string()),
        created_at: "2026-06-29T12:00:00Z".to_string(),
        updated_at: "2026-06-29T12:01:00Z".to_string(),
    };
    let mut output = Vec::new();

    write_predicate_proposal_detail(&mut output, &proposal).expect("write detail");
    let output = String::from_utf8(output).expect("utf8");

    assert!(output.contains("ID: predicate_proposal_1"));
    assert!(output.contains("Source: item_1"));
    assert!(output.contains("Proposed predicate"));
    assert!(output.contains("\"allowed_use_modes\": ["));
    assert!(output.contains("\"answer\""));
    assert!(output.contains("Proposed claim"));
    assert!(output.contains("\"fact\": \"Kevin collects model trains.\""));
}
