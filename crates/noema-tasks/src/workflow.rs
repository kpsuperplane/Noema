use std::{fmt, str::FromStr};

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
/// Seeded Personal Review stage identity.
pub const PERSONAL_REVIEW_STAGE_ID: &str = "stage:personal:review";
/// Seeded Personal Completed stage identity.
pub const PERSONAL_COMPLETED_STAGE_ID: &str = "stage:personal:completed";
/// Seeded Personal Cancelled stage identity.
pub const PERSONAL_CANCELLED_STAGE_ID: &str = "stage:personal:cancelled";

/// Closed runtime behavior of a workflow stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStageBehavior {
    /// Captured work not yet authorized to run.
    Intake,
    /// Authorized work waiting for a worker.
    Dispatch,
    /// Planning, execution, or automated review is active.
    Active,
    /// A human gate is open.
    HumanGate,
    /// A reviewer-approved result awaits human acceptance.
    Acceptance,
    /// Human accepted the result.
    TerminalSuccess,
    /// Human cancelled the task.
    TerminalCancelled,
}

impl WorkflowStageBehavior {
    /// Return the stable persisted value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intake => "intake",
            Self::Dispatch => "dispatch",
            Self::Active => "active",
            Self::HumanGate => "human_gate",
            Self::Acceptance => "acceptance",
            Self::TerminalSuccess => "terminal_success",
            Self::TerminalCancelled => "terminal_cancelled",
        }
    }

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

impl fmt::Display for WorkflowStageBehavior {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for WorkflowStageBehavior {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "intake" => Ok(Self::Intake),
            "dispatch" => Ok(Self::Dispatch),
            "active" => Ok(Self::Active),
            "human_gate" => Ok(Self::HumanGate),
            "acceptance" => Ok(Self::Acceptance),
            "terminal_success" => Ok(Self::TerminalSuccess),
            "terminal_cancelled" => Ok(Self::TerminalCancelled),
            other => Err(WorkDomainError::InvalidInput {
                field: "workflow_stage_behavior",
                message: format!("unknown value {other}"),
            }),
        }
    }
}

/// Durable workflow definition metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Stable workflow identity.
    pub workflow_id: WorkflowId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Human-visible name.
    pub name: String,
    /// Optimistic row revision.
    pub revision: u64,
    /// Whether this is the workspace default workflow.
    pub is_default: bool,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl WorkflowDefinition {
    /// Validate definition-owned fields.
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
pub struct WorkflowStage {
    /// Stable stage identity.
    pub stage_id: WorkflowStageId,
    /// Owning workflow.
    pub workflow_id: WorkflowId,
    /// Machine-owned definition key.
    pub stable_key: String,
    /// Display label; never a runtime authority.
    pub display_name: String,
    /// One-based display order.
    pub ordinal: u32,
    /// Closed runtime behavior.
    pub system_behavior: WorkflowStageBehavior,
    /// Whether the stage appears on the active board.
    pub board_visible: bool,
}

impl WorkflowStage {
    /// Validate stage definition invariants independent of persistence.
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
    pub fn belongs_to(&self, workflow_id: &WorkflowId) -> Result<(), WorkDomainError> {
        if &self.workflow_id == workflow_id {
            Ok(())
        } else {
            Err(WorkDomainError::WorkflowMismatch)
        }
    }
}

/// Return the seven fixed Personal stage definitions in ordinal order.
pub fn personal_stages() -> Vec<WorkflowStage> {
    let workflow_id = WorkflowId::new(PERSONAL_WORKFLOW_ID).expect("fixed workflow id");
    [
        (
            PERSONAL_INBOX_STAGE_ID,
            "inbox",
            "Inbox",
            10,
            WorkflowStageBehavior::Intake,
        ),
        (
            PERSONAL_QUEUE_STAGE_ID,
            "queue",
            "Queue",
            20,
            WorkflowStageBehavior::Dispatch,
        ),
        (
            PERSONAL_DOING_STAGE_ID,
            "doing",
            "Doing",
            30,
            WorkflowStageBehavior::Active,
        ),
        (
            PERSONAL_WAITING_STAGE_ID,
            "waiting",
            "Waiting",
            40,
            WorkflowStageBehavior::HumanGate,
        ),
        (
            PERSONAL_REVIEW_STAGE_ID,
            "review",
            "Review",
            50,
            WorkflowStageBehavior::Acceptance,
        ),
        (
            PERSONAL_COMPLETED_STAGE_ID,
            "completed",
            "Completed",
            60,
            WorkflowStageBehavior::TerminalSuccess,
        ),
        (
            PERSONAL_CANCELLED_STAGE_ID,
            "cancelled",
            "Cancelled",
            70,
            WorkflowStageBehavior::TerminalCancelled,
        ),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_workflow_has_exactly_seven_closed_behaviors() {
        let stages = personal_stages();
        assert_eq!(stages.len(), 7);
        assert_eq!(
            stages
                .iter()
                .map(|stage| stage.system_behavior)
                .collect::<Vec<_>>(),
            vec![
                WorkflowStageBehavior::Intake,
                WorkflowStageBehavior::Dispatch,
                WorkflowStageBehavior::Active,
                WorkflowStageBehavior::HumanGate,
                WorkflowStageBehavior::Acceptance,
                WorkflowStageBehavior::TerminalSuccess,
                WorkflowStageBehavior::TerminalCancelled,
            ]
        );
        assert_eq!(stages.iter().filter(|stage| stage.board_visible).count(), 5);
    }

    #[test]
    fn stage_behavior_parsing_fails_closed_and_stage_ownership_is_explicit() {
        assert!("Doing".parse::<WorkflowStageBehavior>().is_err());
        assert!("active".parse::<WorkflowStageBehavior>().is_ok());
        let stage = &personal_stages()[0];
        let other = WorkflowId::new("workflow:other").unwrap();
        assert!(stage.belongs_to(&other).is_err());
    }
}
