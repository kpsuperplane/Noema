use noema_providers::ProviderSelectionSnapshot;
use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    NewTaskValidationCriterion, TaskComplexity, TaskContractId, TaskExecutionPolicy, TaskId,
    TaskValidationCriterion, WorkDomainError, criteria::normalize_criteria,
    criteria::normalize_new_criteria, error::invalid_input, validation::required,
};

/// Bounded immutable context copied into an execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkspaceContextSnapshot {
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
}

/// Bounded optional project context copied into an execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct ProjectContextSnapshot {
    pub project_id: ProjectId,
    pub name: String,
    pub description: String,
    pub folder: Option<String>,
}

string_enum! {
/// Concrete runtime used for one immutable Executor contract.
pub enum TaskExecutorBackend, "contract.executor.backend" {
    /// Noema's existing provider-backed task executor.
    Provider => "provider",
    /// A configured Agent Client Protocol process.
    Acp => "acp",
}
}

/// Immutable ACP launch configuration captured by a task contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcpExecutorSnapshot {
    /// Revision of the configured ACP agent.
    pub connection_revision: u64,
    /// Executable invoked directly without a shell.
    pub command: String,
    /// Exact executable arguments.
    pub arguments: Vec<String>,
}

/// Immutable executor selection for one contract and its runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskExecutorSelection {
    /// Durable agent identity.
    pub agent_id: String,
    /// Runtime backend.
    pub backend: TaskExecutorBackend,
    /// ACP launch snapshot, present only for the ACP backend.
    pub acp: Option<AcpExecutorSnapshot>,
}

impl TaskExecutorSelection {
    /// Built-in provider executor used by existing tasks.
    #[must_use]
    pub fn provider() -> Self {
        Self {
            agent_id: crate::TASK_EXECUTOR_AGENT_ID.to_string(),
            backend: TaskExecutorBackend::Provider,
            acp: None,
        }
    }

    fn normalized(mut self) -> Result<Self, WorkDomainError> {
        self.agent_id = required(&self.agent_id, "contract.executor.agent_id")?;
        match (&self.backend, &mut self.acp) {
            (TaskExecutorBackend::Provider, None) => Ok(self),
            (TaskExecutorBackend::Acp, Some(acp)) => {
                if acp.connection_revision == 0 {
                    return Err(invalid_input(
                        "contract.executor.connection_revision",
                        "ACP connection revision must be positive",
                    ));
                }
                acp.command = required(&acp.command, "contract.executor.command")?;
                if acp.command.contains('\0')
                    || acp.arguments.iter().any(|argument| argument.contains('\0'))
                {
                    return Err(invalid_input(
                        "contract.executor.command",
                        "ACP launch configuration cannot contain NUL",
                    ));
                }
                Ok(self)
            }
            _ => Err(invalid_input(
                "contract.executor",
                "executor backend and ACP snapshot must agree",
            )),
        }
    }
}

string_enum! {
/// Origin of an immutable executable contract version.
pub enum ContractOrigin, "contract.origin" {
    /// Supplied as a complete delegated intent.
    Delegated => "delegated",
    /// Produced by a bounded Planner run.
    Planned => "planned",
    /// Created after a human requested changes.
    HumanRevision => "human_revision",
}
}

/// Immutable executable task contract version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskExecutionContract {
    pub contract_id: TaskContractId,
    pub task_id: TaskId,
    pub version: u32,
    pub task_generation: u64,
    pub supersedes_contract_id: Option<TaskContractId>,
    pub origin: ContractOrigin,
    pub request_markdown: String,
    pub execution_plan_markdown: Option<String>,
    pub criteria: Vec<TaskValidationCriterion>,
    pub complexity: TaskComplexity,
    pub executor_model: ProviderSelectionSnapshot,
    pub executor: TaskExecutorSelection,
    pub effective_cwd: Option<String>,
    pub reviewer_model: ProviderSelectionSnapshot,
    pub execution_policy: TaskExecutionPolicy,
    pub workspace_context: WorkspaceContextSnapshot,
    pub project_context: Option<ProjectContextSnapshot>,
    pub created_by_actor_id: String,
    pub created_at: String,
}

impl TaskExecutionContract {
    /// Normalize and validate all immutable contract fields.
    /// # Errors
    /// Returns [`WorkDomainError`] when versioning, request or plan text,
    /// criteria, provider selections, execution policy, context, actor, or
    /// timestamp fields violate the immutable contract invariants.
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
        self.executor = self.executor.normalized()?;
        self.effective_cwd = self
            .effective_cwd
            .take()
            .map(|cwd| required(&cwd, "contract.effective_cwd"))
            .transpose()?;
        if self
            .effective_cwd
            .as_deref()
            .is_some_and(|cwd| !Path::new(cwd).is_absolute())
        {
            return Err(invalid_input(
                "contract.effective_cwd",
                "working directory must be absolute",
            ));
        }
        if self.executor.backend == TaskExecutorBackend::Acp && self.effective_cwd.is_none() {
            return Err(invalid_input(
                "contract.effective_cwd",
                "ACP executors require a working directory",
            ));
        }
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
            if project
                .folder
                .as_deref()
                .is_some_and(|folder| folder.contains('\0') || !Path::new(folder).is_absolute())
            {
                return Err(invalid_input(
                    "project.folder",
                    "project folder must be absolute",
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
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskContractAmendment {
    pub feedback_markdown: String,
    pub request_markdown: Option<String>,
    pub replacement_criteria: Option<Vec<NewTaskValidationCriterion>>,
    pub complexity: Option<TaskComplexity>,
}

impl TaskContractAmendment {
    /// Normalize amendment input before a new contract is assembled.
    /// # Errors
    /// Returns [`WorkDomainError`] when versioning, request or plan text,
    /// criteria, provider selections, execution policy, context, actor, or
    /// timestamp fields violate the immutable contract invariants.
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

fn optional_non_empty(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, WorkDomainError> {
    value.map(|value| required(&value, field)).transpose()
}
