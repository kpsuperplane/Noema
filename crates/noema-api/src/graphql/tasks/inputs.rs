use async_graphql::{Enum, InputObject};

use noema_tasks::{
    ApprovalDecision, NewTaskValidationCriterion, TaskComplexity, TaskGateAnswer,
    WorkflowStageBehavior,
};

use crate::graphql::agents::GraphqlReasoningEffort;

/// Complexity selected for a new execution contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "TaskComplexity")]
pub enum GraphqlTaskComplexity {
    /// Small, low-risk work.
    Simple,
    /// Typical multi-step work.
    Medium,
    /// Large or reasoning-intensive work.
    Difficult,
}

impl From<GraphqlTaskComplexity> for TaskComplexity {
    fn from(value: GraphqlTaskComplexity) -> Self {
        match value {
            GraphqlTaskComplexity::Simple => Self::Simple,
            GraphqlTaskComplexity::Medium => Self::Medium,
            GraphqlTaskComplexity::Difficult => Self::Difficult,
        }
    }
}

impl From<TaskComplexity> for GraphqlTaskComplexity {
    fn from(value: TaskComplexity) -> Self {
        match value {
            TaskComplexity::Simple => Self::Simple,
            TaskComplexity::Medium => Self::Medium,
            TaskComplexity::Difficult => Self::Difficult,
        }
    }
}

/// Input for replacing one human-controlled executor pool entry.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "TaskModelPoolEntryInput")]
pub struct GraphqlTaskModelPoolEntryInput {
    /// Complexity tier exposed to the primary agent.
    pub complexity: GraphqlTaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for this entry.
    pub provider_kind: String,
    /// Provider account owning the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional reasoning effort.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether this entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within its tier.
    pub sort_order: i32,
}

/// Input for replacing the four user-controlled Task Executor runtime limits.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "TaskExecutionPolicyInput")]
pub struct GraphqlTaskExecutionPolicyInput {
    /// Maximum provider continuations before terminal-only finalization.
    pub max_provider_continuations: i32,
    /// Maximum tool calls before terminal-only finalization.
    pub max_tool_calls: i32,
    /// Maximum active execution time in minutes, excluding queue time.
    pub max_active_minutes: i32,
    /// Continuation interval between progress audits.
    pub progress_audit_interval: i32,
}

/// Runtime behavior of one workflow stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "WorkflowStageBehavior")]
pub enum GraphqlWorkflowStageBehavior {
    /// Captured work not yet authorized to run.
    Intake,
    /// Authorized work awaiting a worker.
    Dispatch,
    /// Planning, execution, or automated review is active.
    Active,
    /// A human gate is open.
    HumanGate,
    /// A reviewer-approved result awaits acceptance.
    Acceptance,
    /// Human-accepted terminal work.
    TerminalSuccess,
    /// Human-cancelled terminal work.
    TerminalCancelled,
}

impl From<GraphqlWorkflowStageBehavior> for WorkflowStageBehavior {
    fn from(value: GraphqlWorkflowStageBehavior) -> Self {
        match value {
            GraphqlWorkflowStageBehavior::Intake => Self::Intake,
            GraphqlWorkflowStageBehavior::Dispatch => Self::Dispatch,
            GraphqlWorkflowStageBehavior::Active => Self::Active,
            GraphqlWorkflowStageBehavior::HumanGate => Self::HumanGate,
            GraphqlWorkflowStageBehavior::Acceptance => Self::Acceptance,
            GraphqlWorkflowStageBehavior::TerminalSuccess => Self::TerminalSuccess,
            GraphqlWorkflowStageBehavior::TerminalCancelled => Self::TerminalCancelled,
        }
    }
}

