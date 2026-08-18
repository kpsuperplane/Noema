//! Bounded, read-only records assembled for one supervised Work run.

use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, ProjectRunContext, TaskGateRecord, TaskMessageRecord,
    TaskRecord, WorkflowDefinition, WorkflowStage, WorkspaceRunContext,
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

/// Calendar context captured with the human request that created a Task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRequestEnvironment {
    /// Human-local date when the source request was made.
    pub current_date: String,
    /// Human-local timestamp when the source request was made.
    pub current_time: String,
    /// Human-local IANA timezone when the source request was made.
    pub timezone: String,
}
/// One run execution envelope and only the bounded durable evidence a
/// planner, executor, or reviewer needs at its safe run boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkRunExecutionContext {
    /// Exact leased/run-local projection, including its generation and model
    /// and policy snapshots.
    pub run: AgentRunRecord,
    /// Current task projection fenced by `run.task_generation`.
    pub task: TaskRecord,
    /// Calendar context saved immediately before the Task's source human item.
    pub source_runtime_environment: Option<TaskRequestEnvironment>,
    /// Workflow definition and stage that describe the current task stage.
    pub workflow: WorkflowDefinition,
    /// Current workflow stage.
    pub stage: WorkflowStage,
    /// Current bounded workspace description used by the role.
    pub workspace: WorkspaceRunContext,
    /// Current bounded project description used by the role.
    pub project: Option<ProjectRunContext>,
    /// Current open gate, when one exists for this task generation.
    pub active_gate: Option<TaskGateRecord>,
    /// Resolved/open gates and human messages causally relevant to this run.
    pub relevant_gates: Vec<TaskGateRecord>,
    /// Human answers, change requests, and retry notes in causal order.
    pub messages: Vec<TaskMessageRecord>,
    /// Bounded parent-run records used by Executors and Reviewers.
    pub lineage: Vec<AgentRunItemRecord>,
}

/// Exact context and admission marker for one live Work run.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkRunContextAdmission {
    /// The bounded context the Runtime may place into the next provider prompt.
    pub context: WorkRunExecutionContext,
    /// Transcript item that marks which child messages were admitted.
    pub checkpoint: AgentRunItemRecord,
}
