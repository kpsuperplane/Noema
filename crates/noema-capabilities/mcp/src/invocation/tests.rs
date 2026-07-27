use std::{sync::Arc, time::Duration};

use noema_capabilities::{
    CapabilityError, CapabilityInvocation, CapabilityInvoker, OperationToken,
    ReviewedCapabilityAuthorization,
};
use serde_json::json;
use tokio::sync::Semaphore;

use super::capability_output;
use crate::{
    McpClientError, McpDataSharingPolicy, McpDeleteServerCommand, McpOAuthStoredCredentials,
    McpOperations, McpSaveProviderPolicyCommand, McpServerAuthStatus, McpServerHealthStatus,
    McpUnsafeActionPolicy,
    service::test_support::{TestHarness, advertised_invocation},
};

#[derive(Clone, Copy)]
enum FailureStage {
    Preparation,
    ToolCall,
}

#[tokio::test]
async fn authority_policy_and_serialization_contracts() {
    // Case: forged_and_stale_authority_never_reach_the_transport.
    let harness = TestHarness::new();
    let invocation = advertised_invocation(&harness).await;
    let forged = CapabilityInvocation {
        operation: invocation.operation.clone(),
        operation_token: OperationToken::new("forged-authority"),
        arguments: json!({}),
        reviewed_authorization: None,
    };

    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, forged).await,
        Err(CapabilityError::UnknownOperation)
    );

    let mut changed = harness.repository.snapshot();
    changed.server.authority_generation = "generation:v2".to_string();
    harness.repository.set_snapshot(changed);

    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, invocation).await,
        Err(CapabilityError::UnknownOperation)
    );
    assert_eq!(harness.sessions.call_count(), 0);

    let harness = TestHarness::new();
    let invocation = advertised_invocation(&harness).await;
    let mut changed = harness.repository.snapshot();
    changed.policy.as_mut().expect("policy").policy_revision += 1;
    harness.repository.set_snapshot(changed);
    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, invocation).await,
        Err(CapabilityError::UnknownOperation)
    );

    // Case: live safe call is direct while a risky policy requires exact admission.
    let harness = TestHarness::new();
    let invocation = advertised_invocation(&harness).await;

    let output = CapabilityInvoker::invoke(&harness.service, invocation.clone())
        .await
        .expect("read invocation");
    assert!(output.success);
    assert_eq!(harness.sessions.call_count(), 1);

    let mut risky_snapshot = harness.repository.snapshot();
    let risky_policy = risky_snapshot.policy.as_mut().expect("policy");
    risky_policy.read_only.value = Some(false);
    risky_policy.destructive.value = Some(true);
    harness.repository.set_snapshot(risky_snapshot);
    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, invocation.clone()).await,
        Err(CapabilityError::Denied)
    );
    let mut admitted = invocation.clone();
    admitted.reviewed_authorization = Some(ReviewedCapabilityAuthorization::for_action(
        "action:test",
        1,
        &admitted.arguments,
    ));
    assert!(
        CapabilityInvoker::invoke(&harness.service, admitted)
            .await
            .is_ok()
    );

    assert_eq!(harness.sessions.call_count(), 2);

    // Case: refreshed_credentials_commit_before_the_remote_tool_call.
    let harness = TestHarness::new();
    harness.sessions.set_refreshed(McpOAuthStoredCredentials {
        client_id: "private-client".to_string(),
        token_response: json!({"access_token": "private-access-token"}),
        token_received_at: Some(42),
    });

    CapabilityInvoker::invoke(&harness.service, advertised_invocation(&harness).await)
        .await
        .expect("invocation");

    let events = harness.events.snapshot();
    let committed = position(&events, "secret_commit");
    let called = position(&events, "tool_call");
    assert!(committed < called, "events: {events:?}");
    assert!(
        harness
            .secrets
            .material("mcp:docs")
            .oauth_credentials
            .is_some()
    );

    // Case: missing_required_secret_material_fences_the_server_before_transport.
    let harness = TestHarness::new();
    let invocation = advertised_invocation(&harness).await;
    let mut snapshot = harness.repository.snapshot();
    snapshot.server.auth_status = crate::McpServerAuthStatus::Authenticated;
    snapshot.server.safe_config = json!({
        "command": "docs-server",
        "args": [],
        "cwd": null,
        "env": {},
        "secret_refs": {"env": ["TOKEN"]}
    });
    harness.repository.set_snapshot(snapshot);

    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, invocation).await,
        Err(CapabilityError::Unavailable)
    );
    assert_eq!(harness.sessions.call_count(), 0);
    assert!(harness.repository.events().contains(&"record_status"));
    assert!(!harness.events.contains("session_prepare"));

    // Case: delete_waits_for_in_flight_invocation_on_the_same_server.
    let harness = TestHarness::new();
    let invoking = harness.start_blocked_invocation().await;

    let attempted = Arc::new(Semaphore::new(0));
    let deleting = {
        let service = harness.service.clone();
        let attempted = attempted.clone();
        tokio::spawn(async move {
            attempted.add_permits(1);
            McpOperations::delete_server(
                &service,
                McpDeleteServerCommand {
                    mcp_server_id: "mcp:docs".to_string(),
                },
            )
            .await
        })
    };
    attempted
        .acquire()
        .await
        .expect("delete attempted")
        .forget();
    tokio::task::yield_now().await;
    assert!(!harness.repository.events().contains(&"begin_delete"));

    harness.sessions.release_call();
    invoking.await.expect("invoke task").expect("invocation");
    assert!(
        deleting
            .await
            .expect("delete task")
            .expect("delete")
            .deleted
    );
    assert!(harness.repository.events().contains(&"begin_delete"));

    // Case: provider-policy mutation waits for an in-flight call and fences its old binding.
    let harness = TestHarness::new();
    let stale_invocation = advertised_invocation(&harness).await;
    let invoking = harness.start_blocked_invocation().await;

    let saving = {
        let service = harness.service.clone();
        tokio::spawn(async move {
            McpOperations::save_provider_policy(
                &service,
                McpSaveProviderPolicyCommand {
                    mcp_server_id: "mcp:docs".to_string(),
                    data_sharing_policy: McpDataSharingPolicy::AllowAutomatically,
                    unsafe_action_policy: McpUnsafeActionPolicy::ReviewerMayApprove,
                },
            )
            .await
        })
    };
    tokio::task::yield_now().await;
    assert!(
        !harness
            .repository
            .events()
            .contains(&"save_provider_policy"),
        "policy mutation passed an active invocation"
    );

    harness.sessions.release_call();
    invoking.await.expect("invoke task").expect("invocation");
    saving
        .await
        .expect("save task")
        .expect("save provider policy");
    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, stale_invocation).await,
        Err(CapabilityError::UnknownOperation)
    );

    // Case: policy_mutation_for_an_unrelated_server_is_not_globally_serialized.
    let harness = TestHarness::new();
    let context = harness
        .service
        .inner
        .request_context(Duration::from_secs(1));
    let _active_docs_call = harness
        .service
        .inner
        .lock_policy_read("mcp:docs", &context)
        .await
        .expect("docs policy lease");

    tokio::time::timeout(
        Duration::from_millis(50),
        harness
            .service
            .inner
            .lock_policy_write(vec!["mcp:other".to_string()], &context),
    )
    .await
    .expect("unrelated server lock should not wait")
    .expect("unrelated server policy lease");
}

