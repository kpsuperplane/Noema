//! One fenced Planner, Executor, or Reviewer run.

use noema_store::{
    CompletePlan, PlanTerminal, ReportTaskBlocked, SubmitPlan, SubmitTaskResult, SubmitTaskReview,
    WorkCommandService, WorkRunExecutionContext, WorkRunFence, WorkRunTerminal,
};
use noema_tasks::{
    CriterionOutcome, NewTaskReview, NewTaskSubmission, NewTaskValidationCriterion, RunKind,
    TaskExecutorBackend, TaskReviewCriterion, TaskReviewVerdict, TaskSubmissionCitation,
};
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

use crate::{
    daemon::runtime::{BackgroundTaskGenerateRequest, BackgroundTaskGenerateResult},
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
    if run.run_kind == RunKind::Executor && run.executor.backend == TaskExecutorBackend::Acp {
        match crate::acp::execute_acp_run(
            services.store.clone(),
            run,
            fence,
            &context,
            cancellation,
        )
        .await?
        {
            crate::acp::AcpRunOutcome::Terminal(terminal) => {
                command_service
                    .record_work_run_terminal(
                        *terminal,
                        super::WORK_RUNTIME_ACTOR_ID,
                        Some(run.run_id.as_str()),
                        &correlation_id,
                    )
                    .await?;
                publish_committed(&services.subscriptions, &context);
            }
            crate::acp::AcpRunOutcome::WaitingForApproval => {}
        }
        return Ok(());
    }
    let prompt = build_task_role_prompt(&context);
    let runtime_environment = task_runtime_environment(&context);
    let mut generated = generate_once(
        &services.runtime,
        run,
        fence,
        cancellation,
        prompt,
        runtime_environment,
        &services.subscriptions,
    )
    .await?;
    let citations = normalize_task_result(&mut generated, &run.run_id, &services.system_errors);
    let terminal = parse_terminal(
        run,
        &context,
        generated.response.tool_calls.as_slice(),
        fence.clone(),
        &citations,
    )
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

pub(crate) fn parse_terminal(
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    calls: &[noema_providers::GenerateToolCall],
    fence: WorkRunFence,
    citations: &[TaskSubmissionCitation],
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
        RunKind::Executor => execute_executor(run, context, call, fence, citations),
        RunKind::Reviewer => execute_reviewer(run, context, call, fence),
    }
}

fn execute_executor(
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    call: &noema_providers::GenerateToolCall,
    fence: WorkRunFence,
    citations: &[TaskSubmissionCitation],
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
            citations: citations.to_vec(),
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
    runtime_environment: Option<String>,
    subscriptions: &RuntimeEventRegistry,
) -> Result<BackgroundTaskGenerateResult, RuntimeError> {
    let request = background_task_generate_request(
        run,
        fence,
        cancellation,
        prompt,
        runtime_environment,
        subscriptions,
    );
    runtime.generate_background_task(request).await
}

fn normalize_task_result(
    generated: &mut BackgroundTaskGenerateResult,
    run_id: &str,
    system_errors: &noema_home::SystemErrorLogger,
) -> Vec<TaskSubmissionCitation> {
    let BackgroundTaskGenerateResult {
        response,
        citation_sources,
    } = generated;
    let Some(call) = response
        .tool_calls
        .iter_mut()
        .find(|call| call.name == "task.submit_result")
    else {
        return Vec::new();
    };
    let mut citations = Vec::new();
    let mut unresolved_count = 0usize;
    let mut normalize_text =
        |value: &mut serde_json::Value, retain_offsets: bool| {
            let Some(text) = value.as_str() else {
                return;
            };
            let normalized = citation_sources.normalize(text, &[]);
            unresolved_count =
                unresolved_count.saturating_add(normalized.unresolved_references.len());
            *value = serde_json::Value::String(normalized.text);
            citations.extend(normalized.citations.into_iter().map(|citation| {
                TaskSubmissionCitation {
                    title: citation.title,
                    url: citation.url,
                    start_index: if retain_offsets {
                        citation.start_index
                    } else {
                        None
                    },
                    end_index: if retain_offsets {
                        citation.end_index
                    } else {
                        None
                    },
                }
            }));
        };
    if let Some(text) = call.payload.get_mut("result_markdown") {
        normalize_text(text, true);
    }
    if let Some(text) = call.payload.get_mut("summary") {
        normalize_text(text, false);
    }
    if let Some(criteria) = call
        .payload
        .get_mut("criteria")
        .and_then(serde_json::Value::as_array_mut)
    {
        for criterion in criteria {
            if let Some(text) = criterion.get_mut("evidence_markdown") {
                normalize_text(text, false);
            }
        }
    }
    drop(normalize_text);
    let mut cited_urls = citations
        .iter()
        .filter(|citation| citation.end_index.is_some())
        .map(|citation| citation.url.clone())
        .collect::<HashSet<_>>();
    citations
        .retain(|citation| citation.end_index.is_some() || cited_urls.insert(citation.url.clone()));
    if unresolved_count > 0 {
        system_errors.try_append(
            noema_home::SystemErrorEvent::new(
                "provider_citation_unresolved",
                "Provider citation references could not be resolved",
            )
            .with_context(serde_json::json!({
                "scope_kind": "task_run",
                "scope_id": run_id,
                "reference_count": unresolved_count,
            })),
        );
    }
    citations
}

