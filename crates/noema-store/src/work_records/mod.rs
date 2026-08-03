//! Store-owned Work projections and pagination contracts.
//!
//! Domain records remain owned by `noema-workspaces` and `noema-tasks`. This
//! module only composes them into read projections and defines durable-store
//! records whose lifecycle is specific to SQLite persistence.

mod cursor;
mod history;
mod overview;

use noema_tasks::{
    AgentRunRecord, TaskExecutionContract, TaskGateRecord, TaskId, TaskReviewRecord,
    TaskSubmissionRecord, WorkEventRecord, WorkReconciliationSnapshot, WorkflowStage,
    WorkflowStageBehavior,
};
use noema_workspaces::{ProjectId, ProjectRecord, WorkspaceId, WorkspaceRecord};
use serde::{Deserialize, Serialize};

pub use cursor::{ProjectCursor, WorkCursorError, WorkEventCursor, WorkPageSize, WorkTaskCursor};
pub use history::*;
pub use overview::*;

/// Stable board/history scope for a task connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkTaskScope {
    /// The five nonterminal Personal workflow stages.
    Active,
    /// Done and Cancelled history.
    Terminal,
    /// Both active work and terminal history, ordered by last update.
    All,
}

impl WorkTaskScope {
    /// Stable value included in the normalized query fingerprint.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Terminal => "terminal",
            Self::All => "all",
        }
    }
}

/// Bounded, keyset-paginated task query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTaskQuery {
    /// Required workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional project scope.
    pub project_id: Option<ProjectId>,
    /// Optional explicit stage filter.
    pub stage_ids: Vec<noema_tasks::WorkflowStageId>,
    /// Optional semantic stage-behavior filter.
    pub stage_behaviors: Vec<WorkflowStageBehavior>,
    /// Optional case-insensitive title/description search.
    pub text: Option<String>,
    /// Restrict results to human-attention cards.
    pub attention_only: bool,
    /// Active, terminal, or combined scope.
    pub scope: WorkTaskScope,
    /// Validated page size; defaults to 50 and never exceeds 100.
    pub first: WorkPageSize,
    /// Exclusive keyset cursor.
    pub after: Option<WorkTaskCursor>,
}

/// Bounded project query for one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectQuery {
    /// Required workspace scope.
    pub workspace_id: WorkspaceId,
    /// Whether archived projects are included.
    pub include_archived: bool,
    /// Validated page size.
    pub first: WorkPageSize,
    /// Exclusive keyset cursor.
    pub after: Option<ProjectCursor>,
}

/// Global Work ledger query in ascending event-sequence order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkEventQuery {
    /// Required authorization/workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional project filter.
    pub project_id: Option<ProjectId>,
    /// Optional task filter.
    pub task_id: Option<TaskId>,
    /// Optional run filter.
    pub run_id: Option<String>,
    /// Exclusive global cursor.
    pub after: Option<WorkEventCursor>,
    /// Validated page size.
    pub first: WorkPageSize,
}

/// Global Work ledger query in descending event-sequence order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkEventBeforeQuery {
    /// Required authorization/workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional project filter.
    pub project_id: Option<ProjectId>,
    /// Optional task filter.
    pub task_id: Option<TaskId>,
    /// Optional run filter.
    pub run_id: Option<String>,
    /// Exclusive upper-bound cursor; absent starts at the durable head.
    pub before: Option<WorkEventCursor>,
    /// Validated page size.
    pub first: WorkPageSize,
}

/// Human-facing actions currently legal for a task projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkTaskValidAction {
    /// Edit Inbox capture fields.
    Edit,
    /// Authorize the task for dispatch.
    Queue,
    /// Add future execution to an Inbox task.
    Schedule,
    /// Replace a one-time future execution.
    Reschedule,
    /// Return a one-time scheduled task to ordinary Inbox.
    Unschedule,
    /// Answer an open human gate.
    Answer,
    /// Retry an eligible recovery gate.
    Retry,
    /// Cancel active work.
    Cancel,
    /// Reopen terminal task history.
    Reopen,
}

/// Human-attention classification derived from durable stage/gate/review rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkTaskAttention {
    /// The task needs missing or ambiguous information.
    Clarification,
    /// The task needs a structured approval decision.
    Approval,
    /// The task is stopped in recovery and needs a human choice.
    Recovery,
}

