/// Resolve one owner-authorized task detail projection.
use super::*;

pub(in crate::graphql) async fn task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<Option<GraphqlTaskDetail>> {
    let store = state.store()?;
    let Some(task) = store
        .get_task(task_id.trim())
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };
    if task.owner_human_id != principal_subject {
        // Do not reveal whether a task owned by another principal exists.
        return Ok(None);
    }
    detail_from_task(store, task).await.map(Some)
}

/// Resolve one owner-authorized page of a task-run transcript.
pub(in crate::graphql) async fn task_run_items(
    state: &GraphqlState,
    principal_subject: &str,
    run_id: String,
    after: Option<String>,
    first: Option<i32>,
) -> Result<GraphqlTaskRunItemsConnection> {
    let store = state.store()?;
    let run_id = run_id.trim();
    let run = store
        .get_agent_run(run_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("task run is unavailable"))?;
    let is_authorized = store
        .get_task(&run.task_id)
        .await
        .map_err(graphql_error)?
        .is_some_and(|task| task.owner_human_id == principal_subject);
    if !is_authorized {
        return Err(async_graphql::Error::new("task run is unavailable"));
    }

    let first = first.unwrap_or(50);
    if !(1..=100).contains(&first) {
        return Err(async_graphql::Error::new(
            "task run page size must be between 1 and 100",
        ));
    }
    let continuation = after
        .as_deref()
        .map(str::trim)
        .filter(|cursor| !cursor.is_empty())
        .map(|cursor| {
            cursor
                .parse::<i64>()
                .map_err(|_| async_graphql::Error::new("task run transcript cursor is invalid"))
        })
        .transpose()?;
    let page_size = i64::from(first);
    let mut page = store
        .list_agent_run_items_before_page(run_id, continuation, page_size + 1)
        .await
        .map_err(graphql_error)?;
    let has_next_page = page.len() > usize::try_from(page_size).unwrap_or(100);
    if has_next_page {
        page.remove(0);
    }
    let end_cursor = has_next_page
        .then(|| page.first().map(|item| item.sequence_index.to_string()))
        .flatten();
    Ok(GraphqlTaskRunItemsConnection {
        items: page.into_iter().map(Into::into).collect(),
        page_info: GraphqlTaskRunItemsPageInfo {
            end_cursor,
            has_next_page,
        },
    })
}

/// Continue a failed or human-blocked task from its durable context.
pub(in crate::graphql) async fn resume_task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
    message: Option<String>,
) -> Result<GraphqlTaskDetail> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let (task, _) = store
        .resume_task_with_readiness(
            task_id.trim(),
            principal_subject,
            principal_subject,
            message.as_deref(),
            state.provider_registry()?.as_ref(),
        )
        .await
        .map_err(graphql_error)?;
    state
        .subscriptions()
        .publish_task(noema_runtime::TaskRuntimeEvent::Changed {
            task_id: task.task_id.clone(),
        });
    detail_from_task(store, task).await
}

/// Cancel one owner-authorized queued, active, or blocked task.
pub(in crate::graphql) async fn cancel_task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<GraphqlTaskDetail> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let task = store
        .cancel_task(task_id.trim(), principal_subject, principal_subject)
        .await
        .map_err(graphql_error)?;
    state
        .subscriptions()
        .publish_task(noema_runtime::TaskRuntimeEvent::Changed {
            task_id: task.task_id.clone(),
        });
    let _ =
        noema_runtime::deliver_task_status_event(store, state.subscriptions(), &task.task_id).await;
    detail_from_task(store, task).await
}

/// Resolve the human-controlled executor model pool.
pub(in crate::graphql) async fn task_model_pools(
    state: &GraphqlState,
    complexity: Option<GraphqlTaskComplexity>,
) -> Result<Vec<GraphqlTaskModelPoolEntry>> {
    let store = state.store()?;
    store
        .list_task_model_pool_settings(complexity.map(TaskComplexity::from))
        .await
        .map_err(graphql_error)
        .map(|entries| entries.into_iter().map(Into::into).collect())
}

