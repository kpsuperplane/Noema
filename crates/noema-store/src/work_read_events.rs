//! Descending activity pagination and cheap durable high-water reads.

use noema_tasks::TaskId;
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, params};

use crate::{
    NoemaStore, StoreError, WorkEventBeforeQuery, WorkEventConnection, WorkEventCursor,
    WorkEventEdge, WorkPageInfo,
    work_events::{WORK_EVENT_COLUMNS, decode_work_event_record},
};

impl NoemaStore {
    /// List newest-first activity before an exclusive global event cursor.
    ///
    /// # Errors
    ///
    /// Returns an error when query validation, cursor decoding, or the event
    /// read fails.
    pub async fn list_work_events_before(
        &self,
        query: WorkEventBeforeQuery,
    ) -> Result<WorkEventConnection, StoreError> {
        let first = query.first.get();
        let limit = i64::try_from(first + 1).map_err(|error| StoreError::InvariantViolation {
            message: format!("work event page size exceeds SQLite range: {error}"),
        })?;
        let before = query
            .before
            .map(WorkEventCursor::sequence)
            .map(i64::try_from)
            .transpose()
            .map_err(|error| StoreError::InvariantViolation {
                message: format!("work event cursor exceeds SQLite range: {error}"),
            })?;
        let workspace_id = query.workspace_id.into_string();
        let project_id = query.project_id.map(ProjectId::into_string);
        let task_id = query.task_id.map(TaskId::into_string);
        let run_id = query.run_id;
        self.with_connection(move |connection| {
            let sql = format!(
                "SELECT {WORK_EVENT_COLUMNS}
                 FROM work_events
                 WHERE (?1 IS NULL OR event_sequence < ?1) AND workspace_id = ?2
                   AND (?3 IS NULL OR project_id = ?3)
                   AND (?4 IS NULL OR task_id = ?4)
                   AND (?5 IS NULL OR run_id = ?5)
                 ORDER BY event_sequence DESC LIMIT ?6"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params![before, workspace_id, project_id, task_id, run_id, limit],
                decode_work_event_record,
            )?;
            let mut events = rows.collect::<Result<Vec<_>, _>>()?;
            let has_next_page = events.len() > first;
            events.truncate(first);
            let edges = events
                .into_iter()
                .map(|node| {
                    let cursor = WorkEventCursor::new(node.event_sequence()).map_err(|_| {
                        StoreError::InvariantViolation {
                            message: "persisted event sequence is not cursor-safe".to_string(),
                        }
                    })?;
                    Ok(WorkEventEdge { cursor, node })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            Ok(WorkEventConnection {
                page_info: WorkPageInfo {
                    end_cursor: edges.last().map(|edge| edge.cursor.encode()),
                    has_next_page,
                },
                edges,
            })
        })
        .await
    }

    /// Return a workspace's current durable event high-water cursor.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable event high-water mark cannot be read.
    pub async fn latest_work_event_cursor(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkEventCursor>, StoreError> {
        latest_cursor(self, workspace_id.clone(), None).await
    }

    /// Return one task's current durable event high-water cursor in a workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the scoped durable event high-water mark cannot be read.
    pub async fn latest_task_work_event_cursor(
        &self,
        workspace_id: &WorkspaceId,
        task_id: &TaskId,
    ) -> Result<Option<WorkEventCursor>, StoreError> {
        latest_cursor(self, workspace_id.clone(), Some(task_id.clone())).await
    }
}

async fn latest_cursor(
    store: &NoemaStore,
    workspace_id: WorkspaceId,
    task_id: Option<TaskId>,
) -> Result<Option<WorkEventCursor>, StoreError> {
    store
        .with_connection(move |connection| {
            let sequence = connection
                .query_row(
                    "SELECT event_sequence FROM work_events
                     WHERE workspace_id = ?1 AND (?2 IS NULL OR task_id = ?2)
                     ORDER BY event_sequence DESC LIMIT 1",
                    params![workspace_id.as_str(), task_id.as_ref().map(TaskId::as_str)],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            sequence
                .map(|sequence| {
                    let sequence =
                        u64::try_from(sequence).map_err(|_| StoreError::InvariantViolation {
                            message: "persisted event sequence is negative".to_string(),
                        })?;
                    WorkEventCursor::new(sequence).map_err(|_| StoreError::InvariantViolation {
                        message: "persisted event sequence is not cursor-safe".to_string(),
                    })
                })
                .transpose()
        })
        .await
}
