use crate::{
    database::DatabaseConfig,
    memory::{MemoryRetrievalRequest, MemoryRetrievalResult},
    postgres_memory_retrieval, postgres_retrieval_policy_fingerprint,
};
use sqlx::PgPool;

use super::{
    error::MemoryPersistenceError,
    helpers::*,
    models::*,
    objects::{ObjectRef, validate_object_ref_for_pool},
    postgres_schema::POSTGRES_SCHEMA_SQL,
    queries::{POSTGRES_MEMORY_SUMMARY_BY_ID_SQL, POSTGRES_RECENT_MEMORY_SQL},
};

pub(super) const POSTGRES_BOOTSTRAP_MIGRATION_VERSION: i32 = 0;
pub(super) const POSTGRES_BOOTSTRAP_MIGRATION_NAME: &str = "postgres_bootstrap_v0";

struct MemorySummaryRow {
    memory_id: String,
    status: String,
    memory_type: String,
    owner_object_type: String,
    owner_object_id: String,
    sensitivity: String,
    title: String,
    content: String,
    created_at: String,
    source_object_type: Option<String>,
    source_object_id: Option<String>,
    conversation_id: Option<String>,
}

fn postgres_row_to_memory_summary(
    row: MemorySummaryRow,
) -> Result<MemorySummary, MemoryPersistenceError> {
    Ok(MemorySummary {
        id: row.memory_id,
        status: parse_memory_status(&row.status)?,
        memory_type: parse_memory_type(&row.memory_type)?,
        home_scope_id: object_ref_key(&row.owner_object_type, &row.owner_object_id),
        owner_object_type: row.owner_object_type,
        owner_object_id: row.owner_object_id,
        sensitivity: parse_sensitivity(&row.sensitivity)?,
        title: row.title,
        content: row.content,
        created_at: row.created_at,
        source_object_type: row.source_object_type.clone(),
        source_object_id: row.source_object_id.clone(),
        source_type: row.source_object_type,
        source_id: row.source_object_id,
        conversation_id: row.conversation_id,
    })
}

/// Postgres-backed memory repository.
#[derive(Debug, Clone)]
pub struct PostgresMemoryRepository {
    pool: PgPool,
}

impl PostgresMemoryRepository {
    /// Connect to Postgres and bootstrap the memory schema.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when SQLx cannot connect or schema
    /// bootstrap fails.
    pub async fn connect(database: &DatabaseConfig) -> Result<Self, MemoryPersistenceError> {
        let pool = database.connect().await?;
        Self::from_pool(pool).await
    }

    /// Bootstrap the memory schema on an existing Postgres pool.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when schema bootstrap fails.
    pub async fn from_pool(pool: PgPool) -> Result<Self, MemoryPersistenceError> {
        sqlx::raw_sql(POSTGRES_SCHEMA_SQL)
            .execute(&pool)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        sqlx::query(
            r"
            INSERT INTO schema_migrations (version, name)
            VALUES ($1, $2)
            ON CONFLICT (version) DO NOTHING
            ",
        )
        .bind(POSTGRES_BOOTSTRAP_MIGRATION_VERSION)
        .bind(POSTGRES_BOOTSTRAP_MIGRATION_NAME)
        .execute(&pool)
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(Self { pool })
    }

