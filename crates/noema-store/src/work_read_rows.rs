use std::str::FromStr;

use noema_tasks::{
    AgentRunRecord, GateResolutionKind, RunKind, TaskGateId, TaskGateKind, TaskGateRecord,
    TaskGateState, TaskId, TaskProvenance, TaskRecoveryReason, TaskSourceKind, WorkflowDefinition,
    WorkflowId, WorkflowStage, WorkflowStageBehavior, WorkflowStageId,
};
use noema_workspaces::{ProjectId, ProjectRecord, WorkspaceId, WorkspaceRecord};
use rusqlite::{OptionalExtension, Row, Transaction, types::Type};

use super::{PROJECT_COLUMNS, decode_project_record};
use crate::{
    StoreError, WorkTaskAttention, WorkTaskValidAction,
    sqlite::conversion_failure,
    work_row::{invalid, optional_id, positive_u32, positive_u64, strict_bool},
};

pub(crate) fn load_task(
    transaction: &Transaction<'_>,
    task_id: &TaskId,
) -> Result<Option<noema_tasks::TaskRecord>, StoreError> {
    transaction
        .query_row(
            "SELECT task_id, workspace_id, project_id, workflow_id, stage_id, title,
                    description_markdown, executor_agent_id, cwd_override,
                    authorization_context_json, source_kind,
                    source_conversation_id, source_turn_id, source_item_id, source_tool_call_id,
                    created_by_actor_id, generation, revision,
                    current_contract_id, active_gate_id, latest_run_id, latest_submission_id,
                    latest_review_id, completed_submission_id, scheduled_for, schedule_time_zone,
                    missed_run_policy, recurrence_id, recurrence_revision,
                    recurrence_scheduled_for, queued_at, created_at, updated_at,
                    completed_at, cancelled_at
             FROM tasks WHERE task_id = ?1 LIMIT 1",
            [task_id.as_str()],
            decode_task_record,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(crate) fn decode_task_record(row: &Row<'_>) -> rusqlite::Result<noema_tasks::TaskRecord> {
    let task = noema_tasks::TaskRecord {
        task_id: TaskId::new(row.get::<_, String>(0)?)
            .map_err(|e| conversion_failure(0, Type::Text, e))?,
        workspace_id: WorkspaceId::new(row.get::<_, String>(1)?)
            .map_err(|e| conversion_failure(1, Type::Text, e))?,
        project_id: row
            .get::<_, Option<String>>(2)?
            .map(ProjectId::new)
            .transpose()
            .map_err(|e| conversion_failure(2, Type::Text, e))?,
        workflow_id: WorkflowId::new(row.get::<_, String>(3)?)
            .map_err(|e| conversion_failure(3, Type::Text, e))?,
        stage_id: WorkflowStageId::new(row.get::<_, String>(4)?)
            .map_err(|e| conversion_failure(4, Type::Text, e))?,
        title: row.get(5)?,
        description_markdown: row.get(6)?,
        executor_agent_id: row.get(7)?,
        cwd_override: row.get(8)?,
        authorization_context: serde_json::from_str(&row.get::<_, String>(9)?)
            .map_err(|e| conversion_failure(9, Type::Text, e))?,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::from_str(&row.get::<_, String>(10)?)
                .map_err(|e| conversion_failure(10, Type::Text, e))?,
            conversation_id: row.get(11)?,
            turn_id: row.get(12)?,
            item_id: row.get(13)?,
            source_tool_call_id: row.get(14)?,
            created_by_actor_id: row.get(15)?,
        },
        generation: positive_u64(row, 16)?,
        revision: positive_u64(row, 17)?,
        current_contract_id: optional_id(row, 18, noema_tasks::TaskContractId::new)?,
        active_gate_id: optional_id(row, 19, TaskGateId::new)?,
        latest_run_id: row.get(20)?,
        latest_submission_id: row.get(21)?,
        latest_review_id: row.get(22)?,
        completed_submission_id: row.get(23)?,
        scheduled_for: row.get(24)?,
        schedule_time_zone: row.get(25)?,
        missed_run_policy: row
            .get::<_, Option<String>>(26)?
            .map(|value| value.parse())
            .transpose()
            .map_err(|e| conversion_failure(26, Type::Text, e))?,
        recurrence_id: optional_id(row, 27, noema_tasks::TaskRecurrenceId::new)?,
        recurrence_revision: row
            .get::<_, Option<i64>>(28)?
            .map(u64::try_from)
            .transpose()
            .map_err(|e| conversion_failure(28, Type::Integer, e))?,
        recurrence_scheduled_for: row.get(29)?,
        queued_at: row.get(30)?,
        created_at: row.get(31)?,
        updated_at: row.get(32)?,
        completed_at: row.get(33)?,
        cancelled_at: row.get(34)?,
    };
    let normalized = task
        .normalized()
        .map_err(|e| conversion_failure(0, Type::Text, e))?;
    if normalized != task {
        return Err(noncanonical(0, "task row is not canonically normalized"));
    }
    Ok(task)
}

pub(crate) fn load_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
) -> Result<WorkspaceRecord, StoreError> {
    let record = transaction
        .query_row(
            "SELECT workspace_id, name, description, is_personal, archived_at, revision,
                    created_at, updated_at FROM workspaces WHERE workspace_id = ?1 LIMIT 1",
            [workspace_id.as_str()],
            |row| {
                Ok(WorkspaceRecord {
                    workspace_id: WorkspaceId::new(row.get::<_, String>(0)?)
                        .map_err(|e| conversion_failure(0, Type::Text, e))?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    is_personal: strict_bool(row, 3)?,
                    archived_at: row.get(4)?,
                    revision: positive_u64(row, 5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| missing_link("workspace", workspace_id.as_str()))?;
    let normalized = record
        .clone()
        .normalized()
        .map_err(|e| StoreError::Sqlite(conversion_failure(0, Type::Text, e)))?;
    if normalized != record {
        return Err(StoreError::Sqlite(noncanonical(
            0,
            "workspace row is not canonically normalized",
        )));
    }
    Ok(record)
}

pub(crate) fn load_project(
    transaction: &Transaction<'_>,
    project_id: &ProjectId,
) -> Result<ProjectRecord, StoreError> {
    load_project_optional(transaction, project_id)?
        .ok_or_else(|| missing_link("project", project_id.as_str()))
}

pub(crate) fn load_project_optional(
    transaction: &Transaction<'_>,
    project_id: &ProjectId,
) -> Result<Option<ProjectRecord>, StoreError> {
    Ok(transaction
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE project_id = ?1 LIMIT 1"),
            [project_id.as_str()],
            decode_project_record,
        )
        .optional()?)
}

pub(crate) fn load_workflow(
    transaction: &Transaction<'_>,
    workflow_id: &WorkflowId,
) -> Result<WorkflowDefinition, StoreError> {
    let record = transaction
        .query_row(
            "SELECT workflow_id, workspace_id, name, revision, is_default, created_at, updated_at
             FROM workflow_definitions WHERE workflow_id = ?1 LIMIT 1",
            [workflow_id.as_str()],
            |row| {
                Ok(WorkflowDefinition {
                    workflow_id: WorkflowId::new(row.get::<_, String>(0)?)
                        .map_err(|e| conversion_failure(0, Type::Text, e))?,
                    workspace_id: WorkspaceId::new(row.get::<_, String>(1)?)
                        .map_err(|e| conversion_failure(1, Type::Text, e))?,
                    name: row.get(2)?,
                    revision: positive_u64(row, 3)?,
                    is_default: strict_bool(row, 4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| missing_link("workflow", workflow_id.as_str()))?;
    record.validate().map_err(StoreError::Work)?;
    Ok(record)
}

pub(crate) fn load_stage(
    transaction: &Transaction<'_>,
    stage_id: &WorkflowStageId,
) -> Result<WorkflowStage, StoreError> {
    let record = transaction
        .query_row(
            "SELECT stage_id, workflow_id, stable_key, display_name, ordinal, system_behavior,
                    board_visible FROM workflow_stages WHERE stage_id = ?1 LIMIT 1",
            [stage_id.as_str()],
            |row| decode_stage_record(row, 0),
        )
        .optional()?
        .ok_or_else(|| missing_link("workflow stage", stage_id.as_str()))?;
    record.validate().map_err(StoreError::Work)?;
    Ok(record)
}

pub(crate) fn decode_stage_record(row: &Row<'_>, offset: usize) -> rusqlite::Result<WorkflowStage> {
    let record = WorkflowStage {
        stage_id: WorkflowStageId::new(row.get::<_, String>(offset)?)
            .map_err(|e| conversion_failure(offset, Type::Text, e))?,
        workflow_id: WorkflowId::new(row.get::<_, String>(offset + 1)?)
            .map_err(|e| conversion_failure(offset + 1, Type::Text, e))?,
        stable_key: row.get(offset + 2)?,
        display_name: row.get(offset + 3)?,
        ordinal: positive_u32(row, offset + 4)?,
        system_behavior: WorkflowStageBehavior::from_str(&row.get::<_, String>(offset + 5)?)
            .map_err(|e| conversion_failure(offset + 5, Type::Text, e))?,
        board_visible: strict_bool(row, offset + 6)?,
    };
    record
        .validate()
        .map_err(|e| conversion_failure(offset, Type::Text, e))?;
    Ok(record)
}

pub(crate) fn load_current_run(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<AgentRunRecord, StoreError> {
    crate::work_runs::rows::load_run_tx(transaction, run_id)?
        .ok_or_else(|| missing_link("run", run_id))
}

pub(crate) fn load_active_gate(
    transaction: &Transaction<'_>,
    gate_id: &TaskGateId,
) -> Result<TaskGateRecord, StoreError> {
    let gate = load_gate(transaction, gate_id)?;
    if gate.state != TaskGateState::Open {
        return Err(StoreError::InvariantViolation {
            message: format!("task active gate is not open: {}", gate.gate_id),
        });
    }
    Ok(gate)
}

pub(crate) fn load_gate(
    transaction: &Transaction<'_>,
    gate_id: &TaskGateId,
) -> Result<TaskGateRecord, StoreError> {
    load_gate_optional(transaction, gate_id)?.ok_or_else(|| missing_link("gate", gate_id.as_str()))
}

pub(crate) fn load_gate_optional(
    transaction: &Transaction<'_>,
    gate_id: &TaskGateId,
) -> Result<Option<TaskGateRecord>, StoreError> {
    let gate = transaction
        .query_row(
            "SELECT gate_id, task_id, task_generation, contract_id, gate_kind, gate_state,
                    recovery_reason, retry_run_kind, prompt_markdown, context_markdown,
                    suggested_answers_json,
                    opened_by_actor_id, originating_run_id, resolved_by_actor_id,
                    resolution_message_id, opened_at, resolved_at
             FROM task_gates WHERE gate_id = ?1 LIMIT 1",
            [gate_id.as_str()],
            decode_gate,
        )
        .optional()?;
    if let Some(gate) = &gate {
        gate.validate().map_err(StoreError::Work)?;
    }
    Ok(gate)
}

pub(crate) fn decode_gate(row: &Row<'_>) -> rusqlite::Result<TaskGateRecord> {
    Ok(TaskGateRecord {
        gate_id: TaskGateId::new(row.get::<_, String>(0)?)
            .map_err(|e| conversion_failure(0, Type::Text, e))?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|e| conversion_failure(1, Type::Text, e))?,
        task_generation: positive_u64(row, 2)?,
        contract_id: row
            .get::<_, Option<String>>(3)?
            .map(noema_tasks::TaskContractId::new)
            .transpose()
            .map_err(|e| conversion_failure(3, Type::Text, e))?,
        kind: TaskGateKind::from_str(&row.get::<_, String>(4)?)
            .map_err(|e| conversion_failure(4, Type::Text, e))?,
        state: TaskGateState::from_str(&row.get::<_, String>(5)?)
            .map_err(|e| conversion_failure(5, Type::Text, e))?,
        recovery_reason: row
            .get::<_, Option<String>>(6)?
            .map(|value| TaskRecoveryReason::from_str(&value))
            .transpose()
            .map_err(|e| conversion_failure(6, Type::Text, e))?,
        retry_run_kind: row
            .get::<_, Option<String>>(7)?
            .map(|value| RunKind::from_str(&value))
            .transpose()
            .map_err(|e| conversion_failure(7, Type::Text, e))?,
        prompt_markdown: row.get(8)?,
        context_markdown: row.get(9)?,
        suggested_answers: serde_json::from_str(&row.get::<_, String>(10)?)
            .map_err(|e| conversion_failure(10, Type::Text, e))?,
        opened_by_actor_id: row.get(11)?,
        originating_run_id: row.get(12)?,
        resolved_by_actor_id: row.get(13)?,
        resolution_message_id: row
            .get::<_, Option<String>>(14)?
            .map(noema_tasks::TaskMessageId::new)
            .transpose()
            .map_err(|e| conversion_failure(14, Type::Text, e))?,
        opened_at: row.get(15)?,
        resolved_at: row.get(16)?,
    })
}

pub(crate) fn validate_current_links(
    task: &noema_tasks::TaskRecord,
    run: Option<&AgentRunRecord>,
    gate: Option<&TaskGateRecord>,
) -> Result<(), StoreError> {
    if run.is_some_and(|run| run.task_id != task.task_id || run.task_generation != task.generation)
        || gate.is_some_and(|gate| {
            gate.task_id != task.task_id || gate.task_generation != task.generation
        })
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "task current pointers cross a task or generation fence: {}",
                task.task_id
            ),
        });
    }
    Ok(())
}

pub(super) fn derive_attention_actions(
    task: &noema_tasks::TaskRecord,
    behavior: WorkflowStageBehavior,
    gate: Option<&TaskGateRecord>,
) -> (Option<WorkTaskAttention>, Vec<WorkTaskValidAction>) {
    use WorkTaskValidAction as A;
    let attention = match (behavior, gate.map(|gate| gate.kind)) {
        (WorkflowStageBehavior::HumanGate, Some(TaskGateKind::Clarification)) => {
            Some(WorkTaskAttention::Clarification)
        }
        (WorkflowStageBehavior::HumanGate, Some(TaskGateKind::Approval)) => {
            Some(WorkTaskAttention::Approval)
        }
        (WorkflowStageBehavior::HumanGate, Some(TaskGateKind::Recovery)) => {
            Some(WorkTaskAttention::Recovery)
        }
        _ => None,
    };
    let actions = match behavior {
        WorkflowStageBehavior::Intake if task.recurrence_id.is_some() => vec![A::Cancel],
        WorkflowStageBehavior::Intake if task.scheduled_for.is_some() => {
            vec![A::Edit, A::Reschedule, A::Unschedule, A::Cancel]
        }
        WorkflowStageBehavior::Intake => vec![A::Edit, A::Queue, A::Schedule, A::Cancel],
        WorkflowStageBehavior::Dispatch | WorkflowStageBehavior::Active => vec![A::Cancel],
        WorkflowStageBehavior::TerminalSuccess => vec![A::Reopen],
        WorkflowStageBehavior::TerminalCancelled => vec![A::Reopen],
        WorkflowStageBehavior::HumanGate => gate.map_or_else(Vec::new, recovery_actions),
    };
    (attention, actions)
}

fn recovery_actions(gate: &TaskGateRecord) -> Vec<WorkTaskValidAction> {
    use WorkTaskValidAction as A;
    let mut actions = Vec::with_capacity(3);
    for (resolution, action) in [
        (GateResolutionKind::Answer, A::Answer),
        (GateResolutionKind::Retry, A::Retry),
    ] {
        if gate
            .kind
            .allows_resolution(gate.recovery_reason, gate.retry_run_kind, resolution)
        {
            actions.push(action);
        }
    }
    actions.push(A::Cancel);
    actions
}

fn noncanonical(index: usize, message: &'static str) -> rusqlite::Error {
    invalid(index, message)
}

pub(super) fn missing_link(kind: &str, id: &str) -> StoreError {
    StoreError::InvariantViolation {
        message: format!("task references missing {kind}: {id}"),
    }
}
