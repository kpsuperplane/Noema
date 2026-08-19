use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    NewTaskSchedule, TaskComplexity, TaskGateAnswer, TaskGateId, TaskId, TaskProvenance,
    TaskRecurrenceId, TaskReopenDirection, WorkDomainError, WorkEventId,
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

/// Optimistic revision fence for recurring template changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct RecurrencePrecondition {
    pub recurrence_id: TaskRecurrenceId,
    pub expected_revision: u64,
}

impl RecurrencePrecondition {
    fn validate(&self) -> Result<(), WorkDomainError> {
        if self.expected_revision == 0 {
            return Err(invalid_input(
                "recurrence_precondition",
                "revision must be positive",
            ));
        }
        Ok(())
    }
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
    pub complexity: TaskComplexity,
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
        task_document_markdown: String, project_id: Option<ProjectId>, provenance: TaskProvenance,
        schedule: Option<NewTaskSchedule>, executor_agent_id: Option<String>, cwd_override: Option<String>
    }
    /// Update capture fields while a task remains in Inbox.
    UpdateInboxTask => "task.update_inbox", "Edit Inbox capture fields." {
        meta: CommandMeta, precondition: TaskPrecondition, title: Option<String>,
        task_document_markdown: Option<String>, expected_task_document_digest: Option<String>,
        project_id: Option<Option<ProjectId>>,
        executor_agent_id: Option<String>, cwd_override: Option<Option<String>>
    }
    /// Explicitly authorize an Inbox task for planning/execution.
    QueueTask => "task.queue", "Authorize Queue execution." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Add or replace a future execution instant on an Inbox task.
    ScheduleTask => "task.schedule", "Schedule Inbox task." {
        meta: CommandMeta, precondition: TaskPrecondition, schedule: NewTaskSchedule,
        requires_existing: bool
    }
    /// Remove future execution authorization from an Inbox task.
    UnscheduleTask => "task.unschedule", "Unschedule Inbox task." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Start an already scheduled Inbox task immediately without losing schedule provenance.
    RunScheduledTaskNow => "task.schedule.run_now", "Run scheduled task now." { meta: CommandMeta, precondition: TaskPrecondition }
    /// Change timing or content authority for future recurrence slots.
    UpdateTaskRecurrence => "task.recurrence.update", "Update recurring task." {
        meta: CommandMeta, precondition: RecurrencePrecondition, title: Option<String>,
        task_document_markdown: Option<String>, expected_task_document_digest: Option<String>,
        project_id: Option<Option<ProjectId>>,
        starts_at: Option<i64>, cron_expression: Option<String>, time_zone: Option<String>,
        missed_run_policy: Option<crate::MissedRunPolicy>, overlap_policy: Option<crate::OverlapPolicy>
    }
    /// Pause, resume, skip, or end future recurring materialization.
    ChangeTaskRecurrence => "task.recurrence.change", "Change recurring lifecycle." {
        meta: CommandMeta, precondition: RecurrencePrecondition, action: crate::RecurrenceCommandKind
    }
    /// Materialize one extra immediate occurrence without advancing the recurring schedule.
    RunTaskRecurrenceNow => "task.recurrence.run_now", "Run recurring task now." { meta: CommandMeta, precondition: RecurrencePrecondition }
    /// Resolve an open gate with structured human input.
    AnswerTask => "task.answer", "Resolve a human gate." { meta: CommandMeta, precondition: TaskPrecondition, gate_id: TaskGateId, answer: TaskGateAnswer }
    /// Retry a supported Recovery gate, optionally adding a durable note.
    RetryTask => "task.retry", "Retry a Recovery gate." { meta: CommandMeta, precondition: TaskPrecondition, gate_id: TaskGateId, note: Option<String> }
    /// Cancel any nonterminal task and fence its runnable work.
    CancelTask => "task.cancel", "Cancel active work." { meta: CommandMeta, precondition: TaskPrecondition, reason: Option<String> }
    /// Reopen terminal history into a fresh queued generation with new direction.
    ReopenTask => "task.reopen", "Reopen terminal history." { meta: CommandMeta, precondition: TaskPrecondition, direction: TaskReopenDirection }
    /// Create an active project container.
    CreateProject => "project.create", "Create project." { meta: CommandMeta, workspace_id: WorkspaceId, name: String, description: String, folder: Option<String> }
    /// Update an existing project name/description.
    UpdateProject => "project.update", "Update project." { meta: CommandMeta, precondition: ProjectPrecondition, name: Option<String>, description: Option<String>, folder: Option<Option<String>> }
    /// Archive a project without changing existing task stages.
    ArchiveProject => "project.archive", "Archive project." { meta: CommandMeta, precondition: ProjectPrecondition }
    /// Reopen a previously archived project.
    ReopenProject => "project.reopen", "Reopen project." { meta: CommandMeta, precondition: ProjectPrecondition }
    /// Atomically capture and authorize a task from the primary agent.
    DelegateTask => "task.delegate", "Primary-agent-only capture plus queue composition." {
        meta: CommandMeta, workspace_id: WorkspaceId, title: String,
        task_document_markdown: String, project_id: Option<ProjectId>, provenance: TaskProvenance,
        complexity_hint: Option<TaskComplexity>, execution_intent: Option<DelegateExecutionIntent>,
        executor_agent_id: Option<String>, cwd_override: Option<String>
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
                validate_document(
                    &command.task_document_markdown,
                    "task.task_document_markdown",
                )?;
                command.provenance = command.provenance.normalized()?;
                command.schedule = command
                    .schedule
                    .map(NewTaskSchedule::normalized)
                    .transpose()?;
                command.executor_agent_id = normalize_executor_agent(command.executor_agent_id)?;
                command.cwd_override =
                    normalize_absolute_path(command.cwd_override, "task.cwd_override")?;
                Ok(Self::CaptureTask(command))
            }
            Self::UpdateInboxTask(mut command) => {
                command.precondition.validate()?;
                if command.title.is_none()
                    && command.task_document_markdown.is_none()
                    && command.project_id.is_none()
                    && command.executor_agent_id.is_none()
                    && command.cwd_override.is_none()
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
                validate_document_update(
                    command.task_document_markdown.as_deref(),
                    command.expected_task_document_digest.as_deref(),
                )?;
                command.executor_agent_id = normalize_executor_agent(command.executor_agent_id)?;
                command.cwd_override = command
                    .cwd_override
                    .map(|cwd| normalize_absolute_path(cwd, "task.cwd_override"))
                    .transpose()?;
                Ok(Self::UpdateInboxTask(command))
            }
            Self::QueueTask(command) => {
                command.precondition.validate()?;
                Ok(Self::QueueTask(command))
            }
            Self::ScheduleTask(mut command) => {
                command.precondition.validate()?;
                command.schedule = command.schedule.normalized()?;
                Ok(Self::ScheduleTask(command))
            }
            Self::UnscheduleTask(command) => {
                command.precondition.validate()?;
                Ok(Self::UnscheduleTask(command))
            }
            Self::RunScheduledTaskNow(command) => {
                command.precondition.validate()?;
                Ok(Self::RunScheduledTaskNow(command))
            }
            Self::UpdateTaskRecurrence(mut command) => {
                command.precondition.validate()?;
                if command.title.is_none()
                    && command.task_document_markdown.is_none()
                    && command.project_id.is_none()
                    && command.starts_at.is_none()
                    && command.cron_expression.is_none()
                    && command.time_zone.is_none()
                    && command.missed_run_policy.is_none()
                    && command.overlap_policy.is_none()
                {
                    return Err(invalid_input(
                        "task.recurrence.update",
                        "at least one field is required",
                    ));
                }
                command.title = command
                    .title
                    .map(|value| required(&value, "task.title"))
                    .transpose()?;
                validate_document_update(
                    command.task_document_markdown.as_deref(),
                    command.expected_task_document_digest.as_deref(),
                )?;
                Ok(Self::UpdateTaskRecurrence(command))
            }
            Self::ChangeTaskRecurrence(command) => {
                command.precondition.validate()?;
                Ok(Self::ChangeTaskRecurrence(command))
            }
            Self::RunTaskRecurrenceNow(command) => {
                command.precondition.validate()?;
                Ok(Self::RunTaskRecurrenceNow(command))
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
            Self::CancelTask(mut command) => {
                command.precondition.validate()?;
                command.reason = normalize_optional(command.reason.as_deref());
                Ok(Self::CancelTask(command))
            }
            Self::ReopenTask(mut command) => {
                command.precondition.validate()?;
                command.direction = command.direction.normalized()?;
                Ok(Self::ReopenTask(command))
            }
            Self::CreateProject(mut command) => {
                command.name = required(&command.name, "project.name")?;
                command.description = command.description.trim().to_string();
                command.folder = normalize_absolute_path(command.folder, "project.folder")?;
                Ok(Self::CreateProject(command))
            }
            Self::UpdateProject(mut command) => {
                command.precondition.validate()?;
                if command.name.is_none()
                    && command.description.is_none()
                    && command.folder.is_none()
                {
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
                command.folder = command
                    .folder
                    .map(|folder| normalize_absolute_path(folder, "project.folder"))
                    .transpose()?;
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
                validate_document(
                    &command.task_document_markdown,
                    "task.task_document_markdown",
                )?;
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
                command.executor_agent_id = normalize_executor_agent(command.executor_agent_id)?;
                command.cwd_override =
                    normalize_absolute_path(command.cwd_override, "task.cwd_override")?;
                Ok(Self::DelegateTask(command))
            }
        }
    }
}

