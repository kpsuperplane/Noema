async fn execute_run(
    store: &NoemaStore,
    runtime: &RuntimeHandle,
    provider_registry: &ProviderRegistryHandle,
    subscriptions: &RuntimeEventRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    store
        .transition_agent_run(&run.run_id, RunStatus::Running, Some(lease_token), None)
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &run.task_id);
    match run.run_kind {
        RunKind::Executor => {
            execute_executor(
                store,
                runtime,
                provider_registry,
                subscriptions,
                run,
                lease_token,
                cancellation,
            )
            .await
        }
        RunKind::Reviewer => {
            execute_reviewer(
                store,
                runtime,
                provider_registry,
                subscriptions,
                run,
                lease_token,
                cancellation,
            )
            .await
        }
    }
}

async fn execute_executor(
    store: &NoemaStore,
    runtime: &RuntimeHandle,
    provider_registry: &ProviderRegistryHandle,
    subscriptions: &RuntimeEventRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    let task = store
        .get_task(&run.task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before executor run".to_string())?;
    if task.status == TaskStatus::Queued || task.status == TaskStatus::RevisionRequested {
        store
            .transition_task(
                &task.task_id,
                TaskStatus::Executing,
                Some("executor_started"),
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_task_changed(subscriptions, &task.task_id);
    }
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let prompt = with_resume_context(
        store,
        run,
        format_executor_prompt(&task, &criteria, run.revision_index),
    )
    .await?;
    let response = generate_once(
        runtime,
        run,
        lease_token,
        cancellation,
        prompt,
        "You are Noema's background task executor. Work autonomously with the role-approved tools. When finished, call task.submit_result exactly once. If safe progress genuinely requires human input, call task.report_blocked exactly once. Do not return the task result as ordinary assistant text.",
        subscriptions,
    )
    .await?;
    if let Some(call) = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_REPORT_BLOCKED_TOOL)
    {
        let blocked: ExecutorBlockedResponse = serde_json::from_value(call.payload.clone())
            .map_err(|error| format!("invalid executor blocked contract: {error}"))?;
        let blocked_context = match blocked.resume_context {
            Some(resume_context) if !resume_context.trim().is_empty() => format!(
                "{}\n\nResume context:\n{}",
                blocked.work_summary.trim(),
                resume_context.trim()
            ),
            _ => blocked.work_summary.trim().to_string(),
        };
        store
            .report_task_blocked(
                &task.task_id,
                &run.run_id,
                lease_token,
                blocked.question.trim(),
                &blocked_context,
            )
            .await
            .map_err(|error| error.to_string())?;
        publish_task_changed(subscriptions, &task.task_id);
        let _ = crate::daemon::task_delivery::deliver_task_status_event(
            store,
            subscriptions,
            &task.task_id,
        )
        .await;
        return Ok(());
    }
    let call = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_SUBMIT_RESULT_TOOL)
        .ok_or_else(|| "executor terminal contract missing".to_string())?;
    let result: ExecutorSubmissionResponse = serde_json::from_value(call.payload.clone())
        .map_err(|error| format!("invalid executor submission contract: {error}"))?;
    let evidence = result
        .criteria
        .into_iter()
        .map(|criterion| SubmissionCriterionEvidence {
            criterion_id: criterion.criterion_id,
            evidence_markdown: criterion.evidence_markdown,
        })
        .collect();
    store
        .create_task_submission_with_readiness(
            NewTaskSubmission {
                submission_id: None,
                task_id: task.task_id.clone(),
                executor_run_id: run.run_id.clone(),
                revision_index: run.revision_index,
                summary: result.summary,
                result_markdown: result.result_markdown,
                criteria: evidence,
                artifact_ids: result.artifact_ids,
            },
            lease_token,
            provider_registry.as_ref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &task.task_id);
    let _ = crate::daemon::task_delivery::deliver_task_status_event(
        store,
        subscriptions,
        &task.task_id,
    )
    .await;
    Ok(())
}

