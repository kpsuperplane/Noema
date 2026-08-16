//! SQLite-backed canonical Noema store.

mod adapters;
mod agent_runtime_preferences;
mod agents;
mod artifact_metadata_port;
mod artifact_writes;
mod artifacts;
mod authorization_context;
mod auxiliary_model_preferences;
mod clients;
mod context_summaries;
mod conversation_interactions;
mod conversations;
mod error;
mod governed_action_approvals;
mod governed_action_fencing;
mod governed_actions;
mod human_passkeys;
mod ids;
mod live_activity;
mod local_model_activation;
mod local_model_lifecycle;
#[cfg(test)]
mod local_model_lifecycle_tests;
#[cfg(test)]
mod local_model_port_tests;
mod local_model_ports;
mod local_model_rows;
mod local_models;
#[cfg(test)]
mod local_models_tests;
mod mcp;
mod mcp_auth_requests;
mod native_oauth;
mod notifications;
mod observed_urls;
mod provider_account_port;
mod provider_accounts;
mod provider_capability_bindings;
mod provider_capability_port;
mod provider_catalog_port;
#[cfg(test)]
mod provider_persistence_port_tests;
mod provider_selection_initialization;
mod provider_selection_loaders;
mod provider_selections;
mod provider_setup_confirmation;
mod run_items;
mod runtime;
mod runtime_debug;
mod schema;
mod sqlite;
mod task_execution_policy;
mod task_model_pools;
mod tasks;
mod web_push;
mod work_command_result;
mod work_commands;
#[path = "task_events.rs"]
mod work_events;
mod work_notifications;
mod work_reads;
mod work_reconciliation;
mod work_records;
mod work_row;
mod work_run_context;
mod work_run_context_admission;
mod work_run_context_records;
mod work_runs;

#[cfg(test)]
mod work_command_tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
pub(crate) mod tests;

pub use adapters::{AdapterConnectionRecord, AdapterDefinitionRecord};
pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{
    AcpAgentAuthStatus, AcpAgentHealthStatus, AcpAgentRecord, AgentRecord, AgentSystemRole,
};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, AuxiliaryModelTask, NewAuxiliaryModelPreference,
};
pub use clients::ClientRecord;
pub use conversation_interactions::{
    ConversationInteractionKind, ConversationInteractionRecord, ConversationInteractionStatus,
    NewConversationInteraction,
};
pub use conversations::MemoryConversationSourceRange;
pub use error::{SchemaIncompatibility, StoreError};
pub use governed_action_approvals::GovernedActionDecision;
pub use governed_actions::{
    ExecutionReviewRoute, GovernedActionAssessmentRecord, GovernedActionRecord,
    GovernedActionState, GovernedAssessmentStatus, GovernedAuthorization, GovernedExecutionOutcome,
    GovernedRisk, NewGovernedAction, NewGovernedActionAssessment, StoredToolBehavior,
};
pub use human_passkeys::{HumanPasskeyRecord, HumanPasskeyRemoval};
pub use live_activity::{
    ClaimedLiveActivityDelivery, ClientLiveActivityRegistration, ClientTaskActivityRecord,
    LiveActivityEvent, LiveActivityTarget, NewLiveActivityDelivery, TaskNotificationAlert,
};
pub use mcp_auth_requests::{
    CapabilityAuthenticationRequestRecord, CapabilityAuthenticationRequestState,
    NewCapabilityAuthenticationRequest,
};
pub use native_oauth::{
    NativeOAuthGrant, NativeOAuthRefreshGrant, NativeOAuthRefreshLookup, NativeOAuthRotation,
    NativeOAuthRotationOutcome, NewNativeOAuthCode, NewNativeOAuthFamily,
};
pub use notifications::{ApnsEnvironment, ClaimedApnsDelivery, ClientNotificationRecord};
pub use observed_urls::ObservedUrlSource;
pub use provider_setup_confirmation::{ProviderSetupRole, ReadyProviderSetupSelection};
pub use runtime::{NoemaStore, StoreConfig};
pub use runtime_debug::{
    NewRuntimeDebugSpan, RuntimeDebugChildSpan, RuntimeDebugMetadata, RuntimeDebugProfileRecord,
    RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanRecord, RuntimeDebugSpanStatus,
};
pub use web_push::{
    ClaimedWebPushDelivery, NewWebPushSubscription, WebPushIdentity, WebPushPrimaryCheckpoint,
    WebPushSubscription,
};
pub use work_command_result::CommittedWorkCommandResult;
pub use work_commands::WorkCommandService;
pub use work_notifications::{
    ClaimedWorkNotification, CompleteWorkNotification, FailWorkNotification,
    WorkNotificationLeaseRequest,
};
pub use work_reconciliation::{
    ApplyReconciliation, action_run_kind, plan_snapshot, plan_work_reconciliation,
};
pub use work_records::{
    ProjectConnection, ProjectCursor, ProjectEdge, ProjectQuery, WorkConnection, WorkCursorError,
    WorkEdge, WorkEventBeforeQuery, WorkEventConnection, WorkEventCursor, WorkEventEdge,
    WorkEventQuery, WorkOverview, WorkOverviewQuery, WorkPageInfo, WorkPageSize,
    WorkReconciliationEnvelope, WorkRunItemConnection, WorkRunItemCursor, WorkRunItemEdge,
    WorkRunItemOwnerScope, WorkRunItemQuery, WorkStageTaskCount, WorkTaskArtifact,
    WorkTaskAttention, WorkTaskConnection, WorkTaskCursor, WorkTaskDetail, WorkTaskEdge,
    WorkTaskQuery, WorkTaskScope, WorkTaskSummary, WorkTaskValidAction, WorkWorkflowWithStages,
};
pub use work_run_context_records::{
    WORK_RUN_CONTEXT_MAX_GATES, WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN,
    WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS, WORK_RUN_CONTEXT_MAX_MESSAGES, WorkRunContextAdmission,
    WorkRunExecutionContext,
};
pub use work_runs::{
    ClaimedWorkRun, CompletePlan, PlanTerminal, ReportRunFailure, ReportTaskBlocked, SubmitPlan,
    SubmitTaskResult, SubmitTaskReview, WorkRunFence, WorkRunProgress, WorkRunTerminal,
};
