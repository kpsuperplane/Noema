mod queries;
mod rows;

use noema_artifacts::{
    ArtifactVersionRecord, ArtifactWithVersions, NewArtifact, NewArtifactVersion,
};

use super::{
    NoemaStore, StoreError,
    artifact_writes::{
        ArtifactTransactionError, append_artifact_transaction, create_artifact_transaction,
        prepare_artifact_append, prepare_artifact_create,
    },
    ids::allocate_id,
};

pub(super) use rows::{
    ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, artifact_from_row, artifact_row,
    artifact_version_from_row, artifact_version_row,
};

#[derive(Debug)]
pub(super) struct ArtifactRow {
    artifact_id: String,
    owner_object_type: String,
    owner_object_id: String,
    title: String,
    description: Option<String>,
    artifact_kind: String,
    storage_kind: String,
    current_version_id: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
pub(super) struct ArtifactVersionRow {
    artifact_version_id: String,
    artifact_id: String,
    version_index: i64,
    title: Option<String>,
    local_relative_path: Option<String>,
    external_url: Option<String>,
    media_type: Option<String>,
    byte_size: Option<i64>,
    content_sha256: Option<String>,
    created_by_actor_id: String,
    source_conversation_id: Option<String>,
    source_turn_id: Option<String>,
    source_item_id: Option<String>,
    metadata_json: String,
    created_at: String,
}

impl NoemaStore {
    /// Create an artifact and its first immutable version in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the input is invalid, the owner conversation
    /// does not exist, or the embedded store write/read fails.
    pub async fn create_artifact_with_initial_version(
        &self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> Result<ArtifactWithVersions, StoreError> {
        let prepared = prepare_artifact_create(self, artifact, initial_version)?;
        let mut conn = self.conn.lock().await;
        create_artifact_transaction(
            &mut conn,
            &prepared,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(inherent_transaction_error)
    }

    /// Append a new immutable version to an existing artifact.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the artifact is missing, the storage kind is
    /// inconsistent, or the embedded store write/read fails.
    pub async fn append_artifact_version(
        &self,
        artifact_id: &str,
        version: NewArtifactVersion,
    ) -> Result<ArtifactVersionRecord, StoreError> {
        let prepared = prepare_artifact_append(self, artifact_id, None, version)?;
        let mut conn = self.conn.lock().await;
        append_artifact_transaction(
            &mut conn,
            &prepared,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(inherent_transaction_error)
    }

    /// Allocate a new artifact id using the canonical store prefix.
    #[must_use]
    pub fn new_artifact_id(&self) -> String {
        allocate_id("artifact")
    }

    /// Allocate a new artifact version id using the canonical store prefix.
    #[must_use]
    pub fn new_artifact_version_id(&self) -> String {
        allocate_id("artifact_version")
    }
}

fn inherent_transaction_error(error: ArtifactTransactionError) -> StoreError {
    match error {
        ArtifactTransactionError::Busy(error) => StoreError::Sqlite(error),
        ArtifactTransactionError::Store(error) => error,
        ArtifactTransactionError::AppendConflict { artifact_id, .. } => {
            StoreError::InvariantViolation {
                message: format!("unexpected append conflict for {artifact_id}"),
            }
        }
    }
}
