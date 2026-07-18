use async_graphql::SimpleObject;
use noema_providers::ProviderSelectionSnapshot;
use noema_tasks::{
    TaskExecutionContract, TaskValidationCriterion, WorkflowDefinition, WorkflowStage,
};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};

use crate::graphql::tasks::GraphqlTaskConnection;

use super::super::{GraphqlTaskComplexity, GraphqlWorkflowStageBehavior};
use super::{
    exact_u64,
    vocabulary::{
        GraphqlProviderSelectionMode, GraphqlTaskContractOrigin, GraphqlWorkspaceMembershipRole,
    },
};

/// Standard connection metadata.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "PageInfo")]
pub struct GraphqlPageInfo {
    /// Cursor for the final edge in this page.
    pub end_cursor: Option<String>,
    /// Whether another page exists.
    pub has_next_page: bool,
}

impl From<noema_store::WorkPageInfo> for GraphqlPageInfo {
    fn from(value: noema_store::WorkPageInfo) -> Self {
        Self {
            end_cursor: value.end_cursor,
            has_next_page: value.has_next_page,
        }
    }
}

/// Owner-authorized workspace projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Workspace")]
pub struct GraphqlWorkspace {
    /// Opaque workspace identity.
    pub workspace_id: String,
    /// Human-readable name.
    pub name: String,
    /// Descriptive text.
    pub description: String,
    /// Whether this is the Personal workspace.
    pub is_personal: bool,
    /// Membership role for the authenticated owner.
    pub membership_role: GraphqlWorkspaceMembershipRole,
}

impl From<WorkspaceRecord> for GraphqlWorkspace {
    fn from(value: WorkspaceRecord) -> Self {
        Self {
            workspace_id: value.workspace_id.into_string(),
            name: value.name,
            description: value.description,
            is_personal: value.is_personal,
            membership_role: GraphqlWorkspaceMembershipRole::Owner,
        }
    }
}

