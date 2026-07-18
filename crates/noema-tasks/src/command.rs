use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    ContractOrigin, NewTaskValidationCriterion, TaskComplexity, TaskContractAmendment,
    TaskContractId, TaskGateAnswer, TaskGateId, TaskId, TaskProvenance, WorkDomainError,
    WorkEventId, criteria::normalize_new_criteria, error::invalid_input,
};

/// Common causality and idempotency metadata for every mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandMeta {
    /// Authenticated/trusted actor identity.
    pub actor_id: String,
    /// Immediate causal action identity.
    pub causation_id: Option<String>,
    /// Required cross-run/user correlation identity.
    pub correlation_id: String,
    /// Optional idempotency key; required by provider tools/outbox delivery.
    pub idempotency_key: Option<String>,
}

impl CommandMeta {
    /// Normalize and validate metadata before command dispatch.
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
pub struct TaskPrecondition {
    /// Target task identity.
    pub task_id: TaskId,
    /// Expected current projection revision.
    pub expected_revision: u64,
    /// Expected current execution generation.
    pub expected_generation: u64,
}

impl TaskPrecondition {
    /// Validate positive version fences.
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
pub struct ProjectPrecondition {
    /// Target project identity.
    pub project_id: ProjectId,
    /// Expected current project revision.
    pub expected_revision: u64,
}

impl ProjectPrecondition {
    /// Validate a positive revision fence.
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

/// Capture a task into Inbox without authorizing execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Personal workspace target.
    pub workspace_id: WorkspaceId,
    /// Concise title.
    pub title: String,
    /// Fuller capture description.
    pub description_markdown: String,
    /// Optional active project.
    pub project_id: Option<ProjectId>,
    /// Durable source/provenance.
    pub provenance: TaskProvenance,
}

/// Update capture fields while a task remains in Inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInboxTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
    /// Optional replacement title.
    pub title: Option<String>,
    /// Optional replacement description.
    pub description_markdown: Option<String>,
    /// `Some(None)` explicitly removes project association.
    pub project_id: Option<Option<ProjectId>>,
}

/// Explicitly authorize an Inbox task for planning/execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
}

/// Resolve an open gate with structured human input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
    /// Current open gate identity.
    pub gate_id: TaskGateId,
    /// Structured answer/approval.
    pub answer: TaskGateAnswer,
}

/// Retry a supported Recovery gate, optionally adding a durable note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
    /// Current open Recovery gate identity.
    pub gate_id: TaskGateId,
    /// Optional retry note.
    pub note: Option<String>,
}

/// Accept a reviewer-approved result and move the task to Completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
}

/// Human request for a changed result, creating a new contract generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTaskChanges {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
    /// Immutable amendment input.
    pub amendment: TaskContractAmendment,
}

/// Cancel any nonterminal task and fence its runnable work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
    /// Optional safe reason, retained without raw provider payloads.
    pub reason: Option<String>,
}

/// Reopen Completed/Cancelled history into a fresh Inbox generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReopenTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Task revision/generation fence.
    pub precondition: TaskPrecondition,
}

/// Create an active project container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProject {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Nonblank project name.
    pub name: String,
    /// Descriptive project text.
    pub description: String,
}

/// Update an existing project name/description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProject {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Project revision fence.
    pub precondition: ProjectPrecondition,
    /// Optional replacement name.
    pub name: Option<String>,
    /// Optional replacement description.
    pub description: Option<String>,
}

/// Archive a project without changing existing task stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveProject {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Project revision fence.
    pub precondition: ProjectPrecondition,
}

/// Reopen a previously archived project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReopenProject {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Project revision fence.
    pub precondition: ProjectPrecondition,
}

/// Complete normalized intent used by the primary-agent-only DelegateTask composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateExecutionIntent {
    /// Complete immutable request.
    pub request_markdown: String,
    /// Exact nonempty validation criteria.
    pub criteria: Vec<NewTaskValidationCriterion>,
    /// Selected complexity.
    pub complexity: TaskComplexity,
    /// Optional bounded planner guidance.
    pub execution_plan_markdown: Option<String>,
}

