//! Bounded read projections for the canonical Work store.

#[path = "work_read_artifacts.rs"]
pub(crate) mod artifacts;
#[path = "work_read_events.rs"]
mod events;
#[path = "work_read_history.rs"]
pub(crate) mod history;
#[path = "work_read_list.rs"]
mod list;
#[path = "work_read_list_rows.rs"]
pub(crate) mod list_rows;
#[path = "work_read_overview.rs"]
mod overview;
#[path = "work_read_rows.rs"]
pub(crate) mod rows;
#[path = "work_read_task.rs"]
pub(crate) mod task;

use noema_tasks::{TaskId, WorkDomainError};
use noema_workspaces::{ProjectId, ProjectRecord, WorkspaceId};
use rusqlite::{OptionalExtension, Row, params, types::Type};

use crate::{
    NoemaStore, ProjectConnection, ProjectCursor, ProjectEdge, ProjectQuery, StoreError,
    WorkEventConnection, WorkEventCursor, WorkEventEdge, WorkEventQuery, WorkPageInfo,
    WorkTaskDetail, WorkTaskSummary,
    sqlite::conversion_failure,
    work_events::{WORK_EVENT_COLUMNS, decode_work_event_record},
};
use task::load_task_facts;

const PROJECT_COLUMNS: &str = "
    project_id,
    workspace_id,
    name,
    description,
    folder,
    revision,
    archived_at,
    created_at,
    updated_at
";

