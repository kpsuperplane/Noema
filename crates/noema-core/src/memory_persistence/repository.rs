use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    context_graph::{self, ContextGraphFilter, ContextGraphSummary, RelationshipSummary},
    memory::{MemoryRetrievalRequest, MemoryRetrievalResult, RelationshipStatus},
    paths::NoemaPaths,
    retrieval_policy_fingerprint, sqlite_memory_retrieval,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, named_params, params};
use serde_json::json;

use super::{
    error::MemoryPersistenceError,
    helpers::*,
    models::*,
    objects::{ObjectRef, validate_object_ref_for_conn},
    queries::{MEMORY_SUMMARY_BY_ID_SQL, RECENT_MEMORY_SQL},
};

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

    /// Append a chat-created memory with provenance and participants.
    ///
    /// The repository upserts placeholder principals, scope, source, and
    /// conversation episode rows so the canonical foreign keys remain intact
    /// before the higher-level runtime has full object management.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when input cannot be serialized,
    /// generated IDs cannot be allocated, or SQLite writes fail.
    pub fn append_chat_memory_candidate(
        &mut self,
        candidate: &NewChatMemoryCandidate,
    ) -> Result<MemorySummary, MemoryPersistenceError> {
        let metadata = json_to_string(&candidate.metadata)?;
        let retrieval_hints = json_to_string(&candidate.retrieval_hints)?;
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        let memory_id = allocate_id(&tx, "mem")?;
        let event_id = allocate_id(&tx, "evt")?;

        ensure_source(&tx)?;
        ensure_scope(&tx, &candidate.home_scope_id)?;
        ensure_principal(&tx, &candidate.created_by_principal_id)?;
        if let Some(owner_principal_id) = &candidate.owner_principal_id {
            ensure_principal(&tx, owner_principal_id)?;
        }
        for participant in &candidate.participants {
            ensure_principal(&tx, &participant.principal_id)?;
        }
        for subject in &candidate.subjects {
            if let Some(linked_principal_id) = &subject.linked_principal_id {
                ensure_principal(&tx, linked_principal_id)?;
            }
        }

        let title = candidate
            .title
            .clone()
            .unwrap_or_else(|| title_from_content(&candidate.content));
        let observed_at = candidate.observed_at.as_deref();
        tx.execute(
            r"
            INSERT INTO memory_items (
              memory_id,
              home_scope_id,
              memory_type,
              title,
              content,
              retrieval_hints,
              status,
              confidence,
              sensitivity,
              created_by_principal_id,
              owner_principal_id,
              authority_level,
              extraction_method,
              observed_at,
              metadata
            )
            VALUES (
              :memory_id,
              :home_scope_id,
              :memory_type,
              :title,
              :content,
              :retrieval_hints,
              :status,
              :confidence,
              :sensitivity,
              :created_by_principal_id,
              :owner_principal_id,
              :authority_level,
              :extraction_method,
              :observed_at,
              :metadata
            )
            ",
            named_params! {
                ":memory_id": memory_id,
                ":home_scope_id": candidate.home_scope_id,
                ":memory_type": candidate.memory_type.as_str(),
                ":title": title,
                ":content": candidate.content,
                ":retrieval_hints": retrieval_hints,
                ":status": memory_status_to_db(candidate.status),
                ":confidence": candidate.confidence,
                ":sensitivity": sensitivity_to_db(candidate.sensitivity),
                ":created_by_principal_id": candidate.created_by_principal_id,
                ":owner_principal_id": candidate.owner_principal_id,
                ":authority_level": candidate.authority_level.as_str(),
                ":extraction_method": candidate.extraction_method.as_str(),
                ":observed_at": observed_at,
                ":metadata": metadata,
            },
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
        upsert_memory_fts(
            &tx,
            &memory_id,
            &title,
            &candidate.content,
            &retrieval_hints,
        )?;

        for participant in &candidate.participants {
            let participant_metadata = json_to_string(&participant.metadata)?;
            tx.execute(
                r"
                INSERT OR IGNORE INTO memory_participants (
                  memory_id,
                  principal_id,
                  role,
                  metadata
                )
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    memory_id,
                    participant.principal_id,
                    participant_role_to_db(participant.role),
                    participant_metadata,
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        for subject in &candidate.subjects {
            upsert_subject_entity(&tx, &candidate.home_scope_id, subject)?;
            tx.execute(
                r"
                INSERT OR IGNORE INTO memory_subjects (
                  memory_id,
                  entity_id,
                  role
                )
                VALUES (?1, ?2, ?3)
                ",
                params![
                    memory_id,
                    subject.entity_id,
                    subject_role_to_db(subject.role)
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        if let Some(source) = &candidate.source {
            ensure_scope(&tx, &source.conversation_id)?;
            ensure_conversation_episode(&tx, source)?;
            insert_provenance_edge(
                &tx,
                &memory_id,
                "episode",
                &source.conversation_id,
                source.evidence_excerpt.as_deref(),
                &candidate.created_by_principal_id,
                json!({"source": "chat", "conversation_id": source.conversation_id}),
            )?;

            if let Some(message_id) = &source.message_id {
                insert_provenance_edge(
                    &tx,
                    &memory_id,
                    "message",
                    message_id,
                    source.evidence_excerpt.as_deref(),
                    &candidate.created_by_principal_id,
                    json!({"source": "chat", "conversation_id": source.conversation_id}),
                )?;
            }
        } else {
            insert_provenance_edge(
                &tx,
                &memory_id,
                "source",
                CHAT_SOURCE_ID,
                None,
                &candidate.created_by_principal_id,
                json!({"source": "chat"}),
            )?;
        }

        tx.execute(
            r"
            INSERT INTO memory_events (
              event_id,
              event_type,
              actor_principal_id,
              memory_id,
              scope_id,
              reason,
              details
            )
            VALUES (?1, 'created', ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                event_id,
                candidate.created_by_principal_id,
                memory_id,
                candidate.home_scope_id,
                "chat_memory_candidate",
                json_to_string(&json!({"source": "chat"}))?,
            ],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;

        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;
        self.memory_summary(&memory_id)
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

    /// Record a complete chat turn for memory provenance.
    ///
    /// The repository upserts the chat source, conversation scope, principals,
    /// and conversation episode, then inserts the user and assistant messages.
    /// Message inserts are idempotent by `message_id` so repeated recording of
    /// the same turn does not duplicate provenance rows.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when input metadata cannot be
    /// serialized or SQLite writes fail.
    pub fn record_chat_turn(&mut self, turn: &NewChatTurn) -> Result<(), MemoryPersistenceError> {
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        let occurred_at = match &turn.occurred_at {
            Some(occurred_at) => occurred_at.clone(),
            None => tx
                .query_row("SELECT CURRENT_TIMESTAMP", [], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(MemoryPersistenceError::Sqlite)?,
        };

        ensure_source(&tx)?;
        ensure_scope(&tx, &turn.conversation_id)?;
        ensure_principal(&tx, &turn.user_principal_id)?;
        ensure_principal(&tx, &turn.assistant_principal_id)?;
        ensure_conversation_episode_id(&tx, &turn.conversation_id)?;

        insert_chat_message(
            &tx,
            ChatMessageRecord {
                message_id: &turn.user_message_id,
                episode_id: &turn.conversation_id,
                author_principal_id: &turn.user_principal_id,
                role: "human",
                content: &turn.user_content,
                occurred_at: &occurred_at,
                metadata: chat_message_metadata(turn, "user"),
            },
        )?;
        insert_chat_message(
            &tx,
            ChatMessageRecord {
                message_id: &turn.assistant_message_id,
                episode_id: &turn.conversation_id,
                author_principal_id: &turn.assistant_principal_id,
                role: "assistant",
                content: &turn.assistant_content,
                occurred_at: &occurred_at,
                metadata: chat_message_metadata(turn, "assistant"),
            },
        )?;

        tx.commit().map_err(MemoryPersistenceError::Sqlite)
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

    fn memory_summary(&self, memory_id: &str) -> Result<MemorySummary, MemoryPersistenceError> {
        self.get_memory(memory_id)?
            .ok_or_else(|| MemoryPersistenceError::MemoryNotFound {
                memory_id: memory_id.to_string(),
            })
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

        ensure_scope(&tx, &relationship.home_scope_id)?;
        let relationship_id = match &relationship.relationship_id {
            Some(relationship_id) => relationship_id.clone(),
            None => allocate_id(&tx, "rel")?,
        };
        tx.execute(
            r"
            INSERT INTO relationships (
              relationship_id,
              home_scope_id,
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
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                relationship_id,
                relationship.home_scope_id,
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
        extractor_principal_id: &str,
        extractor_version: &str,
    ) -> Result<String, MemoryPersistenceError> {
        let fingerprint = retrieval_policy_fingerprint::current_fingerprint(&self.conn, memory_id)?;
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        ensure_principal(&tx, extractor_principal_id)?;
        let changed = tx
            .execute(
                r"
                UPDATE memory_items
                SET
                  retrieval_policy_status = 'valid',
                  retrieval_policy_fingerprint = ?2,
                  retrieval_policy_extractor_principal_id = ?3,
                  retrieval_policy_extractor_version = ?4,
                  retrieval_policy_validated_at = CURRENT_TIMESTAMP
                WHERE memory_id = ?1
                ",
                params![
                    memory_id,
                    &fingerprint,
                    extractor_principal_id,
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