impl From<WorkflowStageBehavior> for GraphqlWorkflowStageBehavior {
    fn from(value: WorkflowStageBehavior) -> Self {
        match value {
            WorkflowStageBehavior::Intake => Self::Intake,
            WorkflowStageBehavior::Dispatch => Self::Dispatch,
            WorkflowStageBehavior::Active => Self::Active,
            WorkflowStageBehavior::HumanGate => Self::HumanGate,
            WorkflowStageBehavior::Acceptance => Self::Acceptance,
            WorkflowStageBehavior::TerminalSuccess => Self::TerminalSuccess,
            WorkflowStageBehavior::TerminalCancelled => Self::TerminalCancelled,
        }
    }
}

/// Scope of a Work task connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Enum)]
#[graphql(name = "WorkTaskScope")]
pub enum GraphqlWorkTaskScope {
    /// Nonterminal workflow stages.
    #[default]
    Active,
    /// Completed and cancelled history.
    Terminal,
    /// Both active and terminal tasks.
    All,
}

impl From<GraphqlWorkTaskScope> for noema_store::WorkTaskScope {
    fn from(value: GraphqlWorkTaskScope) -> Self {
        match value {
            GraphqlWorkTaskScope::Active => Self::Active,
            GraphqlWorkTaskScope::Terminal => Self::Terminal,
            GraphqlWorkTaskScope::All => Self::All,
        }
    }
}

/// Terminal history filter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "TerminalTaskKind")]
pub enum GraphqlTerminalTaskKind {
    /// Accepted tasks only.
    Accepted,
    /// Cancelled tasks only.
    Cancelled,
    /// Both terminal kinds.
    All,
}

/// Structured approval decision for an Approval gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ApprovalDecision")]
pub enum GraphqlApprovalDecision {
    /// Explicitly approve the gated action.
    Approved,
    /// Explicitly decline the gated action.
    Declined,
}

impl From<GraphqlApprovalDecision> for ApprovalDecision {
    fn from(value: GraphqlApprovalDecision) -> Self {
        match value {
            GraphqlApprovalDecision::Approved => Self::Approved,
            GraphqlApprovalDecision::Declined => Self::Declined,
        }
    }
}

impl From<ApprovalDecision> for GraphqlApprovalDecision {
    fn from(value: ApprovalDecision) -> Self {
        match value {
            ApprovalDecision::Approved => Self::Approved,
            ApprovalDecision::Declined => Self::Declined,
        }
    }
}

/// Board/list task filter input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "WorkTasksInput")]
pub struct GraphqlWorkTasksInput {
    /// Workspace scope.
    pub workspace_id: String,
    /// Optional project scope.
    pub project_id: Option<String>,
    /// Optional explicit stage identities.
    pub stage_ids: Option<Vec<String>>,
    /// Optional semantic stage behavior filter.
    pub stage_behaviors: Option<Vec<GraphqlWorkflowStageBehavior>>,
    /// Case-insensitive title/description search.
    pub text: Option<String>,
    /// Restrict results to unresolved human attention.
    #[graphql(default = false)]
    pub attention_only: bool,
    /// Active, terminal, or combined scope.
    #[graphql(default)]
    pub scope: GraphqlWorkTaskScope,
}

/// New project input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateProjectInput")]
pub struct GraphqlCreateProjectInput {
    /// Workspace owning the project.
    pub workspace_id: String,
    /// Nonblank project name.
    pub name: String,
    /// Descriptive project text.
    #[graphql(default)]
    pub description: String,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Existing project update input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "UpdateProjectInput")]
