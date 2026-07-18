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
    TaskSubmissionRecord, WorkCommandResult, WorkEventRecord, WorkReconciliationSnapshot,
    WorkflowDefinition, WorkflowStage, WorkflowStageBehavior,
};
use noema_workspaces::{ProjectId, ProjectRecord, WorkspaceId, WorkspaceRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use cursor::{
    ProjectCursor, WorkCursorError, WorkEventCursor, WorkPageSize, WorkTaskArtifactCursor,
    WorkTaskCursor,
};
pub use history::*;
pub use overview::*;

/// Stable board/history scope for a task connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkTaskScope {
    /// The five nonterminal Personal workflow stages.
    Active,
    /// Completed and Cancelled history.
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
    /// Answer an open human gate.
    Answer,
    /// Retry an eligible recovery gate.
    Retry,
    /// Accept an approved submission.
    Accept,
    /// Request a revised execution contract.
    RequestChanges,
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
    /// A reviewed result is ready for human acceptance.
    ReadyForAcceptance,
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
    /// Workflow definition that owns the stage.
    pub workflow: WorkflowDefinition,
    /// Complete immutable-at-read workflow stage set used by mutation projection.
    pub workflow_stages: Vec<WorkflowStage>,
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
    /// Submission explicitly accepted by the human, when the task completed.
    pub accepted_submission: Option<TaskSubmissionRecord>,
    /// Most recent immutable review, if any.
    pub latest_review: Option<TaskReviewRecord>,
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

/// One task edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTaskEdge {
    /// Opaque keyset cursor for this task.
    pub cursor: String,
    /// Task card at this edge.
    pub node: WorkTaskSummary,
}

/// Bounded task connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTaskConnection {
    /// Bounded task edges in query order.
    pub edges: Vec<WorkTaskEdge>,
    /// Pagination metadata.
    pub page_info: WorkPageInfo,
}

/// Bounded artifact query for one task owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTaskArtifactQuery {
    /// Task that must directly own every returned artifact.
    pub task_id: TaskId,
    /// Validated page size.
    pub first: WorkPageSize,
    /// Exclusive query-bound keyset cursor.
    pub after: Option<WorkTaskArtifactCursor>,
}

/// One task-owned artifact hydrated with only its current immutable version.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkTaskArtifact {
    /// Artifact metadata owned by the task.
    pub artifact: noema_artifacts::ArtifactRecord,
    /// Current immutable version of the artifact.
    pub current_version: noema_artifacts::ArtifactVersionRecord,
}

/// One task-artifact edge.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkTaskArtifactEdge {
    /// Opaque keyset cursor for this artifact.
    pub cursor: String,
    /// Artifact record at this edge.
    pub node: WorkTaskArtifact,
}

/// Bounded task-artifact connection.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkTaskArtifactConnection {
    /// Bounded artifact edges in query order.
    pub edges: Vec<WorkTaskArtifactEdge>,
    /// Pagination metadata.
    pub page_info: WorkPageInfo,
}

/// One project edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEdge {
    /// Opaque keyset cursor for this project.
    pub cursor: String,
    /// Project record at this edge.
    pub node: ProjectRecord,
}

/// Bounded project connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConnection {
    /// Bounded project edges in query order.
    pub edges: Vec<ProjectEdge>,
    /// Pagination metadata.
    pub page_info: WorkPageInfo,
}

/// One globally ordered Work-event edge.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkEventEdge {
    /// Global event cursor for this edge.
    pub cursor: WorkEventCursor,
    /// Immutable event record at this edge.
    pub node: WorkEventRecord,
}

/// Bounded Work-event connection in the order promised by its read method.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkEventConnection {
    /// Bounded event edges in ascending sequence order.
    pub edges: Vec<WorkEventEdge>,
    /// Pagination metadata.
    pub page_info: WorkPageInfo,
}

/// Notification outbox lifecycle owned by the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkNotificationStatus {
    /// Notification is ready to be leased.
    Pending,
    /// Notification is leased to a delivery worker.
    Leased,
    /// Notification delivery has been acknowledged.
    Delivered,
    /// Delivery failed and is either retryable or parked.
    Failed,
}

impl WorkNotificationStatus {
    /// Return the canonical persisted status value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Leased => "leased",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for WorkNotificationStatus {
    type Err = noema_tasks::WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "leased" => Ok(Self::Leased),
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            other => Err(noema_tasks::WorkDomainError::InvalidInput {
                field: "work_notification.status",
                message: format!("unknown value {other}"),
            }),
        }
    }
}

/// Durable generalized notification row.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkNotificationRecord {
    /// Stable outbox notification identity.
    pub notification_id: String,
    /// Source global event sequence.
    pub event_sequence: u64,
    /// Delivery destination vocabulary.
    pub destination_kind: noema_tasks::NotificationDestination,
    /// Destination identity, currently a human owner.
    pub destination_id: String,
    /// Notification card vocabulary.
    pub notification_kind: noema_tasks::NotificationKind,
    /// Redacted structured card payload.
    pub payload: Value,
    /// Current outbox lifecycle status.
    pub status: WorkNotificationStatus,
    /// Earliest timestamp at which this row may be claimed.
    pub available_at: String,
    /// Worker currently holding the lease.
    pub lease_owner: Option<String>,
    /// Opaque lease token required for acknowledgement.
    pub lease_token: Option<String>,
    /// Lease expiration timestamp.
    pub lease_expires_at: Option<String>,
    /// Number of claim attempts.
    pub attempt_count: u32,
    /// Safe failure code from the latest delivery attempt.
    pub last_error_code: Option<String>,
    /// Bounded diagnostic from the latest delivery attempt.
    pub last_error_message: Option<String>,
    /// Delivery acknowledgement timestamp.
    pub delivered_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last projection update timestamp.
    pub updated_at: String,
}

/// Durable result of receipt-first command idempotency.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkCommandReceiptRecord {
    /// Authenticated actor namespace for the receipt.
    pub actor_id: String,
    /// Stable semantic command name.
    pub command_name: String,
    /// Caller-supplied idempotency key.
    pub idempotency_key: String,
    /// Canonical SHA-256 request fingerprint.
    pub request_fingerprint: String,
    /// Task affected by the command, if any.
    pub result_task_id: Option<TaskId>,
    /// Project affected by the command, if any.
    pub result_project_id: Option<ProjectId>,
    /// Last event sequence committed by the command.
    pub result_event_sequence: u64,
    /// Reconstructed command response.
    pub response: WorkCommandResult,
    /// Receipt creation timestamp.
    pub created_at: String,
}
