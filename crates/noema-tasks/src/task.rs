use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    TaskContractId, TaskGateId, TaskId, WorkDomainError, WorkflowId, WorkflowStageId,
    error::invalid_input,
    validation::{optional as normalize_optional, required},
};

string_enum! {
/// Durable source/provenance classification for a captured task.
pub enum TaskSourceKind, "task.provenance.source_kind" {
    /// Captured from the primary conversation without execution authorization.
    ChatCapture => "chat_capture",
    /// Delegated from the primary conversation for asynchronous execution.
    ChatDelegate => "chat_delegate",
    /// Created through Work UI.
    WorkUi => "work_ui",
    /// Created by a system/reconciliation action.
    System => "system",
}
}

#[allow(
    clippy::derivable_impls,
    reason = "the shared enum macro owns common derives"
)]
impl Default for TaskSourceKind {
    fn default() -> Self {
        Self::System
    }
}

/// Durable provenance linking a task to its source conversation/turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskProvenance {
    pub source_kind: TaskSourceKind,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub source_tool_call_id: Option<String>,
    pub created_by_actor_id: String,
}

impl TaskProvenance {
    /// Normalize optional references and validate the actor identity.
    /// # Errors
    /// Returns [`WorkDomainError`] when the creating actor identity is blank.
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
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskRecord {
    pub task_id: TaskId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub workflow_id: WorkflowId,
    pub stage_id: WorkflowStageId,
    pub title: String,
    pub description_markdown: String,
    pub provenance: TaskProvenance,
    pub generation: u64,
    pub revision: u64,
    pub current_contract_id: Option<TaskContractId>,
    pub active_gate_id: Option<TaskGateId>,
    pub latest_run_id: Option<String>,
    pub latest_submission_id: Option<String>,
    pub latest_review_id: Option<String>,
    pub accepted_submission_id: Option<String>,
    pub queued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
}

impl TaskRecord {
    /// Normalize and retain task-owned text/provenance instead of discarding
    /// the normalized values after validation.
    /// # Errors
    /// Returns [`WorkDomainError`] when the creating actor identity is blank.
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
    /// # Errors
    /// Returns [`WorkDomainError`] under the same conditions as [`Self::normalized`].
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.normalized().map(|_| ())
    }
}