/// Atomically capture and authorize a task from the primary agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegateTask {
    /// Command metadata.
    pub meta: CommandMeta,
    /// Personal workspace target.
    pub workspace_id: WorkspaceId,
    /// Concise title.
    pub title: String,
    /// Fuller capture description.
    pub description_markdown: String,
    /// Optional active project.
    pub project_id: Option<ProjectId>,
    /// Durable source/provenance.
    pub provenance: TaskProvenance,
    /// Optional planner complexity hint; cannot be supplied with intent.
    pub complexity_hint: Option<TaskComplexity>,
    /// Optional complete contract intent.
    pub execution_intent: Option<DelegateExecutionIntent>,
}

/// Semantic Work mutation accepted by the command service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkCommand {
    /// Capture Inbox task.
    CaptureTask(CaptureTask),
    /// Edit Inbox capture fields.
    UpdateInboxTask(UpdateInboxTask),
    /// Authorize Queue execution.
    QueueTask(QueueTask),
    /// Resolve a human gate.
    AnswerTask(AnswerTask),
    /// Retry a Recovery gate.
    RetryTask(RetryTask),
    /// Accept a reviewed result.
    AcceptTask(AcceptTask),
    /// Request a new contract revision.
    RequestTaskChanges(RequestTaskChanges),
    /// Cancel active work.
    CancelTask(CancelTask),
    /// Reopen terminal history.
    ReopenTask(ReopenTask),
    /// Create project.
    CreateProject(CreateProject),
    /// Update project.
    UpdateProject(UpdateProject),
    /// Archive project.
    ArchiveProject(ArchiveProject),
    /// Reopen project.
    ReopenProject(ReopenProject),
    /// Primary-agent-only capture plus queue composition.
    DelegateTask(DelegateTask),
}

impl WorkCommand {
    /// Stable command name used by idempotency receipts.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::CaptureTask(_) => "task.capture",
            Self::UpdateInboxTask(_) => "task.update_inbox",
            Self::QueueTask(_) => "task.queue",
            Self::AnswerTask(_) => "task.answer",
            Self::RetryTask(_) => "task.retry",
            Self::AcceptTask(_) => "task.accept",
            Self::RequestTaskChanges(_) => "task.request_changes",
            Self::CancelTask(_) => "task.cancel",
            Self::ReopenTask(_) => "task.reopen",
            Self::CreateProject(_) => "project.create",
            Self::UpdateProject(_) => "project.update",
            Self::ArchiveProject(_) => "project.archive",
            Self::ReopenProject(_) => "project.reopen",
            Self::DelegateTask(_) => "task.delegate",
        }
    }

    /// Normalize and validate the complete public command envelope before it
    /// reaches a store.  The store still rechecks durable preconditions inside
    /// its transaction, but no unchecked text or partial intent crosses this
    /// domain seam.
    pub fn normalized(self) -> Result<Self, WorkDomainError> {
        match self {
            Self::CaptureTask(mut command) => {
                command.meta = command.meta.normalized()?;
                command.title = required(&command.title, "task.title")?;
                command.provenance = command.provenance.normalized()?;
                Ok(Self::CaptureTask(command))
            }
            Self::UpdateInboxTask(mut command) => {
                command.meta = command.meta.normalized()?;
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
            Self::QueueTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                Ok(Self::QueueTask(command))
            }
            Self::AnswerTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                command.answer.message_markdown = required(
                    &command.answer.message_markdown,
                    "task_gate_answer.message_markdown",
                )?;
                Ok(Self::AnswerTask(command))
            }
            Self::RetryTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                command.note = normalize_optional(command.note.as_deref());
                Ok(Self::RetryTask(command))
            }
            Self::AcceptTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                Ok(Self::AcceptTask(command))
            }
            Self::RequestTaskChanges(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                command.amendment = command.amendment.normalized()?;
                Ok(Self::RequestTaskChanges(command))
            }
            Self::CancelTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                command.reason = normalize_optional(command.reason.as_deref());
                Ok(Self::CancelTask(command))
            }
            Self::ReopenTask(mut command) => {
                normalize_task_command(&mut command.meta, &command.precondition)?;
                Ok(Self::ReopenTask(command))
            }
            Self::CreateProject(mut command) => {
                command.meta = command.meta.normalized()?;
                command.name = required(&command.name, "project.name")?;
                command.description = command.description.trim().to_string();
                Ok(Self::CreateProject(command))
            }
            Self::UpdateProject(mut command) => {
                command.meta = command.meta.normalized()?;
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
            Self::ArchiveProject(mut command) => {
                normalize_project_command(&mut command.meta, &command.precondition)?;
                Ok(Self::ArchiveProject(command))
            }
            Self::ReopenProject(mut command) => {
                normalize_project_command(&mut command.meta, &command.precondition)?;
                Ok(Self::ReopenProject(command))
            }
            Self::DelegateTask(mut command) => {
                command.meta = command.meta.normalized()?;
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

    /// Validate without retaining the normalized value.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.clone().normalized().map(|_| ())
    }
}