/// One bounded task card with related rows loaded in the same query/batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTaskSummary {
    /// Canonical task projection.
    pub task: noema_tasks::TaskRecord,
    /// Authoritative owning workspace record.
    pub workspace: WorkspaceRecord,
    /// Authoritative current project record, when assigned.
    pub project: Option<ProjectRecord>,
    /// Current workflow stage definition.
    pub stage: WorkflowStage,
    /// Current runnable or leased run, if any.
    pub current_run: Option<AgentRunRecord>,
    /// Current open gate, if any.
    pub active_gate: Option<TaskGateRecord>,
    /// Most recent review, if any.
    pub latest_review: Option<TaskReviewRecord>,
    /// Derived human-attention classification.
    pub attention: Option<WorkTaskAttention>,
    /// Commands allowed by the current durable state.
    pub valid_actions: Vec<WorkTaskValidAction>,
}

/// Task detail current projection. Immutable histories use independent
/// cursor-backed reads and are intentionally not embedded without bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkTaskDetail {
    /// Canonical task projection.
    pub task: noema_tasks::TaskRecord,
    /// Owning workspace record.
    pub workspace: WorkspaceRecord,
    /// Current project, if the task is assigned to one.
    pub project: Option<ProjectRecord>,
    /// Current workflow stage definition.
    pub stage: WorkflowStage,
    /// Immutable contract for the current generation, if any.
    pub current_contract: Option<TaskExecutionContract>,
    /// Current runnable or leased run, if any.
    pub current_run: Option<AgentRunRecord>,
    /// Current open gate, if any.
    pub active_gate: Option<TaskGateRecord>,
    /// Most recent immutable submission, if any.
    pub latest_submission: Option<TaskSubmissionRecord>,
    /// Reviewer-approved submission that completed the task.
    pub completed_submission: Option<TaskSubmissionRecord>,
    /// Most recent immutable review, if any.
    pub latest_review: Option<TaskReviewRecord>,
    /// Bounded recent human messages, newest first.
    pub messages: Vec<noema_tasks::TaskMessageRecord>,
    /// Bounded recent runs, newest first.
    pub runs: Vec<AgentRunRecord>,
    /// Bounded recent submissions, newest first.
    pub submissions: Vec<TaskSubmissionRecord>,
    /// Bounded recent reviews, newest first.
    pub reviews: Vec<TaskReviewRecord>,
    /// Bounded current task artifacts, newest first.
    pub artifacts: Vec<WorkTaskArtifact>,
    /// Derived human-attention classification.
    pub attention: Option<WorkTaskAttention>,
    /// Commands allowed by the current durable state.
    pub valid_actions: Vec<WorkTaskValidAction>,
}

/// Durable rows needed to derive and atomically apply one reconciliation step.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkReconciliationEnvelope {
    /// Canonical task projection used by reconciliation.
    pub task: noema_tasks::TaskRecord,
    /// Current workflow stage definition.
    pub stage: WorkflowStage,
    /// Immutable contract for the current generation, if any.
    pub current_contract: Option<TaskExecutionContract>,
    /// Current open gate, if any.
    pub active_gate: Option<TaskGateRecord>,
    /// Most recent run projection, if any.
    pub latest_run: Option<AgentRunRecord>,
    /// Most recent immutable submission, if any.
    pub latest_submission: Option<TaskSubmissionRecord>,
    /// Most recent immutable review, if any.
    pub latest_review: Option<TaskReviewRecord>,
    /// Pure-domain facts used to choose the next action.
    pub snapshot: WorkReconciliationSnapshot,
}

/// Standard cursor connection metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPageInfo {
    /// Cursor for the final edge in this page.
    pub end_cursor: Option<String>,
    /// Whether more edges remain after this page.
    pub has_next_page: bool,
}

/// A cursor-bound node shared by every Work connection family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkEdge<C, N> {
    /// Opaque family-specific cursor.
    pub cursor: C,
    /// Node at this cursor.
    pub node: N,
}

/// A bounded page shared by every Work connection family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkConnection<C, N> {
    /// Ordered page edges.
    pub edges: Vec<WorkEdge<C, N>>,
    /// Pagination metadata.
    pub page_info: WorkPageInfo,
}

/// Task connection edge.
pub type WorkTaskEdge = WorkEdge<String, WorkTaskSummary>;
/// Bounded task connection.
pub type WorkTaskConnection = WorkConnection<String, WorkTaskSummary>;

/// One task-owned artifact hydrated with only its current immutable version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkTaskArtifact {
    /// Artifact metadata owned by the task.
    pub artifact: noema_artifacts::ArtifactRecord,
    /// Current immutable version of the artifact.
    pub current_version: noema_artifacts::ArtifactVersionRecord,
}

/// Project connection edge.
pub type ProjectEdge = WorkEdge<String, ProjectRecord>;
/// Bounded project connection.
pub type ProjectConnection = WorkConnection<String, ProjectRecord>;
/// Work event connection edge.
pub type WorkEventEdge = WorkEdge<WorkEventCursor, WorkEventRecord>;
/// Bounded Work event connection.
pub type WorkEventConnection = WorkConnection<WorkEventCursor, WorkEventRecord>;