/// Resolve global task execution limits shared by every complexity tier.
pub(in crate::graphql) async fn task_execution_policy(
    state: &GraphqlState,
) -> Result<GraphqlTaskExecutionPolicy> {
    state
        .store()?
        .get_task_execution_policy()
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace global task execution limits for future and resumed runs.
pub(in crate::graphql) async fn update_task_execution_policy(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskExecutionPolicyInput,
) -> Result<GraphqlTaskExecutionPolicy> {
    require_local_principal(principal_subject)?;
    state
        .store()?
        .update_task_execution_policy(input.into())
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace one executor model-pool entry for the local human.
pub(in crate::graphql) async fn update_task_model_pool_entry(
    state: &GraphqlState,
    principal_subject: &str,
    pool_entry_id: String,
    input: GraphqlTaskModelPoolEntryInput,
) -> Result<GraphqlTaskModelPoolEntry> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let normalized_pool_entry_id = pool_entry_id.trim().to_string();
    let requested_reasoning_effort = input.reasoning_effort.map(Into::into);
    let existing = store
        .get_task_model_pool_entry(&normalized_pool_entry_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("task model pool entry was not found"))?;
    let retains_exact_route = existing.model.provider_kind
        == input.provider_kind.trim().to_ascii_lowercase()
        && existing.model.provider_account_id == input.provider_account_id.trim()
        && existing.model.model_profile.as_deref() == Some(input.model_profile.trim())
        && existing.model.reasoning_effort == requested_reasoning_effort;
    if !input.enabled && !retains_exact_route {
        return Err(async_graphql::Error::new(
            "A disabled task model entry must retain its existing provider route",
        ));
    }
    if retains_exact_route && (existing.enabled || !input.enabled) {
        let model_profile = existing.model.model_profile.ok_or_else(|| {
            async_graphql::Error::new("task model pool entry has no model profile")
        })?;
        let pool_entry = noema_tasks::NewTaskModelPoolEntry {
            pool_entry_id: Some(normalized_pool_entry_id.clone()),
            complexity: input.complexity.into(),
            label: input.label,
            provider_kind: existing.model.provider_kind,
            provider_account_id: existing.model.provider_account_id,
            model_profile,
            reasoning_effort: existing.model.reasoning_effort,
            enabled: input.enabled,
            sort_order: i64::from(input.sort_order),
        };
        return store
            .update_task_model_pool_entry(&normalized_pool_entry_id, pool_entry)
            .await
            .map(Into::into)
            .map_err(graphql_error);
    }
    let account = selectable_model_account(state, &input.provider_account_id).await?;
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    if input.provider_kind != account.provider_kind {
        return Err(async_graphql::Error::new(
            "provider kind does not match provider account",
        ));
    }
    let profiles = selectable_profiles_from_account(store, &account).await?;
    let profile = require_selectable_profile(&profiles, &input.model_profile)?;
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let ready_selection = crate::graphql::provider_selection::prove_ready_selection(
        state,
        &account.provider_kind,
        &account.provider_account_id,
        &input.model_profile,
        reasoning_effort,
        "graphql_task_model_pool",
    )
    .await?;
    let pool_entry = noema_tasks::NewTaskModelPoolEntry {
        pool_entry_id: Some(normalized_pool_entry_id.clone()),
        complexity: input.complexity.into(),
        label: input.label,
        provider_kind: account.provider_kind,
        provider_account_id: account.provider_account_id,
        model_profile: input.model_profile,
        reasoning_effort,
        enabled: input.enabled,
        sort_order: i64::from(input.sort_order),
    };
    store
        .update_task_model_pool_entry_with_ready_selection(
            &normalized_pool_entry_id,
            pool_entry,
            &ready_selection,
        )
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

fn require_local_principal(principal_subject: &str) -> Result<()> {
    if principal_subject == "human:local" {
        Ok(())
    } else {
        Err(async_graphql::Error::new(
            "task operation is not authorized",
        ))
    }
}

async fn detail_from_task(
    store: &noema_store::NoemaStore,
    task: TaskRecord,
) -> Result<GraphqlTaskDetail> {
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let submissions = store
        .list_task_submissions(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let reviews = store
        .list_task_reviews(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let runs = store
        .list_agent_runs_for_task(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let resumable = matches!(
        task.status,
        noema_tasks::TaskStatus::Failed | noema_tasks::TaskStatus::WaitingForHuman
    );
    let cancellable = matches!(
        task.status,
        noema_tasks::TaskStatus::Queued
            | noema_tasks::TaskStatus::Executing
            | noema_tasks::TaskStatus::Reviewing
            | noema_tasks::TaskStatus::RevisionRequested
            | noema_tasks::TaskStatus::WaitingForHuman
    );
    let blocking_question = task.blocked_question.clone();
    Ok(GraphqlTaskDetail {
        task_id: task.task_id,
        title: task.title,
        request_markdown: task.request_markdown,
        complexity: task.complexity.into(),
        status: task.status.as_str().to_string(),
        owner_human_id: task.owner_human_id,
        source: GraphqlTaskSource {
            conversation_id: task.source.conversation_id,
            turn_id: task.source.turn_id,
            item_id: task.source.item_id,
        },
        created_by_agent_id: task.created_by_agent_id,
        creation_tool_call_id: task.creation_tool_call_id,
        pool_entry_id: task.pool_entry_id,
        executor_model: task.executor_model.into(),
        reviewer_model: task.reviewer_model.into(),
        revision_index: task.revision_index as i32,
        max_review_rounds: task.max_review_rounds as i32,
        final_submission_id: task.final_submission_id,
        latest_run_id: task.latest_run_id,
        terminal_reason: task.terminal_reason,
        error_code: task.error_code,
        error_message: task.error_message,
        created_at: task.created_at,
        updated_at: task.updated_at,
        completed_at: task.completed_at,
        resumable,
        cancellable,
        blocking_question,
        criteria: criteria.into_iter().map(Into::into).collect(),
        submissions: submissions.into_iter().map(Into::into).collect(),
        reviews: reviews.into_iter().map(Into::into).collect(),
        runs: runs.into_iter().map(Into::into).collect(),
    })
}
