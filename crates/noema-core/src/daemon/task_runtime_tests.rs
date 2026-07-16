use super::*;
use noema_providers::{
    ProviderInstanceKey, ProviderSelectionMode, ProviderSelectionSnapshot, ReasoningEffort,
};

#[test]
fn background_generation_request_preserves_complete_provider_selection() {
    let provider_selection = ProviderSelectionSnapshot {
        provider_kind: "openai".to_string(),
        provider_account_id: "provider_account:openai:task-owner".to_string(),
        provider_instance_key: Some(
            ProviderInstanceKey::new("provider_instance:openai:task-owner:generation-7")
                .expect("valid provider instance key"),
        ),
        selection_mode: ProviderSelectionMode::ExplicitProfile,
        model_profile: Some("gpt-5.5-task".to_string()),
        reasoning_effort: Some(ReasoningEffort::High),
        selection_source: Some("task:model_pool:complex".to_string()),
    };
    let run = agent_run(provider_selection.clone());

    let request = background_task_generate_request(
        &run,
        "lease:test",
        &CancellationToken::new(),
        "Do the work.".to_string(),
        "Follow the task contract.",
        &ConversationSubscriptionRegistry::default(),
    );

    assert_eq!(
        request.provider_selection.provider_account_id,
        "provider_account:openai:task-owner"
    );
    assert_eq!(request.provider_selection, provider_selection);
}

fn agent_run(model: ProviderSelectionSnapshot) -> crate::AgentRunRecord {
    crate::AgentRunRecord {
        run_id: "run:test".to_string(),
        task_id: "task:test".to_string(),
        run_kind: RunKind::Reviewer,
        agent_id: "agent:reviewer".to_string(),
        revision_index: 2,
        attempt_index: 1,
        parent_run_id: Some("run:parent".to_string()),
        triggering_submission_id: Some("submission:test".to_string()),
        triggering_review_id: None,
        resume_message: None,
        model,
        actual_provider_kind: None,
        actual_model_profile: None,
        execution_policy: crate::TaskExecutionPolicy::default(),
        status: crate::RunStatus::Running,
        priority: 10,
        queued_at: "2026-07-16T00:00:00Z".to_string(),
        lease_owner: Some("task-worker:test".to_string()),
        lease_token: Some("lease:test".to_string()),
        lease_expires_at: Some("2026-07-16T00:02:00Z".to_string()),
        heartbeat_at: Some("2026-07-16T00:00:30Z".to_string()),
        started_at: Some("2026-07-16T00:00:01Z".to_string()),
        ended_at: None,
        cancellation_requested: false,
        retry_count: 0,
        error_code: None,
        error_message: None,
        provider_call_count: 0,
        tool_call_count: 0,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        active_milliseconds: 0,
        created_at: "2026-07-16T00:00:00Z".to_string(),
        updated_at: "2026-07-16T00:00:01Z".to_string(),
    }
}
