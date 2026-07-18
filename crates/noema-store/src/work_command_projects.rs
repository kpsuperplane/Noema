//! Project command transactions.

use noema_tasks::{
    ArchiveProject, CreateProject, ProjectChangedField, ReopenProject, UpdateProject, WorkCommand,
    WorkDomainError, WorkEventPayload,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{WorkCommandService, helpers};
use crate::{
    StoreError,
    ids::allocate_id,
    work_events::{WorkEventScope, append_work_event_tx},
};

#[derive(Debug, Clone)]
struct ProjectState {
    workspace_id: WorkspaceId,
    name: String,
    description: String,
    revision: u64,
    archived_at: Option<String>,
}

pub(super) async fn execute(
    service: &WorkCommandService,
    command: &WorkCommand,
) -> Result<helpers::CommandWrite, StoreError> {
    match command {
        WorkCommand::CreateProject(value) => create(service, value).await,
        WorkCommand::UpdateProject(value) => update(service, value).await,
        WorkCommand::ArchiveProject(value) => archive(service, value).await,
        WorkCommand::ReopenProject(value) => reopen(service, value).await,
        _ => Err(StoreError::InvariantViolation {
            message: "project writer received a task command".to_string(),
        }),
    }
}

async fn create(
    service: &WorkCommandService,
    command: &CreateProject,
) -> Result<helpers::CommandWrite, StoreError> {
    let project_id = ProjectId::new(allocate_id("project")).map_err(StoreError::Workspace)?;
    let workspace_id = command.workspace_id.clone();
    let write = service
        .store
        .with_immediate_transaction_retry(|transaction| {
            if let Some(replay) = helpers::lookup_receipt_tx(
                transaction,
                &WorkCommand::CreateProject(command.clone()),
            )? {
                return Ok(replay);
            }
            require_personal_workspace(transaction, &workspace_id)?;
            transaction.execute(
                "INSERT INTO projects (project_id, workspace_id, name, description) VALUES (?1, ?2, ?3, ?4)",
                params![
                    project_id.as_str(),
                    workspace_id.as_str(),
                    command.name,
                    command.description,
                ],
            )?;
            let payload = WorkEventPayload::project_created(1).map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                WorkEventScope {
                    workspace_id: workspace_id.clone(),
                    project_id: Some(project_id.clone()),
                    task_id: None,
                    run_id: None,
                    actor_id: command.meta.actor_id.clone(),
                    causation_id: command.meta.causation_id.clone(),
                    correlation_id: command.meta.correlation_id.clone(),
                },
                payload,
            )?;
            let mut write = helpers::write_marker(event, None, Some(project_id.clone()), None, None, None);
            helpers::save_receipt_tx(
                transaction,
                &WorkCommand::CreateProject(command.clone()),
                &mut write,
            )?;
            Ok(write)
        })
        .await?;
    Ok(write)
}

async fn update(
    service: &WorkCommandService,
    command: &UpdateProject,
) -> Result<helpers::CommandWrite, StoreError> {
    let project_id = command.precondition.project_id.clone();
    let write = service
        .store
        .with_immediate_transaction_retry(|transaction| {
            let envelope = WorkCommand::UpdateProject(command.clone());
            if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? {
                return Ok(replay);
            }
            let current = load_project(transaction, &project_id)?;
            check_project_fence(&current, command.precondition.expected_revision)?;
            if current.archived_at.is_some() {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let name = command.name.as_deref().unwrap_or(&current.name);
            let description = command.description.as_deref().unwrap_or(&current.description);
            let revision = current.revision.checked_add(1).ok_or_else(|| {
                StoreError::Work(WorkDomainError::InvalidInput {
                    field: "project.revision",
                    message: "revision overflow".to_string(),
                })
            })?;
            transaction.execute(
                "UPDATE projects SET name = ?2, description = ?3, revision = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE project_id = ?1 AND revision = ?5",
                params![project_id.as_str(), name, description, revision, current.revision],
            )?;
            let mut changed = Vec::new();
            if command.name.is_some() {
                changed.push(ProjectChangedField::Name);
            }
            if command.description.is_some() {
                changed.push(ProjectChangedField::Description);
            }
            let payload = WorkEventPayload::project_updated(revision, changed)
                .map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                WorkEventScope {
                    workspace_id: current.workspace_id.clone(),
                    project_id: Some(project_id.clone()),
                    task_id: None,
                    run_id: None,
                    actor_id: command.meta.actor_id.clone(),
                    causation_id: command.meta.causation_id.clone(),
                    correlation_id: command.meta.correlation_id.clone(),
                },
                payload,
            )?;
            let mut write = helpers::write_marker(event, None, Some(project_id.clone()), None, None, None);
            helpers::save_receipt_tx(transaction, &envelope, &mut write)?;
            Ok(write)
        })
        .await?;
    Ok(write)
}

