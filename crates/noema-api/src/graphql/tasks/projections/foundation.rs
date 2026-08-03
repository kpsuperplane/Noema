use noema_providers::ProviderSelectionSnapshot;
use noema_tasks::{
    TaskExecutionContract, TaskValidationCriterion, WorkflowDefinition, WorkflowStage,
};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};

use crate::graphql::tasks::GraphqlTaskConnection;

use super::super::{GraphqlTaskComplexity, GraphqlWorkflowStageBehavior};
use super::{
    exact_u64,
    vocabulary::{GraphqlProviderSelectionMode, GraphqlWorkspaceMembershipRole},
};

graphql_object_from! { "Standard connection metadata." => pub struct GraphqlPageInfo("PageInfo")
    from noema_store::WorkPageInfo as value {
    "Cursor for the final edge in this page." => end_cursor: Option<String> = value.end_cursor,
    "Whether another page exists." => has_next_page: bool = value.has_next_page,
} }

graphql_object_from! { "Owner-authorized workspace projection." => pub struct GraphqlWorkspace("Workspace")
    from WorkspaceRecord as value {
    "Opaque workspace identity." => workspace_id: String = value.workspace_id.into_string(),
    "Human-readable name." => name: String = value.name,
    "Descriptive text." => description: String = value.description,
    "Whether this is the Personal workspace." => is_personal: bool = value.is_personal,
    "Membership role for the authenticated owner." => membership_role: GraphqlWorkspaceMembershipRole = GraphqlWorkspaceMembershipRole::Owner,
} }

graphql_object_from! { "Owner-authorized project projection." => pub struct GraphqlProject("Project")
    try_from ProjectRecord as value {
    "Opaque project identity." => project_id: String = value.project_id.into_string(),
    "Owning workspace." => workspace_id: String = value.workspace_id.into_string(),
    "Project name." => name: String = value.name,
    "Project description." => description: String = value.description,
    "Optional absolute project working folder." => folder: Option<String> = value.folder,
    "Optimistic project revision." => revision: i64 = exact_u64(value.revision)?,
    "Archive timestamp, if archived." => archived_at: Option<String> = value.archived_at,
    "Creation timestamp." => created_at: String = value.created_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
} }

graphql_object_from! { "Workflow stage behavior is data-driven rather than inferred from display text." => pub struct GraphqlWorkflowStage("WorkflowStage")
    from WorkflowStage as value {
    "Opaque stage identity." => stage_id: String = value.stage_id.into_string(),
    "Owning workflow." => workflow_id: String = value.workflow_id.into_string(),
    "Machine-stable key." => key: String = value.stable_key,
    "Human-facing stage name." => name: String = value.display_name,
    "Display ordering." => display_order: i64 = i64::from(value.ordinal),
    "Closed runtime behavior." => behavior: GraphqlWorkflowStageBehavior = value.system_behavior.into(),
} }

graphql_object! { "Workflow definition with its available stage metadata." => pub struct GraphqlWorkflow("Workflow") {
    "Opaque workflow identity." => workflow_id: String,
    "Workflow name." => name: String,
    "Stage definitions in display order." => stages: Vec<GraphqlWorkflowStage>,
} }

impl GraphqlWorkflow {
    pub(crate) fn from_parts(value: WorkflowDefinition, stages: Vec<WorkflowStage>) -> Self {
        Self {
            workflow_id: value.workflow_id.into_string(),
            name: value.name,
            stages: stages.into_iter().map(Into::into).collect(),
        }
    }
}

graphql_object_from! { "Immutable provider/model selection captured in a contract or run." => pub struct GraphqlTaskModelSnapshot("TaskModelSnapshot")
    from ProviderSelectionSnapshot as value {
    "Provider family." => provider_kind: String = value.provider_kind,
    "Provider account identity." => provider_account_id: String = value.provider_account_id,
    "Exact process/provider instance identity." => provider_instance_key: Option<String> = value.provider_instance_key.map(|key| key.to_string()),
    "Selection mode." => selection_mode: GraphqlProviderSelectionMode = value.selection_mode.into(),
    "Explicit model profile, when used." => model_profile: Option<String> = value.model_profile,
    "Reasoning effort, when selected." => reasoning_effort: Option<super::super::super::agents::GraphqlReasoningEffort> = value.reasoning_effort.map(Into::into),
    "Selection provenance." => selection_source: Option<String> = value.selection_source,
} }

graphql_object_from! { "Immutable execution policy snapshot." => pub struct GraphqlTaskExecutionPolicy("TaskExecutionPolicy")
    from noema_tasks::TaskExecutionPolicy as value {
    "Provider continuation bound." => max_provider_continuations: i64 = i64::from(value.max_provider_continuations),
    "Tool-call bound." => max_tool_calls: i64 = i64::from(value.max_tool_calls),
    "Active-minute bound." => max_active_minutes: i64 = i64::from(value.max_active_minutes),
    "Progress-audit interval." => progress_audit_interval: i64 = i64::from(value.progress_audit_interval),
    "Automatic retry bound." => max_automatic_retries: i64 = i64::from(value.max_automatic_retries),
    "Review-round bound." => max_review_rounds: i64 = i64::from(value.max_review_rounds),
} }

graphql_object_from! { "One immutable contract criterion." => pub struct GraphqlTaskValidationCriterion("TaskValidationCriterion")
    from TaskValidationCriterion as value {
    "Criterion identity." => criterion_id: String = value.criterion_id,
    "One-based order." => ordinal: i64 = i64::from(value.ordinal),
    "Criterion description." => description: String = value.description,
    "Optional expected evidence." => expected_evidence: Option<String> = value.expected_evidence,
} }

graphql_object_from! { "Immutable task execution contract." => pub struct GraphqlTaskExecutionContract("TaskExecutionContract")
    try_from TaskExecutionContract as value {
    "Immutable request Markdown." => request_markdown: String = value.request_markdown,
    "Exact criteria." => criteria: Vec<GraphqlTaskValidationCriterion> = value.criteria.into_iter().map(Into::into).collect(),
    "Complexity tier." => complexity: GraphqlTaskComplexity = value.complexity.into(),
    "Execution policy snapshot." => execution_policy: GraphqlTaskExecutionPolicy = value.execution_policy.into(),
    "Assigned executor agent identity." => executor_agent_id: String = value.executor.agent_id,
    "Executor backend." => executor_backend: String = value.executor.backend.to_string(),
    "Frozen effective working directory." => effective_cwd: Option<String> = value.effective_cwd,
} }

graphql_object! { "One workflow-driven active board column and its authoritative count." => pub struct GraphqlWorkStageColumn("WorkStageColumn") {
    "Stage metadata used to render the column." => stage: GraphqlWorkflowStage,
    "Number of active tasks in this stage and project scope." => task_count: i64,
} }
graphql_object! { "Transactionally coherent board bootstrap projection." => pub struct GraphqlWorkOverview("WorkOverview") {
    "Authorized workspace metadata." => workspace: GraphqlWorkspace,
    "Default workflow and all ordered stages." => workflow: GraphqlWorkflow,
    "Board-visible columns in workflow order." => board_columns: Vec<GraphqlWorkStageColumn>,
    "Bounded recent active task cards." => recent_tasks: GraphqlTaskConnection,
    "Active cards requiring human attention." => needs_you_count: i64,
} }
