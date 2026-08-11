use noema_providers::{
    GenerateInput, GenerateOptions, GenerateRequest, LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
    NoemaToolChoice, ProviderSelectionSnapshot, ProviderToolTransport,
    local_model_provider_instance_key,
};
use noema_store::WorkRunExecutionContext;
use noema_tasks::{
    AgentRunRecord, ContractOrigin, PERSONAL_DOING_STAGE_ID, PERSONAL_WORKFLOW_ID, RunKind,
    RunStatus, SubmissionCriterionEvidence, TASK_EXECUTOR_AGENT_ID, TASK_REVIEWER_AGENT_ID,
    TaskAuthorizationContext, TaskAuthorizationMessage, TaskAuthorizationMessageRole,
    TaskComplexity, TaskContractId, TaskExecutionContract, TaskExecutionPolicy,
    TaskExecutorSelection, TaskId, TaskProvenance, TaskRecord, TaskSourceKind,
    TaskSubmissionRecord, TaskValidationCriterion, WorkflowDefinition, WorkflowId, WorkflowStageId,
    personal_stages,
};
use noema_workspaces::WorkspaceId;

use crate::daemon::{
    runtime::model_tools::task_role_builtin_tool_specs,
    task_run_context::{TaskRolePrompt, build_task_role_prompt},
};

use super::{EvalCase, EvalExpectation, RuntimeEvalRole};
use crate::eval_support::types::ExecutorScenario;

const MODEL_ID: &str = "__MODEL_ID__";

