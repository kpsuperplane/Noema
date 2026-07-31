//! One fenced Planner, Executor, or Reviewer run.

use noema_store::{
    CompletePlan, PlanTerminal, ReportTaskBlocked, SubmitPlan, SubmitTaskResult, SubmitTaskReview,
    WorkCommandService, WorkRunExecutionContext, WorkRunFence, WorkRunTerminal,
};
use noema_tasks::{
    CriterionOutcome, NewTaskReview, NewTaskSubmission, NewTaskValidationCriterion, RunKind,
    TaskReviewCriterion, TaskReviewVerdict,
};
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

use crate::{
    daemon::runtime::BackgroundTaskGenerateRequest,
    daemon::task_run_context::{
        ExecutorBlockedResponse, ExecutorSubmissionResponse, PlannerBlockedResponse,
        PlannerPlanResponse, ReviewerDecisionResponse, ReviewerResponse, TaskRolePrompt,
        build_task_role_prompt,
    },
    daemon::{
        RuntimeError, RuntimeEventRegistry, RuntimeHandle, TaskRuntimeEvent, WorkRuntimeEvent,
    },
};

use super::TaskRuntimeServices;

pub(super) async fn execute_run(
    services: &TaskRuntimeServices,
    run: &noema_tasks::AgentRunRecord,
    fence: &WorkRunFence,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeError> {
    let command_service =
        WorkCommandService::new(services.store.clone(), services.provider_registry.clone());
    let correlation_id = format!("correlation:run:{}", run.run_id);
    command_service
        .start_work_run(
            fence,
            super::WORK_RUNTIME_ACTOR_ID,
            Some(run.run_id.as_str()),
            &correlation_id,
        )
        .await?;
    let admission = command_service
        .admit_work_run_execution_context(
            fence,
            super::WORK_RUNTIME_ACTOR_ID,
            Some(run.run_id.as_str()),
            &correlation_id,
        )
        .await?;
    let context = admission.context;
    let prompt = build_task_role_prompt(&context);
    let response = generate_once(
        &services.runtime,
        run,
        fence,
        cancellation,
        prompt,
        &services.subscriptions,
    )
    .await?;
    let terminal = parse_terminal(run, &context, response.tool_calls.as_slice(), fence.clone())
        .map_err(RuntimeError::Protocol)?;
    command_service
        .record_work_run_terminal(
            terminal,
            super::WORK_RUNTIME_ACTOR_ID,
            Some(run.run_id.as_str()),
            &correlation_id,
        )
        .await?;
    publish_committed(&services.subscriptions, &context);
    Ok(())
}

fn parse_terminal(
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    calls: &[noema_providers::GenerateToolCall],
    fence: WorkRunFence,
) -> Result<WorkRunTerminal, String> {
    if calls.len() != 1 {
        return Err("Work role must return exactly one terminal tool call".to_string());
    }
    let call = &calls[0];
    match run.run_kind {
        RunKind::Planner => {
            if call.name == "task.submit_plan" {
                let plan: PlannerPlanResponse = parse_payload(call, "Planner terminal")?;
                let criteria = plan
                    .criteria
                    .into_iter()
                    .enumerate()
                    .map(|(index, criterion)| {
                        Ok(NewTaskValidationCriterion {
                            criterion_id: None,
                            ordinal: criterion_ordinal(index)?,
                            description: criterion.description,
                            expected_evidence: criterion.expected_evidence,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(WorkRunTerminal::Plan(SubmitPlan {
                    fence,
                    terminal: PlanTerminal::Complete(CompletePlan {
                        request_markdown: plan.request_markdown,
                        execution_plan_markdown: plan.execution_plan_markdown,
                        criteria,
                        complexity: plan.complexity,
                    }),
                }))
            } else if call.name == "task.report_blocked" {
                let blocked: PlannerBlockedResponse = parse_payload(call, "Planner gate")?;
                Ok(WorkRunTerminal::Plan(SubmitPlan {
                    fence,
                    terminal: PlanTerminal::BlockingQuestion {
                        prompt_markdown: blocked.question,
                        context_markdown: blocked.context_markdown,
                        suggested_answers: normalize_suggested_answers(blocked.suggested_answers),
                        gate_kind: blocked.gate_kind,
                    },
                }))
            } else {
                Err("Planner returned a role-inappropriate terminal tool".to_string())
            }
        }
        RunKind::Executor => execute_executor(run, context, call, fence),
        RunKind::Reviewer => execute_reviewer(run, context, call, fence),
    }
}

fn execute_executor(
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    call: &noema_providers::GenerateToolCall,
    fence: WorkRunFence,
) -> Result<WorkRunTerminal, String> {
    if call.name == "task.submit_result" {
        let result: ExecutorSubmissionResponse = parse_payload(call, "Executor terminal")?;
        let contract = context
            .contract
            .as_ref()
            .ok_or_else(|| "Executor terminal has no contract".to_string())?;
        validate_unique_artifact_ids(&result.artifact_ids)?;
        let submission = NewTaskSubmission {
            submission_id: None,
            task_id: context.task.task_id.clone(),
            contract_id: contract.contract_id.clone(),
            executor_run_id: run.run_id.clone(),
            review_round: run.review_round,
            summary: result.summary,
            result_markdown: result.result_markdown,
            criteria: result
                .criteria
                .into_iter()
                .map(|criterion| noema_tasks::SubmissionCriterionEvidence {
                    criterion_id: criterion.criterion_id,
                    evidence_markdown: criterion.evidence_markdown,
                })
                .collect(),
            artifact_ids: result.artifact_ids,
        };
        Ok(WorkRunTerminal::TaskResult(SubmitTaskResult {
            fence,
            submission,
        }))
    } else if call.name == "task.report_blocked" {
        let blocked: ExecutorBlockedResponse = parse_payload(call, "Executor gate")?;
        Ok(WorkRunTerminal::Blocked(ReportTaskBlocked {
            fence,
            gate_kind: blocked.gate_kind,
            prompt_markdown: blocked.question,
            context_markdown: blocked.context_markdown,
            suggested_answers: normalize_suggested_answers(blocked.suggested_answers),
        }))
    } else {
        Err("Executor returned a role-inappropriate terminal tool".to_string())
    }
}

fn execute_reviewer(
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    call: &noema_providers::GenerateToolCall,
    fence: WorkRunFence,
) -> Result<WorkRunTerminal, String> {
    if call.name != "task.submit_review" {
        return Err("Reviewer returned a role-inappropriate terminal tool".to_string());
    }
    let parsed: ReviewerResponse = parse_payload(call, "Reviewer terminal")?;
    let (verdict, human_gate_kind, human_question) = match parsed.decision {
        ReviewerDecisionResponse::Approve {} => (TaskReviewVerdict::Approve, None, None),
        ReviewerDecisionResponse::RequestChanges {} => {
            (TaskReviewVerdict::RequestChanges, None, None)
        }
        ReviewerDecisionResponse::NeedsHuman {
            human_gate_kind,
            human_question,
        } => (
            TaskReviewVerdict::NeedsHuman,
            Some(human_gate_kind),
            Some(human_question),
        ),
    };
    let criteria = parsed
        .criteria
        .into_iter()
        .map(|criterion| {
            Ok(TaskReviewCriterion {
                criterion_id: criterion.criterion_id,
                outcome: criterion
                    .outcome
                    .parse::<CriterionOutcome>()
                    .map_err(|error| format!("invalid criterion outcome: {error}"))?,
                evidence_markdown: criterion.evidence_markdown,
                feedback: criterion.feedback,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let contract = context
        .contract
        .as_ref()
        .ok_or_else(|| "Reviewer terminal has no contract".to_string())?;
    let submission = context
        .latest_submission
        .as_ref()
        .ok_or_else(|| "Reviewer terminal has no submission".to_string())?;
    let prior_review = context
        .latest_review
        .as_ref()
        .filter(|review| review.reviewed_submission_id == submission.submission_id);
    let human_question = human_question
        .as_deref()
        .map(str::trim)
        .filter(|question| !question.is_empty());
    if verdict == TaskReviewVerdict::NeedsHuman && human_question.is_none() {
        return Err("needs_human review requires a human question".to_string());
    }
    let review = NewTaskReview {
        review_id: None,
        task_id: context.task.task_id.clone(),
        contract_id: contract.contract_id.clone(),
        reviewer_run_id: run.run_id.clone(),
        reviewed_submission_id: submission.submission_id.clone(),
        review_attempt_index: prior_review
            .map_or(1, |review| review.review_attempt_index.saturating_add(1)),
        supersedes_review_id: prior_review.map(|review| review.review_id.clone()),
        overall_verdict: verdict,
        human_gate_kind,
        overall_feedback: human_question.map_or_else(
            || parsed.overall_feedback.clone(),
            |question| {
                format!(
                    "{question}\n\nReviewer context:\n{}",
                    parsed.overall_feedback
                )
            },
        ),
        criteria,
    };
    Ok(WorkRunTerminal::Review(SubmitTaskReview { fence, review }))
}

async fn generate_once(
    runtime: &RuntimeHandle,
    run: &noema_tasks::AgentRunRecord,
    fence: &WorkRunFence,
    cancellation: &CancellationToken,
    prompt: TaskRolePrompt,
    subscriptions: &RuntimeEventRegistry,
) -> Result<noema_providers::GenerateResponse, RuntimeError> {
    let request = background_task_generate_request(run, fence, cancellation, prompt, subscriptions);
    runtime.generate_background_task(request).await
}

fn background_task_generate_request(
    run: &noema_tasks::AgentRunRecord,
    fence: &WorkRunFence,
    cancellation: &CancellationToken,
    prompt: TaskRolePrompt,
    subscriptions: &RuntimeEventRegistry,
) -> BackgroundTaskGenerateRequest {
    BackgroundTaskGenerateRequest {
        run_id: run.run_id.clone(),
        task_id: run.task_id.to_string(),
        lease_token: fence.lease_token.clone(),
        task_generation: fence.task_generation,
        contract_id: fence.contract_id.clone(),
        cancellation: cancellation.clone(),
        agent_id: run.agent_id.clone(),
        instance_name: run.instance_name.clone(),
        role: prompt.role,
        provider_selection: run.model.clone(),
        execution_policy: run.execution_policy,
        input: prompt.input,
        instructions: prompt.instructions.to_string(),
        terminal_contract: prompt.terminal_contract,
        runtime_events: subscriptions.clone(),
    }
}

fn publish_committed(subscriptions: &RuntimeEventRegistry, context: &WorkRunExecutionContext) {
    let task_id = context.task.task_id.to_string();
    subscriptions.publish_task(TaskRuntimeEvent::Changed {
        task_id: task_id.clone(),
        run_id: None,
    });
    subscriptions.publish_work(WorkRuntimeEvent::Committed {
        workspace_id: context.task.workspace_id.to_string(),
        task_id: Some(task_id),
    });
}

fn criterion_ordinal(index: usize) -> Result<u32, String> {
    index
        .checked_add(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or_else(|| "criterion count exceeds the supported bound".to_string())
}

fn validate_unique_artifact_ids(artifact_ids: &[String]) -> Result<(), String> {
    let mut seen = HashSet::with_capacity(artifact_ids.len());
    if artifact_ids
        .iter()
        .any(|artifact_id| !seen.insert(artifact_id))
    {
        return Err("Executor terminal contains duplicate artifact ids".to_string());
    }
    Ok(())
}

fn normalize_suggested_answers(answers: Vec<String>) -> Vec<String> {
    answers
        .into_iter()
        .map(|answer| answer.trim().to_string())
        .collect()
}

fn parse_payload<T: serde::de::DeserializeOwned>(
    call: &noema_providers::GenerateToolCall,
    contract: &str,
) -> Result<T, String> {
    serde_json::from_value(call.payload.clone())
        .map_err(|error| format!("invalid {contract} payload: {error}"))
}

#[cfg(test)]
mod tests {
    use super::validate_unique_artifact_ids;

    #[test]
    fn duplicate_artifact_ids_are_rejected_at_runtime() {
        let ids = vec!["artifact:one".to_string(), "artifact:one".to_string()];
        assert!(validate_unique_artifact_ids(&ids).is_err());
    }
}
