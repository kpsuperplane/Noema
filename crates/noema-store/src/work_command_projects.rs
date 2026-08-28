//! Project command transactions.

use noema_tasks::{
    ArchiveProject, CreateProject, ProjectChangedField, ReopenProject, UpdateProject, WorkCommand,
    WorkDomainError, WorkEventPayload,
};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{Transaction, params};

use super::{WorkCommandService, helpers};
use crate::{StoreError, ids::allocate_id, work_events::append_work_event_tx};

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
                "INSERT INTO projects (project_id, workspace_id, name, description, folder) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project_id.as_str(),
                    workspace_id.as_str(),
                    command.name,
                    command.description,
                    command.folder,
                ],
            )?;
            let payload = WorkEventPayload::project_created(1).map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &workspace_id,
                    Some(&project_id),
                    None,
                    None,
                ),
                payload,
            )?;
            helpers::finish_write_tx(
                transaction,
                &WorkCommand::CreateProject(command.clone()),
                helpers::project_write(event, project_id.clone()),
            )
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
            let folder = match &command.folder {
                None => current.folder.as_deref(),
                Some(folder) => folder.as_deref(),
            };
            let revision = helpers::increment(current.revision, "project.revision")?;
            transaction.execute(
                "UPDATE projects SET name = ?2, description = ?3, folder = ?4, revision = ?5, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE project_id = ?1 AND revision = ?6",
                params![project_id.as_str(), name, description, folder, revision, current.revision],
            )?;
            let mut changed = Vec::new();
            if command.name.is_some() {
                changed.push(ProjectChangedField::Name);
            }
            if command.description.is_some() {
                changed.push(ProjectChangedField::Description);
            }
            if command.folder.is_some() {
                changed.push(ProjectChangedField::Folder);
            }
            if command.project_document_markdown.is_some() {
                changed.push(ProjectChangedField::Document);
            }
            let payload = WorkEventPayload::project_updated(revision, changed)
                .map_err(StoreError::Work)?;
            let event = append_work_event_tx(
                transaction,
                helpers::event_context(&command.meta).scope(
                    &current.workspace_id,
                    Some(&project_id),
                    None,
                    None,
                ),
                payload,
            )?;
            helpers::finish_write_tx(
                transaction,
                &envelope,
                helpers::project_write(event, project_id.clone()),
            )
        })
        .await?;
    Ok(write)
}

async fn archive(
    service: &WorkCommandService,
    command: &ArchiveProject,
) -> Result<helpers::CommandWrite, StoreError> {
    lifecycle(
        service,
        WorkCommand::ArchiveProject(command.clone()),
        &command.precondition.project_id,
        command.precondition.expected_revision,
        &command.meta,
        true,
    )
    .await
}

async fn reopen(
    service: &WorkCommandService,
    command: &ReopenProject,
) -> Result<helpers::CommandWrite, StoreError> {
    lifecycle(
        service,
        WorkCommand::ReopenProject(command.clone()),
        &command.precondition.project_id,
        command.precondition.expected_revision,
        &command.meta,
        false,
    )
    .await
}

async fn lifecycle(
    service: &WorkCommandService,
    envelope: WorkCommand,
    project_id: &ProjectId,
    expected_revision: u64,
    meta: &noema_tasks::CommandMeta,
    archive: bool,
) -> Result<helpers::CommandWrite, StoreError> {
    let project_id = project_id.clone();
    let write = service
        .store
        .with_immediate_transaction_retry(|transaction| {
            if let Some(replay) = helpers::lookup_receipt_tx(transaction, &envelope)? {
                return Ok(replay);
            }
            let current = load_project(transaction, &project_id)?;
            check_project_fence(&current, expected_revision)?;
            if archive && current.archived_at.is_some() {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            if !archive && current.archived_at.is_none() {
                return Err(StoreError::Work(WorkDomainError::InvalidTransition));
            }
            let revision = helpers::increment(current.revision, "project.revision")?;
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
                helpers::event_context(meta).scope(
                    &current.workspace_id,
                    Some(&project_id),
                    None,
                    None,
                ),
                payload,
            )?;
            helpers::finish_write_tx(
                transaction,
                &envelope,
                helpers::project_write(event, project_id.clone()),
            )
        })
        .await?;
    Ok(write)
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
) -> Result<noema_workspaces::ProjectRecord, StoreError> {
    crate::work_reads::rows::load_project_optional(transaction, project_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))
}

fn check_project_fence(
    project: &noema_workspaces::ProjectRecord,
    expected_revision: u64,
) -> Result<(), StoreError> {
    if project.revision == expected_revision {
        Ok(())
    } else {
        Err(StoreError::Work(WorkDomainError::StaleRevision))
    }
}