pub struct GraphqlUpdateProjectInput {
    /// Project target.
    pub project_id: String,
    /// Expected project revision.
    pub expected_revision: i64,
    /// Optional replacement name.
    pub name: Option<String>,
    /// Optional replacement description.
    pub description: Option<String>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Project archive input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ArchiveProjectInput")]
pub struct GraphqlArchiveProjectInput {
    /// Project target.
    pub project_id: String,
    /// Expected project revision.
    pub expected_revision: i64,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Project reopen input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ReopenProjectInput")]
pub struct GraphqlReopenProjectInput {
    /// Project target.
    pub project_id: String,
    /// Expected project revision.
    pub expected_revision: i64,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Capture a task in Inbox.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CaptureTaskInput")]
pub struct GraphqlCaptureTaskInput {
    /// Workspace target.
    pub workspace_id: String,
    /// Optional project association.
    pub project_id: Option<String>,
    /// Concise title.
    pub title: String,
    /// Fuller capture description.
    #[graphql(default)]
    pub description: String,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Inbox task edit input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "UpdateInboxTaskInput")]
pub struct GraphqlUpdateInboxTaskInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Optional replacement title.
    pub title: Option<String>,
    /// Optional replacement description.
    pub description: Option<String>,
    /// Optional project assignment. Null means omitted unless `clearProject` is true.
    pub project_id: Option<String>,
    /// Explicitly clear the project association.
    pub clear_project: Option<bool>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Queue a captured task.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "QueueTaskInput")]
pub struct GraphqlQueueTaskInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Resolve a human gate.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "AnswerTaskInput")]
pub struct GraphqlAnswerTaskInput {
    /// Task target.
    pub task_id: String,
    /// Current open gate.
    pub gate_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Human answer Markdown.
    pub answer_markdown: String,
    /// Required for Approval gates.
    pub approval_decision: Option<GraphqlApprovalDecision>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Retry an eligible Recovery gate.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "RetryTaskInput")]
pub struct GraphqlRetryTaskInput {
    /// Task target.
    pub task_id: String,
    /// Current open gate.
    pub gate_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Optional bounded retry note.
    pub retry_note: Option<String>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Accept an approved task result.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "AcceptTaskInput")]
pub struct GraphqlAcceptTaskInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// One replacement validation criterion.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "TaskValidationCriterionInput")]
pub struct GraphqlTaskValidationCriterionInput {
    /// Optional stable criterion identity.
    pub criterion_id: Option<String>,
    /// One-based ordinal.
    pub ordinal: i32,
    /// Criterion description.
    pub description: String,
    /// Optional evidence guidance.
    pub expected_evidence: Option<String>,
}

impl GraphqlTaskValidationCriterionInput {
    pub(crate) fn try_into_domain(self) -> async_graphql::Result<NewTaskValidationCriterion> {
        let ordinal = u32::try_from(self.ordinal)
            .ok()
            .filter(|ordinal| *ordinal > 0)
            .ok_or_else(|| super::resolvers::invalid_input_error("criterion ordinal"))?;
        Ok(NewTaskValidationCriterion {
            criterion_id: self.criterion_id,
            ordinal,
            description: self.description,
            expected_evidence: self.expected_evidence,
        })
    }
}

/// Human request for a changed result.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "RequestTaskChangesInput")]
pub struct GraphqlRequestTaskChangesInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Nonblank feedback.
    pub feedback_markdown: String,
    /// Optional replacement request.
    pub request_markdown: Option<String>,
    /// Complete replacement criteria set.
    pub replacement_criteria: Option<Vec<GraphqlTaskValidationCriterionInput>>,
    /// Optional replacement complexity.
    pub complexity: Option<GraphqlTaskComplexity>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Cancel a task.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CancelTaskInput")]
pub struct GraphqlCancelTaskInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Optional safe cancellation reason.
    pub reason: Option<String>,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Reopen terminal task history.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ReopenTaskInput")]
pub struct GraphqlReopenTaskInput {
    /// Task target.
    pub task_id: String,
    /// Expected current revision.
    pub expected_revision: i64,
    /// Expected execution generation.
    pub expected_generation: i64,
    /// Caller idempotency key.
    pub client_mutation_id: String,
}

/// Construct the domain gate answer after GraphQL input validation.
pub(crate) fn gate_answer(
    message_markdown: String,
    approval_decision: Option<GraphqlApprovalDecision>,
) -> TaskGateAnswer {
    TaskGateAnswer {
        message_markdown,
        approval_decision: approval_decision.map(Into::into),
    }
}
