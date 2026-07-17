//! Shared synchronous artifact write transactions.

use noema_artifacts::{
    ArtifactStorageKind, ArtifactVersionRecord, ArtifactVersionStorage, ArtifactWithVersions,
    NewArtifact, NewArtifactVersion, validate_external_artifact_url,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    NoemaStore, StoreError,
    artifacts::{
        ARTIFACT_SELECT, ARTIFACT_VERSION_SELECT, artifact_from_row, artifact_row,
        artifact_version_from_row, artifact_version_row,
    },
    sqlite::{json_to_string, now_timestamp_sql},
};

pub(super) struct PreparedArtifactCreate {
    artifact: NewArtifact,
    initial_version: NewArtifactVersion,
    artifact_id: String,
    artifact_version_id: String,
    artifact_metadata_json: String,
    version_metadata_json: String,
    storage: VersionStorageParts,
}

pub(super) struct PreparedArtifactAppend<'a> {
    artifact_id: &'a str,
    expected_next_version_index: Option<i64>,
    version: NewArtifactVersion,
    artifact_version_id: String,
    version_metadata_json: String,
    storage: VersionStorageParts,
}

#[derive(Debug)]
pub(super) enum ArtifactTransactionError {
    Busy(rusqlite::Error),
    Store(StoreError),
    AppendConflict {
        artifact_id: String,
        expected_next_version_index: i64,
        actual_next_version_index: i64,
    },
}

pub(super) fn prepare_artifact_create(
    store: &NoemaStore,
    artifact: NewArtifact,
    initial_version: NewArtifactVersion,
) -> Result<PreparedArtifactCreate, StoreError> {
    let artifact = artifact.validated()?;
    let initial_version = initial_version.validated()?;
    if initial_version.storage.storage_kind() != artifact.storage_kind {
        return Err(StoreError::ArtifactStorageKindMismatch);
    }
    let artifact_id = artifact
        .artifact_id
        .clone()
        .unwrap_or_else(|| store.new_artifact_id());
    let artifact_version_id = initial_version
        .artifact_version_id
        .clone()
        .unwrap_or_else(|| store.new_artifact_version_id());
    let artifact_metadata_json = json_to_string(&artifact.metadata)?;
    let version_metadata_json = json_to_string(&initial_version.metadata)?;
    let storage = VersionStorageParts::try_from_storage(initial_version.storage.clone())?;
    Ok(PreparedArtifactCreate {
        artifact,
        initial_version,
        artifact_id,
        artifact_version_id,
        artifact_metadata_json,
        version_metadata_json,
        storage,
    })
}

pub(super) fn prepare_artifact_append<'a>(
    store: &NoemaStore,
    artifact_id: &'a str,
    expected_next_version_index: Option<i64>,
    version: NewArtifactVersion,
) -> Result<PreparedArtifactAppend<'a>, StoreError> {
    if expected_next_version_index.is_some_and(|index| index < 1) {
        return Err(StoreError::Schema(
            "artifact version index must be positive".to_string(),
        ));
    }
    let version = version.validated()?;
    let artifact_version_id = version
        .artifact_version_id
        .clone()
        .unwrap_or_else(|| store.new_artifact_version_id());
    let version_metadata_json = json_to_string(&version.metadata)?;
    let storage = VersionStorageParts::try_from_storage(version.storage.clone())?;
    Ok(PreparedArtifactAppend {
        artifact_id,
        expected_next_version_index,
        version,
        artifact_version_id,
        version_metadata_json,
        storage,
    })
}

