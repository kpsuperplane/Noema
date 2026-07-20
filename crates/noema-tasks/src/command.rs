use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    NewTaskValidationCriterion, TaskComplexity, TaskContractAmendment, TaskContractId,
    TaskGateAnswer, TaskGateId, TaskId, TaskProvenance, WorkDomainError, WorkEventId,
    criteria::normalize_new_criteria,
    error::invalid_input,
    validation::{optional as normalize_optional, required},
};

/// Common causality and idempotency metadata for every mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct CommandMeta {
    pub actor_id: String,
    pub causation_id: Option<String>,
    pub correlation_id: String,
    pub idempotency_key: Option<String>,
}

impl CommandMeta {
    /// Normalize and validate metadata before command dispatch.
    /// # Errors
    /// Returns [`WorkDomainError`] when metadata, fences, text, provenance, or
    /// command-specific authority constraints are invalid.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let actor_id = required(&self.actor_id, "command.actor_id")?;
        let correlation_id = required(&self.correlation_id, "command.correlation_id")?;
        Ok(Self {
            actor_id,
            causation_id: normalize_optional(self.causation_id.as_deref()),
            correlation_id,
            idempotency_key: normalize_optional(self.idempotency_key.as_deref()),
        })
    }
}

/// Optimistic task revision and execution-generation fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskPrecondition {
    pub task_id: TaskId,
    pub expected_revision: u64,
    pub expected_generation: u64,
}

impl TaskPrecondition {
    /// Validate positive version fences.
    /// # Errors
    /// Returns [`WorkDomainError`] under the same conditions as [`Self::normalized`].
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.expected_revision == 0 || self.expected_generation == 0 {
            return Err(invalid_input(
                "task_precondition",
                "revision and generation must be positive",
            ));
        }
        Ok(())
    }
}

/// Optimistic project revision fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct ProjectPrecondition {
    pub project_id: ProjectId,
    pub expected_revision: u64,
}

impl ProjectPrecondition {
    /// Validate a positive revision fence.
    /// # Errors
    /// Returns [`WorkDomainError`] when either revision or generation is zero.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.expected_revision == 0 {
            return Err(invalid_input(
                "project_precondition.expected_revision",
                "revision must be positive",
            ));
        }
        Ok(())
    }
}

/// Complete normalized intent used by the primary-agent-only DelegateTask composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct DelegateExecutionIntent {
    pub request_markdown: String,
    pub criteria: Vec<NewTaskValidationCriterion>,
    pub complexity: TaskComplexity,
    pub execution_plan_markdown: Option<String>,
}

