use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    TaskContractId, TaskGateId, TaskId, WorkDomainError, WorkflowId, WorkflowStageId,
    error::invalid_input,
};

/// Durable source/provenance classification for a captured task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSourceKind {
    /// Captured from the primary conversation without execution authorization.
    ChatCapture,
    /// Delegated from the primary conversation for asynchronous execution.
    ChatDelegate,
    /// Created through Work UI.
    WorkUi,
    /// Created by a system/reconciliation action.
    System,
}

impl Default for TaskSourceKind {
    fn default() -> Self {
        Self::System
    }
}

impl TaskSourceKind {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatCapture => "chat_capture",
            Self::ChatDelegate => "chat_delegate",
            Self::WorkUi => "work_ui",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for TaskSourceKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskSourceKind {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "chat_capture" => Ok(Self::ChatCapture),
            "chat_delegate" => Ok(Self::ChatDelegate),
            "work_ui" => Ok(Self::WorkUi),
            "system" => Ok(Self::System),
            other => Err(invalid_input(
                "task.provenance.source_kind",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Durable provenance linking a task to its source conversation/turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProvenance {
    /// Source kind.
    pub source_kind: TaskSourceKind,
    /// Source conversation identity, when applicable.
    pub conversation_id: Option<String>,
    /// Source turn identity, when applicable.
    pub turn_id: Option<String>,
    /// Source conversation item identity, when applicable.
    pub item_id: Option<String>,
    /// Source primary-agent tool call identity, when applicable.
    pub source_tool_call_id: Option<String>,
    /// Actor that captured or delegated the task.
    pub created_by_actor_id: String,
}

impl TaskProvenance {
    /// Normalize optional references and validate the actor identity.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let created_by_actor_id = required(
            &self.created_by_actor_id,
            "task.provenance.created_by_actor_id",
        )?;
        Ok(Self {
            source_kind: self.source_kind,
            conversation_id: normalize_optional(self.conversation_id.as_deref()),
            turn_id: normalize_optional(self.turn_id.as_deref()),
            item_id: normalize_optional(self.item_id.as_deref()),
            source_tool_call_id: normalize_optional(self.source_tool_call_id.as_deref()),
            created_by_actor_id,
        })
    }
}

/// Durable current task projection.  Stage is the only task-level workflow state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    /// Stable task identity.
    pub task_id: TaskId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Optional project container.
    pub project_id: Option<ProjectId>,
    /// Workflow definition owning the stage.
    pub workflow_id: WorkflowId,
    /// Current workflow stage.
    pub stage_id: WorkflowStageId,
    /// Human-visible concise title.
    pub title: String,
    /// Fuller captured request/description Markdown.
    pub description_markdown: String,
    /// Durable source/provenance.
    pub provenance: TaskProvenance,
    /// Monotonic execution-cycle fence.
    pub generation: u64,
    /// Optimistic current-projection revision.
    pub revision: u64,
    /// Current immutable execution contract, when admitted.
    pub current_contract_id: Option<TaskContractId>,
    /// Current open gate, when any.
    pub active_gate_id: Option<TaskGateId>,
    /// Latest run identity.
    pub latest_run_id: Option<String>,
    /// Latest executor submission identity.
    pub latest_submission_id: Option<String>,
    /// Latest reviewer decision identity.
    pub latest_review_id: Option<String>,
    /// Accepted submission identity after human acceptance.
    pub accepted_submission_id: Option<String>,
    /// Queue admission timestamp.
    pub queued_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Completion timestamp after acceptance.
    pub completed_at: Option<String>,
    /// Cancellation timestamp.
    pub cancelled_at: Option<String>,
}

impl TaskRecord {
    /// Normalize and retain task-owned text/provenance instead of discarding
    /// the normalized values after validation.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let mut normalized = self.clone();
        normalized.title = required(&normalized.title, "task.title")?;
        normalized.description_markdown = normalized.description_markdown.trim().to_string();
        normalized.provenance = normalized.provenance.normalized()?;
        if normalized.generation == 0 || normalized.revision == 0 {
            return Err(invalid_input(
                "task",
                "generation and revision must be positive",
            ));
        }
        if normalized.created_at.trim().is_empty() || normalized.updated_at.trim().is_empty() {
            return Err(invalid_input(
                "task.timestamp",
                "timestamps cannot be blank",
            ));
        }
        Ok(normalized)
    }

    /// Validate task-owned invariants without consulting persistence.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.normalized().map(|_| ())
    }

    /// Whether the task can edit its capture fields directly.
    #[must_use]
    pub fn can_edit_capture_fields(&self, stage_behavior: crate::WorkflowStageBehavior) -> bool {
        matches!(stage_behavior, crate::WorkflowStageBehavior::Intake)
    }
}

fn required(value: &str, field: &'static str) -> Result<String, WorkDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid_input(field, "value cannot be blank"))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_provenance_normalizes_optional_source_references() {
        let provenance = TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            conversation_id: Some(" conversation:1 ".to_string()),
            created_by_actor_id: " actor:human:local ".to_string(),
            ..Default::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(
            provenance.conversation_id.as_deref(),
            Some("conversation:1")
        );
        assert_eq!(provenance.created_by_actor_id, "actor:human:local");
    }
}
