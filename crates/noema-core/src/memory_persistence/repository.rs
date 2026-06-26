use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    context_graph::{self, ContextGraphFilter, ContextGraphSummary, RelationshipSummary},
    database::DatabaseConfig,
    memory::{MemoryRetrievalRequest, MemoryRetrievalResult, RelationshipStatus},
    paths::NoemaPaths,
    retrieval_policy_fingerprint, sqlite_memory_retrieval,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sqlx::PgPool;

use super::{
    error::MemoryPersistenceError,
    helpers::*,
    models::*,
    objects::{ObjectRef, validate_object_ref_for_conn},
    postgres_schema::POSTGRES_SCHEMA_SQL,
    queries::{MEMORY_SUMMARY_BY_ID_SQL, RECENT_MEMORY_SQL},
};

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
    /// Returns [`MemoryPersistenceError`] when the database URL is invalid,
    /// SQLx cannot connect, or schema bootstrap fails.
    pub async fn connect(database_url: impl Into<String>) -> Result<Self, MemoryPersistenceError> {
        let config = DatabaseConfig::new(database_url)?;
        let pool = config.connect().await?;
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
        Ok(Self { pool })
    }

    /// Underlying Postgres pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// SQLite-backed memory repository.
#[derive(Debug)]
pub struct SqliteMemoryRepository {
    pub(super) conn: Connection,
    db_path: PathBuf,
}

impl SqliteMemoryRepository {
    /// Open the repository at the canonical `db/noema.sqlite` path.
    ///
    /// This creates the root `db/` directory and initializes the minimal
    /// canonical schema if needed.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the database directory cannot be
    /// created, SQLite cannot be opened, or schema migration fails.
    pub fn open(paths: &NoemaPaths) -> Result<Self, MemoryPersistenceError> {
        Self::open_at(paths.root().join("db").join("noema.sqlite"))
    }

    /// Open the canonical database for read-only inspection.
    ///
    /// This constructor is intended for commands such as `noema memory list`
    /// and `noema memory show`. It never creates directories or database files
    /// and does not run migrations.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::MissingDatabase`] when
    /// `db/noema.sqlite` does not already exist. Returns
    /// [`MemoryPersistenceError`] when SQLite cannot open the existing database
    /// in read-only mode or read-only pragmas fail.
    pub fn open_existing_readonly(paths: &NoemaPaths) -> Result<Self, MemoryPersistenceError> {
        Self::open_read_only_at(paths.root().join("db").join("noema.sqlite"))
    }

    /// Open the repository at a specific SQLite database path.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the database directory cannot be
    /// created, SQLite cannot be opened, or schema migration fails.
    pub fn open_at(db_path: impl Into<PathBuf>) -> Result<Self, MemoryPersistenceError> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                MemoryPersistenceError::CreateDatabaseDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let conn = Connection::open(&db_path).map_err(|source| MemoryPersistenceError::Open {
            path: db_path.clone(),
            source,
        })?;
        configure_connection(&conn)?;
        migrate(&conn)?;

