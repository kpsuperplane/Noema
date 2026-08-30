//! Semantic Work command service and canonical idempotency helpers.
//!
//! The command service is the only writer for the Work state machine.  The
//! transaction-heavy implementations live beside the row adapters, while this
//! module owns the public service envelope and the request fingerprint rules.

use noema_providers::ProviderRegistryHandle;
use noema_tasks::{WorkCommand, WorkCommandResult};

use crate::{CommittedWorkCommandResult, NoemaStore, StoreError};

#[path = "governed_action_resume.rs"]
mod governed_action_resume;
#[path = "work_command_helpers.rs"]
pub(crate) mod helpers;
#[path = "work_command_human.rs"]
mod human;
#[path = "mcp_auth_resume.rs"]
mod mcp_auth_resume;
#[path = "work_command_projects.rs"]
mod projects;
#[path = "work_command_recovery.rs"]
pub(crate) mod recovery;
#[path = "work_schedules.rs"]
mod schedules;
#[path = "work_command_tasks.rs"]
pub(crate) mod tasks;

/// The sole semantic writer for Work commands.
#[derive(Clone)]
pub struct WorkCommandService {
    /// Canonical SQLite store used by every Work transaction.
    pub(crate) store: NoemaStore,
    /// Ready-provider registry used when a run starts.
    pub(crate) provider_registry: ProviderRegistryHandle,
}

