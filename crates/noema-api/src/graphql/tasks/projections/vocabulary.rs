//! Exhaustive GraphQL enums for closed Work-domain vocabularies.

use async_graphql::Enum;

/// Membership role for the Personal workspace owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "WorkspaceMembershipRole")]
pub enum GraphqlWorkspaceMembershipRole {
    Owner,
}

macro_rules! graphql_enum {
    (
        $(#[$meta:meta])*
        $graphql:ident, $name:literal, $domain:path,
        { $($(#[$variant_meta:meta])* $variant:ident),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
        #[graphql(name = $name)]
        pub enum $graphql {
            $($(#[$variant_meta])* $variant),+
        }

        impl From<$domain> for $graphql {
            fn from(value: $domain) -> Self {
                match value {
                    $(<$domain>::$variant => Self::$variant),+
                }
            }
        }
    };
}

graphql_enum!(
    /// Immutable provider-selection mode.
    GraphqlProviderSelectionMode,
    "ProviderSelectionMode",
    noema_providers::ProviderSelectionMode,
    { ExplicitProfile, ProviderDefault }
);

graphql_enum!(
    /// Why a task currently needs the owner.
    GraphqlTaskAttentionKind,
    "TaskAttentionKind",
    noema_store::WorkTaskAttention,
    {
        /// Missing or ambiguous information is required.
        #[graphql(name = "CLARIFICATION_REQUIRED")]
        Clarification,
        /// A governed human decision is required.
        #[graphql(name = "APPROVAL_REQUIRED")]
        Approval,
        /// Automated work requires a recovery choice.
        #[graphql(name = "RECOVERY_REQUIRED")]
        Recovery,
    }
);

graphql_enum!(
    /// Closed server-authorized task action.
    GraphqlValidTaskAction,
    "ValidTaskAction",
    noema_store::WorkTaskValidAction,
    {
        /// Edit Inbox capture fields.
        Edit,
        /// Authorize dispatch.
        Queue,
        /// Answer a gate.
        Answer,
        /// Retry a recovery gate.
        Retry,
        /// Cancel work.
        Cancel,
        /// Reopen terminal history.
        Reopen,
    }
);

graphql_enum!(
    /// Human gate category.
    GraphqlTaskGateKind,
    "TaskGateKind",
    noema_tasks::TaskGateKind,
    { Clarification, Approval, Recovery }
);

graphql_enum!(
    /// Human gate lifecycle.
    GraphqlTaskGateState,
    "TaskGateState",
    noema_tasks::TaskGateState,
    { Open, Resolved, Superseded }
);

graphql_enum!(
    /// Closed reason for a Recovery gate.
    GraphqlTaskRecoveryReason,
    "TaskRecoveryReason",
    noema_tasks::TaskRecoveryReason,
    {
        InfrastructureRetriesExhausted,
        ReviewRoundsExhausted,
        UnsafeEffectUncertain,
        ConfigurationUnavailable,
        InvariantFault,
    }
);

graphql_enum!(
    /// Role of one bounded task run.
    GraphqlTaskRunKind,
    "TaskRunKind",
    noema_tasks::RunKind,
    { Planner, Executor, Reviewer }
);

graphql_enum!(
    /// Run-local queue and lease status.
    GraphqlTaskRunStatus,
    "TaskRunStatus",
    noema_tasks::RunStatus,
    {
        Queued,
        Leased,
        Running,
        Completed,
        WaitingForApproval,
        Interrupted,
        Failed,
        Cancelled,
    }
);

graphql_enum!(
    /// Immutable reviewer disposition.
    GraphqlTaskReviewVerdict,
    "TaskReviewVerdict",
    noema_tasks::TaskReviewVerdict,
    { Approve, RequestChanges, NeedsHuman }
);

graphql_enum!(
    /// Immutable outcome for one contract criterion.
    GraphqlTaskCriterionOutcome,
    "TaskCriterionOutcome",
    noema_tasks::CriterionOutcome,
    { Pass, Fail, Uncertain }
);

graphql_enum!(
    /// Closed durable task-run transcript item kind.
    GraphqlTaskRunItemKind,
    "TaskRunItemKind",
    noema_tasks::AgentRunItemKind,
    {
        ModelInput,
        AssistantOutput,
        ToolCall,
        ToolResult,
        ProgressNotice,
        ContextCheckpoint,
        TaskSubmission,
        TaskReview,
        ArtifactReference,
        Failure,
        Cancellation,
    }
);

graphql_enum!(
    /// Closed durable task-run transcript item status.
    GraphqlTaskRunItemStatus,
    "TaskRunItemStatus",
    noema_tasks::AgentRunItemStatus,
    { Pending, Running, Completed, Failed, Cancelled, Skipped }
);
