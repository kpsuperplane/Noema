use noema_providers::ProviderSelectionSnapshot;
use noema_store::WorkRunExecutionContext;
use noema_tasks::{
    AgentRunRecord, ApprovalDecision, ContractOrigin, RunKind, RunStatus,
    SubmissionCriterionEvidence, TASK_EXECUTOR_AGENT_ID, TASK_REVIEWER_AGENT_ID, TaskComplexity,
    TaskContractId, TaskExecutionContract, TaskExecutionPolicy, TaskGateId, TaskGateKind,
    TaskGateRecord, TaskGateState, TaskId, TaskMessageId, TaskMessageKind, TaskMessageRecord,
    TaskProvenance, TaskRecord, TaskSourceKind, TaskSubmissionRecord, TaskValidationCriterion,
    WorkflowDefinition, WorkflowId, WorkflowStageId, personal_stages,
};
use noema_workspaces::WorkspaceId;

use super::build_task_role_prompt;

#[test]
fn continuation_gate_context_is_rendered_exactly_once_for_every_role() {
    for role in [RunKind::Planner, RunKind::Executor, RunKind::Reviewer] {
        let prompt = build_task_role_prompt(&fixture_context(role))
            .expect("role prompt")
            .input;
        for marker in [
            "QUESTION-UNIQUE-17",
            "CONTEXT-UNIQUE-29",
            "ANSWER-UNIQUE-41",
            "Structured decision: approved",
        ] {
            assert_eq!(
                prompt.matches(marker).count(),
                1,
                "{role} rendered {marker:?} more than once"
            );
        }
    }
}