pub(super) fn task_cases(model_id: &str) -> Result<Vec<EvalCase>, String> {
    let criteria = vec![
        TaskValidationCriterion {
            criterion_id: "criterion:alpha".to_string(),
            ordinal: 1,
            description: "The answer states that the launch code is ORBIT-52.".to_string(),
            expected_evidence: Some("Quote the launch code from the request.".to_string()),
        },
        TaskValidationCriterion {
            criterion_id: "criterion:beta".to_string(),
            ordinal: 2,
            description: "The result uses Markdown bold around ORBIT-52.".to_string(),
            expected_evidence: Some("The result must contain **ORBIT-52**.".to_string()),
        },
    ];
    let planner_context = fixture_work_context(
        "Find hikes near Vancouver, BC",
        "Find some good hikes near Vancouver, BC and report the recommendations.",
        Vec::new(),
        TaskComplexity::Medium,
        RunKind::Planner,
    );
    let planner_request = terminal_tool_request(model_id, &planner_context)?;
    let executor_context = fixture_work_context(
        "Recommend a fictional hike",
        "Recommend one good easy hike from these supplied fictional options: Cedar Loop is 4 km and easy; Alpine Pond is 7 km and moderate; Lookout Ridge is 12 km and hard. Give one primary recommendation and at most two concise alternatives, end with `Primary recommendation: <option name>`, and do not provide an itinerary.",
        vec![TaskValidationCriterion {
            criterion_id: "criterion:recommendation".to_string(),
            ordinal: 1,
            description: "The result gives a concise recommendation from the supplied options."
                .to_string(),
            expected_evidence: None,
        }],
        TaskComplexity::Simple,
        RunKind::Executor,
    );
    let medium_executor_context = fixture_work_context(
        "Compare fictional hikes",
        "Compare these supplied fictional options and recommend the best fit for a moderate outing: Cedar Loop is 4 km and easy; Alpine Pond is 7 km and moderate; Lookout Ridge is 12 km and hard. Explain the tradeoff briefly, do not provide an itinerary, and end with `Primary recommendation: <option name>`.",
        vec![
            TaskValidationCriterion {
                criterion_id: "criterion:comparison".to_string(),
                ordinal: 1,
                description: "The result compares the supplied options by distance and difficulty."
                    .to_string(),
                expected_evidence: None,
            },
            TaskValidationCriterion {
                criterion_id: "criterion:recommendation".to_string(),
                ordinal: 2,
                description: "The result recommends one supplied option.".to_string(),
                expected_evidence: None,
            },
        ],
        TaskComplexity::Medium,
        RunKind::Executor,
    );
    let difficult_executor_context = fixture_work_context(
        "Rank fictional hikes under constraints",
        "Rank the supplied fictional options from best to worst for a moderate outing, choose one primary recommendation, and explain the distance-versus-difficulty tradeoff: Cedar Loop is 4 km and easy; Alpine Pond is 7 km and moderate; Lookout Ridge is 12 km and hard. Use numbered lines beginning `1.`, `2.`, and `3.`, end with `Primary recommendation: <option name>`, and do not provide an itinerary.",
        vec![
            TaskValidationCriterion {
                criterion_id: "criterion:ranking".to_string(),
                ordinal: 1,
                description: "The result ranks all three supplied options.".to_string(),
                expected_evidence: None,
            },
            TaskValidationCriterion {
                criterion_id: "criterion:tradeoff".to_string(),
                ordinal: 2,
                description: "The result explains the distance-versus-difficulty tradeoff."
                    .to_string(),
                expected_evidence: None,
            },
            TaskValidationCriterion {
                criterion_id: "criterion:recommendation".to_string(),
                ordinal: 3,
                description: "The result chooses one primary recommendation.".to_string(),
                expected_evidence: None,
            },
        ],
        TaskComplexity::Difficult,
        RunKind::Executor,
    );
    let executor_request = terminal_tool_request(model_id, &executor_context)?;
    let blocked_criteria = vec![TaskValidationCriterion {
        criterion_id: "criterion:region".to_string(),
        ordinal: 1,
        description: "The deployment command targets the exact region supplied by the user."
            .to_string(),
        expected_evidence: Some("Quote the user-supplied region.".to_string()),
    }];
    let blocked_context = fixture_work_context(
        "Prepare the regional deployment",
        "Prepare a deployment command for the user's required region. The user has not supplied a region, and no default is authorized. Ask one blocking question rather than inventing a region.",
        blocked_criteria,
        TaskComplexity::Simple,
        RunKind::Executor,
    );
    let blocked_request = terminal_tool_request(model_id, &blocked_context)?;
    let reviewer_context = fixture_work_context(
        "Return the launch code",
        "State that the launch code is **ORBIT-52** using that exact Markdown bold syntax. All required information is present; do not ask a question.",
        criteria,
        TaskComplexity::Simple,
        RunKind::Reviewer,
    );
    let reviewer_request = terminal_tool_request(model_id, &reviewer_context)?;
    let mut contradictory_context = reviewer_context;
    let contradictory = contradictory_context
        .latest_submission
        .as_mut()
        .expect("reviewer submission");
    contradictory.result_markdown =
        "The launch code is **ORBIT-52**. The launch code is also NOVA-11.".to_string();
    contradictory.criteria[0].evidence_markdown =
        "The result makes two incompatible claims about the only launch code.".to_string();
    let contradictory_request = terminal_tool_request(model_id, &contradictory_context)?;

    Ok(vec![
        EvalCase {
            id: "task_planner_simple_contract",
            role: RuntimeEvalRole::TaskSimple,
            category: "tasks",
            critical: true,
            request: planner_request.clone(),
            expectation: EvalExpectation::SimplePlannerPlan,
        },
        EvalCase {
            id: "task_planner_contract",
            role: RuntimeEvalRole::TaskMedium,
            category: "tasks",
            critical: true,
            request: planner_request.clone(),
            expectation: EvalExpectation::SimplePlannerPlan,
        },
        EvalCase {
            id: "task_planner_difficult_contract",
            role: RuntimeEvalRole::TaskDifficult,
            category: "tasks",
            critical: true,
            request: planner_request,
            expectation: EvalExpectation::SimplePlannerPlan,
        },
        EvalCase {
            id: "task_executor_submission",
            role: RuntimeEvalRole::TaskSimple,
            category: "tasks",
            critical: true,
            request: executor_request,
            expectation: EvalExpectation::ExecutorSubmission(
                ExecutorScenario::SimpleRecommendation,
            ),
        },
        EvalCase {
            id: "task_reviewer_approval",
            role: RuntimeEvalRole::TaskReviewer,
            category: "tasks",
            critical: true,
            request: reviewer_request,
            expectation: EvalExpectation::ReviewerApproval,
        },
        EvalCase {
            id: "task_reviewer_internal_contradiction",
            role: RuntimeEvalRole::TaskReviewer,
            category: "tasks",
            critical: true,
            request: contradictory_request,
            expectation: EvalExpectation::ReviewerRequestChanges,
        },
        EvalCase {
            id: "task_executor_blocked",
            role: RuntimeEvalRole::TaskSimple,
            category: "tasks",
            critical: true,
            request: blocked_request,
            expectation: EvalExpectation::BlockedTask,
        },
        EvalCase {
            id: "task_executor_medium_submission",
            role: RuntimeEvalRole::TaskMedium,
            category: "tasks",
            critical: true,
            request: terminal_tool_request(model_id, &medium_executor_context)?,
            expectation: EvalExpectation::ExecutorSubmission(ExecutorScenario::MediumComparison),
        },
        EvalCase {
            id: "task_executor_difficult_submission",
            role: RuntimeEvalRole::TaskDifficult,
            category: "tasks",
            critical: true,
            request: terminal_tool_request(model_id, &difficult_executor_context)?,
            expectation: EvalExpectation::ExecutorSubmission(ExecutorScenario::DifficultRanking),
        },
    ])
}