macro_rules! work_commands {
    ($($(#[$record_meta:meta])* $variant:ident => $name:literal, $variant_doc:literal {
        $($field:ident: $ty:ty),* $(,)?
    })+ $(,)?) => {
        $(
            $(#[$record_meta])*
            #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
            #[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
            pub struct $variant { $(pub $field: $ty),* }
        )+

        /// Semantic Work mutation accepted by the command service.
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum WorkCommand {
            $(#[doc = $variant_doc] $variant($variant)),+
        }

        impl WorkCommand {
            /// Stable command name used by idempotency receipts.
            #[must_use]
            pub const fn name(&self) -> &'static str {
                match self { $(Self::$variant(_) => $name),+ }
            }

            fn meta_mut(&mut self) -> &mut CommandMeta {
                match self { $(Self::$variant(command) => &mut command.meta),+ }
            }
        }
    };
}

work_commands! {
    /// Capture a task into Inbox without authorizing execution.
    CaptureTask => "task.capture", "Capture Inbox task." {
        meta: CommandMeta, workspace_id: WorkspaceId, title: String,
        description_markdown: String, project_id: Option<ProjectId>, provenance: TaskProvenance
    }
    /// Update capture fields while a task remains in Inbox.
    UpdateInboxTask => "task.update_inbox", "Edit Inbox capture fields." {
        meta: CommandMeta, precondition: TaskPrecondition, title: Option<String>,
        description_markdown: Option<String>, project_id: Option<Option<ProjectId>>
    }
    /// Explicitly authorize an Inbox task for planning/execution.
    QueueTask => "task.queue", "Authorize Queue execution." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Resolve an open gate with structured human input.
    AnswerTask => "task.answer", "Resolve a human gate." { meta: CommandMeta, precondition: TaskPrecondition, gate_id: TaskGateId, answer: TaskGateAnswer }
    /// Retry a supported Recovery gate, optionally adding a durable note.
    RetryTask => "task.retry", "Retry a Recovery gate." { meta: CommandMeta, precondition: TaskPrecondition, gate_id: TaskGateId, note: Option<String> }
    /// Accept a reviewer-approved result and move the task to Archive.
    AcceptTask => "task.accept", "Accept a reviewed result." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Human request for a changed result, creating a new contract generation.
    RequestTaskChanges => "task.request_changes", "Request a new contract revision." { meta: CommandMeta, precondition: TaskPrecondition, amendment: TaskContractAmendment }
    /// Cancel any nonterminal task and fence its runnable work.
    CancelTask => "task.cancel", "Cancel active work." { meta: CommandMeta, precondition: TaskPrecondition, reason: Option<String> }
    /// Reopen Archive/Cancelled history into a fresh Inbox generation.
    ReopenTask => "task.reopen", "Reopen terminal history." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Create an active project container.
    CreateProject => "project.create", "Create project." { meta: CommandMeta, workspace_id: WorkspaceId, name: String, description: String }
    /// Update an existing project name/description.
    UpdateProject => "project.update", "Update project." { meta: CommandMeta, precondition: ProjectPrecondition, name: Option<String>, description: Option<String> }
    /// Archive a project without changing existing task stages.
    ArchiveProject => "project.archive", "Archive project." { meta: CommandMeta, precondition: ProjectPrecondition }
    /// Reopen a previously archived project.
    ReopenProject => "project.reopen", "Reopen project." { meta: CommandMeta, precondition: ProjectPrecondition }
    /// Atomically capture and authorize a task from the primary agent.
    DelegateTask => "task.delegate", "Primary-agent-only capture plus queue composition." {
        meta: CommandMeta, workspace_id: WorkspaceId, title: String,
        description_markdown: String, project_id: Option<ProjectId>, provenance: TaskProvenance,
        complexity_hint: Option<TaskComplexity>, execution_intent: Option<DelegateExecutionIntent>
    }
}

impl WorkCommand {
    /// Normalize and validate the complete public command envelope before it
    /// reaches a store.  The store still rechecks durable preconditions inside
    /// its transaction, but no unchecked text or partial intent crosses this
    /// domain seam.
    /// # Errors
    /// Returns [`WorkDomainError`] when the actor or correlation identity is blank.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        let meta = self.meta_mut();
        *meta = meta.normalized()?;
        match self {
            Self::CaptureTask(mut command) => {
                command.title = required(&command.title, "task.title")?;
                command.provenance = command.provenance.normalized()?;
                Ok(Self::CaptureTask(command))
            }
            Self::UpdateInboxTask(mut command) => {
                command.precondition.validate()?;
                if command.title.is_none()
                    && command.description_markdown.is_none()
                    && command.project_id.is_none()
                {
                    return Err(invalid_input(
                        "task.update_inbox",
                        "at least one replacement field is required",
                    ));
                }
                command.title = command
                    .title
                    .map(|value| required(&value, "task.title"))
                    .transpose()?;
                command.description_markdown = command
                    .description_markdown
                    .map(|value| value.trim().to_string());
                Ok(Self::UpdateInboxTask(command))
            }
            Self::QueueTask(command) => {
                command.precondition.validate()?;
                Ok(Self::QueueTask(command))
            }
            Self::AnswerTask(mut command) => {
                command.precondition.validate()?;
                command.answer.message_markdown = required(
                    &command.answer.message_markdown,
                    "task_gate_answer.message_markdown",
                )?;
                Ok(Self::AnswerTask(command))
            }
            Self::RetryTask(mut command) => {
                command.precondition.validate()?;
                command.note = normalize_optional(command.note.as_deref());
                Ok(Self::RetryTask(command))
            }
            Self::AcceptTask(command) => {
                command.precondition.validate()?;
                Ok(Self::AcceptTask(command))
            }
            Self::RequestTaskChanges(mut command) => {
                command.precondition.validate()?;
                command.amendment = command.amendment.normalized()?;
                Ok(Self::RequestTaskChanges(command))
            }
            Self::CancelTask(mut command) => {
                command.precondition.validate()?;
                command.reason = normalize_optional(command.reason.as_deref());
                Ok(Self::CancelTask(command))
            }
            Self::ReopenTask(command) => {
                command.precondition.validate()?;
                Ok(Self::ReopenTask(command))
            }
            Self::CreateProject(mut command) => {
                command.name = required(&command.name, "project.name")?;
                command.description = command.description.trim().to_string();
                Ok(Self::CreateProject(command))
            }
            Self::UpdateProject(mut command) => {
                command.precondition.validate()?;
                if command.name.is_none() && command.description.is_none() {
                    return Err(invalid_input(
                        "project.update",
                        "at least one replacement field is required",
                    ));
                }
                command.name = command
                    .name
                    .map(|value| required(&value, "project.name"))
                    .transpose()?;
                command.description = command.description.map(|value| value.trim().to_string());
                Ok(Self::UpdateProject(command))
            }
            Self::ArchiveProject(command) => {
                command.precondition.validate()?;
                Ok(Self::ArchiveProject(command))
            }
            Self::ReopenProject(command) => {
                command.precondition.validate()?;
                Ok(Self::ReopenProject(command))
            }
            Self::DelegateTask(mut command) => {
                command.title = required(&command.title, "task.title")?;
                command.provenance = command.provenance.normalized()?;
                if command.complexity_hint.is_some() && command.execution_intent.is_some() {
                    return Err(invalid_input(
                        "task.delegate",
                        "complexity hint and complete intent are mutually exclusive",
                    ));
                }
                command.execution_intent = command
                    .execution_intent
                    .map(DelegateExecutionIntent::normalized)
                    .transpose()?;
                Ok(Self::DelegateTask(command))
            }
        }
    }
}

impl DelegateExecutionIntent {
    /// Normalize and validate a complete delegated execution intent.
    /// # Errors
    /// Returns [`WorkDomainError`] when request, plan, or criteria are invalid.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        self.request_markdown = required(&self.request_markdown, "delegate.request_markdown")?;
        self.criteria = normalize_new_criteria(&self.criteria)?;
        self.execution_plan_markdown = self
            .execution_plan_markdown
            .map(|value| required(&value, "delegate.execution_plan_markdown"))
            .transpose()?;
        Ok(self)
    }
}

/// Stable result shape returned by every semantic command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkCommandResult {
    pub task: Option<crate::TaskRecord>,
    pub project: Option<noema_workspaces::ProjectRecord>,
    pub contract_id: Option<TaskContractId>,
    pub gate_id: Option<TaskGateId>,
    pub run_id: Option<String>,
    pub event_id: WorkEventId,
    pub event_sequence: u64,
}
