async fn inspect_inner(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = task_id_arguments(payload)?;
    let task = store
        .get_task(arguments.task_id.trim())
        .await
        .map_err(|error| error.to_string())?
        .filter(|task| task.owner_human_id == context.owner_human_id)
        .ok_or_else(|| "task is unavailable".to_string())?;
    let runs = store
        .list_agent_runs_for_task(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let submissions = store
        .list_task_submissions(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let reviews = store
        .list_task_reviews(&task.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let inspected_run = if context.actor_id == TASK_REVIEWER_AGENT_ID {
        submissions
            .last()
            .and_then(|submission| {
                runs.iter()
                    .find(|run| run.run_id == submission.executor_run_id)
            })
            .or_else(|| runs.last())
    } else {
        runs.last()
    };
    let latest_items = if let Some(run) = inspected_run {
        store
            .list_recent_agent_run_items(&run.run_id, 50)
            .await
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let transcript_cursor = latest_items
        .last()
        .map(|item| item.sequence_index.to_string());
    let recent_items = latest_items
        .iter()
        .map(|item| {
            json!({
                "item_id": item.item_id,
                "cursor": item.sequence_index.to_string(),
                "round": item.round_index,
                "kind": item.kind,
                "status": item.status.as_str(),
                "correlation_id": item.correlation_id,
                "parent_item_id": item.parent_item_id,
                "content_text": item.content_text,
                "payload": item.payload,
                "created_at": item.created_at,
                "updated_at": item.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let latest_run = runs.last().map(|run| {
        json!({
            "run_id": run.run_id,
            "kind": run.run_kind.as_str(),
            "status": run.status.as_str(),
            "revision": run.revision_index,
            "attempt": run.attempt_index,
            "model": run.actual_model_profile.as_ref().or(run.model.model_profile.as_ref()),
            "queued_at": run.queued_at,
            "started_at": run.started_at,
            "ended_at": run.ended_at,
            "error_code": run.error_code,
            "error_message": run.error_message,
        })
    });
    let latest_submission = submissions.last().map(|submission| {
        json!({
            "submission_id": submission.submission_id,
            "revision": submission.revision_index,
            "summary": submission.summary,
            "artifacts": submission.artifacts.iter().map(|linked| json!({
                "artifact_id": linked.artifact.artifact_id,
                "artifact_version_id": linked.version.artifact_version_id,
                "title": linked.artifact.title,
                "artifact_kind": linked.artifact.artifact_kind,
                "media_type": linked.version.media_type,
            })).collect::<Vec<_>>(),
            "created_at": submission.created_at,
        })
    });
    let latest_review = reviews.last().map(|review| {
        json!({
            "review_id": review.review_id,
            "verdict": review.overall_verdict.as_str(),
            "feedback": review.overall_feedback,
            "created_at": review.created_at,
        })
    });
    let last_activity_at = latest_items
        .last()
        .map(|item| item.updated_at.as_str())
        .or_else(|| runs.last().map(|run| run.updated_at.as_str()))
        .unwrap_or(task.updated_at.as_str());
    let can_resume = matches!(
        task.status,
        TaskStatus::Failed | TaskStatus::WaitingForHuman
    );
    let can_cancel = matches!(
        task.status,
        TaskStatus::Queued
            | TaskStatus::Executing
            | TaskStatus::Reviewing
            | TaskStatus::RevisionRequested
            | TaskStatus::WaitingForHuman
    );
    let current_phase = runs.last().map_or_else(
        || task.status.as_str().to_string(),
        |run| format!("{}.{}", run.run_kind.as_str(), run.status.as_str()),
    );
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "current_phase": current_phase,
        "complexity": task.complexity.as_str(),
        "revision": task.revision_index,
        "max_review_rounds": task.max_review_rounds,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
        "last_activity_at": last_activity_at,
        "completed_at": task.completed_at,
        "can_resume": can_resume,
        "can_cancel": can_cancel,
        "blocking_question": task.blocked_question,
        "terminal_reason": task.terminal_reason,
        "error_code": task.error_code,
        "error_message": task.error_message,
        "policy_consumption": {
            "provider_calls": inspected_run.map_or(0, |run| run.provider_call_count),
            "tool_calls": inspected_run.map_or(0, |run| run.tool_call_count),
            "active_milliseconds": inspected_run.map_or(0, |run| run.active_milliseconds),
            "input_tokens": inspected_run.map_or(0, |run| run.input_tokens),
            "cached_input_tokens": inspected_run.map_or(0, |run| run.cached_input_tokens),
            "output_tokens": inspected_run.map_or(0, |run| run.output_tokens),
            "limits": inspected_run.map(|run| json!({
                "provider_continuations": run.execution_policy.max_provider_continuations,
                "tool_calls": run.execution_policy.max_tool_calls,
                "active_minutes": run.execution_policy.max_active_minutes,
                "progress_audit_interval": run.execution_policy.progress_audit_interval,
            })),
        },
        "transcript_run": inspected_run.map(|run| json!({
            "run_id": run.run_id,
            "kind": run.run_kind.as_str(),
            "revision": run.revision_index,
        })),
        "transcript_cursor": transcript_cursor,
        "recent_items": recent_items,
        "latest_run": latest_run,
        "latest_submission": latest_submission,
        "latest_review": latest_review,
    }))
}

async fn resume_inner(
    store: &NoemaStore,
    provider_registry: &ProviderRegistry,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = resume_arguments(payload)?;
    let (task, run) = store
        .resume_task_with_readiness(
            arguments.task_id.trim(),
            &context.owner_human_id,
            &context.actor_id,
            arguments.message.as_deref(),
            provider_registry,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "run_id": run.run_id,
        "run_kind": run.run_kind.as_str(),
        "attempt": run.attempt_index,
        "revision": run.revision_index,
        "model": run.model.model_profile,
    }))
}

async fn cancel_inner(
    store: &NoemaStore,
    context: &TaskAccessRuntimeContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = task_id_arguments(payload)?;
    let task = store
        .cancel_task(
            arguments.task_id.trim(),
            &context.owner_human_id,
            &context.actor_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": task.status.as_str(),
        "cancelled": task.status == TaskStatus::Cancelled,
    }))
}

fn task_id_arguments(payload: &Value) -> Result<TaskIdArguments, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    serde_json::from_value(arguments).map_err(|error| format!("invalid task arguments: {error}"))
}