#[tokio::test]
async fn transport_failure_status_and_diagnostic_contracts() {
    // Case: raw_transport_detail_is_diagnostic_only.
    let harness = TestHarness::new();
    harness.sessions.set_call_error(McpClientError::Unavailable(
        "private backend socket and credential detail".to_string(),
    ));
    let mut invocation = advertised_invocation(&harness).await;
    invocation.arguments = json!({"document": "private user payload"});
    let error = CapabilityInvoker::invoke(&harness.service, invocation)
        .await
        .expect_err("transport failure");

    assert_eq!(error, CapabilityError::Unavailable);
    assert!(!error.to_string().contains("private backend"));
    let diagnostic = harness
        .diagnostics
        .take()
        .into_iter()
        .find(|event| event.operation == "tools/call")
        .expect("diagnostic");
    assert!(diagnostic.error_chain[0].contains("private backend"));
    assert_eq!(diagnostic.raw, Some(json!({"arguments_omitted": true})));
    assert!(!format!("{diagnostic:?}").contains("private user payload"));

    // Case: typed_authentication_failure_marks_auth_required_at_every_transport_stage.
    for stage in [FailureStage::Preparation, FailureStage::ToolCall] {
        assert_failure_status(
            stage,
            McpClientError::AuthenticationRequired("credentials expired".to_string()),
            McpServerAuthStatus::NeedsAuth,
        )
        .await;
    }

    // Case: unavailable_failure_detail_never_changes_the_prior_auth_status.
    for detail in [
        "backend socket closed",
        "MCP authentication is required: credential rejected",
    ] {
        for stage in [FailureStage::Preparation, FailureStage::ToolCall] {
            assert_failure_status(
                stage,
                McpClientError::Unavailable(detail.to_string()),
                McpServerAuthStatus::None,
            )
            .await;
        }
    }
}

async fn assert_failure_status(
    stage: FailureStage,
    error: McpClientError,
    expected_auth_status: McpServerAuthStatus,
) {
    let harness = TestHarness::new();
    let authentication_required = matches!(error, McpClientError::AuthenticationRequired(_));
    match stage {
        FailureStage::Preparation => harness.sessions.set_prepare_error(error),
        FailureStage::ToolCall => harness.sessions.set_call_error(error),
    }

    let expected_error = if authentication_required {
        CapabilityError::AuthenticationRequired {
            challenge: noema_capabilities::CapabilityAuthenticationChallenge::new(
                noema_capabilities::CapabilityAuthenticationChallengeKind::Reauthenticate,
                noema_capabilities::CapabilityAuthenticationAuthorityKind::McpServer,
                "mcp:docs",
                "generation:v1",
            )
            .expect("challenge"),
        }
    } else {
        match stage {
            FailureStage::Preparation => CapabilityError::Unavailable,
            FailureStage::ToolCall => CapabilityError::Unavailable,
        }
    };
    assert_eq!(
        CapabilityInvoker::invoke(&harness.service, advertised_invocation(&harness).await).await,
        Err(expected_error)
    );
    let servers = McpOperations::list_servers(&harness.service)
        .await
        .expect("server list");
    let server = &servers.servers[0];
    assert_eq!(server.health_status, McpServerHealthStatus::Unavailable);
    assert_eq!(server.auth_status, expected_auth_status);
}

fn position(events: &[&str], expected: &str) -> usize {
    events
        .iter()
        .position(|event| *event == expected)
        .unwrap_or_else(|| panic!("missing {expected:?} in {events:?}"))
}

#[test]
fn connection_result_redacts_secret_fields_without_scanning_text() {
    let output = capability_output(crate::McpToolCallOutput {
        result: json!({
            "access_token": "private",
            "nested": {"password": "private", "description": "contains access_token text"}
        }),
        is_error: false,
    });
    assert_eq!(
        output.payload,
        json!({
            "access_token": "[REDACTED]",
            "nested": {"password": "[REDACTED]", "description": "contains access_token text"}
        })
    );
}