impl DelegateExecutionIntent {
    /// Normalize and validate a complete delegated execution intent.
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

fn normalize_task_command(
    meta: &mut CommandMeta,
    precondition: &TaskPrecondition,
) -> Result<(), WorkDomainError> {
    *meta = meta.normalized()?;
    precondition.validate()
}

fn normalize_project_command(
    meta: &mut CommandMeta,
    precondition: &ProjectPrecondition,
) -> Result<(), WorkDomainError> {
    *meta = meta.normalized()?;
    precondition.validate()
}

/// Stable result shape returned by every semantic command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkCommandResult {
    /// Updated task projection, when task-targeting.
    pub task: Option<crate::TaskRecord>,
    /// Updated project projection, when project-targeting.
    pub project: Option<noema_workspaces::ProjectRecord>,
    /// Contract created by this command, when any.
    pub contract_id: Option<TaskContractId>,
    /// Gate created/resolved by this command, when any.
    pub gate_id: Option<TaskGateId>,
    /// Run queued/affected by this command, when any.
    pub run_id: Option<String>,
    /// Last committed event identity.
    pub event_id: WorkEventId,
    /// Last committed global event sequence.
    pub event_sequence: u64,
}

/// Internal helper preserving the ContractOrigin in delegated intent assembly.
#[must_use]
pub const fn delegated_origin() -> ContractOrigin {
    ContractOrigin::Delegated
}

