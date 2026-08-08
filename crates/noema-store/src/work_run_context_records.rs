//! Bounded, read-only records assembled for one supervised Work run.

use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, ProjectContextSnapshot, TaskExecutionContract,
    TaskGateRecord, TaskMessageRecord, TaskRecord, TaskReviewRecord, TaskSubmissionRecord,
    WorkflowDefinition, WorkflowStage, WorkspaceContextSnapshot,
};
use serde::{Deserialize, Serialize};

/// Maximum number of human messages copied into one run context.
pub const WORK_RUN_CONTEXT_MAX_MESSAGES: usize = 64;
/// Maximum number of gates copied into one run context.
pub const WORK_RUN_CONTEXT_MAX_GATES: usize = 32;
/// Maximum number of ancestor runs whose transcript is included.
pub const WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS: usize = 4;
/// Maximum number of transcript items copied for each lineage run.
pub const WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN: usize = 24;
/// One immutable execution envelope and only the bounded durable evidence a
/// planner, executor, or reviewer needs at its safe run boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkRunExecutionContext {
    /// Exact leased/run-local projection, including its generation and model
    /// and policy snapshots.
    pub run: AgentRunRecord,
    /// Current task projection fenced by `run.task_generation`.
    pub task: TaskRecord,
    /// Workflow definition and stage that describe the current task stage.
    pub workflow: WorkflowDefinition,
    /// Current workflow stage.
    pub stage: WorkflowStage,
    /// Immutable contract permitted by this run. Planner runs intentionally
    /// have no contract.
    pub contract: Option<TaskExecutionContract>,
    /// Workspace description used by the role. For contract-bound runs this
    /// is copied from the immutable contract snapshot; for Planner it is read
    /// from the current bounded workspace row.
    pub workspace: WorkspaceContextSnapshot,
    /// Optional project description used by the role, with the same immutable
    /// contract rule as `workspace`.
    pub project: Option<ProjectContextSnapshot>,
    /// Current open gate, when one exists for this task generation.
    pub active_gate: Option<TaskGateRecord>,
    /// Resolved/open gates and human messages causally relevant to this run.
    pub relevant_gates: Vec<TaskGateRecord>,
    /// Human answers, change requests, and retry notes in causal order.
    pub messages: Vec<TaskMessageRecord>,
    /// Submission evidence relevant to this run, normally the submission under
    /// review or the task's latest immutable submission.
    pub latest_submission: Option<TaskSubmissionRecord>,
    /// Review evidence relevant to this run, normally the review that triggered
    /// a continuation or the task's latest immutable review.
    pub latest_review: Option<TaskReviewRecord>,
    /// Recent transcript material from this run and its bounded parent lineage,
    /// ordered from oldest run/item to newest. Reviewer contexts intentionally
    /// leave this empty because their submission projection is authoritative.
    pub lineage: Vec<AgentRunItemRecord>,
}

/// Exact context and immutable checkpoint admitted for one live Work run.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkRunContextAdmission {
    /// The bounded context the Runtime may place into the next provider prompt.
    pub context: WorkRunExecutionContext,
    /// Deterministic transcript item proving which child messages were admitted.
    pub checkpoint: AgentRunItemRecord,
}
