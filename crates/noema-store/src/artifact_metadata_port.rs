//! Artifact metadata port backed by the embedded SQLite store.

use std::time::Duration;

use noema_artifacts::{
    ArtifactAppendTarget, ArtifactDomainError, ArtifactFuture, ArtifactMetadataError,
    ArtifactMetadataStore, ArtifactStorageKind, ArtifactVersionRecord, ArtifactWithVersions,
    NewArtifact, NewArtifactVersion,
};
use rusqlite::{OptionalExtension, TransactionBehavior};

use super::{
    NoemaStore,
    artifact_writes::{
        ArtifactTransactionError, append_artifact_transaction, create_artifact_transaction,
        metadata_persistence_error, prepare_artifact_append, prepare_artifact_create,
    },
    ids::allocate_id,
};

const MAX_BUSY_RETRIES: usize = 12;

impl ArtifactMetadataStore for NoemaStore {
    fn new_artifact_id(&self) -> String {
        allocate_id("artifact")
    }

    fn new_artifact_version_id(&self) -> String {
        allocate_id("artifact_version")
    }

    fn load_append_target<'a>(
        &'a self,
        artifact_id: &'a str,
    ) -> ArtifactFuture<'a, Option<ArtifactAppendTarget>> {
        Box::pin(async move {
            let conn = self.conn.lock().await;
            let target = conn
                .query_row(
                    r#"
                    SELECT artifact.owner_object_type,
                           artifact.owner_object_id,
                           artifact.storage_kind,
                           COALESCE(MAX(version.version_index), 0) + 1
                    FROM artifacts AS artifact
                    LEFT JOIN artifact_versions AS version
                      ON version.artifact_id = artifact.artifact_id
                    WHERE artifact.artifact_id = ?1
                      AND artifact.deleted_at IS NULL
                    GROUP BY artifact.artifact_id
                    LIMIT 1
                    "#,
                    [artifact_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(metadata_sql_error)?;
            target
                .map(
                    |(owner_object_type, owner_object_id, storage_kind, next_index)| {
                        if next_index < 1 {
                            return Err(ArtifactDomainError::InvalidVersionIndex {
                                version_index: next_index,
                            }
                            .into());
                        }
                        let owner = noema_artifacts::ArtifactOwnerRef {
                            object_type: owner_object_type,
                            object_id: owner_object_id,
                        };
                        owner.validate()?;
                        Ok(ArtifactAppendTarget {
                            artifact_id: artifact_id.to_string(),
                            owner,
                            storage_kind: ArtifactStorageKind::parse(&storage_kind)?,
                            expected_next_version_index: next_index,
                        })
                    },
                )
                .transpose()
        })
    }

    fn create_artifact_with_initial_version<'a>(
        &'a self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactWithVersions> {
        Box::pin(async move {
            let prepared = prepare_artifact_create(artifact, initial_version)?;
            retry_artifact_write(self, |conn| {
                create_artifact_transaction(conn, &prepared, TransactionBehavior::Immediate)
            })
            .await
        })
    }

    fn append_artifact_version<'a>(
        &'a self,
        artifact_id: &'a str,
        expected_next_version_index: i64,
        version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactVersionRecord> {
        Box::pin(async move {
            if expected_next_version_index < 1 {
                return Err(ArtifactDomainError::InvalidVersionIndex {
                    version_index: expected_next_version_index,
                }
                .into());
            }
            let prepared =
                prepare_artifact_append(artifact_id, expected_next_version_index, version)?;
            retry_artifact_write(self, |conn| {
                append_artifact_transaction(conn, &prepared, TransactionBehavior::Immediate)
            })
            .await
        })
    }
}

async fn retry_artifact_write<T>(
    store: &NoemaStore,
    mut write: impl FnMut(&mut rusqlite::Connection) -> Result<T, ArtifactTransactionError>,
) -> Result<T, ArtifactMetadataError> {
    for attempt in 0..=MAX_BUSY_RETRIES {
        let result = {
            let mut conn = store.conn.lock().await;
            write(&mut conn)
        };
        match result {
            Ok(value) => return Ok(value),
            Err(ArtifactTransactionError::Metadata(error)) => return Err(error),
            Err(ArtifactTransactionError::AppendConflict {
                artifact_id,
                expected_next_version_index,
                actual_next_version_index,
            }) => {
                return Err(ArtifactMetadataError::AppendConflict {
                    artifact_id,
                    expected_next_version_index,
                    actual_next_version_index,
                });
            }
            Err(ArtifactTransactionError::Busy) if attempt < MAX_BUSY_RETRIES => {
                tokio::time::sleep(busy_retry_delay(attempt)).await;
            }
            Err(ArtifactTransactionError::Busy) => {
                return Err(metadata_persistence_error(
                    "SQLite artifact metadata operation remained busy",
                ));
            }
        }
    }
    unreachable!("bounded artifact metadata retry loop always returns")
}

fn busy_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis((5 * (attempt as u64 + 1)).min(50))
}

fn metadata_sql_error(_error: rusqlite::Error) -> ArtifactMetadataError {
    metadata_persistence_error("SQLite artifact metadata operation failed")
}