async fn execute_reviewer(
    store: &NoemaStore,
    runtime: &RuntimeHandle,
    provider_registry: &ProviderRegistryHandle,
    subscriptions: &RuntimeEventRegistry,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    let task = store
        .get_task(&run.task_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "task disappeared before reviewer run".to_string())?;
    let submission_id = run
        .triggering_submission_id
        .as_deref()
        .ok_or_else(|| "reviewer run has no submission".to_string())?;
    let submission = store
        .get_task_submission(submission_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "submission disappeared before review".to_string())?;
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let human_context = human_continuation_context_for_submission(store, &submission).await?;
    let prompt = with_resume_context(
        store,
        run,
        format_reviewer_prompt(&task, &submission, &criteria, &human_context),
    )
    .await?;
    let response = generate_once(
        runtime,
        run,
        lease_token,
        cancellation,
        prompt,
        "You are Noema's adversarial task reviewer. Inspect the submission and call task.submit_review exactly once with the typed verdict. Do not return review JSON as ordinary assistant text.",
        subscriptions,
    )
    .await?;
    let call = response
        .tool_calls
        .iter()
        .find(|call| call.name == TASK_SUBMIT_REVIEW_TOOL)
        .ok_or_else(|| "reviewer terminal contract missing".to_string())?;
    let parsed: ReviewerResponse = serde_json::from_value(call.payload.clone())
        .map_err(|error| format!("invalid reviewer contract: {error}"))?;
    let verdict = parsed
        .overall_verdict
        .parse::<TaskReviewVerdict>()
        .map_err(|_| "reviewer verdict was invalid".to_string())?;
    let review_criteria = parsed
        .criteria
        .into_iter()
        .map(|criterion| {
            Ok(TaskReviewCriterion {
                criterion_id: criterion.criterion_id,
                outcome: criterion
                    .outcome
                    .parse::<CriterionOutcome>()
                    .map_err(|_| "review criterion outcome was invalid")?,
                evidence_markdown: criterion.evidence_markdown,
                feedback: criterion.feedback,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    store
        .create_task_review_with_readiness(
            NewTaskReview {
                review_id: None,
                task_id: task.task_id.clone(),
                reviewer_run_id: run.run_id.clone(),
                reviewed_submission_id: submission_id.to_string(),
                overall_verdict: verdict,
                overall_feedback: parsed.overall_feedback,
                criteria: review_criteria,
            },
            lease_token,
            provider_registry.as_ref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    publish_task_changed(subscriptions, &task.task_id);
    let _ = crate::daemon::task_delivery::deliver_task_status_event(
        store,
        subscriptions,
        &task.task_id,
    )
    .await;
    Ok(())
}

async fn generate_once(
    runtime: &RuntimeHandle,
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
    input: String,
    instructions: &str,
    subscriptions: &RuntimeEventRegistry,
) -> Result<noema_providers::GenerateResponse, String> {
    let request = background_task_generate_request(
        run,
        lease_token,
        cancellation,
        input,
        instructions,
        subscriptions,
    );
    runtime
        .generate_background_task(request)
        .await
        .map_err(|error| error.to_string())
}

fn background_task_generate_request(
    run: &noema_tasks::AgentRunRecord,
    lease_token: &str,
    cancellation: &CancellationToken,
    input: String,
    instructions: &str,
    subscriptions: &RuntimeEventRegistry,
) -> BackgroundTaskGenerateRequest {
    BackgroundTaskGenerateRequest {
        run_id: run.run_id.clone(),
        task_id: run.task_id.clone(),
        lease_token: lease_token.to_string(),
        cancellation: cancellation.clone(),
        agent_id: run.agent_id.clone(),
        role: if run.run_kind == RunKind::Reviewer {
            ExecutionRole::TaskReviewer
        } else {
            ExecutionRole::TaskExecutor
        },
        provider_selection: run.model.clone(),
        execution_policy: run.execution_policy,
        input,
        instructions: instructions.to_string(),
        runtime_events: subscriptions.clone(),
    }
}