/// Owner-authorized project projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Project")]
pub struct GraphqlProject {
    /// Opaque project identity.
    pub project_id: String,
    /// Owning workspace.
    pub workspace_id: String,
    /// Project name.
    pub name: String,
    /// Project description.
    pub description: String,
    /// Optimistic project revision.
    pub revision: i64,
    /// Archive timestamp, if archived.
    pub archived_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl TryFrom<ProjectRecord> for GraphqlProject {
    type Error = async_graphql::Error;

    fn try_from(value: ProjectRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            project_id: value.project_id.into_string(),
            workspace_id: value.workspace_id.into_string(),
            name: value.name,
            description: value.description,
            revision: exact_u64(value.revision)?,
            archived_at: value.archived_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

/// Workflow stage behavior is data-driven rather than inferred from display text.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkflowStage")]
pub struct GraphqlWorkflowStage {
    /// Opaque stage identity.
    pub stage_id: String,
    /// Owning workflow.
    pub workflow_id: String,
    /// Machine-stable key.
    pub key: String,
    /// Human-facing stage name.
    pub name: String,
    /// Display ordering.
    pub display_order: i64,
    /// Closed runtime behavior.
    pub behavior: GraphqlWorkflowStageBehavior,
}

impl From<WorkflowStage> for GraphqlWorkflowStage {
    fn from(value: WorkflowStage) -> Self {
        Self {
            stage_id: value.stage_id.into_string(),
            workflow_id: value.workflow_id.into_string(),
            key: value.stable_key,
            name: value.display_name,
            display_order: i64::from(value.ordinal),
            behavior: value.system_behavior.into(),
        }
    }
}

/// Workflow definition with its available stage metadata.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Workflow")]
pub struct GraphqlWorkflow {
    /// Opaque workflow identity.
    pub workflow_id: String,
    /// Workflow name.
    pub name: String,
    /// Stage definitions in display order.
    pub stages: Vec<GraphqlWorkflowStage>,
}

impl GraphqlWorkflow {
    pub(crate) fn from_parts(value: WorkflowDefinition, stages: Vec<WorkflowStage>) -> Self {
        Self {
            workflow_id: value.workflow_id.into_string(),
            name: value.name,
            stages: stages.into_iter().map(Into::into).collect(),
        }
    }
}

/// Immutable provider/model selection captured in a contract or run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskModelSnapshot")]
pub struct GraphqlTaskModelSnapshot {
    /// Provider family.
    pub provider_kind: String,
    /// Provider account identity.
    pub provider_account_id: String,
    /// Exact process/provider instance identity.
    pub provider_instance_key: Option<String>,
    /// Selection mode.
    pub selection_mode: GraphqlProviderSelectionMode,
    /// Explicit model profile, when used.
    pub model_profile: Option<String>,
    /// Reasoning effort, when selected.
    pub reasoning_effort: Option<super::super::super::agents::GraphqlReasoningEffort>,
    /// Selection provenance.
    pub selection_source: Option<String>,
}

impl From<ProviderSelectionSnapshot> for GraphqlTaskModelSnapshot {
    fn from(value: ProviderSelectionSnapshot) -> Self {
        Self {
            provider_kind: value.provider_kind,
            provider_account_id: value.provider_account_id,
            provider_instance_key: value.provider_instance_key.map(|key| key.to_string()),
            selection_mode: value.selection_mode.into(),
            model_profile: value.model_profile,
            reasoning_effort: value
                .reasoning_effort
                .map(super::super::super::agents::GraphqlReasoningEffort::from),
            selection_source: value.selection_source,
        }
    }
}

/// Immutable execution policy snapshot.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskExecutionPolicy")]
pub struct GraphqlTaskExecutionPolicy {
    /// Provider continuation bound.
    pub max_provider_continuations: i64,
    /// Tool-call bound.
    pub max_tool_calls: i64,
    /// Active-minute bound.
    pub max_active_minutes: i64,
    /// Progress-audit interval.
    pub progress_audit_interval: i64,
    /// Automatic retry bound.
    pub max_automatic_retries: i64,
    /// Review-round bound.
    pub max_review_rounds: i64,
}

impl From<noema_tasks::TaskExecutionPolicy> for GraphqlTaskExecutionPolicy {
    fn from(value: noema_tasks::TaskExecutionPolicy) -> Self {
        Self {
            max_provider_continuations: i64::from(value.max_provider_continuations),
            max_tool_calls: i64::from(value.max_tool_calls),
            max_active_minutes: i64::from(value.max_active_minutes),
            progress_audit_interval: i64::from(value.progress_audit_interval),
            max_automatic_retries: i64::from(value.max_automatic_retries),
            max_review_rounds: i64::from(value.max_review_rounds),
        }
    }
}

/// One immutable contract criterion.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskValidationCriterion")]
pub struct GraphqlTaskValidationCriterion {
    /// Criterion identity.
    pub criterion_id: String,
    /// One-based order.
    pub ordinal: i64,
    /// Criterion description.
    pub description: String,
    /// Optional expected evidence.
    pub expected_evidence: Option<String>,
}

impl From<TaskValidationCriterion> for GraphqlTaskValidationCriterion {
    fn from(value: TaskValidationCriterion) -> Self {
        Self {
            criterion_id: value.criterion_id,
            ordinal: i64::from(value.ordinal),
            description: value.description,
            expected_evidence: value.expected_evidence,
        }
    }
}

/// Immutable task execution contract.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskExecutionContract")]
pub struct GraphqlTaskExecutionContract {
    /// Contract identity.
    pub contract_id: String,
    /// Contract version.
    pub version: i64,
    /// Task generation fenced by this contract.
    pub task_generation: i64,
    /// Superseded contract, when any.
    pub supersedes_contract_id: Option<String>,
    /// Contract origin.
    pub origin: GraphqlTaskContractOrigin,
    /// Immutable request Markdown.
    pub request_markdown: String,
    /// Planner guidance, when present.
    pub execution_plan_markdown: Option<String>,
    /// Exact criteria.
    pub criteria: Vec<GraphqlTaskValidationCriterion>,
    /// Complexity tier.
    pub complexity: GraphqlTaskComplexity,
    /// Executor selection snapshot.
    pub executor_model: GraphqlTaskModelSnapshot,
    /// Reviewer selection snapshot.
    pub reviewer_model: GraphqlTaskModelSnapshot,
    /// Execution policy snapshot.
    pub execution_policy: GraphqlTaskExecutionPolicy,
    /// Workspace context snapshot.
    pub workspace_context: GraphqlWorkspaceContextSnapshot,
    /// Optional project context snapshot.
    pub project_context: Option<GraphqlProjectContextSnapshot>,
    /// Contract creation timestamp.
    pub created_at: String,
}

/// Bounded workspace context captured by a contract.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkspaceContextSnapshot")]
pub struct GraphqlWorkspaceContextSnapshot {
    /// Workspace identity.
    pub workspace_id: String,
    /// Workspace name.
    pub name: String,
    /// Workspace description.
    pub description: String,
}

/// Bounded project context captured by a contract.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProjectContextSnapshot")]
pub struct GraphqlProjectContextSnapshot {
    /// Project identity.
    pub project_id: String,
    /// Project name.
    pub name: String,
    /// Project description.
    pub description: String,
}

impl TryFrom<TaskExecutionContract> for GraphqlTaskExecutionContract {
    type Error = async_graphql::Error;

    fn try_from(value: TaskExecutionContract) -> Result<Self, Self::Error> {
        Ok(Self {
            contract_id: value.contract_id.into_string(),
            version: i64::from(value.version),
            task_generation: exact_u64(value.task_generation)?,
            supersedes_contract_id: value.supersedes_contract_id.map(|id| id.into_string()),
            origin: value.origin.into(),
            request_markdown: value.request_markdown,
            execution_plan_markdown: value.execution_plan_markdown,
            criteria: value.criteria.into_iter().map(Into::into).collect(),
            complexity: value.complexity.into(),
            executor_model: value.executor_model.into(),
            reviewer_model: value.reviewer_model.into(),
            execution_policy: value.execution_policy.into(),
            workspace_context: GraphqlWorkspaceContextSnapshot {
                workspace_id: value.workspace_context.workspace_id.into_string(),
                name: value.workspace_context.name,
                description: value.workspace_context.description,
            },
            project_context: value
                .project_context
                .map(|context| GraphqlProjectContextSnapshot {
                    project_id: context.project_id.into_string(),
                    name: context.name,
                    description: context.description,
                }),
            created_at: value.created_at,
        })
    }
}

/// One workflow-driven active board column and its authoritative count.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkStageColumn")]
pub struct GraphqlWorkStageColumn {
    /// Stage metadata used to render the column.
    pub stage: GraphqlWorkflowStage,
    /// Number of active tasks in this stage and project scope.
    pub task_count: i64,
}

/// Transactionally coherent board bootstrap projection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkOverview")]
pub struct GraphqlWorkOverview {
    /// Authorized workspace metadata.
    pub workspace: GraphqlWorkspace,
    /// Default workflow and all ordered stages.
    pub workflow: GraphqlWorkflow,
    /// Board-visible active columns in workflow order.
    pub active_columns: Vec<GraphqlWorkStageColumn>,
    /// Bounded recent active task cards.
    pub recent_tasks: GraphqlTaskConnection,
    /// Active cards requiring human attention.
    pub needs_you_count: i64,
}