async fn archive(
    service: &WorkCommandService,
    command: &ArchiveProject,
) -> Result<helpers::CommandWrite, StoreError> {
    lifecycle(service, command, true).await
}

async fn reopen(
    service: &WorkCommandService,
    command: &ReopenProject,
) -> Result<helpers::CommandWrite, StoreError> {
    lifecycle(service, command, false).await
}

async fn lifecycle<C>(
    service: &WorkCommandService,
    command: &C,
    archive: bool,
) -> Result<helpers::CommandWrite, StoreError>
where
    C: ProjectLifecycleCommand + Clone,
{
    let project_id = command.project_id().clone();
    let envelope = command.as_work_command();
    let write = service
        .store
        .with_immediate_transaction_retry(|transaction| {
            if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? {
                return Ok(replay);
            }
            let current = load_project(transaction, &project_id)?;
            check_project_fence(&current, command.expected_revision())?;
            if archive && current.archived_at.is_some() {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            if !archive && current.archived_at.is_none() {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let revision = current.revision.checked_add(1).ok_or_else(|| {
                StoreError::Work(WorkDomainError::InvalidInput {
                    field: "project.revision",
                    message: "revision overflow".to_string(),
                })
            })?;
            transaction.execute(
                "UPDATE projects SET archived_at = CASE WHEN ?2 = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE NULL END, revision = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE project_id = ?1 AND revision = ?4",
                params![project_id.as_str(), archive as i64, revision, current.revision],
            )?;
            let kind = if archive {
                noema_tasks::WorkEventKind::ProjectArchived
            } else {
                noema_tasks::WorkEventKind::ProjectReopened
            };
            let payload = WorkEventPayload::project_lifecycle(kind, revision)
                .map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                WorkEventScope {
                    workspace_id: current.workspace_id.clone(),
                    project_id: Some(project_id.clone()),
                    task_id: None,
                    run_id: None,
                    actor_id: command.actor_id().to_string(),
                    causation_id: command.causation_id().map(ToOwned::to_owned),
                    correlation_id: command.correlation_id().to_string(),
                },
                payload,
            )?;
            let mut write = helpers::write_marker(event, None, Some(project_id.clone()), None, None, None);
            helpers::save_receipt_tx(transaction, &envelope, &mut write)?;
            Ok(write)
        })
        .await?;
    Ok(write)
}

trait ProjectLifecycleCommand {
    fn project_id(&self) -> &ProjectId;
    fn expected_revision(&self) -> u64;
    fn actor_id(&self) -> &str;
    fn causation_id(&self) -> Option<&str>;
    fn correlation_id(&self) -> &str;
    fn as_work_command(&self) -> WorkCommand;
}

impl ProjectLifecycleCommand for ArchiveProject {
    fn project_id(&self) -> &ProjectId {
        &self.precondition.project_id
    }
    fn expected_revision(&self) -> u64 {
        self.precondition.expected_revision
    }
    fn actor_id(&self) -> &str {
        &self.meta.actor_id
    }
    fn causation_id(&self) -> Option<&str> {
        self.meta.causation_id.as_deref()
    }
    fn correlation_id(&self) -> &str {
        &self.meta.correlation_id
    }
    fn as_work_command(&self) -> WorkCommand {
        WorkCommand::ArchiveProject(self.clone())
    }
}

impl ProjectLifecycleCommand for ReopenProject {
    fn project_id(&self) -> &ProjectId {
        &self.precondition.project_id
    }
    fn expected_revision(&self) -> u64 {
        self.precondition.expected_revision
    }
    fn actor_id(&self) -> &str {
        &self.meta.actor_id
    }
    fn causation_id(&self) -> Option<&str> {
        self.meta.causation_id.as_deref()
    }
    fn correlation_id(&self) -> &str {
        &self.meta.correlation_id
    }
    fn as_work_command(&self) -> WorkCommand {
        WorkCommand::ReopenProject(self.clone())
    }
}

fn require_personal_workspace(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
) -> Result<(), StoreError> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id = ?1 AND is_personal = 1 AND archived_at IS NULL)",
        [workspace_id.as_str()],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::WorkUnavailable))
    }
}

fn load_project(
    transaction: &Transaction<'_>,
    project_id: &ProjectId,
) -> Result<ProjectState, StoreError> {
    let row = transaction
        .query_row(
            "SELECT workspace_id, name, description, revision, archived_at FROM projects WHERE project_id = ?1",
            [project_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((workspace, name, description, revision, archived_at)) = row else {
        return Err(StoreError::Work(WorkDomainError::WorkUnavailable));
    };
    Ok(ProjectState {
        workspace_id: WorkspaceId::new(workspace).map_err(StoreError::Workspace)?,
        name,
        description,
        revision: helpers::positive_u64(revision, "project.revision")?,
        archived_at,
    })
}

fn check_project_fence(project: &ProjectState, expected_revision: u64) -> Result<(), StoreError> {
    if project.revision == expected_revision {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::StaleRevision))
    }
}