fn resume_arguments(payload: &Value) -> Result<ResumeArguments, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    serde_json::from_value(arguments).map_err(|error| format!("invalid task arguments: {error}"))
}

async fn execute_inner(
    store: &NoemaStore,
    provider_registry: &ProviderRegistry,
    context: &TaskDelegateRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> Result<Value, String> {
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    let arguments: DelegateArguments = serde_json::from_value(arguments)
        .map_err(|error| format!("invalid task delegation arguments: {error}"))?;
    let pool = store
        .select_task_model_pool_entry(
            arguments.complexity,
            arguments.executor_model_pool_entry_id.trim(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let reviewer = reviewer_model_snapshot(store, context).await?;
    let criteria = arguments
        .validation_criteria
        .into_iter()
        .enumerate()
        .map(|(index, criterion)| NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
            description: criterion.description,
            expected_evidence: criterion.evidence_required,
        })
        .collect();
    let (task, run) = store
        .create_task_with_executor_with_readiness(
            NewTask {
                task_id: None,
                title: arguments.title,
                request_markdown: arguments.request,
                complexity: arguments.complexity,
                owner_human_id: "human:local".to_string(),
                source: TaskSource {
                    conversation_id: Some(context.conversation_id.clone()),
                    turn_id: Some(context.turn_id.clone()),
                    item_id: Some(context.user_item_id.clone()),
                },
                created_by_agent_id: context.agent_id.clone(),
                creation_tool_call_id: call_id.or_else(|| call_id_from_payload(payload)),
                pool_entry_id: pool.pool_entry_id.clone(),
                executor_model: pool.model.clone(),
                reviewer_model: reviewer,
                max_review_rounds: None,
                criteria,
            },
            provider_registry,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "task_id": task.task_id,
        "title": task.title,
        "status": TaskStatus::Queued.as_str(),
        "complexity": task.complexity.as_str(),
        "executor_model_pool_entry_id": pool.pool_entry_id,
        "executor_model": task.executor_model.model_profile,
        "reviewer_model": task.reviewer_model.model_profile,
        "executor_run_id": run.run_id,
    }))
}
