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
    /// An approved result completed successfully.
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

    /// Whether this stage appears on the board.
    #[must_use]
    pub const fn board_visible(self) -> bool {
        !matches!(self, Self::TerminalCancelled)
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
    pub name: String,
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
                message: "cancelled history is hidden from the board and other stages are visible"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Validate that a stage belongs to the one code-owned workflow.
    /// # Errors
    /// Returns [`WorkDomainError::WorkflowMismatch`] for a different identity.
    pub fn belongs_to(&self, workflow_id: &WorkflowId) -> Result<(), WorkDomainError> {
        if &self.workflow_id == workflow_id {
            Ok(())
        } else {
            Err(WorkDomainError::WorkflowMismatch)
        }
    }
}

/// Return the fixed Personal workflow definition.
#[must_use]
pub fn personal_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        workflow_id: WorkflowId::new(PERSONAL_WORKFLOW_ID).expect("fixed workflow id"),
        name: "Personal workflow".to_string(),
    }
}

/// Return the six fixed Personal stage definitions in ordinal order.
#[must_use]
pub fn personal_stages() -> Vec<WorkflowStage> {
    let workflow_id = WorkflowId::new(PERSONAL_WORKFLOW_ID).expect("fixed workflow id");
    #[rustfmt::skip]
    let definitions = [
        (PERSONAL_INBOX_STAGE_ID, "inbox", "Inbox", 10, WorkflowStageBehavior::Intake),
        (PERSONAL_QUEUE_STAGE_ID, "queue", "Queue", 20, WorkflowStageBehavior::Dispatch),
        (PERSONAL_DOING_STAGE_ID, "doing", "Doing", 30, WorkflowStageBehavior::Active),
        (PERSONAL_WAITING_STAGE_ID, "waiting", "Waiting", 40, WorkflowStageBehavior::HumanGate),
        (PERSONAL_DONE_STAGE_ID, "done", "Done", 50, WorkflowStageBehavior::TerminalSuccess),
        (PERSONAL_CANCELLED_STAGE_ID, "cancelled", "Cancelled", 60, WorkflowStageBehavior::TerminalCancelled),
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

/// Resolve one code-owned Personal stage.
/// # Errors
/// Returns [`WorkDomainError`] when the stage is not part of the fixed workflow.
pub fn personal_stage(stage_id: &WorkflowStageId) -> Result<WorkflowStage, WorkDomainError> {
    personal_stages()
        .into_iter()
        .find(|stage| stage.stage_id == *stage_id)
        .ok_or_else(|| {
            invalid_input(
                "task.stage_id",
                "stage is not part of the Personal workflow",
            )
        })
}
