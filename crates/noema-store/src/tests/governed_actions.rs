use serde_json::json;

use crate::{
    GovernedActionDecision, GovernedActionEffect, GovernedActionState, GovernedAssessmentStatus,
    GovernedAuthorization, GovernedExecutionOutcome, GovernedRisk, McpAuthenticationRequestState,
    NewGovernedAction, NewGovernedActionAssessment, NewMcpAuthenticationRequest, ObservedUrlSource,
    tests::test_store,
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
        authorization_context: json!({"human_or_task_request":"update the record"}),
        safe_summary: "mcp.example.write wants to write external data".to_string(),
    }
}

#[tokio::test]
async fn mcp_authentication_request_is_idempotent_and_revision_fenced() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("conversation");
    store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, safe_config_json, auth_status, health_status, enabled) VALUES ('mcp:docs', 'Docs', 'streamable_http', '{}', 'needs_auth', 'unavailable', 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("server");
    let input = || NewMcpAuthenticationRequest {
        owner_human_id: "human:local".to_string(),
        conversation_id: Some(conversation.conversation_id.clone()),
        turn_id: Some("turn:auth".to_string()),
        task_id: None,
        run_id: None,
        task_generation: None,
        requesting_agent_id: "agent:primary".to_string(),
        mcp_server_id: "mcp:docs".to_string(),
        capability_name: "mcp.docs.search".to_string(),
        operation_token: "exact-token".to_string(),
        input_schema: json!({"type":"object"}),
        arguments: json!({"query":"private"}),
        output_index: 3,
        call_id: Some("call:3".to_string()),
        provider_call_id: None,
        provider_name: None,
        governed_action: None,
    };
    let first = store
        .create_mcp_authentication_request(input(), None)
        .await
        .expect("request");
    let duplicate = store
        .create_mcp_authentication_request(input(), None)
        .await
        .expect("duplicate");
    assert_eq!(first.request_id, duplicate.request_id);
    assert_eq!(first.arguments_sha256, duplicate.arguments_sha256);

    let authorizing = store
        .begin_mcp_authentication(
            &first.request_id,
            first.revision,
            "human:local",
            "attempt:1",
        )
        .await
        .expect("begin");
    assert_eq!(
        authorizing.state,
        McpAuthenticationRequestState::Authorizing
    );
    let claimed = store
        .claim_mcp_authentication_resumption(&first.request_id, first.revision)
        .await
        .expect("claim");
    assert_eq!(claimed.state, McpAuthenticationRequestState::Resuming);
    assert!(
        store
            .claim_mcp_authentication_resumption(&first.request_id, first.revision)
            .await
            .is_err()
    );
    let completed = store
        .finish_mcp_authentication_request(
            &first.request_id,
            first.revision,
            McpAuthenticationRequestState::Completed,
            Some(&json!({"success":true,"payload":{"result":"ok"}})),
            None,
        )
        .await
        .expect("finish");
    assert_eq!(completed.state, McpAuthenticationRequestState::Completed);
    assert!(
        store
            .list_pending_mcp_authentication_requests(
                "human:local",
                Some(&conversation.conversation_id),
                None,
                10,
            )
            .await
            .expect("pending")
            .is_empty()
    );

    let mut proposed = proposed_action(json!({"record_id":"42"}));
    proposed.conversation_id = Some(conversation.conversation_id.clone());
    proposed.turn_id = Some("turn:approved-auth".to_string());
    let action = store
        .create_governed_action(proposed)
        .await
        .expect("governed action");
    store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                reason_codes: vec!["reviewer_unavailable".to_string()],
                explanation: "reviewer unavailable".to_string(),
            },
            None,
        )
        .await
        .expect("assessment");
    store
        .decide_governed_action(
            &action.action_id,
            action.revision,
            "human:local",
            GovernedActionDecision::Approve,
        )
        .await
        .expect("approve");
    store
        .claim_governed_action_execution(&action.action_id, action.revision, None)
        .await
        .expect("claim approved action");
    let auth = store
        .create_mcp_authentication_request(
            NewMcpAuthenticationRequest {
                governed_action: Some((action.action_id.clone(), action.revision)),
                output_index: 4,
                arguments: json!({"record_id":"42"}),
                ..input()
            },
            None,
        )
        .await
        .expect("approved action authentication");
    assert_eq!(
        store
            .get_governed_action(&action.action_id, action.revision)
            .await
            .expect("read action")
            .expect("action")
            .state,
        GovernedActionState::AwaitingAuthentication
    );
    store
        .resume_governed_action_after_authentication(&action.action_id, action.revision)
        .await
        .expect("resume approved action");
    assert!(
        store
            .claim_governed_action_execution(&action.action_id, action.revision, None)
            .await
            .is_err(),
        "authentication must not recreate or consume another approval"
    );
    assert_eq!(
        auth.governed_action,
        Some((action.action_id, action.revision))
    );
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

#[tokio::test]
async fn composed_authorization_risk_policy_has_one_global_matrix() {
    let cases = [
        (GovernedAuthorization::Explicit, GovernedRisk::Low, true),
        (
            GovernedAuthorization::Substantive,
            GovernedRisk::Medium,
            true,
        ),
        (GovernedAuthorization::Weak, GovernedRisk::Low, true),
        (GovernedAuthorization::Absent, GovernedRisk::Low, false),
        (GovernedAuthorization::Weak, GovernedRisk::Medium, false),
        (GovernedAuthorization::Explicit, GovernedRisk::High, false),
    ];
    for (authorization, risk, executable) in cases {
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
                    authorization: Some(authorization),
                    risk: Some(risk),
                    reason_codes: vec!["action_matches_request".to_string()],
                    explanation: "bounded review".to_string(),
                },
                None,
            )
            .await
            .expect("record assessment");
        assert_eq!(
            reviewed.state == GovernedActionState::Executable,
            executable,
            "authorization={authorization:?}, risk={risk:?}"
        );
    }
}