impl std::fmt::Debug for WorkCommandService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkCommandService")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl WorkCommandService {
    /// Construct the semantic writer around one store and provider registry.
    #[must_use]
    pub fn new(store: NoemaStore, provider_registry: ProviderRegistryHandle) -> Self {
        Self {
            store,
            provider_registry,
        }
    }

    /// Borrow the underlying store for bounded read/query helpers.
    #[must_use]
    pub const fn store(&self) -> &NoemaStore {
        &self.store
    }

    /// Normalize and execute one semantic Work command.
    ///
    /// The actual command handlers are kept in this module so every path goes
    /// through the same receipt-first fingerprint boundary.  Keeping this
    /// small wrapper separate also makes it impossible for a future caller to
    /// bypass normalization by invoking an individual SQL helper.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation, authorization, provider selection,
    /// or the atomic SQLite write fails.
    pub async fn execute(&self, command: WorkCommand) -> Result<WorkCommandResult, StoreError> {
        Ok(self.execute_committed(command).await?.result)
    }

    /// Execute a command and return the full task projection captured inside
    /// the same transaction as its receipt and event marker.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] under the same validation and persistence
    /// conditions as [`Self::execute`].
    pub async fn execute_committed(
        &self,
        mut command: WorkCommand,
    ) -> Result<CommittedWorkCommandResult, StoreError> {
        self.attach_current_document(&mut command).await?;
        let command = command.normalized().map_err(StoreError::Work)?;
        let task_document = task_document_seed(&command);
        let project_document = project_document_seed(&command);
        let obsolete_recurrence = self.obsolete_recurrence(&command).await?;
        let project_move = self.prepare_project_move(&command).await?;
        let rollback = self.replace_document_before_command(&command).await?;
        let write = match execute_normalized_command(self, command.clone()).await {
            Ok(write) => write,
            Err(error) => {
                self.restore_document(rollback).await?;
                if let Some(project_move) = project_move {
                    NoemaStore::rollback_project_document_move(project_move)
                        .map_err(project_file_error)?;
                }
                return Err(error);
            }
        };
        if let Some(task_id) = write.task_id.as_ref() {
            if let Some(content) = task_document.as_deref() {
                self.store
                    .ensure_task_document_from(task_id, content)
                    .await
                    .map_err(file_invariant)?;
            }
            if matches!(command, WorkCommand::RunTaskRecurrenceNow(_)) {
                self.copy_occurrence_document(task_id).await?;
            }
            if matches!(
                command,
                WorkCommand::CaptureTask(_) | WorkCommand::ScheduleTask(_)
            ) && let Some(recurrence_id) = self.task_recurrence_id(task_id).await?
            {
                let document = self
                    .store
                    .read_task_document(task_id)
                    .await
                    .map_err(file_invariant)?;
                self.store
                    .ensure_recurrence_document_from(&recurrence_id, &document.content)
                    .await
                    .map_err(file_invariant)?;
            }
        }
        if let Some(recurrence_id) = obsolete_recurrence {
            self.store
                .delete_recurrence_document(&recurrence_id)
                .await
                .map_err(file_invariant)?;
        }
        let mut committed = crate::work_command_result::materialize_committed_result(write)?;
        if let (Some(project), Some(content)) = (
            committed.result.project.as_ref(),
            project_document.as_deref(),
        ) {
            let adopted = self
                .store
                .ensure_project_document_from(project, content)
                .await
                .map_err(project_file_error)?;
            committed.project_document_adopted = Some(adopted);
        }
        if let Some(detail) = committed.task_detail.as_mut() {
            self.store.hydrate_work_task_files(detail).await?;
        }
        Ok(committed)
    }

    async fn attach_current_document(&self, command: &mut WorkCommand) -> Result<(), StoreError> {
        match command {
            WorkCommand::UpdateInboxTask(value)
                if value.meta.actor_id.starts_with("actor:human:")
                    && value.title.is_some()
                    && value.task_document_markdown.is_none() =>
            {
                let current = self
                    .store
                    .read_task_document(&value.precondition.task_id)
                    .await
                    .map_err(file_invariant)?;
                value.task_document_markdown = Some(current.content);
                value.expected_task_document_digest = Some(current.digest);
            }
            WorkCommand::UpdateTaskRecurrence(value)
                if value.meta.actor_id.starts_with("actor:human:")
                    && value.title.is_some()
                    && value.task_document_markdown.is_none() =>
            {
                let current = self
                    .store
                    .read_recurrence_document(&value.precondition.recurrence_id)
                    .await
                    .map_err(file_invariant)?;
                value.task_document_markdown = Some(current.content);
                value.expected_task_document_digest = Some(current.digest);
            }
            _ => {}
        }
        Ok(())
    }

    async fn replace_document_before_command(
        &self,
        command: &WorkCommand,
    ) -> Result<Option<DocumentRollback>, StoreError> {
        match command {
            WorkCommand::UpdateInboxTask(value) => {
                let (Some(content), Some(expected)) = (
                    value.task_document_markdown.as_deref(),
                    value.expected_task_document_digest.as_deref(),
                ) else {
                    return Ok(None);
                };
                let current = self
                    .store
                    .read_task_document(&value.precondition.task_id)
                    .await
                    .map_err(file_invariant)?;
                require_document_digest(&current.digest, expected)?;
                self.store
                    .write_task_file(&value.precondition.task_id, crate::TASK_DOCUMENT, content)
                    .await
                    .map_err(file_invariant)?;
                Ok(Some(DocumentRollback::Task(
                    value.precondition.task_id.clone(),
                    current.content,
                )))
            }
            WorkCommand::UpdateTaskRecurrence(value) => {
                let (Some(content), Some(expected)) = (
                    value.task_document_markdown.as_deref(),
                    value.expected_task_document_digest.as_deref(),
                ) else {
                    return Ok(None);
                };
                let current = self
                    .store
                    .read_recurrence_document(&value.precondition.recurrence_id)
                    .await
                    .map_err(file_invariant)?;
                require_document_digest(&current.digest, expected)?;
                self.store
                    .write_recurrence_document(&value.precondition.recurrence_id, content)
                    .await
                    .map_err(file_invariant)?;
                Ok(Some(DocumentRollback::Recurrence(
                    value.precondition.recurrence_id.clone(),
                    current.content,
                )))
            }
            WorkCommand::UpdateProject(value) => {
                let (Some(content), Some(expected)) = (
                    value.project_document_markdown.as_deref(),
                    value.expected_project_document_digest.as_deref(),
                ) else {
                    return Ok(None);
                };
                let previous = self
                    .store
                    .replace_project_document(&value.precondition.project_id, expected, content)
                    .await
                    .map_err(project_file_error)?;
                Ok(Some(DocumentRollback::Project(
                    value.precondition.project_id.clone(),
                    previous,
                )))
            }
            _ => Ok(None),
        }
    }

    async fn restore_document(&self, rollback: Option<DocumentRollback>) -> Result<(), StoreError> {
        match rollback {
            Some(DocumentRollback::Task(task_id, content)) => self
                .store
                .write_task_file(&task_id, crate::TASK_DOCUMENT, &content)
                .await
                .map_err(file_invariant),
            Some(DocumentRollback::Recurrence(recurrence_id, content)) => self
                .store
                .write_recurrence_document(&recurrence_id, &content)
                .await
                .map_err(file_invariant),
            Some(DocumentRollback::Project(project_id, content)) => self
                .store
                .restore_project_document(&project_id, &content)
                .await
                .map_err(project_file_error),
            None => Ok(()),
        }
    }

    async fn prepare_project_move(
        &self,
        command: &WorkCommand,
    ) -> Result<Option<crate::ProjectFileMove>, StoreError> {
        let WorkCommand::UpdateProject(value) = command else {
            return Ok(None);
        };
        let Some(folder) = value.folder.as_ref() else {
            return Ok(None);
        };
        self.store
            .prepare_project_document_move(&value.precondition.project_id, folder.as_deref())
            .await
            .map(Some)
            .map_err(project_file_error)
    }

    async fn task_recurrence_id(
        &self,
        task_id: &noema_tasks::TaskId,
    ) -> Result<Option<noema_tasks::TaskRecurrenceId>, StoreError> {
        let task_id = task_id.clone();
        self.store
            .with_connection(|connection| {
                let value = connection.query_row(
                    "SELECT recurrence_id FROM tasks WHERE task_id = ?1",
                    [task_id.as_str()],
                    |row| row.get::<_, Option<String>>(0),
                )?;
                value
                    .map(noema_tasks::TaskRecurrenceId::new)
                    .transpose()
                    .map_err(StoreError::Work)
            })
            .await
    }

    async fn obsolete_recurrence(
        &self,
        command: &WorkCommand,
    ) -> Result<Option<noema_tasks::TaskRecurrenceId>, StoreError> {
        let task_id = match command {
            WorkCommand::ScheduleTask(value) => Some(&value.precondition.task_id),
            WorkCommand::UnscheduleTask(value) => Some(&value.precondition.task_id),
            _ => None,
        };
        match task_id {
            Some(task_id) => self.task_recurrence_id(task_id).await,
            None => Ok(None),
        }
    }
}

