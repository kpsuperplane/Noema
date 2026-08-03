use async_graphql::{Enum, InputObject};

use noema_tasks::{
    ApprovalDecision, MissedRunPolicy, NewTaskValidationCriterion, OverlapPolicy, TaskComplexity,
    WorkflowStageBehavior,
};

use super::{GraphqlMissedRunPolicy, GraphqlOverlapPolicy};
use crate::graphql::agents::{GraphqlModelPreferenceSelectionMode, GraphqlReasoningEffort};

macro_rules! fenced_task_input {
    ($(#[$meta:meta])* $rust:ident, $graphql:literal) => {
        $(#[$meta])*
        #[derive(Clone, Debug, InputObject)]
        #[graphql(name = $graphql)]
        pub struct $rust {
            /// Task target.
            pub task_id: String,
            /// Expected current revision.
            pub expected_revision: i64,
            /// Expected execution generation.
            pub expected_generation: i64,
            /// Caller idempotency key.
            pub client_mutation_id: String,
        }
    };
}

macro_rules! graphql_input {
    ($doc:literal => $name:ident($graphql_name:literal) { $(
        $field_doc:literal $(, #[$field_meta:meta])* => $field:ident: $field_type:ty
    ),* $(,)? }) => {
        #[doc = $doc]
        #[derive(Clone, Debug, InputObject)]
        #[graphql(name = $graphql_name)]
        pub struct $name {
            $(#[doc = $field_doc] $(#[$field_meta])* pub $field: $field_type,)*
        }
    };
}

macro_rules! graphql_enum_bridge {
    ($doc:literal => $graphql:ident($name:literal) <=> $domain:path {
        $($variant_doc:literal => $variant:ident),+ $(,)?
    }) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
        #[graphql(name = $name)]
        pub enum $graphql { $(#[doc = $variant_doc] $variant),+ }

        impl From<$graphql> for $domain {
            fn from(value: $graphql) -> Self {
                match value { $($graphql::$variant => Self::$variant),+ }
            }
        }

        impl From<$domain> for $graphql {
            fn from(value: $domain) -> Self {
                match value { $(<$domain>::$variant => Self::$variant),+ }
            }
        }
    };
}

graphql_enum_bridge! { "Complexity selected for a new execution contract." =>
    GraphqlTaskComplexity("TaskComplexity") <=> TaskComplexity {
        "Small, low-risk work." => Simple,
        "Typical multi-step work." => Medium,
        "Large or reasoning-intensive work." => Difficult,
    }
}

graphql_input! { "Input for replacing one human-controlled executor pool entry." => GraphqlTaskModelPoolEntryInput("TaskModelPoolEntryInput") {
    "Complexity tier exposed to the primary agent." => complexity: GraphqlTaskComplexity,
    "Optional human-facing label." => label: Option<String>,
    "Provider family for this entry." => provider_kind: String,
    "Provider account owning the model profile." => provider_account_id: String,
    "Whether Noema or the human chooses the concrete model." => selection_mode: GraphqlModelPreferenceSelectionMode,
    "Exact provider model/profile for an explicit selection." => model_profile: Option<String>,
    "Optional reasoning effort." => reasoning_effort: Option<GraphqlReasoningEffort>,
    "Whether this entry can be selected for new tasks." => enabled: bool,
    "Human-controlled ordering within its tier." => sort_order: i32,
} }

graphql_input! { "Input for replacing the four user-controlled Task Executor runtime limits." => GraphqlTaskExecutionPolicyInput("TaskExecutionPolicyInput") {
    "Maximum provider continuations before terminal-only finalization." => max_provider_continuations: i32,
    "Maximum tool calls before terminal-only finalization." => max_tool_calls: i32,
    "Maximum active execution time in minutes, excluding queue time." => max_active_minutes: i32,
    "Continuation interval between progress audits." => progress_audit_interval: i32,
} }

graphql_enum_bridge! { "Runtime behavior of one workflow stage." =>
    GraphqlWorkflowStageBehavior("WorkflowStageBehavior") <=> WorkflowStageBehavior {
        "Captured work not yet authorized to run." => Intake,
        "Authorized work awaiting a worker." => Dispatch,
        "Planning, execution, or automated review is active." => Active,
        "A human gate is open." => HumanGate,
        "Reviewer-approved terminal work." => TerminalSuccess,
        "Human-cancelled terminal work." => TerminalCancelled,
    }
}

/// Scope of a Work task connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Enum)]
#[graphql(name = "WorkTaskScope")]
pub enum GraphqlWorkTaskScope {
    /// Nonterminal workflow stages.
    #[default]
    Active,
    /// Done and cancelled history.
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
    /// Completed tasks only.
    Completed,
    /// Cancelled tasks only.
    Cancelled,
    /// Both terminal kinds.
    All,
}

graphql_enum_bridge! { "Structured approval decision for an Approval gate." =>
    GraphqlApprovalDecision("ApprovalDecision") <=> ApprovalDecision {
        "Explicitly approve the gated action." => Approved,
        "Explicitly decline the gated action." => Declined,
    }
}

graphql_input! { "Board/list task filter input." => GraphqlWorkTasksInput("WorkTasksInput") {
    "Workspace scope." => workspace_id: String,
    "Optional project scope." => project_id: Option<String>,
    "Optional explicit stage identities." => stage_ids: Option<Vec<String>>,
    "Optional semantic stage behavior filter." => stage_behaviors: Option<Vec<GraphqlWorkflowStageBehavior>>,
    "Case-insensitive title/description search." => text: Option<String>,
    "Restrict results to unresolved human attention.", #[graphql(default = false)] => attention_only: bool,
    "Active, terminal, or combined scope.", #[graphql(default)] => scope: GraphqlWorkTaskScope,
} }
graphql_input! { "New project input." => GraphqlCreateProjectInput("CreateProjectInput") {
    "Workspace owning the project." => workspace_id: String,
    "Nonblank project name." => name: String,
    "Descriptive project text.", #[graphql(default)] => description: String,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Existing project update input." => GraphqlUpdateProjectInput("UpdateProjectInput") {
    "Project target." => project_id: String,
    "Expected project revision." => expected_revision: i64,
    "Optional replacement name." => name: Option<String>,
    "Optional replacement description." => description: Option<String>,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Project archive input." => GraphqlArchiveProjectInput("ArchiveProjectInput") {
    "Project target." => project_id: String,
    "Expected project revision." => expected_revision: i64,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Project reopen input." => GraphqlReopenProjectInput("ReopenProjectInput") {
    "Project target." => project_id: String,
    "Expected project revision." => expected_revision: i64,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Capture a task in Inbox." => GraphqlCaptureTaskInput("CaptureTaskInput") {
    "Workspace target." => workspace_id: String,
    "Optional project association." => project_id: Option<String>,
    "Concise title." => title: String,
    "Fuller capture description.", #[graphql(default)] => description: String,
    "Optional one-time or repeating execution schedule." => schedule: Option<GraphqlNewTaskScheduleInput>,
    "Caller idempotency key." => client_mutation_id: String,
} }

graphql_input! { "Optional Repeat configuration for scheduled work." => GraphqlNewTaskRecurrenceInput("NewTaskRecurrenceInput") {
    "Inclusive RFC3339 lower bound for cron matches." => starts_at: String,
    "Five-field cron expression." => cron_expression: String,
    "Behavior while another occurrence is nonterminal." => overlap_policy: Option<GraphqlOverlapPolicy>,
} }

graphql_input! { "Future execution attached directly to an Inbox task." => GraphqlNewTaskScheduleInput("NewTaskScheduleInput") {
    "Exact RFC3339 UTC execution instant." => scheduled_for: String,
    "Validated authoring IANA timezone." => time_zone: String,
    "Restart/missed-window behavior." => missed_run_policy: Option<GraphqlMissedRunPolicy>,
    "Present only when Repeat is enabled." => recurrence: Option<GraphqlNewTaskRecurrenceInput>,
} }

graphql_input! { "Schedule or reschedule an Inbox task." => GraphqlScheduleTaskInput("ScheduleTaskInput") {
    "Task target." => task_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Future execution configuration." => schedule: GraphqlNewTaskScheduleInput,
    "Caller idempotency key." => client_mutation_id: String,
} }

fenced_task_input!(
    /// Remove future execution from an Inbox task.
    GraphqlUnscheduleTaskInput,
    "UnscheduleTaskInput"
);

graphql_input! { "Edit future authority for a recurring task." => GraphqlUpdateTaskRecurrenceInput("UpdateTaskRecurrenceInput") {
    "Recurring template target." => recurrence_id: String,
    "Expected template revision." => expected_revision: i64,
    "Optional future title snapshot." => title: Option<String>,
    "Optional future description snapshot." => description: Option<String>,
    "Optional project assignment." => project_id: Option<String>,
    "Explicitly clear project assignment." => clear_project: Option<bool>,
    "Optional inclusive RFC3339 start bound." => starts_at: Option<String>,
    "Optional five-field cron expression." => cron_expression: Option<String>,
    "Optional IANA timezone." => time_zone: Option<String>,
    "Optional missed-window behavior." => missed_run_policy: Option<GraphqlMissedRunPolicy>,
    "Optional overlap behavior." => overlap_policy: Option<GraphqlOverlapPolicy>,
    "Caller idempotency key." => client_mutation_id: String,
} }

graphql_input! { "Fenced recurring lifecycle command." => GraphqlTaskRecurrenceCommandInput("TaskRecurrenceCommandInput") {
    "Recurring template target." => recurrence_id: String,
    "Expected template revision." => expected_revision: i64,
    "Caller idempotency key." => client_mutation_id: String,
} }

graphql_input! { "Preview a local schedule." => GraphqlTaskSchedulePreviewInput("TaskSchedulePreviewInput") {
    "Inclusive RFC3339 start instant." => starts_at: String,
    "IANA timezone." => time_zone: String,
    "Optional five-field cron expression; absent previews only the start." => cron_expression: Option<String>,
} }

impl From<GraphqlMissedRunPolicy> for MissedRunPolicy {
    fn from(value: GraphqlMissedRunPolicy) -> Self {
        match value {
            GraphqlMissedRunPolicy::Skip => Self::Skip,
            GraphqlMissedRunPolicy::RunOnce => Self::RunOnce,
        }
    }
}

impl From<GraphqlOverlapPolicy> for OverlapPolicy {
    fn from(value: GraphqlOverlapPolicy) -> Self {
        match value {
            GraphqlOverlapPolicy::Skip => Self::Skip,
            GraphqlOverlapPolicy::QueueOne => Self::QueueOne,
            GraphqlOverlapPolicy::Allow => Self::Allow,
        }
    }
}
graphql_input! { "Inbox task edit input." => GraphqlUpdateInboxTaskInput("UpdateInboxTaskInput") {
    "Task target." => task_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Optional replacement title." => title: Option<String>,
    "Optional replacement description." => description: Option<String>,
    "Optional project assignment. Null means omitted unless clearProject is true." => project_id: Option<String>,
    "Explicitly clear the project association." => clear_project: Option<bool>,
    "Caller idempotency key." => client_mutation_id: String,
} }

fenced_task_input!(
    /// Queue a captured task.
    GraphqlQueueTaskInput,
    "QueueTaskInput"
);

graphql_input! { "Resolve a human gate." => GraphqlAnswerTaskInput("AnswerTaskInput") {
    "Task target." => task_id: String,
    "Current open gate." => gate_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Human answer Markdown." => answer_markdown: String,
    "Required for Approval gates." => approval_decision: Option<GraphqlApprovalDecision>,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Retry an eligible Recovery gate." => GraphqlRetryTaskInput("RetryTaskInput") {
    "Task target." => task_id: String,
    "Current open gate." => gate_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Optional bounded retry note." => retry_note: Option<String>,
    "Caller idempotency key." => client_mutation_id: String,
} }

graphql_input! { "One replacement validation criterion." => GraphqlTaskValidationCriterionInput("TaskValidationCriterionInput") {
    "Optional stable criterion identity." => criterion_id: Option<String>,
    "One-based ordinal." => ordinal: i32,
    "Criterion description." => description: String,
    "Optional evidence guidance." => expected_evidence: Option<String>,
} }

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

graphql_input! { "Reopen a completed task with new direction." => GraphqlReopenTaskInput("ReopenTaskInput") {
    "Task target." => task_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Nonblank feedback." => feedback_markdown: String,
    "Optional replacement request." => request_markdown: Option<String>,
    "Complete replacement criteria set." => replacement_criteria: Option<Vec<GraphqlTaskValidationCriterionInput>>,
    "Optional replacement complexity." => complexity: Option<GraphqlTaskComplexity>,
    "Caller idempotency key." => client_mutation_id: String,
} }
graphql_input! { "Cancel a task." => GraphqlCancelTaskInput("CancelTaskInput") {
    "Task target." => task_id: String,
    "Expected current revision." => expected_revision: i64,
    "Expected execution generation." => expected_generation: i64,
    "Optional safe cancellation reason." => reason: Option<String>,
    "Caller idempotency key." => client_mutation_id: String,
} }
