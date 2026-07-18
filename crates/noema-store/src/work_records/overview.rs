//! Typed board-bootstrap projections assembled by one Store read transaction.

use noema_tasks::{WorkflowDefinition, WorkflowStage};
use noema_workspaces::{ProjectId, WorkspaceId, WorkspaceRecord};

use super::{WorkPageSize, WorkTaskConnection};

/// One workflow and all of its stages in canonical ordinal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkWorkflowWithStages {
    /// Workflow definition.
    pub workflow: WorkflowDefinition,
    /// Strict stages ordered by `(ordinal, stage_id)`.
    pub stages: Vec<WorkflowStage>,
}

/// Active task count for one ordered workflow stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkStageTaskCount {
    /// Stage definition represented by this count.
    pub stage: WorkflowStage,
    /// Active tasks in the requested workspace/project scope.
    pub task_count: u64,
}

/// Bounded board-bootstrap query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkOverviewQuery {
    /// Required workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional project scope applied to counts and recent tasks.
    pub project_id: Option<ProjectId>,
    /// Bounded number of recent active task cards.
    pub first: WorkPageSize,
}

/// One transactionally coherent board bootstrap.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkOverview {
    /// Workspace record.
    pub workspace: WorkspaceRecord,
    /// Default workflow and every ordered stage.
    pub default_workflow: WorkWorkflowWithStages,
    /// Board-visible active stages and counts in workflow order.
    pub active_stage_counts: Vec<WorkStageTaskCount>,
    /// Bounded recent active cards in Store connection order.
    pub recent_tasks: WorkTaskConnection,
    /// Total active cards requiring human attention in the same scope.
    pub needs_you_count: u64,
}