impl NoemaStore {
    /// Load one task summary without task files or history.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite fails or persisted task links are invalid.
    pub async fn get_work_task_summary(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkTaskSummary>, StoreError> {
        let task_id = task_id.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            load_task_facts(&transaction, &task_id)
                .map(|task| task.map(task::LoadedWorkTask::into_summary))
        })
        .await
    }

    /// Read one project by its exact identity within a workspace.
    ///
    /// Archived projects are included because this read is used for
    /// authorization, where a caller must receive the same result for an
    /// active or archived project while still being fenced to its workspace.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot execute the read or a
    /// persisted project is invalid.
    pub async fn get_work_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> Result<Option<ProjectRecord>, StoreError> {
        let workspace_id = workspace_id.as_str().to_owned();
        let project_id = project_id.as_str().to_owned();
        self.with_connection(move |conn| {
            conn.query_row(
                &format!(
                    "SELECT {PROJECT_COLUMNS}
                     FROM projects
                     WHERE workspace_id = ?1 AND project_id = ?2
                     LIMIT 1"
                ),
                params![workspace_id, project_id],
                decode_project_record,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List one workspace's projects in descending update order using an
    /// exclusive, query-bound keyset cursor.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the cursor belongs to a different query,
    /// SQLite cannot execute the read, or a persisted project is invalid.
    pub async fn list_work_projects(
        &self,
        query: ProjectQuery,
    ) -> Result<ProjectConnection, StoreError> {
        let query_hash = project_query_hash(&query.workspace_id, query.include_archived);
        if query
            .after
            .as_ref()
            .is_some_and(|cursor| cursor.query_hash != query_hash)
        {
            return Err(invalid_project_cursor());
        }

        let first = query.first.get();
        let limit = i64::try_from(first + 1).map_err(|error| StoreError::InvariantViolation {
            message: format!("project page size could not be represented in SQLite: {error}"),
        })?;
        let after_updated_at = query.after.as_ref().map(|cursor| cursor.updated_at.clone());
        let after_project_id = query
            .after
            .as_ref()
            .map(|cursor| cursor.project_id.as_str().to_owned());
        let workspace_id = query.workspace_id.into_string();
        let include_archived = i64::from(query.include_archived);

        self.with_connection(move |conn| {
            let sql = format!(
                "SELECT {PROJECT_COLUMNS}
                 FROM projects
                 WHERE workspace_id = ?1
                   AND (?2 = 1 OR archived_at IS NULL)
                   AND (
                       ?3 IS NULL
                       OR updated_at < ?3
                       OR (updated_at = ?3 AND project_id < ?4)
                   )
                 ORDER BY updated_at DESC, project_id DESC
                 LIMIT ?5"
            );
            let mut statement = conn.prepare(&sql)?;
            let mapped = statement.query_map(
                params![
                    workspace_id,
                    include_archived,
                    after_updated_at,
                    after_project_id,
                    limit,
                ],
                decode_project_record,
            )?;
            let mut projects = mapped.collect::<Result<Vec<_>, _>>()?;
            let has_next_page = projects.len() > first;
            projects.truncate(first);

            let edges = projects
                .into_iter()
                .map(|node| {
                    let cursor = ProjectCursor::new(
                        query_hash.clone(),
                        node.updated_at.clone(),
                        node.project_id.clone(),
                    )
                    .map_err(|_| invalid_project_cursor())?
                    .encode();
                    Ok(ProjectEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            let end_cursor = edges.last().map(|edge| edge.cursor.clone());

            Ok(ProjectConnection {
                edges,
                page_info: WorkPageInfo {
                    end_cursor,
                    has_next_page,
                },
            })
        })
        .await
    }

    /// Replay a filtered slice of the global Work ledger in ascending sequence
    /// order from an exclusive cursor.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite cannot execute the read or a
    /// persisted event fails its closed kind, payload, identity, or scope
    /// validation.
    pub async fn list_work_events_after(
        &self,
        query: WorkEventQuery,
    ) -> Result<WorkEventConnection, StoreError> {
        let first = query.first.get();
        let limit = i64::try_from(first + 1).map_err(|error| StoreError::InvariantViolation {
            message: format!("work event page size could not be represented in SQLite: {error}"),
        })?;
        let after = query.after.map_or(0, WorkEventCursor::sequence);
        let after = i64::try_from(after).map_err(|error| StoreError::InvariantViolation {
            message: format!("work event cursor could not be represented in SQLite: {error}"),
        })?;
        let workspace_id = query.workspace_id.into_string();
        let project_id = query.project_id.map(ProjectId::into_string);
        let task_id = query.task_id.map(TaskId::into_string);
        let run_id = query.run_id;

        self.with_connection(move |conn| {
            let sql = format!(
                "SELECT {WORK_EVENT_COLUMNS}
                 FROM work_events
                 WHERE event_sequence > ?1
                   AND workspace_id = ?2
                   AND (?3 IS NULL OR project_id = ?3)
                   AND (?4 IS NULL OR task_id = ?4)
                   AND (?5 IS NULL OR run_id = ?5)
                 ORDER BY event_sequence ASC
                 LIMIT ?6"
            );
            let mut statement = conn.prepare(&sql)?;
            let mapped = statement.query_map(
                params![after, workspace_id, project_id, task_id, run_id, limit],
                decode_work_event_record,
            )?;
            let mut events = mapped.collect::<Result<Vec<_>, _>>()?;
            let has_next_page = events.len() > first;
            events.truncate(first);

            let edges = events
                .into_iter()
                .map(|node| {
                    let cursor = WorkEventCursor::new(node.event_sequence()).map_err(|_| {
                        StoreError::InvariantViolation {
                            message: format!(
                                "persisted work event sequence is not cursor-safe: {}",
                                node.event_sequence()
                            ),
                        }
                    })?;
                    Ok(WorkEventEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            let end_cursor = edges.last().map(|edge| edge.cursor.encode());

            Ok(WorkEventConnection {
                edges,
                page_info: WorkPageInfo {
                    end_cursor,
                    has_next_page,
                },
            })
        })
        .await
    }

    /// Load one strict current task projection and its bounded related rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SQLite fails or any persisted row violates
    /// its domain contract.
    pub async fn get_work_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkTaskDetail>, StoreError> {
        let task_id = task_id.clone();
        let detail = self
            .with_connection({
                let task_id = task_id.clone();
                move |conn| {
                    let transaction = conn.transaction()?;
                    let Some(facts) = load_task_facts(&transaction, &task_id)? else {
                        return Ok(None);
                    };
                    let history =
                        crate::work_reads::history::load_task_history(&transaction, &task_id)?;
                    let artifacts = crate::work_reads::artifacts::load_recent_task_artifacts(
                        &transaction,
                        &task_id,
                    )?;
                    Ok(Some(facts.into_detail(history, artifacts)))
                }
            })
            .await?;
        let Some(mut detail) = detail else {
            return Ok(None);
        };
        self.hydrate_work_task_files(&mut detail).await?;
        Ok(Some(detail))
    }

    pub(crate) async fn hydrate_work_task_files(
        &self,
        detail: &mut WorkTaskDetail,
    ) -> Result<(), StoreError> {
        let task_id = detail.task.task_id.clone();
        let task_document = self.read_task_document(&task_id).await.map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        })?;
        detail.task_document = task_document.content;
        detail.task_document_digest = task_document.digest;
        detail.result_document = match self.read_task_file(&task_id, crate::TASK_RESULT).await {
            Ok(content) => Some(content),
            Err(crate::TaskFileError::Io(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                None
            }
            Err(error) => {
                return Err(StoreError::InvariantViolation {
                    message: error.to_string(),
                });
            }
        };
        detail.review_document = match self.read_task_file(&task_id, crate::TASK_REVIEW).await {
            Ok(content) => Some(content),
            Err(crate::TaskFileError::Io(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                None
            }
            Err(error) => {
                return Err(StoreError::InvariantViolation {
                    message: error.to_string(),
                });
            }
        };
        detail.working_directory = self
            .task_working_directory(&task_id)
            .await
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?
            .to_string_lossy()
            .into_owned();
        Ok(())
    }
}

fn project_query_hash(workspace_id: &WorkspaceId, include_archived: bool) -> String {
    let canonical = format!(
        "work-project-query:v1\0{}\0{}",
        workspace_id.as_str(),
        u8::from(include_archived)
    );
    crate::work_row::sha256_hex(canonical.as_bytes())
}

fn invalid_project_cursor() -> StoreError {
    WorkDomainError::InvalidInput {
        field: "work_project.cursor",
        message: "invalid_cursor".to_string(),
    }
    .into()
}

fn decode_project_record(row: &Row<'_>) -> rusqlite::Result<ProjectRecord> {
    let raw_project_id = row.get::<_, String>(0)?;
    let raw_workspace_id = row.get::<_, String>(1)?;
    let raw_revision = row.get::<_, i64>(5)?;
    let project_id =
        ProjectId::new(raw_project_id).map_err(|error| conversion_failure(0, Type::Text, error))?;
    let workspace_id = WorkspaceId::new(raw_workspace_id)
        .map_err(|error| conversion_failure(1, Type::Text, error))?;
    let revision =
        u64::try_from(raw_revision).map_err(|error| conversion_failure(5, Type::Integer, error))?;
    let project = ProjectRecord {
        project_id,
        workspace_id,
        name: row.get(2)?,
        description: row.get(3)?,
        folder: row.get(4)?,
        revision,
        archived_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    };
    let normalized = project
        .clone()
        .normalized()
        .map_err(|error| conversion_failure(0, Type::Text, error))?;
    if normalized != project {
        return Err(conversion_failure(
            0,
            Type::Text,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "project row is not canonically normalized",
            ),
        ));
    }
    Ok(project)
}
