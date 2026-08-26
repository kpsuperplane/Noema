//! Typed queued-run persistence shared by Work commands and reconciliation.

use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{
    AcpExecutorLaunch, AgentRunRecord, RunKind, TaskComplexity, TaskExecutionPolicy,
    TaskExecutorBackend, TaskExecutorSelection, WorkDomainError, WorkEventRecord,
};
use rusqlite::{Transaction, params};

use super::{CommandEventContext, TaskState, load_policy_tx};
use crate::{
    StoreError,
    ids::{allocate_id, allocate_instance_name},
    provider_selections::prove_selection_ready,
    tasks::provider_selection::{pool_selection_tx, reviewer_preference_tx},
    work_events::append_work_event_tx,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueueRun<'a> {
    pub run_kind: RunKind,
    pub planner_complexity: Option<TaskComplexity>,
    pub review_round: u32,
    pub attempt_index: u32,
    pub parent_run_id: Option<&'a str>,
    pub event: CommandEventContext<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueuePinnedChildRun<'a> {
    pub attempt_index: u32,
    pub event: CommandEventContext<'a>,
}

pub(crate) fn queue_run_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    task: &TaskState,
    request: QueueRun<'_>,
) -> Result<(String, WorkEventRecord), StoreError> {
    let model = if request.run_kind == RunKind::Reviewer {
        reviewer_preference_tx(transaction)?
    } else {
        let complexity = if request.run_kind == RunKind::Planner {
            request.planner_complexity.unwrap_or(TaskComplexity::Medium)
        } else {
            if request.planner_complexity.is_some() {
                return Err(StoreError::Work(WorkDomainError::InvalidInput {
                    field: "run.planner_complexity",
                    message: "planner complexity applies only to Planner runs".to_string(),
                }));
            }
            task.execution_complexity.unwrap_or(TaskComplexity::Medium)
        };
        let pool_entry_id: String = transaction.query_row(
            "SELECT pool_entry_id FROM task_model_pool_entries WHERE complexity = ?1 AND enabled = 1 ORDER BY sort_order, label, pool_entry_id LIMIT 1",
            [complexity.as_str()],
            |row| row.get(0),
        )?;
        pool_selection_tx(transaction, complexity, &pool_entry_id)?
    };
    let model = model
        .normalized_for_persistence()
        .map_err(|_| StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
    let _ready = prove_selection_ready(&model, registry)?;
    let policy = load_policy_tx(transaction)?;
    policy.validated().map_err(StoreError::Work)?;
    let (executor, effective_cwd) = run_executor_tx(transaction, task, request.run_kind)?;
    insert_run_snapshot_tx(
        transaction,
        task,
        &request,
        &model,
        &policy,
        &executor,
        effective_cwd.as_deref(),
    )
}

pub(crate) fn queue_pinned_child_run_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    task: &TaskState,
    parent: &AgentRunRecord,
    request: QueuePinnedChildRun<'_>,
) -> Result<(String, WorkEventRecord), StoreError> {
    if parent.task_id != task.task_id || parent.task_generation != task.generation {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    let model = parent
        .model
        .clone()
        .normalized_for_persistence()
        .map_err(|_| StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
    let _ready = prove_selection_ready(&model, registry)?;
    let policy = parent
        .execution_policy
        .validated()
        .map_err(StoreError::Work)?;
    let request = QueueRun {
        run_kind: parent.run_kind,
        planner_complexity: None,
        review_round: parent.review_round,
        attempt_index: request.attempt_index,
        parent_run_id: Some(&parent.run_id),
        event: request.event,
    };
    insert_run_snapshot_tx(
        transaction,
        task,
        &request,
        &model,
        &policy,
        &parent.executor,
        parent.effective_cwd.as_deref(),
    )
}

fn run_executor_tx(
    transaction: &Transaction<'_>,
    task: &TaskState,
    run_kind: RunKind,
) -> Result<(TaskExecutorSelection, Option<String>), StoreError> {
    if run_kind != RunKind::Executor {
        let agent_id = if run_kind == RunKind::Reviewer {
            noema_tasks::TASK_REVIEWER_AGENT_ID
        } else {
            noema_tasks::TASK_EXECUTOR_AGENT_ID
        };
        return Ok((
            TaskExecutorSelection {
                agent_id: agent_id.to_string(),
                backend: TaskExecutorBackend::Provider,
                acp: None,
            },
            None,
        ));
    }
    let (backend, acp) = if task.executor_agent_id == noema_tasks::TASK_EXECUTOR_AGENT_ID {
        (TaskExecutorBackend::Provider, None)
    } else {
        let (command, arguments, revision, enabled) = transaction.query_row(
            "SELECT command, arguments_json, connection_revision, enabled FROM acp_agents WHERE agent_id = ?1",
            [task.executor_agent_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, bool>(3)?)),
        )?;
        if !enabled {
            return Err(StoreError::Work(WorkDomainError::ConfigurationUnavailable));
        }
        (
            TaskExecutorBackend::Acp,
            Some(AcpExecutorLaunch {
                connection_revision: u64::try_from(revision).map_err(|_| {
                    StoreError::InvariantViolation {
                        message: "ACP connection revision exceeds supported range".to_string(),
                    }
                })?,
                command,
                arguments: serde_json::from_str(&arguments)?,
            }),
        )
    };
    let project_folder = task
        .project_id
        .as_ref()
        .map(|project_id| {
            transaction.query_row(
                "SELECT folder FROM projects WHERE project_id = ?1",
                [project_id.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
        })
        .transpose()?
        .flatten();
    let cwd = task
        .cwd_override
        .as_ref()
        .map(|directory| {
            std::path::Path::new(directory)
                .join(&task.task_directory)
                .to_string_lossy()
                .into_owned()
        })
        .or_else(|| {
            project_folder.map(|folder| {
                std::path::Path::new(&folder)
                    .join(&task.task_directory)
                    .to_string_lossy()
                    .into_owned()
            })
        });
    Ok((
        TaskExecutorSelection {
            agent_id: task.executor_agent_id.clone(),
            backend,
            acp,
        },
        cwd,
    ))
}

pub(crate) fn provider_route_unavailable(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::ProviderInstanceKeyMissing
            | StoreError::ProviderInstanceKeyMismatch { .. }
            | StoreError::ProviderInstanceUnavailable { .. }
            | StoreError::ConfiguredDefaultUnresolvable { .. }
            | StoreError::Work(WorkDomainError::ConfigurationUnavailable)
    )
}

pub(crate) fn refresh_run_settings_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    task: &TaskState,
    run: &AgentRunRecord,
) -> Result<(), StoreError> {
    let model = if run.run_kind == RunKind::Reviewer {
        reviewer_preference_tx(transaction)?
    } else {
        let complexity = task.execution_complexity.unwrap_or(TaskComplexity::Medium);
        let pool_entry_id: String = transaction.query_row(
            "SELECT pool_entry_id FROM task_model_pool_entries WHERE complexity = ?1 AND enabled = 1 ORDER BY sort_order, label, pool_entry_id LIMIT 1",
            [complexity.as_str()],
            |row| row.get(0),
        )?;
        pool_selection_tx(transaction, complexity, &pool_entry_id)?
    };
    let model = model
        .normalized_for_persistence()
        .map_err(|_| StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
    let _ready = prove_selection_ready(&model, registry)?;
    let policy = load_policy_tx(transaction)?
        .validated()
        .map_err(StoreError::Work)?;
    let (executor, effective_cwd) = run_executor_tx(transaction, task, run.run_kind)?;
    let acp_revision = executor
        .acp
        .as_ref()
        .map(|snapshot| snapshot.connection_revision);
    let acp_launch_json = executor
        .acp
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    transaction.execute(
        "UPDATE agent_runs SET agent_id = ?2, provider_kind = ?3, provider_account_id = ?4, provider_instance_key = ?5, selection_mode = ?6, model_profile = ?7, reasoning_effort = ?8, fast_mode = ?9, selection_source = ?10, max_provider_continuations = ?11, max_tool_calls = ?12, max_active_minutes = ?13, progress_audit_interval = ?14, max_automatic_retries = ?15, max_review_rounds = ?16, execution_backend_kind = ?17, acp_connection_revision = ?18, acp_launch_json = ?19, effective_cwd = ?20, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND status = 'leased'",
        params![
            run.run_id,
            executor.agent_id,
            model.provider_kind,
            model.provider_account_id,
            model.provider_instance_key.as_ref().map(ToString::to_string),
            model.selection_mode.as_str(),
            model.model_profile,
            model.reasoning_effort.map(|value| value.as_persistence_str()),
            model.fast_mode,
            model.selection_source,
            policy.max_provider_continuations,
            policy.max_tool_calls,
            policy.max_active_minutes,
            policy.progress_audit_interval,
            policy.max_automatic_retries,
            policy.max_review_rounds,
            executor.backend.as_str(),
            acp_revision,
            acp_launch_json,
            effective_cwd,
        ],
    )?;
    Ok(())
}

fn insert_run_snapshot_tx(
    transaction: &Transaction<'_>,
    task: &TaskState,
    request: &QueueRun<'_>,
    model: &ProviderSelectionSnapshot,
    policy: &TaskExecutionPolicy,
    executor: &TaskExecutorSelection,
    effective_cwd: Option<&str>,
) -> Result<(String, WorkEventRecord), StoreError> {
    let run_id = allocate_id("run");
    let instance_name = allocate_instance_name(transaction)?;
    let acp_revision = executor
        .acp
        .as_ref()
        .map(|snapshot| snapshot.connection_revision);
    let acp_launch_json = executor
        .acp
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    transaction.execute(
        r#"INSERT INTO agent_runs (
             run_id, instance_name, task_id, task_generation, run_kind, agent_id,
             attempt_index, review_round, parent_run_id, provider_kind, provider_account_id,
             provider_instance_key, selection_mode, model_profile,
             reasoning_effort, fast_mode, selection_source, max_provider_continuations,
             max_tool_calls, max_active_minutes, progress_audit_interval,
             max_automatic_retries, max_review_rounds, execution_backend_kind,
             acp_connection_revision, acp_launch_json, effective_cwd, status
           ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                     ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
                     ?23, ?24, ?25, ?26, ?27, 'queued')"#,
        params![
            run_id,
            instance_name,
            task.task_id.as_str(),
            task.generation,
            request.run_kind.as_str(),
            executor.agent_id,
            request.attempt_index,
            request.review_round,
            request.parent_run_id,
            model.provider_kind,
            model.provider_account_id,
            model
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string),
            model.selection_mode.as_str(),
            model.model_profile,
            model
                .reasoning_effort
                .map(ReasoningEffort::as_persistence_str),
            model.fast_mode,
            model.selection_source,
            policy.max_provider_continuations,
            policy.max_tool_calls,
            policy.max_active_minutes,
            policy.progress_audit_interval,
            policy.max_automatic_retries,
            policy.max_review_rounds,
            executor.backend.as_str(),
            acp_revision,
            acp_launch_json,
            effective_cwd,
        ],
    )?;
    transaction.execute(
        "UPDATE tasks SET latest_run_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1",
        params![task.task_id.as_str(), run_id],
    )?;
    let payload = noema_tasks::WorkEventPayload::run_queued(
        request.run_kind,
        task.generation,
        request.attempt_index,
        request.review_round,
        request.parent_run_id.map(ToOwned::to_owned),
    )
    .map_err(StoreError::Work)?;
    let event = append_work_event_tx(
        transaction,
        request.event.scope(
            &task.workspace_id,
            task.project_id.as_ref(),
            Some(&task.task_id),
            Some(&run_id),
        ),
        payload,
    )?;
    Ok((run_id, event))
}