fn terminal_tool_request(
    model_id: &str,
    context: &WorkRunExecutionContext,
) -> Result<GenerateRequest, String> {
    let TaskRolePrompt {
        role,
        input,
        instructions,
        terminal_contract,
    } = build_task_role_prompt(context);
    let tools = task_role_builtin_tool_specs(role, &terminal_contract)
        .map_err(|error| error.to_string())?;
    Ok(GenerateRequest {
        conversation_id: None,
        model: Some(model_id.to_string()),
        input: GenerateInput::Text(input),
        instructions: Some(instructions.to_string()),
        options: GenerateOptions {
            max_output_tokens: Some(768),
            temperature: Some(0.0),
            ..GenerateOptions::default()
        },
        tools: tools.into_iter().map(Into::into).collect(),
        tool_transport: ProviderToolTransport::Native,
        tool_choice: NoemaToolChoice::Required,
        parallel_tool_calls: false,
    })
}

fn fixture_model() -> ProviderSelectionSnapshot {
    let mut model = ProviderSelectionSnapshot::explicit(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        MODEL_ID,
        None,
        Some("evaluation".to_string()),
    );
    model.provider_instance_key = Some(
        local_model_provider_instance_key(
            LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            "local_model_installation:evaluation",
            MODEL_ID,
        )
        .expect("fixture local model provider key"),
    );
    model
}

