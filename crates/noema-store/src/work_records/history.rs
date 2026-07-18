//! Typed bounded connections for immutable task history and run transcripts.

use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, TaskExecutionContract, TaskGateRecord, TaskId,
    TaskMessageRecord, TaskReviewRecord, TaskSubmissionRecord,
};
use noema_workspaces::WorkspaceId;

use super::{WorkCursorError, WorkPageInfo, WorkPageSize, cursor};

macro_rules! history_cursor {
    ($name:ident, $family:literal) => {
        #[doc = concat!("Opaque query-bound cursor for the ", $family, " history family.")]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            pub(crate) query_hash: String,
            pub(crate) created_at: String,
            pub(crate) id: String,
        }

        impl $name {
            /// Decode and strictly validate this cursor family.
            ///
            /// # Errors
            ///
            /// Returns [`WorkCursorError`] for malformed, noncanonical, or
            /// cross-family cursor input.
            pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
                let (query_hash, created_at, id) = cursor::decode_history(encoded, $family)?;
                Ok(Self {
                    query_hash,
                    created_at,
                    id,
                })
            }

            pub(crate) fn new(
                query_hash: String,
                created_at: String,
                id: String,
            ) -> Result<Self, WorkCursorError> {
                Ok(Self {
                    query_hash,
                    created_at: cursor::canonical_sqlite_timestamp(&created_at)?,
                    id,
                })
            }

            /// Encode this cursor as canonical unpadded base64url.
            #[must_use]
            pub fn encode(&self) -> String {
                cursor::encode_history($family, &self.query_hash, &self.created_at, &self.id)
            }
        }
    };
}

history_cursor!(WorkContractCursor, "work-history:contract:v1");
history_cursor!(WorkGateCursor, "work-history:gate:v1");
history_cursor!(WorkMessageCursor, "work-history:message:v1");
history_cursor!(WorkRunCursor, "work-history:run:v1");
history_cursor!(WorkSubmissionCursor, "work-history:submission:v1");
history_cursor!(WorkReviewCursor, "work-history:review:v1");

macro_rules! history_connection {
    ($query:ident, $cursor:ident, $edge:ident, $connection:ident, $node:ty) => {
        #[doc = "Bounded query for one task's immutable history family."]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $query {
            /// Owning task.
            pub task_id: TaskId,
            /// Validated page size.
            pub first: WorkPageSize,
            /// Exclusive keyset cursor.
            pub after: Option<$cursor>,
        }

        #[doc = "One typed immutable-history edge."]
        #[derive(Debug, Clone, PartialEq)]
        pub struct $edge {
            /// Opaque query-bound keyset cursor.
            pub cursor: String,
            /// Strict hydrated domain record.
            pub node: $node,
        }

        #[doc = "One bounded immutable-history connection."]
        #[derive(Debug, Clone, PartialEq)]
        pub struct $connection {
            /// Edges in descending creation/id order.
            pub edges: Vec<$edge>,
            /// Pagination metadata.
            pub page_info: WorkPageInfo,
        }
    };
}

history_connection!(
    WorkContractHistoryQuery,
    WorkContractCursor,
    WorkContractEdge,
    WorkContractConnection,
    TaskExecutionContract
);
history_connection!(
    WorkGateHistoryQuery,
    WorkGateCursor,
    WorkGateEdge,
    WorkGateConnection,
    TaskGateRecord
);
history_connection!(
    WorkMessageHistoryQuery,
    WorkMessageCursor,
    WorkMessageEdge,
    WorkMessageConnection,
    TaskMessageRecord
);
history_connection!(
    WorkRunHistoryQuery,
    WorkRunCursor,
    WorkRunEdge,
    WorkRunConnection,
    AgentRunRecord
);
history_connection!(
    WorkSubmissionHistoryQuery,
    WorkSubmissionCursor,
    WorkSubmissionEdge,
    WorkSubmissionConnection,
    TaskSubmissionRecord
);
history_connection!(
    WorkReviewHistoryQuery,
    WorkReviewCursor,
    WorkReviewEdge,
    WorkReviewConnection,
    TaskReviewRecord
);

/// Owner scope that prevents transcript discovery outside an authorized task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRunItemOwnerScope {
    /// Required workspace owner scope.
    pub workspace_id: WorkspaceId,
    /// Optional narrower task scope.
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
    /// Decode and strictly validate a transcript cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] for malformed, noncanonical, or out-of-range input.
    pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
        let (query_hash, sequence_index, item_id) = cursor::decode_run_item(encoded)?;
        Ok(Self {
            query_hash,
            sequence_index,
            item_id,
        })
    }

    pub(crate) fn new(query_hash: String, sequence_index: u64, item_id: String) -> Self {
        Self {
            query_hash,
            sequence_index,
            item_id,
        }
    }

    /// Encode this cursor as canonical unpadded base64url.
    #[must_use]
    pub fn encode(&self) -> String {
        cursor::encode_run_item(&self.query_hash, self.sequence_index, &self.item_id)
    }
}

/// Bounded newest-page-first transcript query; each returned page is chronological.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRunItemQuery {
    /// Authorization/discovery scope.
    pub owner: WorkRunItemOwnerScope,
    /// Run whose immutable transcript is read.
    pub run_id: String,
    /// Validated page size.
    pub first: WorkPageSize,
    /// Exclusive sequence cursor toward older items.
    pub before: Option<WorkRunItemCursor>,
}

/// One chronological transcript edge.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkRunItemEdge {
    /// Opaque owner/run-bound sequence cursor.
    pub cursor: String,
    /// Strict transcript item.
    pub node: AgentRunItemRecord,
}

/// Bounded transcript connection, chronological within each backward page.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkRunItemConnection {
    /// Chronological edges for the selected newest/older window.
    pub edges: Vec<WorkRunItemEdge>,
    /// `end_cursor` is the oldest edge cursor used to fetch the next older page.
    pub page_info: WorkPageInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    }

    #[test]
    fn history_cursor_families_are_independent_and_canonical() {
        let contract = WorkContractCursor::new(
            hash(),
            "2026-07-17T00:00:00.000Z".to_string(),
            "contract:a".to_string(),
        )
        .expect("canonical cursor");
        let encoded = contract.encode();
        assert_eq!(WorkContractCursor::decode(&encoded), Ok(contract));
        assert!(WorkGateCursor::decode(&encoded).is_err());
        assert!(WorkMessageCursor::decode(&encoded).is_err());
        assert!(WorkRunCursor::decode(&encoded).is_err());
        assert!(WorkSubmissionCursor::decode(&encoded).is_err());
        assert!(WorkReviewCursor::decode(&encoded).is_err());
    }

    #[test]
    fn run_item_cursor_rejects_history_and_noncanonical_sequences() {
        let cursor = WorkRunItemCursor::new(hash(), 42, "run_item:test".to_string());
        assert_eq!(
            WorkRunItemCursor::decode(&cursor.encode()),
            Ok(cursor.clone())
        );
        assert!(WorkContractCursor::decode(&cursor.encode()).is_err());
        assert!(WorkRunItemCursor::decode("cnVuLWl0ZW06djE6MDAx").is_err());
    }
}
