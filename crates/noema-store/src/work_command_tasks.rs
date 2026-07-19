//! Task capture/queue/delegation command transactions.

use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{
    CaptureTask, ContractOrigin, DelegateTask, NewTaskValidationCriterion, PERSONAL_INBOX_STAGE_ID,
    PERSONAL_QUEUE_STAGE_ID, QueueTask, TaskComplexity, TaskContractId, TaskSourceKind,
    UpdateInboxTask, WorkCommand, WorkDomainError, WorkEventPayload, WorkflowStageBehavior,
    WorkflowStageId,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{WorkCommandService, helpers};
use crate::{
    StoreError,
    ids::allocate_id,
    tasks::provider_selection::{pool_selection_tx, reviewer_preference_tx},
    work_events::append_work_event_tx,
    work_notifications::enqueue_work_notification_tx,
};

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    match command {
        WorkCommand::CaptureTask(value) => capture(service, value).await,
        WorkCommand::UpdateInboxTask(value) => update_inbox(service, value).await,
        WorkCommand::QueueTask(value) => queue(service, value).await,
        WorkCommand::DelegateTask(value) => delegate(service, value).await,
        _ => Err(StoreError::InvariantViolation {
            message: "task writer received a project command".to_string(),
        }),
    }
}

async fn capture(
    service: &WorkCommandService,
    command: &CaptureTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let task_id = noema_tasks::TaskId::new(allocate_id("task")).map_err(StoreError::Work)?;
    let workspace_id = command.workspace_id.clone();
    let envelope = WorkCommand::CaptureTask(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        if let Some(replay) = helpers::lookup_capture_source_receipt_tx(transaction, command)? {
            return Ok(helpers::CommandTransactionOutcome::Replay(replay));
        }
        require_workspace(transaction, &workspace_id)?;
        validate_project_target(transaction, &workspace_id, command.project_id.as_ref())?;
        transaction.execute(
            r#"INSERT INTO tasks (
                     task_id, workspace_id, project_id, workflow_id, stage_id,
                     title, description_markdown, source_kind, source_conversation_id,
                     source_turn_id, source_item_id, source_tool_call_id,
                     created_by_actor_id
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                task_id.as_str(),
                workspace_id.as_str(),
                command.project_id.as_ref().map(ProjectId::as_str),
                noema_tasks::PERSONAL_WORKFLOW_ID,
                PERSONAL_INBOX_STAGE_ID,
                command.title,
                command.description_markdown,
                command.provenance.source_kind.as_str(),
                command.provenance.conversation_id,
                command.provenance.turn_id,
                command.provenance.item_id,
                command.provenance.source_tool_call_id,
                command.provenance.created_by_actor_id,
            ],
        )?;
        let payload = WorkEventPayload::task_captured(
            1,
            1,
            WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?,
            command.provenance.source_kind,
        )
        .map_err(StoreError::Work)?;
        let mut event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).scope(
                &workspace_id,
                command.project_id.as_ref(),
                Some(&task_id),
                None,
            ),
            payload,
        )?;
        if let Some(notification_event) = enqueue_work_notification_tx(
            transaction,
            &event,
            noema_tasks::NotificationKind::TaskCreated,
            &serde_json::json!({"task_id": task_id.as_str(), "title": command.title}),
        )? {
            event = notification_event;
        }
        Ok(helpers::task_write(event, task_id.clone()).into())
    })
    .await
}

async fn update_inbox(
    service: &WorkCommandService,
    command: &UpdateInboxTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::UpdateInboxTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
            let task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
            if task.stage_behavior != WorkflowStageBehavior::Intake {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let project_id = match &command.project_id {
                None => task.project_id.as_ref(),
                Some(None) => None,
                Some(Some(project_id)) => Some(project_id),
            };
            validate_project_target(transaction, &task.workspace_id, project_id)?;
            let title = command.title.as_deref().unwrap_or(&task.title);
            let description = command
                .description_markdown
                .as_deref()
                .unwrap_or(&task.description_markdown);
            let revision = helpers::increment(task.revision, "task.revision")?;
            transaction.execute(
                "UPDATE tasks SET title = ?2, description_markdown = ?3, project_id = ?4, revision = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?6 AND generation = ?7",
                params![
                    task_id.as_str(),
                    title,
                    description,
                    project_id.map(ProjectId::as_str),
                    revision,
                    task.revision,
                    task.generation,
                ],
            )?;
            let mut changed = Vec::new();
            if command.title.is_some() {
                changed.push(noema_tasks::TaskChangedField::Title);
            }
            if command.description_markdown.is_some() {
                changed.push(noema_tasks::TaskChangedField::Description);
            }
            if command.project_id.is_some() {
                changed.push(noema_tasks::TaskChangedField::Project);
            }
            let payload = WorkEventPayload::task_updated(revision, task.generation, changed)
                .map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &task.workspace_id,
                    project_id,
                    Some(&task_id),
                    None,
                ),
                payload,
            )?;
            Ok(helpers::task_write(event, task_id.clone()).into())
        })
        .await
}