enum DocumentRollback {
    Task(noema_tasks::TaskId, String),
    Recurrence(noema_tasks::TaskRecurrenceId, String),
    Project(noema_workspaces::ProjectId, String),
}

fn require_document_digest(actual: &str, expected: &str) -> Result<(), StoreError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::Work(
            noema_tasks::WorkDomainError::StaleDocument,
        ))
    }
}

fn file_invariant(error: crate::TaskFileError) -> StoreError {
    StoreError::InvariantViolation {
        message: error.to_string(),
    }
}

fn project_file_error(error: crate::ProjectFileError) -> StoreError {
    match error {
        crate::ProjectFileError::StaleDigest => {
            StoreError::Work(noema_tasks::WorkDomainError::StaleDocument)
        }
        crate::ProjectFileError::Conflict => {
            StoreError::Work(noema_tasks::WorkDomainError::InvalidInput {
                field: "project.folder",
                message: "The destination folder contains a different PROJECT.md".to_string(),
            })
        }
        error => StoreError::InvariantViolation {
            message: error.to_string(),
        },
    }
}

fn task_document_seed(command: &WorkCommand) -> Option<String> {
    match command {
        WorkCommand::CaptureTask(command) => Some(command.task_document_markdown.clone()),
        WorkCommand::DelegateTask(command) => Some(command.task_document_markdown.clone()),
        _ => None,
    }
}

fn project_document_seed(command: &WorkCommand) -> Option<String> {
    let WorkCommand::CreateProject(command) = command else {
        return None;
    };
    Some(
        command
            .project_document_markdown
            .clone()
            .unwrap_or_else(|| {
                crate::project_files::default_project_document(&command.name, &command.description)
            }),
    )
}

/// Return the canonical command fingerprint used by idempotency receipts.
///
/// `WorkCommand` is serialized only after domain normalization.  Serde emits
/// struct fields in declaration order, so the resulting byte sequence is
/// deterministic for semantically equivalent commands.  The digest is lower
/// case hexadecimal, matching the `work_command_receipts` schema check.
pub(crate) fn canonical_command_fingerprint(command: &WorkCommand) -> Result<String, StoreError> {
    json_fingerprint(command)
}

/// Fingerprint the normalized Delegate payload independently from its caller
/// idempotency namespace. Source conversation/tool-call identity is a second
/// durable retry key, so transport retry metadata does not participate while
/// actor, source provenance, and semantic task content remain exact.
pub(crate) fn delegate_source_fingerprint(
    command: &noema_tasks::DelegateTask,
) -> Result<String, StoreError> {
    let mut command = command.clone();
    normalize_source_replay_meta(&mut command.meta);
    json_fingerprint(&WorkCommand::DelegateTask(command))
}