    /// Underlying Postgres pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// List recent memories for CLI or dashboard inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub async fn list_recent_memories(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<MemorySummary>, MemoryPersistenceError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(POSTGRES_RECENT_MEMORY_SQL)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        rows.into_iter()
            .map(
                |(
                    memory_id,
                    status,
                    memory_type,
                    owner_object_type,
                    owner_object_id,
                    sensitivity,
                    title,
                    content,
                    created_at,
                    source_object_type,
                    source_object_id,
                    conversation_id,
                )| {
                    postgres_row_to_memory_summary(MemorySummaryRow {
                        memory_id,
                        status,
                        memory_type,
                        owner_object_type,
                        owner_object_id,
                        sensitivity,
                        title,
                        content,
                        created_at,
                        source_object_type,
                        source_object_id,
                        conversation_id,
                    })
                    .map(redact_for_list)
                },
            )
            .collect()
    }

    /// Fetch one memory by id for inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub async fn get_memory(
        &self,
        memory_id: &str,
    ) -> Result<Option<MemorySummary>, MemoryPersistenceError> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(POSTGRES_MEMORY_SUMMARY_BY_ID_SQL)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        row.map(
            |(
                memory_id,
                status,
                memory_type,
                owner_object_type,
                owner_object_id,
                sensitivity,
                title,
                content,
                created_at,
                source_object_type,
                source_object_id,
                conversation_id,
            )| {
                postgres_row_to_memory_summary(MemorySummaryRow {
                    memory_id,
                    status,
                    memory_type,
                    owner_object_type,
                    owner_object_id,
                    sensitivity,
                    title,
                    content,
                    created_at,
                    source_object_type,
                    source_object_id,
                    conversation_id,
                })
            },
        )
        .transpose()
    }

    /// Upsert the built-in local human and primary Noema agent actors.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres cannot write either
    /// actor.
    pub async fn ensure_default_actors(&self) -> Result<(), MemoryPersistenceError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        sqlx::query(
            r"
            INSERT INTO humans (human_id, display_name, handle)
            VALUES ($1, $2, $3)
            ON CONFLICT (human_id) DO UPDATE SET updated_at = now()
            ",
        )
        .bind("human:local")
        .bind("Local human")
        .bind("local")
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;
        sqlx::query(
            r"
            INSERT INTO agents (agent_id, display_name, handle)
            VALUES ($1, $2, $3)
            ON CONFLICT (agent_id) DO UPDATE SET updated_at = now()
            ",
        )
        .bind("agent:primary")
        .bind("Noema")
        .bind("primary")
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;
        tx.commit().await.map_err(MemoryPersistenceError::Database)
    }

    /// Retrieve memories from canonical Postgres state using deterministic gates.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail, stored enum
    /// values are outside Noema's closed vocabularies, or graph invariants are
    /// violated by stored rows.
    pub async fn retrieve_memories(
        &self,
        request: &MemoryRetrievalRequest,
    ) -> Result<MemoryRetrievalResult, MemoryPersistenceError> {
        postgres_memory_retrieval::retrieve(&self.pool, request).await
    }

    /// Mark a memory retrieval policy valid for its current canonical basis.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if the extractor or memory does not
    /// exist, fingerprint basis rows cannot be read, metadata cannot be
    /// serialized, or Postgres writes fail.
    pub async fn refresh_retrieval_policy_fingerprint(
        &self,
        memory_id: &str,
        extractor: ObjectRef,
        extractor_version: &str,
    ) -> Result<String, MemoryPersistenceError> {
        validate_object_ref_for_pool(&self.pool, &extractor).await?;
        let fingerprint =
            postgres_retrieval_policy_fingerprint::current_fingerprint(&self.pool, memory_id)
                .await?;
        let changed = sqlx::query(
            r"
            UPDATE memory_items
            SET
              retrieval_policy_status = 'valid',
              retrieval_policy_fingerprint = $2,
              retrieval_policy_extractor_object_type = $3,
              retrieval_policy_extractor_object_id = $4,
              retrieval_policy_extractor_version = $5,
              retrieval_policy_validated_at = now()
            WHERE memory_id = $1
            ",
        )
        .bind(memory_id)
        .bind(fingerprint.as_str())
        .bind(extractor.object_type.as_str())
        .bind(extractor.object_id.as_str())
        .bind(extractor_version)
        .execute(&self.pool)
        .await
        .map_err(MemoryPersistenceError::Database)?
        .rows_affected();
        if changed == 0 {
            return Err(MemoryPersistenceError::MemoryNotFound {
                memory_id: memory_id.to_string(),
            });
        }
        Ok(fingerprint)
    }
}
