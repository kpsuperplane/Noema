use serde_json::json;

use crate::{
    GovernedActionDecision, GovernedActionEffect, GovernedActionState, GovernedAssessmentStatus,
    GovernedAuthorization, GovernedExecutionOutcome, GovernedRecommendation, GovernedRisk,
    NewGovernedAction, NewGovernedActionAssessment, ObservedUrlSource, tests::test_store,
};

fn proposed_action(arguments: serde_json::Value) -> NewGovernedAction {
    NewGovernedAction {
        owner_human_id: "human:local".to_string(),
        conversation_id: None,
        turn_id: None,
        task_id: None,
        run_id: None,
        requesting_agent_id: "agent:primary".to_string(),
        capability_name: "mcp.example.write".to_string(),
        operation_token: "exact-token".to_string(),
        effect: GovernedActionEffect::Write,
        arguments,
        input_schema: json!({"type":"object"}),
        trusted_authority: json!({"human_or_task_request":"update the record"}),
        safe_summary: "mcp.example.write wants to write external data".to_string(),
    }
}

#[tokio::test]
async fn governed_action_preserves_exact_payload_and_digest() {
    let store = test_store().await;
    let arguments = json!({"record_id":"42","body":{"value":"exact"}});

    let action = store
        .create_governed_action(proposed_action(arguments.clone()))
        .await
        .expect("create action");

    assert_eq!(action.arguments, arguments);
    assert_eq!(action.state, GovernedActionState::Proposed);
    assert_eq!(action.arguments_sha256.len(), 64);

    let observed = vec!["https://example.com/result?q=1".to_string()];
    store
        .record_observed_urls(
            ObservedUrlSource::SearchResult,
            "tool_call:search",
            &observed,
        )
        .await
        .expect("record observed URL");
    assert!(
        store
            .has_observed_url(&observed[0])
            .await
            .expect("match observed URL")
    );
    assert!(
        !store
            .has_observed_url("https://example.com/result?q=2")
            .await
            .expect("reject changed URL")
    );
}

#[tokio::test]
async fn unavailable_reviewer_requires_approval_and_cannot_be_claimed() {
    let store = test_store().await;
    let action = store
        .create_governed_action(proposed_action(json!({"record_id":"42"})))
        .await
        .expect("create action");

    let reviewed = store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                recommendation: GovernedRecommendation::RequireApproval,
                reason_codes: vec!["authorization_ambiguous".to_string()],
                explanation: "reviewer is unavailable".to_string(),
            },
            None,
        )
        .await
        .expect("record fallback assessment");

    assert_eq!(reviewed.state, GovernedActionState::AwaitingApproval);
    assert!(
        store
            .claim_governed_action_execution(&action.action_id, action.revision, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn human_approval_is_owner_scoped_and_consumed_by_one_claim() {
    let store = test_store().await;
    let action = store
        .create_governed_action(proposed_action(json!({"record_id":"42"})))
        .await
        .expect("create action");
    store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                recommendation: GovernedRecommendation::RequireApproval,
                reason_codes: vec!["authorization_ambiguous".to_string()],
                explanation: "reviewer is unavailable".to_string(),
            },
            None,
        )
        .await
        .expect("record assessment");

    assert!(
        store
            .decide_governed_action(
                &action.action_id,
                action.revision,
                "human:someone-else",
                GovernedActionDecision::Approve,
            )
            .await
            .is_err()
    );
    let approved = store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            GovernedActionDecision::Approve,
        )
        .await
        .expect("approve action");
    assert_eq!(approved.state, GovernedActionState::Executable);
    assert_eq!(
        store
            .list_pending_governed_actions("human:local", None, None, 10)
            .await
            .expect("list pending"),
        Vec::new()
    );
    store
        .claim_governed_action_execution(&action.action_id, action.revision, None)
        .await
        .expect("consume approval");
    assert!(
        store
            .claim_governed_action_execution(&action.action_id, action.revision, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn clear_review_is_claimed_once_and_records_uncertain_outcome() {
    let store = test_store().await;
    let action = store
        .create_governed_action(proposed_action(json!({"record_id":"42"})))
        .await
        .expect("create action");
    let reviewed = store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::Completed,
                reviewer_selection: Some(json!({"model_profile":"reviewer"})),
                authorization: Some(GovernedAuthorization::Explicit),
                risk: Some(GovernedRisk::Low),
                recommendation: GovernedRecommendation::AutoExecute,
                reason_codes: vec!["action_matches_request".to_string()],
                explanation: "exact action is authorized".to_string(),
            },
            None,
        )
        .await
        .expect("record assessment");
    assert_eq!(reviewed.state, GovernedActionState::Executable);

    let claimed = store
        .claim_governed_action_execution(&action.action_id, action.revision, None)
        .await
        .expect("claim action");
    assert_eq!(claimed.state, GovernedActionState::Executing);
    assert!(
        store
            .claim_governed_action_execution(&action.action_id, action.revision, None)
            .await
            .is_err()
    );
    let finished = store
        .finish_governed_action_execution(
            &action.action_id,
            action.revision,
            GovernedExecutionOutcome::OutcomeUncertain,
            None,
            Some("outcome_uncertain"),
        )
        .await
        .expect("finish action");
    assert_eq!(finished.state, GovernedActionState::OutcomeUncertain);
}
