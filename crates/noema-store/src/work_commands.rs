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
    /// Ready-provider registry used when a new contract/run snapshot is made.
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
        command: WorkCommand,
    ) -> Result<CommittedWorkCommandResult, StoreError> {
        let command = command.normalized().map_err(StoreError::Work)?;
        let write = execute_normalized_command(self, command).await?;
        crate::work_command_result::materialize_committed_result(write)
    }
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
        WorkCommand::UpdateTaskRecurrence(input) => &input.meta,
        WorkCommand::ChangeTaskRecurrence(input) => &input.meta,
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
            description_markdown: String::new(),
            project_id: None,
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::ChatCapture,
                created_by_actor_id: "actor:human:local".to_string(),
                ..Default::default()
            },
            schedule: None,
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