fn background_task_generate_request(
    run: &noema_tasks::AgentRunRecord,
    fence: &WorkRunFence,
    cancellation: &CancellationToken,
    prompt: TaskRolePrompt,
    runtime_environment: Option<String>,
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
        runtime_environment,
        instructions: prompt.instructions.to_string(),
        terminal_contract: prompt.terminal_contract,
        runtime_events: subscriptions.clone(),
    }
}

fn task_runtime_environment(context: &WorkRunExecutionContext) -> Option<String> {
    context.task.schedule_time_zone.as_deref().map_or_else(
        || context.source_runtime_environment.clone(),
        |time_zone| {
            Some(
                crate::daemon::runtime::turn::current_runtime_environment_with_timezone(
                    None,
                    Some(time_zone),
                )
                .render(),
            )
        },
    )
}

#[cfg(test)]
mod citation_tests {
    use noema_providers::{
        GenerateHostedWebSearch, GenerateResponse, GenerateToolCall, GenerateWebSource,
    };

    use super::*;

    #[test]
    fn task_submission_markers_normalize_across_all_persisted_text() {
        let mut generated = BackgroundTaskGenerateResult {
            response: GenerateResponse {
                responses: Vec::new(),
                tool_calls: vec![GenerateToolCall {
                    id: None,
                    provider_call_id: None,
                    provider_name: None,
                    name: "task.submit_result".to_string(),
                    payload: serde_json::json!({
                        "summary": "Summary\u{e200}cite\u{e202}turn0search0\u{e201}",
                        "result_markdown": "Claim\u{e200}cite\u{e202}turn0search0\u{e201}",
                        "criteria": [
                            {
                                "criterion_id": "criterion:one",
                                "evidence_markdown": "Evidence\u{e200}cite\u{e202}turn0search1\u{e201}"
                            },
                            {
                                "criterion_id": "criterion:two",
                                "evidence_markdown": "Also\u{e200}cite\u{e202}turn0search1\u{e201}"
                            }
                        ]
                    }),
                }],
                reasoning_items: Vec::new(),
                hosted_web_searches: Vec::new(),
                provider: "codex".to_string(),
                model: "test".to_string(),
                response_id: None,
                usage: None,
            },
            citation_sources: Default::default(),
        };
        generated.citation_sources.observe(
            0,
            &[GenerateHostedWebSearch {
                output_index: 0,
                id: None,
                tool_name: "web.search".to_string(),
                arguments: serde_json::json!({}),
                result: serde_json::json!({}),
                status: "completed".to_string(),
                sources: vec![
                    GenerateWebSource {
                        title: None,
                        url: "https://example.com/source".to_string(),
                    },
                    GenerateWebSource {
                        title: Some("Evidence source".to_string()),
                        url: "https://evidence.example/source".to_string(),
                    },
                ],
            }],
        );

        let citations = normalize_task_result(
            &mut generated,
            "run:test",
            &crate::test_support::system_error_logger(),
        );

        assert_eq!(
            generated.response.tool_calls[0].payload["result_markdown"],
            "Claim"
        );
        assert_eq!(
            generated.response.tool_calls[0].payload["summary"],
            "Summary"
        );
        assert_eq!(
            generated.response.tool_calls[0].payload["criteria"][0]["evidence_markdown"],
            "Evidence"
        );
        assert_eq!(
            generated.response.tool_calls[0].payload["criteria"][1]["evidence_markdown"],
            "Also"
        );
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].end_index, Some(5));
        assert_eq!(citations[0].title, "example.com");
        assert_eq!(citations[1].title, "Evidence source");
        assert_eq!(citations[1].start_index, None);
        assert_eq!(citations[1].end_index, None);
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
