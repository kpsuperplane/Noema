//! Typed queued-run persistence shared by Work commands and reconciliation.

use noema_providers::{ProviderRegistry, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{
    AcpExecutorSnapshot, AgentRunRecord, RunKind, TaskComplexity, TaskContractId,
    TaskExecutionPolicy, TaskExecutorBackend, TaskExecutorSelection, WorkDomainError,
    WorkEventRecord,
};
use rusqlite::{Transaction, params};

use super::{
    CommandEventContext, TaskState, load_contract_model_tx, load_policy_tx, nonnegative_u32,
    positive_u32,
};
use crate::{
    StoreError,
    ids::{allocate_id, allocate_instance_name},
    provider_selections::prove_selection_ready,
    tasks::provider_selection::pool_selection_tx,
    work_events::append_work_event_tx,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueueRun<'a> {
    pub run_kind: RunKind,
    pub contract_id: Option<&'a TaskContractId>,
    pub planner_complexity: Option<TaskComplexity>,
    pub review_round: u32,
    pub attempt_index: u32,
    pub parent_run_id: Option<&'a str>,
    pub triggering_submission_id: Option<&'a str>,
    pub triggering_review_id: Option<&'a str>,
    pub event: CommandEventContext<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueuePinnedChildRun<'a> {
    pub attempt_index: u32,
    pub triggering_submission_id: Option<&'a str>,
    pub triggering_review_id: Option<&'a str>,
    pub event: CommandEventContext<'a>,
}

pub(crate) fn queue_run_tx(
    transaction: &Transaction<'_>,
    registry: &ProviderRegistry,
    task: &TaskState,
    request: QueueRun<'_>,
) -> Result<(String, WorkEventRecord), StoreError> {
    let model = if request.run_kind == RunKind::Planner {
        if request.contract_id.is_some() {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let complexity = request.planner_complexity.unwrap_or(TaskComplexity::Medium);
        let pool_entry_id: String = transaction.query_row(
            "SELECT pool_entry_id FROM task_model_pool_entries WHERE complexity = ?1 AND enabled = 1 ORDER BY sort_order, label, pool_entry_id LIMIT 1",
            [complexity.as_str()],
            |row| row.get(0),
        )?;
        pool_selection_tx(transaction, complexity, &pool_entry_id)?
    } else {
        if request.planner_complexity.is_some() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.planner_complexity",
                message: "planner complexity applies only to Planner runs".to_string(),
            }));
        }
        let contract_id = request
            .contract_id
            .ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
        load_contract_model_tx(
            transaction,
            contract_id,
            request.run_kind == RunKind::Reviewer,
        )?
    };
    let model = model
        .normalized_for_persistence()
        .map_err(|_| StoreError::Work(WorkDomainError::ConfigurationUnavailable))?;
    let _ready = prove_selection_ready(&model, registry)?;
    let policy = match request.contract_id {
        Some(contract_id) => contract_policy_tx(transaction, contract_id)?,
        None => load_policy_tx(transaction)?,
    };
    policy.validated().map_err(StoreError::Work)?;
    let (executor, effective_cwd) =
        run_executor_tx(transaction, request.run_kind, request.contract_id)?;
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
    if parent.task_id != task.task_id
        || parent.task_generation != task.generation
        || parent.contract_id != task.current_contract_id
    {
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
        contract_id: parent.contract_id.as_ref(),
        planner_complexity: None,
        review_round: parent.review_round,
        attempt_index: request.attempt_index,
        parent_run_id: Some(&parent.run_id),
        triggering_submission_id: request.triggering_submission_id,
        triggering_review_id: request.triggering_review_id,
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
    run_kind: RunKind,
    contract_id: Option<&TaskContractId>,
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
    let contract_id = contract_id.ok_or(StoreError::Work(WorkDomainError::ContractRequired))?;
    let (backend, agent_id, revision, launch_json, cwd) = transaction.query_row(
        "SELECT executor_backend_kind, executor_agent_id, executor_acp_connection_revision, executor_acp_launch_json, effective_cwd FROM task_execution_contracts WHERE contract_id = ?1",
        [contract_id.as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?)),
    )?;
    let backend = backend.parse().map_err(StoreError::Work)?;
    let acp = launch_json
        .map(|json| serde_json::from_str::<AcpExecutorSnapshot>(&json))
        .transpose()?;
    if acp
        .as_ref()
        .and_then(|snapshot| i64::try_from(snapshot.connection_revision).ok())
        != revision
    {
        return Err(StoreError::InvariantViolation {
            message: "contract ACP revision does not match launch snapshot".to_string(),
        });
    }
    Ok((
        TaskExecutorSelection {
            agent_id,
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

fn contract_policy_tx(
    transaction: &Transaction<'_>,
    contract_id: &TaskContractId,
) -> Result<TaskExecutionPolicy, StoreError> {
    let values = transaction
        .query_row(
            "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_contracts WHERE contract_id = ?1",
            [contract_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .map_err(StoreError::Sqlite)?;
    Ok(TaskExecutionPolicy {
        max_provider_continuations: positive_u32(values.0, "contract.max_provider_continuations")?,
        max_tool_calls: positive_u32(values.1, "contract.max_tool_calls")?,
        max_active_minutes: positive_u32(values.2, "contract.max_active_minutes")?,
        progress_audit_interval: positive_u32(values.3, "contract.progress_audit_interval")?,
        max_automatic_retries: nonnegative_u32(values.4, "contract.max_automatic_retries")?,
        max_review_rounds: positive_u32(values.5, "contract.max_review_rounds")?,
    })
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
             run_id, instance_name, task_id, task_generation, contract_id, run_kind, agent_id,
             attempt_index, review_round, parent_run_id, triggering_submission_id,
             triggering_review_id, provider_kind, provider_account_id,
             provider_instance_key, selection_mode, model_profile,
             reasoning_effort, fast_mode, selection_source, max_provider_continuations,
             max_tool_calls, max_active_minutes, progress_audit_interval,
             max_automatic_retries, max_review_rounds, execution_backend_kind,
             acp_connection_revision, acp_launch_json, effective_cwd, status
           ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24,
                     ?25, ?26, ?27, ?28, ?29, ?30, 'queued')"#,
        params![
            run_id,
            instance_name,
            task.task_id.as_str(),
            task.generation,
            request.contract_id.map(ToString::to_string),
            request.run_kind.as_str(),
            executor.agent_id,
            request.attempt_index,
            request.review_round,
            request.parent_run_id,
            request.triggering_submission_id,
            request.triggering_review_id,
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
        request.contract_id.cloned(),
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
