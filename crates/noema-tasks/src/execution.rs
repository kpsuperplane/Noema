use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{TaskComplexity, WorkDomainError, error::invalid_input, validation::required};

/// Current workspace data admitted for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are stable domain vocabulary")]
pub struct WorkspaceRunContext {
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
}

/// Current project data admitted for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are stable domain vocabulary")]
pub struct ProjectRunContext {
    pub project_id: ProjectId,
    pub name: String,
    pub description: String,
    pub folder: Option<String>,
}

string_enum! {
/// Concrete runtime used for one Executor run.
pub enum TaskExecutorBackend, "task_executor.backend" {
    /// Noema's provider-backed Task Executor.
    Provider => "provider",
    /// A configured Agent Client Protocol process.
    Acp => "acp",
}
}

/// ACP launch configuration resolved when a run starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcpExecutorLaunch {
    /// Revision of the configured ACP agent.
    pub connection_revision: u64,
    /// Executable invoked directly without a shell.
    pub command: String,
    /// Exact executable arguments.
    pub arguments: Vec<String>,
}

/// Executor selected for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskExecutorSelection {
    /// Durable agent identity.
    pub agent_id: String,
    /// Runtime backend.
    pub backend: TaskExecutorBackend,
    /// ACP launch data for an ACP Executor.
    pub acp: Option<AcpExecutorLaunch>,
}

impl TaskExecutorSelection {
    /// Built-in provider Executor.
    #[must_use]
    pub fn provider() -> Self {
        Self {
            agent_id: crate::TASK_EXECUTOR_AGENT_ID.to_string(),
            backend: TaskExecutorBackend::Provider,
            acp: None,
        }
    }

    /// Validate one resolved Executor selection.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when the agent or ACP launch is invalid.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        self.agent_id = required(&self.agent_id, "task_executor.agent_id")?;
        match (&self.backend, &mut self.acp) {
            (TaskExecutorBackend::Provider, None) => Ok(self),
            (TaskExecutorBackend::Acp, Some(acp)) => {
                if acp.connection_revision == 0 {
                    return Err(invalid_input(
                        "task_executor.connection_revision",
                        "ACP connection revision must be positive",
                    ));
                }
                acp.command = required(&acp.command, "task_executor.command")?;
                if acp.command.contains('\0')
                    || acp.arguments.iter().any(|argument| argument.contains('\0'))
                {
                    return Err(invalid_input(
                        "task_executor.command",
                        "ACP launch configuration cannot contain NUL",
                    ));
                }
                Ok(self)
            }
            _ => Err(invalid_input(
                "task_executor",
                "Executor backend and ACP launch must agree",
            )),
        }
    }
}

/// Human direction used to reopen a terminal Task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are stable domain vocabulary")]
pub struct TaskReopenDirection {
    pub feedback_markdown: String,
    pub request_markdown: Option<String>,
    pub complexity: Option<TaskComplexity>,
}

impl TaskReopenDirection {
    /// Normalize new direction before a Task is reopened.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when required text is blank.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        Ok(Self {
            feedback_markdown: required(
                &self.feedback_markdown,
                "reopen_direction.feedback_markdown",
            )?,
            request_markdown: self
                .request_markdown
                .as_deref()
                .map(|value| required(value, "reopen_direction.request_markdown"))
                .transpose()?,
            complexity: self.complexity,
        })
    }
}
