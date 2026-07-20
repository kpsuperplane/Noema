use noema_workspaces::WorkspaceId;
use serde::{Deserialize, Serialize};

use crate::{WorkDomainError, WorkflowId, WorkflowStageId, error::invalid_input};

/// Seeded executable Personal workflow identity.
pub const PERSONAL_WORKFLOW_ID: &str = "workflow:personal:default";

/// Seeded Personal stage identities.
pub const PERSONAL_INBOX_STAGE_ID: &str = "stage:personal:inbox";
/// Seeded Personal Queue stage identity.
pub const PERSONAL_QUEUE_STAGE_ID: &str = "stage:personal:queue";
/// Seeded Personal Doing stage identity.
pub const PERSONAL_DOING_STAGE_ID: &str = "stage:personal:doing";
/// Seeded Personal Waiting stage identity.
pub const PERSONAL_WAITING_STAGE_ID: &str = "stage:personal:waiting";
/// Seeded Personal Done stage identity.
pub const PERSONAL_DONE_STAGE_ID: &str = "stage:personal:done";
/// Seeded Personal Archive stage identity.
pub const PERSONAL_ARCHIVE_STAGE_ID: &str = "stage:personal:archive";
/// Seeded Personal Cancelled stage identity.
pub const PERSONAL_CANCELLED_STAGE_ID: &str = "stage:personal:cancelled";

string_enum! {
/// Closed runtime behavior of a workflow stage.
pub enum WorkflowStageBehavior, "workflow_stage_behavior" {
    /// Captured work not yet authorized to run.
    Intake => "intake",
    /// Authorized work waiting for a worker.
    Dispatch => "dispatch",
    /// Planning, execution, or automated review is active.
    Active => "active",
    /// A human gate is open.
    HumanGate => "human_gate",
    /// A reviewer-approved result awaits human acceptance.
    Acceptance => "acceptance",
    /// Human accepted the result.
    TerminalSuccess => "terminal_success",
    /// Human cancelled the task.
    TerminalCancelled => "terminal_cancelled",
}
}

impl WorkflowStageBehavior {
    /// Whether this behavior is terminal history.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::TerminalSuccess | Self::TerminalCancelled)
    }

    /// Whether this stage appears in the active five-column board.
    #[must_use]
    pub const fn board_visible(self) -> bool {
        !self.is_terminal()
    }
}

#[allow(
    clippy::derivable_impls,
    reason = "the shared enum macro owns common derives"
)]
impl Default for WorkflowStageBehavior {
    fn default() -> Self {
        Self::Intake
    }
}

/// Durable workflow definition metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkflowDefinition {
    pub workflow_id: WorkflowId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub revision: u64,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkflowDefinition {
    /// Validate definition-owned fields.
    /// # Errors
    /// Returns [`WorkDomainError`] when the name or timestamps are blank or
    /// the optimistic revision is zero.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.name.trim().is_empty() {
            return Err(invalid_input("workflow.name", "name cannot be blank"));
        }
        if self.revision == 0 {
            return Err(invalid_input(
                "workflow.revision",
                "revision must be positive",
            ));
        }
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(invalid_input(
                "workflow.timestamp",
                "timestamps cannot be blank",
            ));
        }
        Ok(())
    }
}

/// Durable workflow stage definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkflowStage {
    pub stage_id: WorkflowStageId,
    pub workflow_id: WorkflowId,
    pub stable_key: String,
    pub display_name: String,
    pub ordinal: u32,
    pub system_behavior: WorkflowStageBehavior,
    pub board_visible: bool,
}

impl WorkflowStage {
    /// Validate stage definition invariants independent of persistence.
    /// # Errors
    /// Returns [`WorkDomainError`] when the name or timestamps are blank or
    /// the optimistic revision is zero.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.stable_key.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "workflow_stage",
                message: "stable key and display name cannot be blank".to_string(),
            });
        }
        if self.ordinal == 0 {
            return Err(WorkDomainError::InvalidInput {
                field: "workflow_stage.ordinal",
                message: "ordinal must be positive".to_string(),
            });
        }
        if self.board_visible != self.system_behavior.board_visible() {
            return Err(WorkDomainError::InvalidInput {
                field: "workflow_stage.board_visible",
                message: "terminal stages are history-only and other stages are board-visible"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Validate that a task stage belongs to the supplied workflow.
    /// # Errors
    /// Returns [`WorkDomainError::WorkflowMismatch`] when the workflow identities differ.
    pub fn belongs_to(&self, workflow_id: &WorkflowId) -> Result<(), WorkDomainError> {
        if &self.workflow_id == workflow_id {
            Ok(())
        } else {
            Err(WorkDomainError::WorkflowMismatch)
        }
    }
}

/// Return the seven fixed Personal stage definitions in ordinal order.
#[must_use]
pub fn personal_stages() -> Vec<WorkflowStage> {
    let workflow_id = WorkflowId::new(PERSONAL_WORKFLOW_ID).expect("fixed workflow id");
    #[rustfmt::skip]
    let definitions = [
        (PERSONAL_INBOX_STAGE_ID, "inbox", "Inbox", 10, WorkflowStageBehavior::Intake),
        (PERSONAL_QUEUE_STAGE_ID, "queue", "Queue", 20, WorkflowStageBehavior::Dispatch),
        (PERSONAL_DOING_STAGE_ID, "doing", "Doing", 30, WorkflowStageBehavior::Active),
        (PERSONAL_WAITING_STAGE_ID, "waiting", "Waiting", 40, WorkflowStageBehavior::HumanGate),
        (PERSONAL_DONE_STAGE_ID, "done", "Done", 50, WorkflowStageBehavior::Acceptance),
        (PERSONAL_ARCHIVE_STAGE_ID, "archive", "Archive", 60, WorkflowStageBehavior::TerminalSuccess),
        (PERSONAL_CANCELLED_STAGE_ID, "cancelled", "Cancelled", 70, WorkflowStageBehavior::TerminalCancelled),
    ];
    definitions
        .into_iter()
        .map(
            |(stage_id, stable_key, display_name, ordinal, system_behavior)| WorkflowStage {
                stage_id: WorkflowStageId::new(stage_id).expect("fixed stage id"),
                workflow_id: workflow_id.clone(),
                stable_key: stable_key.to_string(),
                display_name: display_name.to_string(),
                ordinal,
                system_behavior,
                board_visible: system_behavior.board_visible(),
            },
        )
        .collect()
}