async fn queue(
    service: &WorkCommandService,
    command: &QueueTask,
) -> Result<helpers::CommandWrite, StoreError> {
    let envelope = WorkCommand::QueueTask(command.clone());
    let task_id = command.precondition.task_id.clone();
    helpers::command_transaction(&service.store, &envelope, |transaction| {
            let mut task = helpers::load_fenced_task_tx(transaction, &command.precondition)?;
            if task.stage_behavior != WorkflowStageBehavior::Intake {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            validate_project_target(transaction, &task.workspace_id, task.project_id.as_ref())?;
            let revision = helpers::increment(task.revision, "task.revision")?;
            transaction.execute(
                "UPDATE tasks SET stage_id = ?2, queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND revision = ?4 AND generation = ?5",
                params![task_id.as_str(), PERSONAL_QUEUE_STAGE_ID, revision, task.revision, task.generation],
            )?;
            task.stage_id = WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?;
            task.stage_behavior = WorkflowStageBehavior::Dispatch;
            task.revision = revision;
            let queued = WorkEventPayload::task_queued(
                revision,
                task.generation,
                None,
                noema_tasks::RunKind::Planner,
            )
            .map_err(StoreError::Work)?;
            let _queued_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &task.workspace_id,
                    task.project_id.as_ref(),
                    Some(&task_id),
                    None,
                ),
                queued,
            )?;
            let changed = WorkEventPayload::task_stage_changed(
                revision,
                task.generation,
                WorkflowStageId::new(PERSONAL_INBOX_STAGE_ID).map_err(StoreError::Work)?,
                task.stage_id.clone(),
                noema_tasks::TaskStageChangeReason::Queued,
            )
            .map_err(StoreError::Work)?;
            let _stage_event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &task.workspace_id,
                    task.project_id.as_ref(),
                    Some(&task_id),
                    None,
                ),
                changed,
            )?;
            let (run_id, run_event) = helpers::queue_run_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                helpers::QueueRun {
                    run_kind: noema_tasks::RunKind::Planner,
                    contract_id: None,
                    planner_complexity: None,
                    review_round: 0,
                    attempt_index: 0,
                    parent_run_id: None,
                    triggering_submission_id: None,
                    triggering_review_id: None,
                    event: helpers::event_context(&command.meta),
                },
            )?;
            Ok(helpers::task_write(run_event, task_id.clone())
                .run(Some(run_id))
                .into())
        })
        .await
}