fn validate_document(value: &str, field: &'static str) -> Result<(), WorkDomainError> {
    if value.len() > 64 * 1024 {
        return Err(invalid_input(field, "Task document exceeds 64 KiB"));
    }
    Ok(())
}

fn validate_document_update(
    document: Option<&str>,
    digest: Option<&str>,
) -> Result<(), WorkDomainError> {
    if document.is_some() != digest.is_some() {
        return Err(invalid_input(
            "task.expected_task_document_digest",
            "document and expected digest must be present together",
        ));
    }
    if let Some(document) = document {
        validate_document(document, "task.task_document_markdown")?;
    }
    if digest.is_some_and(|value| {
        value.len() != 64
            || value != value.to_ascii_lowercase()
            || value
                .chars()
                .any(|character| !character.is_ascii_hexdigit())
    }) {
        return Err(invalid_input(
            "task.expected_task_document_digest",
            "digest must be lower-case SHA-256",
        ));
    }
    Ok(())
}

fn normalize_executor_agent(value: Option<String>) -> Result<Option<String>, WorkDomainError> {
    value
        .map(|agent_id| required(&agent_id, "task.executor_agent_id"))
        .transpose()
}

fn normalize_absolute_path(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, WorkDomainError> {
    let value = value.map(|path| required(&path, field)).transpose()?;
    if value
        .as_deref()
        .is_some_and(|path| !Path::new(path).is_absolute())
    {
        return Err(invalid_input(field, "path must be absolute"));
    }
    Ok(value)
}

impl DelegateExecutionIntent {
    /// Normalize and validate a complete delegated execution intent.
    /// # Errors
    /// Returns [`WorkDomainError`] when the request is blank.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        self.request_markdown = required(&self.request_markdown, "delegate.request_markdown")?;
        Ok(self)
    }
}

/// Stable result shape returned by every semantic command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkCommandResult {
    pub task: Option<crate::TaskRecord>,
    pub project: Option<noema_workspaces::ProjectRecord>,
    pub gate_id: Option<TaskGateId>,
    pub run_id: Option<String>,
    pub event_id: WorkEventId,
    pub event_sequence: u64,
}
