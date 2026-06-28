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
    objects::{ActorRef, validate_actor_ref_for_pool},
    postgres_schema::POSTGRES_SCHEMA_SQL,
    queries::{POSTGRES_MEMORY_SUMMARY_BY_ID_SQL, POSTGRES_RECENT_MEMORY_SQL},
};

pub(super) const POSTGRES_BOOTSTRAP_MIGRATION_VERSION: i32 = 0;
pub(super) const POSTGRES_BOOTSTRAP_MIGRATION_NAME: &str = "postgres_bootstrap_v0";

pub(super) struct MemorySummaryRow {
    pub(super) memory_id: String,
    pub(super) status: String,
    pub(super) memory_type: String,
    pub(super) owner_object_type: String,
    pub(super) owner_object_id: String,
    pub(super) sensitivity: String,
    pub(super) title: String,
    pub(super) content: String,
    pub(super) dedupe_fingerprint: Option<String>,
    pub(super) created_at: String,
    pub(super) source_object_type: Option<String>,
    pub(super) source_object_id: Option<String>,
    pub(super) conversation_id: Option<String>,
}

pub(super) fn postgres_row_to_memory_summary(
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
        dedupe_fingerprint: row.dedupe_fingerprint,
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
                Option<String>,
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
                    dedupe_fingerprint,
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
                        dedupe_fingerprint,
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
                Option<String>,
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
                dedupe_fingerprint,
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
                    dedupe_fingerprint,
                    created_at,
                    source_object_type,
                    source_object_id,
                    conversation_id,
                })
            },
        )
        .transpose()
    }

    /// Find bounded plausible matches for daemon memory consolidation.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub async fn find_memory_consolidation_matches(
        &self,
        candidate: &NewMemoryCandidate,
        limit: u32,
    ) -> Result<Vec<MemorySummary>, MemoryPersistenceError> {
        let limit = limit.clamp(1, 12);
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
                Option<String>,
            ),
        >(
            r"
            SELECT
              memory_id,
              status,
              memory_type,
              owner_object_type,
              owner_object_id,
              sensitivity,
              title,
              content,
              created_at,
              memory_dedupe_fingerprint,
              source_object_type,
              source_object_id,
              conversation_id
            FROM (
              SELECT DISTINCT
                m.memory_id,
                m.status,
                m.memory_type,
                m.owner_object_type,
                m.owner_object_id,
                m.sensitivity,
                m.title,
                m.content,
                m.created_at::text AS created_at,
                m.memory_dedupe_fingerprint,
                source.source_object_type,
                source.source_object_id,
                source_conversation.conversation_id,
                CASE
                  WHEN m.search_vector @@ plainto_tsquery('simple', $5) THEN 0
                  WHEN strpos(lower(m.content), lower($5)) > 0 THEN 1
                  ELSE 2
                END AS match_rank,
                m.created_at AS created_at_sort
              FROM memory_items m
              LEFT JOIN LATERAL (
                SELECT source_object_type, source_object_id
                FROM object_provenance_edges
                WHERE target_object_type = 'memory_item'
                  AND target_object_id = m.memory_id
                  AND deleted_at IS NULL
                ORDER BY created_at ASC
                LIMIT 1
              ) source ON true
              LEFT JOIN conversation_items source_conversation
                ON source.source_object_type = 'conversation_item'
               AND source.source_object_id = source_conversation.item_id
              LEFT JOIN memory_participants mp ON mp.memory_id = m.memory_id
              WHERE m.deleted_at IS NULL
                AND m.status IN ('candidate', 'active', 'confirmed')
                AND m.sensitivity IN ('public', 'normal')
                AND m.memory_type = $1
                AND (
                  (m.owner_object_type = $2 AND m.owner_object_id = $3)
                  OR mp.participant_actor_id = ANY($4)
                )
            ) matches
            ORDER BY
              match_rank,
              created_at_sort DESC
            LIMIT $6
            ",
        )
        .bind(candidate.memory_type.as_str())
        .bind(candidate.owner.object_type.as_str())
        .bind(candidate.owner.object_id.as_str())
        .bind(
            candidate
                .participants
                .iter()
                .map(|participant| participant.participant.actor_id.clone())
                .collect::<Vec<_>>(),
        )
        .bind(candidate.content.as_str())
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
                    dedupe_fingerprint,
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
                        dedupe_fingerprint,
                        created_at,
                        source_object_type,
                        source_object_id,
                        conversation_id,
                    })
                },
            )
            .collect()
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
            INSERT INTO actors (actor_id, actor_kind, display_name, handle)
            VALUES ($1, 'human', $2, $3)
            ON CONFLICT (actor_id) DO UPDATE SET
              display_name = EXCLUDED.display_name,
              handle = EXCLUDED.handle,
              updated_at = now()
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
            INSERT INTO actors (actor_id, actor_kind, display_name, handle)
            VALUES ($1, 'agent', $2, $3)
            ON CONFLICT (actor_id) DO UPDATE SET
              display_name = EXCLUDED.display_name,
              handle = EXCLUDED.handle,
              updated_at = now()
            ",
        )
        .bind("agent:primary")
        .bind("Noema")
        .bind("primary")
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;
        sqlx::query(
            r"
            INSERT INTO humans (human_id, actor_id, display_name, handle)
            VALUES ($1, $1, $2, $3)
            ON CONFLICT (human_id) DO UPDATE SET
              actor_id = EXCLUDED.actor_id,
              updated_at = now()
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
            INSERT INTO agents (agent_id, actor_id, display_name, handle)
            VALUES ($1, $1, $2, $3)
            ON CONFLICT (agent_id) DO UPDATE SET
              actor_id = EXCLUDED.actor_id,
              updated_at = now()
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
        extractor: ActorRef,
        extractor_version: &str,
    ) -> Result<String, MemoryPersistenceError> {
        validate_actor_ref_for_pool(&self.pool, &extractor).await?;
        let fingerprint =
            postgres_retrieval_policy_fingerprint::current_fingerprint(&self.pool, memory_id)
                .await?;
        let changed = sqlx::query(
            r"
            UPDATE memory_items
            SET
              retrieval_policy_status = 'valid',
              retrieval_policy_fingerprint = $2,
              retrieval_policy_extractor_actor_id = $3,
              retrieval_policy_extractor_version = $4,
              retrieval_policy_validated_at = now()
            WHERE memory_id = $1
            ",
        )
        .bind(memory_id)
        .bind(fingerprint.as_str())
        .bind(extractor.actor_id.as_str())
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