async fn delegate(
    service: &WorkCommandService,
    command: &DelegateTask,
) -> Result<helpers::CommandWrite, StoreError> {
    if command.meta.idempotency_key.is_none() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "task.delegate.meta.idempotency_key",
            message: "Delegate requires an idempotency key".to_string(),
        }));
    }
    let task_id = noema_tasks::TaskId::new(allocate_id("task")).map_err(StoreError::Work)?;
    let workspace_id = command.workspace_id.clone();
    let envelope = WorkCommand::DelegateTask(command.clone());
    helpers::command_transaction(&service.store, &envelope, |transaction| {
        if let Some(replay) = helpers::lookup_delegate_source_receipt_tx(transaction, command)? {
            return Ok(helpers::CommandTransactionOutcome::Replay(replay));
        }
        require_workspace(transaction, &workspace_id)?;
        validate_project_target(transaction, &workspace_id, command.project_id.as_ref())?;
        transaction.execute(
            r#"INSERT INTO tasks (
                     task_id, workspace_id, project_id, workflow_id, stage_id,
                     title, description_markdown, source_kind, source_conversation_id,
                     source_turn_id, source_item_id, source_tool_call_id,
                     created_by_actor_id, queued_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                             strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"#,
            params![
                task_id.as_str(),
                workspace_id.as_str(),
                command.project_id.as_ref().map(ProjectId::as_str),
                noema_tasks::PERSONAL_WORKFLOW_ID,
                PERSONAL_QUEUE_STAGE_ID,
                command.title,
                command.description_markdown,
                TaskSourceKind::ChatDelegate.as_str(),
                command.provenance.conversation_id,
                command.provenance.turn_id,
                command.provenance.item_id,
                command.provenance.source_tool_call_id,
                command.provenance.created_by_actor_id,
            ],
        )?;
        let task = helpers::load_task_state_tx(transaction, &task_id)?;
        let captured = WorkEventPayload::task_captured(
            1,
            1,
            WorkflowStageId::new(PERSONAL_QUEUE_STAGE_ID).map_err(StoreError::Work)?,
            TaskSourceKind::ChatDelegate,
        )
        .map_err(StoreError::Work)?;
        let captured_event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).task_scope(&task, None),
            captured,
        )?;
        let _notification_event = enqueue_work_notification_tx(
            transaction,
            &captured_event,
            noema_tasks::NotificationKind::TaskCreated,
            &serde_json::json!({"task_id": task_id.as_str(), "title": command.title}),
        )?;
        let contract = if let Some(intent) = &command.execution_intent {
            let (contract_id, _contract_event) = create_contract_tx(
                transaction,
                service.provider_registry.as_ref(),
                &task,
                CreateContract {
                    origin: ContractOrigin::Delegated,
                    request_markdown: &intent.request_markdown,
                    execution_plan_markdown: intent.execution_plan_markdown.as_deref(),
                    criteria: &intent.criteria,
                    complexity: intent.complexity,
                    supersedes_contract_id: None,
                    event: helpers::event_context(&command.meta),
                },
            )?;
            Some(contract_id)
        } else {
            None
        };
        let next_kind = if contract.is_some() {
            noema_tasks::RunKind::Executor
        } else {
            noema_tasks::RunKind::Planner
        };
        let queued = WorkEventPayload::task_queued(1, 1, contract.clone(), next_kind)
            .map_err(StoreError::Work)?;
        let _queued_event = append_work_event_tx(
            transaction,
            helpers::event_context(&command.meta).task_scope(&task, None),
            queued,
        )?;
        let (run_id, run_event) = helpers::queue_run_tx(
            transaction,
            service.provider_registry.as_ref(),
            &task,
            helpers::QueueRun {
                run_kind: next_kind,
                contract_id: contract.as_ref(),
                planner_complexity: command.complexity_hint,
                review_round: u32::from(contract.is_some()),
                attempt_index: 0,
                parent_run_id: None,
                triggering_submission_id: None,
                triggering_review_id: None,
                event: helpers::event_context(&command.meta),
            },
        )?;
        Ok(helpers::task_write(run_event, task_id.clone())
            .contract(contract)
            .run(Some(run_id))
            .into())
    })
    .await
}

fn require_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
) -> Result<(), StoreError> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND is_personal = 1 AND archived_at IS NULL)",
        [workspace_id.as_str()],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::WorkUnavailable))
    }
}

fn validate_project_target(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
) -> Result<(), StoreError> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let row = transaction
        .query_row(
            "SELECT workspace_id, archived_at FROM projects WHERE project_id = ?1",
            [project_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let Some((workspace, archived_at)) = row else {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    };
    if workspace != workspace_id.as_str() {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    }
    if archived_at.is_some() {
        return Err(StoreError::Work(WorkDomainError::ProjectArchived));
    }
    Ok(())
}

/// Create one immutable execution contract and its exact criteria rows.
pub(crate) struct CreateContract<'a> {
    pub origin: ContractOrigin,
    pub request_markdown: &'a str,
    pub execution_plan_markdown: Option<&'a str>,
    pub criteria: &'a [NewTaskValidationCriterion],
    pub complexity: TaskComplexity,
    pub supersedes_contract_id: Option<&'a TaskContractId>,
    pub event: helpers::CommandEventContext<'a>,
}

