use noema_providers::ProviderSelectionSnapshot;
use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    NewTaskValidationCriterion, TaskComplexity, TaskContractId, TaskExecutionPolicy, TaskId,
    TaskValidationCriterion, WorkDomainError, criteria::normalize_criteria,
    criteria::normalize_new_criteria, error::invalid_input,
};

/// Bounded immutable context copied into an execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContextSnapshot {
    /// Workspace identity at admission time.
    pub workspace_id: WorkspaceId,
    /// Workspace name at admission time.
    pub name: String,
    /// Workspace description at admission time.
    pub description: String,
}

/// Bounded optional project context copied into an execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContextSnapshot {
    /// Project identity at admission time.
    pub project_id: ProjectId,
    /// Project name at admission time.
    pub name: String,
    /// Project description at admission time.
    pub description: String,
}

/// Origin of an immutable executable contract version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractOrigin {
    /// Supplied as a complete delegated intent.
    Delegated,
    /// Produced by a bounded Planner run.
    Planned,
    /// Created after a human requested changes.
    HumanRevision,
}

impl ContractOrigin {
    /// Return the stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delegated => "delegated",
            Self::Planned => "planned",
            Self::HumanRevision => "human_revision",
        }
    }
}

impl std::fmt::Display for ContractOrigin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ContractOrigin {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "delegated" => Ok(Self::Delegated),
            "planned" => Ok(Self::Planned),
            "human_revision" => Ok(Self::HumanRevision),
            other => Err(invalid_input(
                "contract.origin",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// Immutable executable task contract version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionContract {
    /// Stable contract identity.
    pub contract_id: TaskContractId,
    /// Owning task.
    pub task_id: TaskId,
    /// One-based contract version for this task.
    pub version: u32,
    /// Task generation fenced by this contract.
    pub task_generation: u64,
    /// Prior contract version, when superseded.
    pub supersedes_contract_id: Option<TaskContractId>,
    /// How this version entered the task history.
    pub origin: ContractOrigin,
    /// Immutable normalized request.
    pub request_markdown: String,
    /// Bounded planner guidance, required for Planned contracts.
    pub execution_plan_markdown: Option<String>,
    /// Exact immutable validation criteria.
    pub criteria: Vec<TaskValidationCriterion>,
    /// Complexity selected for this version.
    pub complexity: TaskComplexity,
    /// Executor provider selection snapshot.
    pub executor_model: ProviderSelectionSnapshot,
    /// Reviewer provider selection snapshot.
    pub reviewer_model: ProviderSelectionSnapshot,
    /// Immutable execution safety policy.
    pub execution_policy: TaskExecutionPolicy,
    /// Workspace context snapshot.
    pub workspace_context: WorkspaceContextSnapshot,
    /// Optional project context snapshot.
    pub project_context: Option<ProjectContextSnapshot>,
    /// Actor that created this immutable row.
    pub created_by_actor_id: String,
    /// Creation timestamp.
    pub created_at: String,
}

impl TaskExecutionContract {
    /// Normalize and validate all immutable contract fields.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        if self.version == 0 || self.task_generation == 0 {
            return Err(invalid_input(
                "contract.version",
                "version and generation must be positive",
            ));
        }
        self.request_markdown = required(&self.request_markdown, "contract.request_markdown")?;
        self.execution_plan_markdown = optional_non_empty(
            self.execution_plan_markdown,
            "contract.execution_plan_markdown",
        )?;
        if self.origin == ContractOrigin::Planned && self.execution_plan_markdown.is_none() {
            return Err(invalid_input(
                "contract.execution_plan_markdown",
                "planned contracts require an execution plan",
            ));
        }
        self.criteria = normalize_criteria(&self.criteria)?;
        self.executor_model = self
            .executor_model
            .normalized_for_persistence()
            .map_err(|error| invalid_input("contract.executor_model", error.to_string()))?;
        self.reviewer_model = self
            .reviewer_model
            .normalized_for_persistence()
            .map_err(|error| invalid_input("contract.reviewer_model", error.to_string()))?;
        self.execution_policy = self.execution_policy.validated()?;
        self.workspace_context.name = required(&self.workspace_context.name, "workspace.name")?;
        if self.workspace_context.description.contains('\0') {
            return Err(invalid_input(
                "workspace.description",
                "context cannot contain NUL",
            ));
        }
        if let Some(project) = &mut self.project_context {
            project.name = required(&project.name, "project.name")?;
            if project.description.contains('\0') {
                return Err(invalid_input(
                    "project.description",
                    "context cannot contain NUL",
                ));
            }
        }
        self.created_by_actor_id =
            required(&self.created_by_actor_id, "contract.created_by_actor_id")?;
        self.created_at = required(&self.created_at, "contract.created_at")?;
        Ok(self)
    }

    /// Return whether this contract's immutable identity matches a run fence.
    #[must_use]
    pub fn matches_fence(&self, task_id: &TaskId, generation: u64) -> bool {
        &self.task_id == task_id && self.task_generation == generation
    }
}

/// Human amendment used to create a new immutable contract version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContractAmendment {
    /// Durable human feedback linked to the new version.
    pub feedback_markdown: String,
    /// Optional replacement request; omitted values copy the current request.
    pub request_markdown: Option<String>,
    /// Optional complete replacement criterion set.
    pub replacement_criteria: Option<Vec<NewTaskValidationCriterion>>,
    /// Optional replacement complexity.
    pub complexity: Option<TaskComplexity>,
}

impl TaskContractAmendment {
    /// Normalize amendment input before a new contract is assembled.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let feedback_markdown =
            required(&self.feedback_markdown, "change_request.feedback_markdown")?;
        let request_markdown = self
            .request_markdown
            .as_deref()
            .map(|value| required(value, "change_request.request_markdown"))
            .transpose()?;
        let replacement_criteria = self
            .replacement_criteria
            .as_ref()
            .map(|criteria| normalize_new_criteria(criteria))
            .transpose()?;
        Ok(Self {
            feedback_markdown,
            request_markdown,
            replacement_criteria,
            complexity: self.complexity,
        })
    }
}

fn required(value: &str, field: &'static str) -> Result<String, WorkDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid_input(field, "value cannot be blank"))
    } else {
        Ok(value.to_string())
    }
}

fn optional_non_empty(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, WorkDomainError> {
    value.map(|value| required(&value, field)).transpose()
}
