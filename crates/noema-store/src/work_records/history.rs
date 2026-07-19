use noema_tasks::{AgentRunItemRecord, TaskId};
use noema_workspaces::WorkspaceId;

use super::{WorkConnection, WorkCursorError, WorkEdge, WorkPageSize, cursor};

/// Owner scope that prevents transcript discovery outside an authorized task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRunItemOwnerScope {
    /// Workspace that owns the run.
    pub workspace_id: WorkspaceId,
    /// Optional task fence for task-scoped transcript reads.
    pub task_id: Option<TaskId>,
}

/// Opaque owner/run-bound cursor for backward transcript pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRunItemCursor {
    pub(crate) query_hash: String,
    pub(crate) sequence_index: u64,
    pub(crate) item_id: String,
}

impl WorkRunItemCursor {
    /// Decode and validate an opaque transcript cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] for malformed or noncanonical input.
    pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
        let [family, query_hash, sequence, item_id] = cursor::decode_nul_fields(encoded)?;
        if family != "work-run-item:v1" {
            return Err(WorkCursorError);
        }
        Ok(Self {
            query_hash: cursor::canonical_hash(&query_hash)?,
            sequence_index: cursor::canonical_sequence(&sequence)?,
            item_id: cursor::canonical_text(&item_id)?,
        })
    }

    pub(crate) fn new(query_hash: String, sequence_index: u64, item_id: String) -> Self {
        Self {
            query_hash,
            sequence_index,
            item_id,
        }
    }

    #[must_use]
    /// Encode this cursor as its canonical opaque value.
    pub fn encode(&self) -> String {
        cursor::encode_nul_fields(&[
            "work-run-item:v1",
            &self.query_hash,
            &self.sequence_index.to_string(),
            &self.item_id,
        ])
    }
}

/// Bounded newest-page-first transcript query; each returned page is chronological.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRunItemQuery {
    /// Owner scope that fences the read.
    pub owner: WorkRunItemOwnerScope,
    /// Run whose transcript is requested.
    pub run_id: String,
    /// Maximum number of items returned.
    pub first: WorkPageSize,
    /// Exclusive backward-pagination cursor.
    pub before: Option<WorkRunItemCursor>,
}

/// Task-run transcript connection edge.
pub type WorkRunItemEdge = WorkEdge<String, AgentRunItemRecord>;
/// Bounded task-run transcript connection.
pub type WorkRunItemConnection = WorkConnection<String, AgentRunItemRecord>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_item_cursor_round_trips_and_rejects_noncanonical_sequences() {
        let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cursor = WorkRunItemCursor::new(hash.to_string(), 42, "run_item:test".to_string());
        assert_eq!(WorkRunItemCursor::decode(&cursor.encode()), Ok(cursor));
        assert!(WorkRunItemCursor::decode("cnVuLWl0ZW06djE6MDAx").is_err());
    }
}