        Ok(Self { conn, db_path })
    }

    /// Open an existing database path for read-only inspection.
    ///
    /// This constructor never creates parent directories, never creates the
    /// SQLite database file, and never runs migrations.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::MissingDatabase`] when `db_path` does
    /// not point to an existing file. Returns [`MemoryPersistenceError`] when
    /// SQLite cannot open the database in read-only mode or read-only pragmas
    /// fail.
    pub fn open_read_only_at(db_path: impl Into<PathBuf>) -> Result<Self, MemoryPersistenceError> {
        let db_path = db_path.into();
        if !db_path.is_file() {
            return Err(MemoryPersistenceError::MissingDatabase { path: db_path });
        }

        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|source| MemoryPersistenceError::Open {
                path: db_path.clone(),
                source,
            })?;
        configure_read_only_connection(&conn)?;

        Ok(Self { conn, db_path })
    }

    /// Filesystem path of the opened SQLite database.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Validate that a typed object reference points at an existing row.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::ObjectRefNotFound`] when the
    /// referenced row does not exist, or [`MemoryPersistenceError`] for SQLite
    /// query failures.
    pub fn validate_object_ref(
        &self,
        object_ref: &ObjectRef,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_conn(&self.conn, object_ref)
    }

    /// Upsert the built-in local human and primary Noema agent actors.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite cannot write either actor.
    pub fn ensure_default_actors(&mut self) -> Result<(), MemoryPersistenceError> {
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        tx.execute(
            r"
            INSERT INTO humans (human_id, display_name, handle)
            VALUES ('human:local', 'Local human', 'local')
            ON CONFLICT(human_id) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
            ",
            [],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
        tx.execute(
            r"
            INSERT INTO agents (agent_id, display_name, handle)
            VALUES ('agent:primary', 'Noema', 'primary')
            ON CONFLICT(agent_id) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
            ",
            [],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
        tx.commit().map_err(MemoryPersistenceError::Sqlite)
    }

    /// Rebuild the durable memory search index from canonical memory rows.
    ///
    /// The FTS table is derived state. This method makes index recovery
    /// explicit for development, repair, import, and future export/restore
    /// workflows.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when SQLite reads or writes fail.
    pub fn rebuild_memory_search_index(&mut self) -> Result<usize, MemoryPersistenceError> {
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        tx.execute("DELETE FROM memory_fts", [])?;
        let memories = {
            let mut stmt = tx.prepare(
                r"
                SELECT memory_id, title, content, retrieval_hints
                FROM memory_items
                WHERE status != 'deleted'
                ORDER BY created_at ASC, rowid ASC
                ",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            let mut memories = Vec::new();
            for row in rows {
                memories.push(row?);
            }
            memories
        };
        for (memory_id, title, content, retrieval_hints) in &memories {
            upsert_memory_fts(&tx, memory_id, title, content, retrieval_hints)?;
        }
        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;
        Ok(memories.len())
    }

    /// List recent memories for CLI or dashboard inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub fn list_recent_memories(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<MemorySummary>, MemoryPersistenceError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let mut stmt = self
            .conn
            .prepare(RECENT_MEMORY_SQL)
            .map_err(MemoryPersistenceError::Sqlite)?;
        let rows = stmt
            .query_map(params![limit], row_to_memory_summary)
            .map_err(MemoryPersistenceError::Sqlite)?;

        let mut memories = Vec::new();
        for row in rows {
            memories.push(redact_for_list(row?));
        }
        Ok(memories)
    }

    /// Fetch one memory by id for inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub fn get_memory(
        &self,
        memory_id: &str,
    ) -> Result<Option<MemorySummary>, MemoryPersistenceError> {
        self.conn
            .query_row(
                MEMORY_SUMMARY_BY_ID_SQL,
                params![memory_id],
                row_to_memory_summary,
            )
            .optional()
            .map_err(MemoryPersistenceError::Sqlite)
    }

    /// Append a graph relationship claim edge.
    ///
    /// Active and confirmed relationship claims must name a supporting memory,
    /// and that memory must already have provenance. Candidate relationships
    /// may be stored without supporting memory so review workflows can inspect
    /// them before promotion.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the relationship violates graph
    /// invariants, metadata cannot be serialized, or SQLite writes fail.
    pub fn append_relationship_claim(
        &mut self,
        relationship: &NewRelationshipClaim,
    ) -> Result<RelationshipSummary, MemoryPersistenceError> {
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        if relationship.status != RelationshipStatus::Candidate {
            let memory_id = relationship.memory_id.as_deref().ok_or_else(|| {
                MemoryPersistenceError::RelationshipRequiresSupportingMemory {
                    relationship_id: relationship
                        .relationship_id
                        .clone()
                        .unwrap_or_else(|| "<new relationship>".to_string()),
                }
            })?;
            if !memory_has_provenance(&tx, memory_id)? {
                return Err(
                    MemoryPersistenceError::RelationshipSupportingMemoryMissingProvenance {
                        relationship_id: relationship
                            .relationship_id
                            .clone()
                            .unwrap_or_else(|| "<new relationship>".to_string()),
                        memory_id: memory_id.to_string(),
                    },
                );
            }
        }

        validate_object_ref_for_conn(&tx, &relationship.owner)?;
        let relationship_id = match &relationship.relationship_id {
            Some(relationship_id) => relationship_id.clone(),
            None => allocate_id(&tx, "rel")?,
        };
        tx.execute(
            r"
            INSERT INTO relationships (
              relationship_id,
              owner_object_type,
              owner_object_id,
              subject_entity_id,
              predicate,
              object_entity_id,
              memory_id,
              status,
              confidence,
              valid_from,
              valid_to,
              metadata
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ",
            params![
                relationship_id,
                relationship.owner.object_type.as_str(),
                relationship.owner.object_id.as_str(),
                relationship.subject_entity_id,
                relationship.predicate,
                relationship.object_entity_id,
                relationship.memory_id,
                relationship_status_to_db(relationship.status),
                relationship.confidence,
                relationship.valid_from,
                relationship.valid_to,
                json_to_string(&relationship.metadata)?,
            ],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;

        self.get_relationship(&relationship_id)?
            .ok_or_else(|| MemoryPersistenceError::RelationshipNotFound { relationship_id })
    }

    /// Fetch one relationship claim edge by id.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub fn get_relationship(
        &self,
        relationship_id: &str,
    ) -> Result<Option<RelationshipSummary>, MemoryPersistenceError> {
        context_graph::relationship_by_id(&self.conn, relationship_id)
    }

    /// Inspect the recent persisted context graph.
    ///
    /// This returns recent memory nodes, related entity nodes, subject edges,
    /// participant edges, provenance edges, and recent relationship claim edges.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub fn inspect_context_graph(
        &self,
        limit: Option<u32>,
    ) -> Result<ContextGraphSummary, MemoryPersistenceError> {
        self.inspect_context_graph_with_filter(&ContextGraphFilter::default(), limit)
    }

    /// Inspect the persisted context graph for a run or context packet.
    ///
    /// When the filter is empty this behaves like recent graph inspection.
    /// When `run_id` or `context_packet_id` is set, memory and policy sections
    /// are scoped to memories referenced by the selected packet records.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail or stored enum
    /// values are outside Noema's closed vocabularies.
    pub fn inspect_context_graph_with_filter(
        &self,
        filter: &ContextGraphFilter,
        limit: Option<u32>,
    ) -> Result<ContextGraphSummary, MemoryPersistenceError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        context_graph::inspect(&self.conn, limit, filter)
    }

    /// Retrieve memories from canonical SQLite state using deterministic gates.
    ///
    /// The durable repository loads memory items, subjects, participants,
    /// purpose rules, trusted object links, access grants, provenance, and
    /// relationship claim edges into the shared V1 policy engine before
    /// evaluating the request.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if SQLite reads fail, stored enum
    /// values are outside Noema's closed vocabularies, JSON retrieval hints
    /// cannot be parsed, or graph invariants are violated by stored rows.
    pub fn retrieve_memories(
        &self,
        request: &MemoryRetrievalRequest,
    ) -> Result<MemoryRetrievalResult, MemoryPersistenceError> {
        sqlite_memory_retrieval::retrieve(&self.conn, request)
    }

    /// Mark a memory retrieval policy valid for its current canonical basis.
    ///
    /// The fingerprint covers the memory content and retrieval-authorizing
    /// metadata stored in SQLite, including subjects, participants, purpose
    /// rules, trusted object links, participant visibility, egress policy, and
    /// provenance. Retrieval recomputes this fingerprint and treats mismatches
    /// as stale, so later changes fail closed for private or stronger memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if the memory does not exist,
    /// fingerprint basis rows cannot be read, metadata cannot be serialized, or
    /// SQLite writes fail.
    pub fn refresh_retrieval_policy_fingerprint(
        &mut self,
        memory_id: &str,
        extractor: ObjectRef,
        extractor_version: &str,
    ) -> Result<String, MemoryPersistenceError> {
        validate_object_ref_for_conn(&self.conn, &extractor)?;
        let fingerprint = retrieval_policy_fingerprint::current_fingerprint(&self.conn, memory_id)?;
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        let changed = tx
            .execute(
                r"
                UPDATE memory_items
                SET
                  retrieval_policy_status = 'valid',
                  retrieval_policy_fingerprint = ?2,
                  retrieval_policy_extractor_object_type = ?3,
                  retrieval_policy_extractor_object_id = ?4,
                  retrieval_policy_extractor_version = ?5,
                  retrieval_policy_validated_at = CURRENT_TIMESTAMP
                WHERE memory_id = ?1
                ",
                params![
                    memory_id,
                    &fingerprint,
                    extractor.object_type.as_str(),
                    extractor.object_id.as_str(),
                    extractor_version,
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        if changed == 0 {
            return Err(MemoryPersistenceError::MemoryNotFound {
                memory_id: memory_id.to_string(),
            });
        }
        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;
        Ok(fingerprint)
    }
}
