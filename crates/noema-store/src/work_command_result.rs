//! Commit-time command snapshots retained for exact receipt replay.

use noema_tasks::{TaskId, WorkCommandResult};
use noema_workspaces::ProjectId;
use serde::{Deserialize, Serialize};

use crate::{StoreError, WorkTaskDetail, work_commands::helpers::CommandWrite};

/// Exact API-facing projection captured before the command transaction commits.
#[derive(Debug, Clone, PartialEq)]
pub struct CommittedWorkCommandResult {
    /// Stable semantic command result retained for runtime callers.
    pub result: WorkCommandResult,
    /// Full task projection from the command transaction, when task-targeting.
    pub task_detail: Option<WorkTaskDetail>,
    /// Whether project creation adopted an existing working-folder document.
    pub project_document_adopted: Option<bool>,
}

/// Compact response retained in receipt JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReceiptResponse {
    pub task_id: Option<TaskId>,
    pub project_id: Option<ProjectId>,
    pub gate_id: Option<noema_tasks::TaskGateId>,
    pub run_id: Option<String>,
    pub event_id: noema_tasks::WorkEventId,
    pub event_sequence: u64,
    #[serde(default)]
    pub task_snapshot: Option<noema_tasks::TaskRecord>,
    #[serde(default)]
    pub task_detail_snapshot: Option<WorkTaskDetail>,
    #[serde(default)]
    pub project_snapshot: Option<noema_workspaces::ProjectRecord>,
    #[serde(default)]
    pub delegate_source_fingerprint: Option<String>,
    #[serde(default)]
    pub capture_source_fingerprint: Option<String>,
}

impl From<&CommandWrite> for ReceiptResponse {
    fn from(value: &CommandWrite) -> Self {
        Self {
            task_id: value.task_id.clone(),
            project_id: value.project_id.clone(),
            gate_id: value.gate_id.clone(),
            run_id: value.run_id.clone(),
            event_id: value.event_id.clone(),
            event_sequence: value.event_sequence,
            task_snapshot: value.task_snapshot.clone(),
            task_detail_snapshot: value.task_detail_snapshot.clone(),
            project_snapshot: value.project_snapshot.clone(),
            delegate_source_fingerprint: None,
            capture_source_fingerprint: None,
        }
    }
}

impl From<ReceiptResponse> for CommandWrite {
    fn from(value: ReceiptResponse) -> Self {
        Self {
            task_id: value.task_id,
            project_id: value.project_id,
            gate_id: value.gate_id,
            run_id: value.run_id,
            event_id: value.event_id,
            event_sequence: value.event_sequence,
            task_snapshot: value.task_snapshot,
            task_detail_snapshot: value.task_detail_snapshot,
            project_snapshot: value.project_snapshot,
        }
    }
}

pub(crate) async fn materialize_result(
    _store: &crate::NoemaStore,
    write: CommandWrite,
) -> Result<WorkCommandResult, StoreError> {
    Ok(materialize_committed_result(write)?.result)
}

pub(crate) fn materialize_committed_result(
    write: CommandWrite,
) -> Result<CommittedWorkCommandResult, StoreError> {
    let task_detail = require_task_snapshot(&write)?;
    let task = task_detail.as_ref().map(|detail| detail.task.clone());
    Ok(CommittedWorkCommandResult {
        result: WorkCommandResult {
            task,
            project: write.project_snapshot,
            gate_id: write.gate_id,
            run_id: write.run_id,
            event_id: write.event_id,
            event_sequence: write.event_sequence,
        },
        task_detail,
        project_document_adopted: None,
    })
}

fn require_task_snapshot(write: &CommandWrite) -> Result<Option<WorkTaskDetail>, StoreError> {
    match (&write.task_id, &write.task_detail_snapshot) {
        (Some(task_id), None) => Err(StoreError::InvariantViolation {
            message: format!("command result for task {task_id} has no commit-time detail"),
        }),
        (None, Some(_)) => Err(StoreError::InvariantViolation {
            message: "project command result unexpectedly retained task detail".to_string(),
        }),
        (_, detail) => Ok(detail.clone()),
    }
}