pub(crate) fn create_contract_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    task: &helpers::TaskState,
    request: CreateContract<'_>,
) -> Result<(TaskContractId, noema_tasks::WorkEventRecord), StoreError> {
    let executor = select_pool(transaction, request.complexity)?;
    let reviewer = reviewer_preference_tx(transaction)?;
    let _executor_ready =
        crate::provider_selections::prove_selection_ready(&executor, Some(registry))?;
    let _reviewer_ready =
        crate::provider_selections::prove_selection_ready(&reviewer, Some(registry))?;
    let policy = helpers::load_policy_tx(transaction)?;
    let contract_id = TaskContractId::new(allocate_id("contract")).map_err(StoreError::Work)?;
    let version: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM task_execution_contracts WHERE task_id = ?1",
        [task.task_id.as_str()],
        |row| row.get(0),
    )?;
    if request.origin == ContractOrigin::Planned && request.execution_plan_markdown.is_none() {
        return Err(StoreError::Work(WorkDomainError::InvalidInput {
            field: "contract.execution_plan_markdown",
            message: "planned contract requires a plan".to_string(),
        }));
    }
    let (workspace_name, workspace_description) = transaction.query_row(
        "SELECT name, description FROM workspaces WHERE workspace_id = ?1",
        [task.workspace_id.as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    let project_context = task
        .project_id
        .as_ref()
        .map(|project_id| {
            transaction.query_row(
                "SELECT name, description FROM projects WHERE project_id = ?1",
                [project_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
        })
        .transpose()?;
    transaction.execute(
        r#"INSERT INTO task_execution_contracts (
             contract_id, task_id, version, task_generation, supersedes_contract_id, origin,
             request_markdown, execution_plan_markdown, complexity,
             executor_provider_kind, executor_provider_account_id,
             executor_provider_instance_key, executor_selection_mode,
             executor_model_profile, executor_reasoning_effort,
             executor_selection_source, reviewer_provider_kind,
             reviewer_provider_account_id, reviewer_provider_instance_key,
             reviewer_selection_mode, reviewer_model_profile,
             reviewer_reasoning_effort, reviewer_selection_source,
             max_provider_continuations, max_tool_calls, max_active_minutes,
             progress_audit_interval, max_automatic_retries, max_review_rounds,
             workspace_id_snapshot, workspace_name_snapshot,
             workspace_description_snapshot, project_id_snapshot,
             project_name_snapshot, project_description_snapshot,
             created_by_actor_id
           ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                     ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33,
                     ?34, ?35, ?36)"#,
        params![
            contract_id.as_str(),
            task.task_id.as_str(),
            version,
            task.generation,
            request.supersedes_contract_id.map(ToString::to_string),
            request.origin.as_str(),
            request.request_markdown,
            request.execution_plan_markdown,
            request.complexity.as_str(),
            executor.provider_kind,
            executor.provider_account_id,
            executor
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string),
            executor.selection_mode.as_str(),
            executor.model_profile,
            executor
                .reasoning_effort
                .map(ReasoningEffort::as_persistence_str),
            executor.selection_source,
            reviewer.provider_kind,
            reviewer.provider_account_id,
            reviewer
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string),
            reviewer.selection_mode.as_str(),
            reviewer.model_profile,
            reviewer
                .reasoning_effort
                .map(ReasoningEffort::as_persistence_str),
            reviewer.selection_source,
            policy.max_provider_continuations,
            policy.max_tool_calls,
            policy.max_active_minutes,
            policy.progress_audit_interval,
            policy.max_automatic_retries,
            policy.max_review_rounds,
            task.workspace_id.as_str(),
            workspace_name,
            workspace_description,
            task.project_id.as_ref().map(ProjectId::as_str),
            project_context.as_ref().map(|value| value.0.as_str()),
            project_context.as_ref().map(|value| value.1.as_str()),
            request.event.actor_id,
        ],
    )?;
    for criterion in request.criteria {
        let criterion_id = if let Some(criterion_id) = &criterion.criterion_id {
            if !criterion_id.starts_with("criterion:") {
                return Err(StoreError::Work(WorkDomainError::InvalidInput {
                    field: "contract.criteria.criterion_id",
                    message: "criterion id must use the criterion: namespace".to_string(),
                }));
            }
            criterion_id.clone()
        } else {
            allocate_id("criterion")
        };
        transaction.execute(
            "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description, expected_evidence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![criterion_id, contract_id.as_str(), criterion.ordinal, criterion.description, criterion.expected_evidence],
        )?;
    }
    transaction.execute(
        "UPDATE tasks SET current_contract_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1",
        params![task.task_id.as_str(), contract_id.as_str()],
    )?;
    let payload = WorkEventPayload::contract_created(
        contract_id.clone(),
        u32::try_from(version).map_err(|_| StoreError::InvariantViolation {
            message: "contract version overflow".to_string(),
        })?,
        task.generation,
        request.origin,
        request.complexity,
        u32::try_from(request.criteria.len()).map_err(|_| StoreError::InvariantViolation {
            message: "criteria count overflow".to_string(),
        })?,
        request.supersedes_contract_id.cloned(),
    )
    .map_err(StoreError::Work)?;
    let event = append_work_event_tx(
        transaction,
        request.event.scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            None,
        ),
        payload,
    )?;
    Ok((contract_id, event))
}

fn select_pool(
    transaction: &Transaction<'_>,
    complexity: TaskComplexity,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let complexity_str = complexity.as_str();
    let id: String = transaction.query_row(
        "SELECT pool_entry_id FROM task_model_pool_entries WHERE complexity = ?1 AND enabled = 1 ORDER BY sort_order, label, pool_entry_id LIMIT 1",
        [complexity_str],
        |row| row.get(0),
    )?;
    pool_selection_tx(transaction, complexity, &id)
}