/// Fingerprint a normalized Capture independently from transport retry metadata.
pub(crate) fn capture_source_fingerprint(
    command: &noema_tasks::CaptureTask,
) -> Result<String, StoreError> {
    let mut command = command.clone();
    normalize_source_replay_meta(&mut command.meta);
    json_fingerprint(&WorkCommand::CaptureTask(command))
}

fn normalize_source_replay_meta(meta: &mut noema_tasks::CommandMeta) {
    meta.causation_id = None;
    meta.correlation_id = "source-replay".to_string();
    meta.idempotency_key = None;
}

fn json_fingerprint(value: &impl serde::Serialize) -> Result<String, StoreError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(crate::work_row::sha256_hex(&bytes))
}

/// Return the command's optional idempotency key without inferring intent from
/// any human text.  Provider/tool commands can require this at their boundary;
/// UI commands may intentionally remain receipt-free.
pub(crate) fn command_idempotency_key(command: &WorkCommand) -> Option<&str> {
    command_meta(command).idempotency_key.as_deref()
}

/// Return the normalized actor identity carried by a command.
pub(crate) fn command_actor_id(command: &WorkCommand) -> &str {
    &command_meta(command).actor_id
}

fn command_meta(command: &WorkCommand) -> &noema_tasks::CommandMeta {
    match command {
        WorkCommand::CaptureTask(input) => &input.meta,
        WorkCommand::UpdateInboxTask(input) => &input.meta,
        WorkCommand::QueueTask(input) => &input.meta,
        WorkCommand::ScheduleTask(input) => &input.meta,
        WorkCommand::UnscheduleTask(input) => &input.meta,
        WorkCommand::RunScheduledTaskNow(input) => &input.meta,
        WorkCommand::UpdateTaskRecurrence(input) => &input.meta,
        WorkCommand::ChangeTaskRecurrence(input) => &input.meta,
        WorkCommand::RunTaskRecurrenceNow(input) => &input.meta,
        WorkCommand::AnswerTask(input) => &input.meta,
        WorkCommand::RetryTask(input) => &input.meta,
        WorkCommand::CancelTask(input) => &input.meta,
        WorkCommand::ReopenTask(input) => &input.meta,
        WorkCommand::CreateProject(input) => &input.meta,
        WorkCommand::UpdateProject(input) => &input.meta,
        WorkCommand::ArchiveProject(input) => &input.meta,
        WorkCommand::ReopenProject(input) => &input.meta,
        WorkCommand::DelegateTask(input) => &input.meta,
    }
}

/// Dispatch a normalized command to the semantic transaction handlers.
///
/// This declaration is intentionally private: every caller enters through
/// [`WorkCommandService::execute`].  The handler is filled by the write-path
/// modules once row adapters are available.
async fn execute_normalized_command(
    service: &WorkCommandService,
    command: WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    match &command {
        WorkCommand::CreateProject(_)
        | WorkCommand::UpdateProject(_)
        | WorkCommand::ArchiveProject(_)
        | WorkCommand::ReopenProject(_) => projects::execute(service, &command).await,
        WorkCommand::AnswerTask(_)
        | WorkCommand::RetryTask(_)
        | WorkCommand::CancelTask(_)
        | WorkCommand::ReopenTask(_) => human::execute(service, &command).await,
        _ => tasks::execute(service, &command).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_tasks::{CaptureTask, CommandMeta, TaskProvenance, TaskSourceKind};
    use noema_workspaces::WorkspaceId;

    fn command(key: Option<&str>) -> WorkCommand {
        WorkCommand::CaptureTask(CaptureTask {
            meta: CommandMeta {
                actor_id: "actor:human:local".to_string(),
                causation_id: None,
                correlation_id: "correlation:test".to_string(),
                idempotency_key: key.map(ToOwned::to_owned),
            },
            workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
            title: " capture ".to_string(),
            task_document_markdown: String::new(),
            project_id: None,
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::ChatCapture,
                created_by_actor_id: "actor:human:local".to_string(),
                ..Default::default()
            },
            schedule: None,
            executor_agent_id: None,
            cwd_override: None,
        })
    }

    #[test]
    fn normalized_commands_have_stable_sha256_fingerprints() {
        let first = command(Some("idem:one")).normalized().unwrap();
        let second = command(Some("idem:one")).normalized().unwrap();
        assert_eq!(
            canonical_command_fingerprint(&first).unwrap(),
            canonical_command_fingerprint(&second).unwrap()
        );
        assert_eq!(canonical_command_fingerprint(&first).unwrap().len(), 64);
    }
}