fn fixture_work_context(
    title: &str,
    request_markdown: &str,
    criteria: Vec<TaskValidationCriterion>,
    complexity: TaskComplexity,
    run_kind: RunKind,
) -> WorkRunExecutionContext {
    let task_id = TaskId::new("task:evaluation").expect("fixture task id");
    let workspace_id = WorkspaceId::new("workspace:personal").expect("fixture workspace id");
    let workflow_id = WorkflowId::new(PERSONAL_WORKFLOW_ID).expect("fixture workflow id");
    let stage_id =
        WorkflowStageId::new(PERSONAL_DOING_STAGE_ID).expect("fixture workflow stage id");
    let contract_id = TaskContractId::new("contract:evaluation").expect("fixture contract id");
    let execution_policy = TaskExecutionPolicy::default();
    let model = fixture_model();
    let workspace = noema_tasks::WorkspaceContextSnapshot {
        workspace_id: workspace_id.clone(),
        name: "Personal".to_string(),
        description: "The user's personal workspace.".to_string(),
    };
    let contract = (run_kind != RunKind::Planner).then(|| TaskExecutionContract {
        contract_id: contract_id.clone(),
        task_id: task_id.clone(),
        version: 1,
        task_generation: 1,
        supersedes_contract_id: None,
        origin: ContractOrigin::Delegated,
        request_markdown: request_markdown.to_string(),
        execution_plan_markdown: Some(
            "Produce the requested result and attach evidence for every criterion.".to_string(),
        ),
        criteria: criteria.clone(),
        complexity,
        executor_model: model.clone(),
        executor: TaskExecutorSelection::provider(),
        effective_cwd: None,
        reviewer_model: model.clone(),
        execution_policy,
        workspace_context: workspace.clone(),
        project_context: None,
        created_by_actor_id: "actor:agent:primary".to_string(),
        created_at: "2026-07-15T00:00:00Z".to_string(),
    });
    let latest_submission = (run_kind == RunKind::Reviewer)
        .then(|| fixture_submission(&task_id, &contract_id, &criteria));
    let latest_submission_id = latest_submission
        .as_ref()
        .map(|submission| submission.submission_id.clone());
    let (run_id, agent_id, parent_run_id) = match run_kind {
        RunKind::Planner => (
            "run:evaluation:planner".to_string(),
            TASK_EXECUTOR_AGENT_ID.to_string(),
            None,
        ),
        RunKind::Executor => (
            "run:evaluation:executor".to_string(),
            TASK_EXECUTOR_AGENT_ID.to_string(),
            None,
        ),
        RunKind::Reviewer => (
            "run:evaluation:reviewer".to_string(),
            TASK_REVIEWER_AGENT_ID.to_string(),
            Some("run:evaluation:executor".to_string()),
        ),
    };
    let task = TaskRecord {
        task_id: task_id.clone(),
        workspace_id: workspace_id.clone(),
        project_id: None,
        workflow_id: workflow_id.clone(),
        stage_id: stage_id.clone(),
        title: title.to_string(),
        description_markdown: request_markdown.to_string(),
        executor_agent_id: TASK_EXECUTOR_AGENT_ID.to_string(),
        cwd_override: None,
        authorization_context: TaskAuthorizationContext::ConversationExcerpt {
            messages: vec![TaskAuthorizationMessage {
                item_id: "item:evaluation:source".to_string(),
                role: TaskAuthorizationMessageRole::Human,
                text: title.to_string(),
            }],
        },
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatDelegate,
            item_id: Some("item:evaluation:source".to_string()),
            created_by_actor_id: "actor:agent:primary".to_string(),
            ..TaskProvenance::default()
        },
        generation: 1,
        revision: 1,
        current_contract_id: contract
            .as_ref()
            .map(|contract| contract.contract_id.clone()),
        active_gate_id: None,
        latest_run_id: Some(run_id.clone()),
        latest_submission_id: latest_submission_id.clone(),
        latest_review_id: None,
        completed_submission_id: None,
        scheduled_for: None,
        schedule_time_zone: None,
        missed_run_policy: None,
        recurrence_id: None,
        recurrence_revision: None,
        recurrence_scheduled_for: None,
        queued_at: Some("2026-07-15T00:00:00Z".to_string()),
        created_at: "2026-07-15T00:00:00Z".to_string(),
        updated_at: "2026-07-15T00:00:01Z".to_string(),
        completed_at: None,
        cancelled_at: None,
    };
    let run = AgentRunRecord {
        run_id,
        instance_name: format!("Evaluation {}", run_kind.as_str()),
        task_id,
        task_generation: 1,
        contract_id: contract
            .as_ref()
            .map(|contract| contract.contract_id.clone()),
        run_kind,
        agent_id,
        attempt_index: 0,
        review_round: match run_kind {
            RunKind::Planner => 0,
            RunKind::Executor | RunKind::Reviewer => 1,
        },
        parent_run_id,
        triggering_submission_id: if run_kind == RunKind::Reviewer {
            latest_submission_id
        } else {
            None
        },
        triggering_review_id: None,
        model,
        executor: TaskExecutorSelection::provider(),
        effective_cwd: None,
        acp_session_id: None,
        actual_provider_kind: Some("local_models".to_string()),
        actual_model_profile: Some(MODEL_ID.to_string()),
        execution_policy,
        status: RunStatus::Running,
        queued_at: "2026-07-15T00:00:00Z".to_string(),
        lease_owner: Some("runtime:evaluation".to_string()),
        lease_token: Some("lease:evaluation".to_string()),
        lease_expires_at: Some("2026-07-15T00:05:00Z".to_string()),
        heartbeat_at: Some("2026-07-15T00:00:01Z".to_string()),
        started_at: Some("2026-07-15T00:00:01Z".to_string()),
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
        created_at: "2026-07-15T00:00:00Z".to_string(),
        updated_at: "2026-07-15T00:00:01Z".to_string(),
    };
    let workflow = WorkflowDefinition {
        workflow_id,
        workspace_id,
        name: "Personal".to_string(),
        revision: 1,
        is_default: true,
        created_at: "2026-07-15T00:00:00Z".to_string(),
        updated_at: "2026-07-15T00:00:00Z".to_string(),
    };
    let stage = personal_stages()
        .into_iter()
        .find(|stage| stage.stage_id == stage_id)
        .expect("fixture workflow stage");

    task.validate().expect("valid fixture task");
    run.validate_contract_lineage()
        .expect("valid fixture run lineage");
    if let Some(contract) = contract.as_ref() {
        contract
            .clone()
            .normalized()
            .expect("valid fixture execution contract");
    }
    workflow.validate().expect("valid fixture workflow");
    stage.validate().expect("valid fixture workflow stage");

    WorkRunExecutionContext {
        run,
        task,
        workflow,
        stage,
        contract,
        workspace,
        project: None,
        active_gate: None,
        relevant_gates: Vec::new(),
        messages: Vec::new(),
        latest_submission,
        latest_review: None,
        lineage: Vec::new(),
    }
}

fn fixture_submission(
    task_id: &TaskId,
    contract_id: &TaskContractId,
    criteria: &[TaskValidationCriterion],
) -> TaskSubmissionRecord {
    TaskSubmissionRecord {
        submission_id: "submission:evaluation".to_string(),
        task_id: task_id.clone(),
        contract_id: contract_id.clone(),
        executor_run_id: "run:evaluation:executor".to_string(),
        review_round: 1,
        summary: "Launch code supplied.".to_string(),
        result_markdown: "The launch code is **ORBIT-52**.".to_string(),
        citations: Vec::new(),
        criteria: criteria
            .iter()
            .map(|criterion| SubmissionCriterionEvidence {
                criterion_id: criterion.criterion_id.clone(),
                evidence_markdown: format!(
                    "The submitted result contains **ORBIT-52** and addresses: {}",
                    criterion.description
                ),
            })
            .collect(),
        artifacts: Vec::new(),
        created_at: "2026-07-15T00:00:00Z".to_string(),
    }
}