fn fixture_context(run_kind: RunKind) -> WorkRunExecutionContext {
    let task_id = TaskId::new("task:prompt-fixture").expect("task id");
    let contract_id = TaskContractId::new("contract:prompt-fixture").expect("contract id");
    let workspace_id = WorkspaceId::new("workspace:personal").expect("workspace id");
    let workflow_id = WorkflowId::new(noema_tasks::PERSONAL_WORKFLOW_ID).expect("workflow id");
    let stage_id = WorkflowStageId::new(noema_tasks::PERSONAL_DOING_STAGE_ID).expect("stage id");
    let model = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "test-model",
        None,
        Some("prompt_fixture".to_string()),
    );
    let policy = TaskExecutionPolicy::default();
    let criterion = TaskValidationCriterion {
        criterion_id: "criterion:prompt-fixture".to_string(),
        ordinal: 1,
        description: "Produce a complete answer.".to_string(),
        expected_evidence: None,
    };
    let workspace = noema_tasks::WorkspaceContextSnapshot {
        workspace_id: workspace_id.clone(),
        name: "Personal".to_string(),
        description: "Personal workspace".to_string(),
    };
    let contract = (run_kind != RunKind::Planner).then(|| TaskExecutionContract {
        contract_id: contract_id.clone(),
        task_id: task_id.clone(),
        version: 1,
        task_generation: 1,
        supersedes_contract_id: None,
        origin: ContractOrigin::Delegated,
        request_markdown: "Complete the fixture task.".to_string(),
        execution_plan_markdown: Some("Produce the result.".to_string()),
        criteria: vec![criterion.clone()],
        complexity: TaskComplexity::Simple,
        executor_model: model.clone(),
        reviewer_model: model.clone(),
        execution_policy: policy,
        workspace_context: workspace.clone(),
        project_context: None,
        created_by_actor_id: "actor:test".to_string(),
        created_at: "2026-07-18T00:00:00Z".to_string(),
    });
    let run_id = format!("run:prompt-fixture:{}", run_kind.as_str());
    let agent_id = match run_kind {
        RunKind::Planner => TASK_EXECUTOR_AGENT_ID,
        RunKind::Executor => TASK_EXECUTOR_AGENT_ID,
        RunKind::Reviewer => TASK_REVIEWER_AGENT_ID,
    };
    let submission = (run_kind == RunKind::Reviewer).then(|| TaskSubmissionRecord {
        submission_id: "submission:prompt-fixture".to_string(),
        task_id: task_id.clone(),
        contract_id: contract_id.clone(),
        executor_run_id: "run:prompt-fixture:executor".to_string(),
        review_round: 1,
        summary: "Fixture summary".to_string(),
        result_markdown: "Fixture result".to_string(),
        criteria: vec![SubmissionCriterionEvidence {
            criterion_id: criterion.criterion_id.clone(),
            evidence_markdown: "Fixture evidence".to_string(),
        }],
        artifacts: Vec::new(),
        created_at: "2026-07-18T00:00:01Z".to_string(),
    });
    let task = TaskRecord {
        task_id: task_id.clone(),
        workspace_id: workspace_id.clone(),
        project_id: None,
        workflow_id: workflow_id.clone(),
        stage_id: stage_id.clone(),
        title: "Prompt fixture".to_string(),
        description_markdown: "Complete the fixture task.".to_string(),
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::System,
            created_by_actor_id: "actor:test".to_string(),
            ..TaskProvenance::default()
        },
        generation: 1,
        revision: 1,
        current_contract_id: contract.as_ref().map(|value| value.contract_id.clone()),
        active_gate_id: None,
        latest_run_id: Some(run_id.clone()),
        latest_submission_id: submission.as_ref().map(|value| value.submission_id.clone()),
        latest_review_id: None,
        accepted_submission_id: None,
        queued_at: Some("2026-07-18T00:00:00Z".to_string()),
        created_at: "2026-07-18T00:00:00Z".to_string(),
        updated_at: "2026-07-18T00:00:01Z".to_string(),
        completed_at: None,
        cancelled_at: None,
    };
    let run = AgentRunRecord {
        run_id: run_id.clone(),
        task_id: task_id.clone(),
        task_generation: 1,
        contract_id: contract.as_ref().map(|value| value.contract_id.clone()),
        run_kind,
        agent_id: agent_id.to_string(),
        attempt_index: 0,
        review_round: 1,
        parent_run_id: (run_kind == RunKind::Reviewer)
            .then(|| "run:prompt-fixture:executor".to_string()),
        triggering_submission_id: submission.as_ref().map(|value| value.submission_id.clone()),
        triggering_review_id: None,
        model,
        actual_provider_kind: None,
        actual_model_profile: None,
        execution_policy: policy,
        status: RunStatus::Running,
        queued_at: "2026-07-18T00:00:00Z".to_string(),
        lease_owner: Some("runtime:test".to_string()),
        lease_token: Some("lease:test".to_string()),
        lease_expires_at: Some("2026-07-18T00:05:00Z".to_string()),
        heartbeat_at: Some("2026-07-18T00:00:01Z".to_string()),
        started_at: Some("2026-07-18T00:00:01Z".to_string()),
        ended_at: None,
        cancellation_requested: false,
        error_code: None,
        error_message: None,
        provider_call_count: 0,
        tool_call_count: 0,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        active_milliseconds: 0,
        created_at: "2026-07-18T00:00:00Z".to_string(),
        updated_at: "2026-07-18T00:00:01Z".to_string(),
    };
    let gate_id = TaskGateId::new("gate:prompt-fixture").expect("gate id");
    let message_id = TaskMessageId::new("task_message:prompt-fixture").expect("message id");
    let gate = TaskGateRecord {
        gate_id: gate_id.clone(),
        task_id: task_id.clone(),
        task_generation: 1,
        contract_id: contract.as_ref().map(|value| value.contract_id.clone()),
        kind: TaskGateKind::Approval,
        state: TaskGateState::Resolved,
        recovery_reason: None,
        retry_run_kind: None,
        prompt_markdown: "QUESTION-UNIQUE-17".to_string(),
        context_markdown: "CONTEXT-UNIQUE-29".to_string(),
        opened_by_actor_id: "actor:test".to_string(),
        originating_run_id: Some("run:prior".to_string()),
        resolved_by_actor_id: Some("actor:human:local".to_string()),
        resolution_message_id: Some(message_id.clone()),
        opened_at: "2026-07-18T00:00:00Z".to_string(),
        resolved_at: Some("2026-07-18T00:00:01Z".to_string()),
    };
    let message = TaskMessageRecord {
        message_id,
        task_id,
        task_generation: 1,
        contract_id: contract.as_ref().map(|value| value.contract_id.clone()),
        gate_id: Some(gate_id),
        review_id: None,
        kind: TaskMessageKind::HumanAnswer,
        body_markdown: "ANSWER-UNIQUE-41".to_string(),
        approval_decision: Some(ApprovalDecision::Approved),
        author_actor_id: "actor:human:local".to_string(),
        consumed_by_run_id: Some(run_id),
        consumed_at: Some("2026-07-18T00:00:02Z".to_string()),
        created_at: "2026-07-18T00:00:01Z".to_string(),
    };
    let workflow = WorkflowDefinition {
        workflow_id,
        workspace_id,
        name: "Personal".to_string(),
        revision: 1,
        is_default: true,
        created_at: "2026-07-18T00:00:00Z".to_string(),
        updated_at: "2026-07-18T00:00:00Z".to_string(),
    };
    let stage = personal_stages()
        .into_iter()
        .find(|stage| stage.stage_id == stage_id)
        .expect("stage");
    WorkRunExecutionContext {
        run,
        task,
        workflow,
        stage,
        contract,
        workspace,
        project: None,
        active_gate: None,
        relevant_gates: vec![gate],
        messages: vec![message],
        latest_submission: submission,
        latest_review: None,
        lineage: Vec::new(),
    }
}