pub(super) fn create_artifact_transaction(
    conn: &mut rusqlite::Connection,
    prepared: &PreparedArtifactCreate,
    behavior: TransactionBehavior,
) -> Result<ArtifactWithVersions, ArtifactTransactionError> {
    let tx = conn
        .transaction_with_behavior(behavior)
        .map_err(transaction_sql_error)?;
    require_owner(&tx, &prepared.artifact.owner)?;
    tx.execute(
        format!(
            r#"
            INSERT INTO artifacts (
              artifact_id, owner_object_type, owner_object_id, title, description,
              artifact_kind, storage_kind, current_version_id, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, {now}, {now})
            "#,
            now = now_timestamp_sql()
        )
        .as_str(),
        params![
            prepared.artifact_id,
            prepared.artifact.owner.object_type,
            prepared.artifact.owner.object_id,
            prepared.artifact.title,
            prepared.artifact.description,
            prepared.artifact.artifact_kind,
            prepared.artifact.storage_kind.as_str(),
            prepared.artifact_version_id,
            prepared.artifact.created_by_actor_id,
            prepared.artifact.source.conversation_id,
            prepared.artifact.source.turn_id,
            prepared.artifact.source.item_id,
            prepared.artifact_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;
    tx.execute(
        format!(
            r#"
            INSERT INTO artifact_versions (
              artifact_version_id, artifact_id, version_index, title, local_relative_path,
              external_url, media_type, byte_size, content_sha256, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at
            )
            VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, {now})
            "#,
            now = now_timestamp_sql()
        )
        .as_str(),
        params![
            prepared.artifact_version_id,
            prepared.artifact_id,
            prepared.initial_version.title,
            prepared.storage.local_relative_path,
            prepared.storage.external_url,
            prepared.initial_version.media_type,
            prepared.initial_version.byte_size,
            prepared.initial_version.content_sha256,
            prepared.initial_version.created_by_actor_id,
            prepared.initial_version.source.conversation_id,
            prepared.initial_version.source.turn_id,
            prepared.initial_version.source.item_id,
            prepared.version_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;

    let artifact = tx
        .query_row(
            format!("SELECT {ARTIFACT_SELECT} FROM artifacts WHERE artifact_id = ?1 LIMIT 1")
                .as_str(),
            [&prepared.artifact_id],
            artifact_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| artifact_from_row(row).map_err(ArtifactTransactionError::Store))?;
    let version = tx
        .query_row(
            format!(
                "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"
            )
            .as_str(),
            [&prepared.artifact_version_id],
            artifact_version_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| artifact_version_from_row(row).map_err(ArtifactTransactionError::Store))?;
    let result = ArtifactWithVersions {
        artifact,
        current_version: version.clone(),
        versions: vec![version],
    };
    tx.commit().map_err(transaction_sql_error)?;
    Ok(result)
}

pub(super) fn append_artifact_transaction(
    conn: &mut rusqlite::Connection,
    prepared: &PreparedArtifactAppend<'_>,
    behavior: TransactionBehavior,
) -> Result<ArtifactVersionRecord, ArtifactTransactionError> {
    let tx = conn
        .transaction_with_behavior(behavior)
        .map_err(transaction_sql_error)?;
    let (stored_kind, actual_next_version_index) = tx
        .query_row(
            r#"
            SELECT artifact.storage_kind,
                   COALESCE(MAX(version.version_index), 0) + 1
            FROM artifacts AS artifact
            LEFT JOIN artifact_versions AS version
              ON version.artifact_id = artifact.artifact_id
            WHERE artifact.artifact_id = ?1
              AND artifact.deleted_at IS NULL
            GROUP BY artifact.artifact_id
            LIMIT 1
            "#,
            [prepared.artifact_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(transaction_sql_error)?
        .ok_or_else(|| {
            ArtifactTransactionError::Store(StoreError::ArtifactNotFound {
                artifact_id: prepared.artifact_id.to_string(),
            })
        })?;
    let actual_storage_kind = ArtifactStorageKind::parse(&stored_kind)
        .map_err(|error| ArtifactTransactionError::Store(error.into()))?;
    if prepared.version.storage.storage_kind() != actual_storage_kind {
        return Err(ArtifactTransactionError::Store(
            StoreError::ArtifactStorageKindMismatch,
        ));
    }
    if let Some(expected_next_version_index) = prepared.expected_next_version_index
        && expected_next_version_index != actual_next_version_index
    {
        return Err(ArtifactTransactionError::AppendConflict {
            artifact_id: prepared.artifact_id.to_string(),
            expected_next_version_index,
            actual_next_version_index,
        });
    }

    tx.execute(
        format!(
            r#"
            INSERT INTO artifact_versions (
              artifact_version_id, artifact_id, version_index, title, local_relative_path,
              external_url, media_type, byte_size, content_sha256, created_by_actor_id,
              source_conversation_id, source_turn_id, source_item_id, metadata_json,
              created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, {now})
            "#,
            now = now_timestamp_sql()
        )
        .as_str(),
        params![
            prepared.artifact_version_id,
            prepared.artifact_id,
            actual_next_version_index,
            prepared.version.title,
            prepared.storage.local_relative_path,
            prepared.storage.external_url,
            prepared.version.media_type,
            prepared.version.byte_size,
            prepared.version.content_sha256,
            prepared.version.created_by_actor_id,
            prepared.version.source.conversation_id,
            prepared.version.source.turn_id,
            prepared.version.source.item_id,
            prepared.version_metadata_json,
        ],
    )
    .map_err(transaction_sql_error)?;
    let updated = tx
        .execute(
            format!(
                "UPDATE artifacts SET current_version_id = ?2, updated_at = {now} WHERE artifact_id = ?1 AND deleted_at IS NULL",
                now = now_timestamp_sql()
            )
            .as_str(),
            params![prepared.artifact_id, prepared.artifact_version_id],
        )
        .map_err(transaction_sql_error)?;
    if updated != 1 {
        return Err(ArtifactTransactionError::Store(
            StoreError::InvariantViolation {
                message: "artifact current version update affected an unexpected row count"
                    .to_string(),
            },
        ));
    }
    let result = tx
        .query_row(
            format!(
                "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions WHERE artifact_version_id = ?1 LIMIT 1"
            )
            .as_str(),
            [&prepared.artifact_version_id],
            artifact_version_row,
        )
        .map_err(transaction_sql_error)
        .and_then(|row| artifact_version_from_row(row).map_err(ArtifactTransactionError::Store))?;
    tx.commit().map_err(transaction_sql_error)?;
    Ok(result)
}

fn require_owner(
    conn: &rusqlite::Connection,
    owner: &noema_artifacts::ArtifactOwnerRef,
) -> Result<(), ArtifactTransactionError> {
    let exists = match owner.object_type.as_str() {
        "conversation" => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE conversation_id = ?1 AND lifecycle_status = 'active' AND deleted_at IS NULL)",
            [&owner.object_id],
            |row| row.get::<_, bool>(0),
        ),
        "task" => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id = ?1)",
            [&owner.object_id],
            |row| row.get::<_, bool>(0),
        ),
        _ => unreachable!("prepared artifact owners are validated"),
    }
    .map_err(transaction_sql_error)?;
    if exists {
        Ok(())
    } else {
        Err(ArtifactTransactionError::Store(
            StoreError::UnsupportedArtifactOwner {
                owner_object_type: owner.object_type.clone(),
                owner_object_id: owner.object_id.clone(),
            },
        ))
    }
}

fn transaction_sql_error(error: rusqlite::Error) -> ArtifactTransactionError {
    if matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ) {
        ArtifactTransactionError::Busy(error)
    } else {
        ArtifactTransactionError::Store(StoreError::Sqlite(error))
    }
}

struct VersionStorageParts {
    local_relative_path: Option<String>,
    external_url: Option<String>,
}

impl VersionStorageParts {
    fn try_from_storage(storage: ArtifactVersionStorage) -> Result<Self, StoreError> {
        match storage {
            ArtifactVersionStorage::LocalFile { relative_path } => Ok(Self {
                local_relative_path: Some(relative_path),
                external_url: None,
            }),
            ArtifactVersionStorage::ExternalUrl { url } => Ok(Self {
                local_relative_path: None,
                external_url: Some(validate_external_artifact_url(&url)?),
            }),
        }
    }
}
