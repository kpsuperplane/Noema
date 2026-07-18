//! SQLite-backed canonical Noema store.

mod agent_runtime_preferences;
mod agents;
mod artifact_metadata_port;
mod artifact_writes;
mod artifacts;
mod auxiliary_model_preferences;
mod context_summaries;
mod conversations;
mod error;
mod ids;
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
mod memory_repository;
mod memory_service;
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
mod run_items;
mod runtime;
mod schema;
mod sqlite;
mod task_execution_policy;
mod task_model_pools;
mod tasks;
mod work_command_result;
mod work_commands;
mod work_events;
mod work_notifications;
mod work_reads;
mod work_reconciliation;
mod work_records;
mod work_run_context;
mod work_run_context_admission;
mod work_run_context_records;
mod work_runs;

#[cfg(test)]
mod work_command_tests;
#[cfg(test)]
mod work_notification_tests;
#[cfg(test)]
mod work_planner_terminal_tests;
#[cfg(test)]
mod work_read_tests;
#[cfg(test)]
mod work_reconciliation_tests;
#[cfg(test)]
mod work_run_context_tests;
#[cfg(test)]
mod work_run_tests;
#[cfg(test)]
mod work_terminal_tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
pub(crate) mod tests;

pub use agent_runtime_preferences::{AgentRuntimePreferenceRecord, NewAgentRuntimePreference};
pub use agents::{AgentRecord, AgentSystemRole};
pub use auxiliary_model_preferences::{
    AuxiliaryModelPreferenceRecord, NewAuxiliaryModelPreference, TOOL_PROGRESS_AUDIT_TASK_ID,
    WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use error::{SchemaIncompatibility, StoreError};
pub use runtime::{NoemaStore, StoreConfig};
pub use work_command_result::CommittedWorkCommandResult;
pub use work_commands::WorkCommandService;
pub use work_notifications::{
    ClaimedWorkNotification, CompleteWorkNotification, FailWorkNotification,
    WorkNotificationLeaseRequest,
};
pub use work_reconciliation::{
    ApplyReconciliation, ReconciliationOutcome, action_run_kind, plan_snapshot,
    plan_work_reconciliation,
};
pub use work_records::{
    ProjectConnection, ProjectCursor, ProjectEdge, ProjectQuery, WorkCommandReceiptRecord,
    WorkContractConnection, WorkContractCursor, WorkContractEdge, WorkContractHistoryQuery,
    WorkCursorError, WorkEventBeforeQuery, WorkEventConnection, WorkEventCursor, WorkEventEdge,
    WorkEventQuery, WorkGateConnection, WorkGateCursor, WorkGateEdge, WorkGateHistoryQuery,
    WorkMessageConnection, WorkMessageCursor, WorkMessageEdge, WorkMessageHistoryQuery,
    WorkNotificationRecord, WorkNotificationStatus, WorkOverview, WorkOverviewQuery, WorkPageInfo,
    WorkPageSize, WorkReconciliationEnvelope, WorkReviewConnection, WorkReviewCursor,
    WorkReviewEdge, WorkReviewHistoryQuery, WorkRunConnection, WorkRunCursor, WorkRunEdge,
    WorkRunHistoryQuery, WorkRunItemConnection, WorkRunItemCursor, WorkRunItemEdge,
    WorkRunItemOwnerScope, WorkRunItemQuery, WorkStageTaskCount, WorkSubmissionConnection,
    WorkSubmissionCursor, WorkSubmissionEdge, WorkSubmissionHistoryQuery, WorkTaskArtifact,
    WorkTaskArtifactConnection, WorkTaskArtifactCursor, WorkTaskArtifactEdge,
    WorkTaskArtifactQuery, WorkTaskAttention, WorkTaskConnection, WorkTaskCursor, WorkTaskDetail,
    WorkTaskEdge, WorkTaskQuery, WorkTaskScope, WorkTaskSummary, WorkTaskValidAction,
    WorkWorkflowWithStages,
};
pub use work_run_context_records::{
    WORK_RUN_CONTEXT_MAX_CRITERIA, WORK_RUN_CONTEXT_MAX_GATES,
    WORK_RUN_CONTEXT_MAX_ITEMS_PER_LINEAGE_RUN, WORK_RUN_CONTEXT_MAX_LINEAGE_RUNS,
    WORK_RUN_CONTEXT_MAX_MESSAGES, WorkRunContextAdmission, WorkRunExecutionContext,
};
pub use work_runs::{
    ClaimedWorkRun, CompletePlan, PlanTerminal, ReportRunFailure, ReportTaskBlocked, SubmitPlan,
    SubmitTaskResult, SubmitTaskReview, WorkRunFence, WorkRunProgress, WorkRunTerminal,
};