fn required(value: &str, field: &'static str) -> Result<String, WorkDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid_input(field, "value cannot be blank"))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use noema_workspaces::{ProjectId, WorkspaceId};

    use super::*;
    use crate::{
        NewTaskValidationCriterion, TaskComplexity, TaskGateAnswer, TaskGateId, TaskId,
        TaskProvenance, TaskSourceKind,
    };

    fn meta() -> CommandMeta {
        CommandMeta {
            actor_id: " actor:human ".to_string(),
            causation_id: Some(" cause:one ".to_string()),
            correlation_id: " correlation:one ".to_string(),
            idempotency_key: Some(" idem:one ".to_string()),
        }
    }

    fn task_precondition() -> TaskPrecondition {
        TaskPrecondition {
            task_id: TaskId::new("task:one").unwrap(),
            expected_revision: 1,
            expected_generation: 1,
        }
    }

    fn project_precondition() -> ProjectPrecondition {
        ProjectPrecondition {
            project_id: ProjectId::new("project:one").unwrap(),
            expected_revision: 1,
        }
    }

    fn provenance() -> TaskProvenance {
        TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: " actor:human ".to_string(),
            conversation_id: Some(" conversation:one ".to_string()),
            ..Default::default()
        }
    }

    fn criterion() -> NewTaskValidationCriterion {
        NewTaskValidationCriterion {
            criterion_id: Some("criterion:one".to_string()),
            ordinal: 1,
            description: "The result is complete".to_string(),
            expected_evidence: None,
        }
    }

    #[test]
    fn normalized_command_seam_covers_every_command_family() {
        let task_id = task_precondition();
        let project_id = project_precondition();
        let commands = vec![
            WorkCommand::CaptureTask(CaptureTask {
                meta: meta(),
                workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
                title: " Capture ".to_string(),
                description_markdown: " description ".to_string(),
                project_id: Some(ProjectId::new("project:one").unwrap()),
                provenance: provenance(),
            }),
            WorkCommand::UpdateInboxTask(UpdateInboxTask {
                meta: meta(),
                precondition: task_id.clone(),
                title: Some(" Updated ".to_string()),
                description_markdown: None,
                project_id: None,
            }),
            WorkCommand::QueueTask(QueueTask {
                meta: meta(),
                precondition: task_id.clone(),
            }),
            WorkCommand::AnswerTask(AnswerTask {
                meta: meta(),
                precondition: task_id.clone(),
                gate_id: TaskGateId::new("gate:one").unwrap(),
                answer: TaskGateAnswer {
                    message_markdown: " answer ".to_string(),
                    approval_decision: None,
                },
            }),
            WorkCommand::RetryTask(RetryTask {
                meta: meta(),
                precondition: task_id.clone(),
                gate_id: TaskGateId::new("gate:one").unwrap(),
                note: Some(" retry note ".to_string()),
            }),
            WorkCommand::AcceptTask(AcceptTask {
                meta: meta(),
                precondition: task_id.clone(),
            }),
            WorkCommand::RequestTaskChanges(RequestTaskChanges {
                meta: meta(),
                precondition: task_id.clone(),
                amendment: TaskContractAmendment {
                    feedback_markdown: " feedback ".to_string(),
                    request_markdown: Some(" revised request ".to_string()),
                    replacement_criteria: Some(vec![criterion()]),
                    complexity: Some(TaskComplexity::Medium),
                },
            }),
            WorkCommand::CancelTask(CancelTask {
                meta: meta(),
                precondition: task_id.clone(),
                reason: Some(" cancelled ".to_string()),
            }),
            WorkCommand::ReopenTask(ReopenTask {
                meta: meta(),
                precondition: task_id,
            }),
            WorkCommand::CreateProject(CreateProject {
                meta: meta(),
                workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
                name: " Project ".to_string(),
                description: " description ".to_string(),
            }),
            WorkCommand::UpdateProject(UpdateProject {
                meta: meta(),
                precondition: project_id.clone(),
                name: Some(" Renamed ".to_string()),
                description: None,
            }),
            WorkCommand::ArchiveProject(ArchiveProject {
                meta: meta(),
                precondition: project_id.clone(),
            }),
            WorkCommand::ReopenProject(ReopenProject {
                meta: meta(),
                precondition: project_id,
            }),
            WorkCommand::DelegateTask(DelegateTask {
                meta: meta(),
                workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
                title: " Delegate ".to_string(),
                description_markdown: " request ".to_string(),
                project_id: None,
                provenance: provenance(),
                complexity_hint: Some(TaskComplexity::Simple),
                execution_intent: None,
            }),
        ];

        for command in commands {
            let normalized = command.normalized().unwrap();
            assert_eq!(normalized.name(), command.name());
            assert!(normalized.validate().is_ok());
        }
    }

    #[test]
    fn command_validation_rejects_partial_updates_and_ambiguous_delegation() {
        let mut blank_meta = QueueTask {
            meta: meta(),
            precondition: task_precondition(),
        };
        blank_meta.meta.actor_id = " ".to_string();
        assert!(WorkCommand::QueueTask(blank_meta).normalized().is_err());

        let no_op = WorkCommand::UpdateInboxTask(UpdateInboxTask {
            meta: meta(),
            precondition: task_precondition(),
            title: None,
            description_markdown: None,
            project_id: None,
        });
        assert!(no_op.normalized().is_err());

        let both = WorkCommand::DelegateTask(DelegateTask {
            meta: meta(),
            workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
            title: "Delegate".to_string(),
            description_markdown: String::new(),
            project_id: None,
            provenance: provenance(),
            complexity_hint: Some(TaskComplexity::Simple),
            execution_intent: Some(DelegateExecutionIntent {
                request_markdown: "request".to_string(),
                criteria: vec![criterion()],
                complexity: TaskComplexity::Simple,
                execution_plan_markdown: None,
            }),
        });
        assert!(both.normalized().is_err());

        let incomplete_intent = WorkCommand::DelegateTask(DelegateTask {
            meta: meta(),
            workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
            title: "Delegate".to_string(),
            description_markdown: String::new(),
            project_id: None,
            provenance: provenance(),
            complexity_hint: None,
            execution_intent: Some(DelegateExecutionIntent {
                request_markdown: "request".to_string(),
                criteria: Vec::new(),
                complexity: TaskComplexity::Simple,
                execution_plan_markdown: None,
            }),
        });
        assert!(incomplete_intent.normalized().is_err());
    }
}
