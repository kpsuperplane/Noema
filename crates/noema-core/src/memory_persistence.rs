//! SQLite-backed durable memory repository.
//!
//! This module is the small persistence slice used by chat integration. It
//! initializes `db/noema.sqlite`, creates the canonical memory tables needed
//! for chat-created memories, writes provenance and participants, and exposes
//! a recent-memory listing for CLI inspection.

use crate::{
    context_graph::{self, ContextGraphFilter, ContextGraphSummary, RelationshipSummary},
    memory::{
        DenialReason, EligibilityReason, MemoryId, MemoryRetrievalRequest, MemoryRetrievalResult,
        MemoryStatus, MemoryStoreError, MemoryUseStage, ParticipantRole, PrincipalId, Purpose,
        RankReason, RelationshipStatus, ScopeId, Sensitivity, SubjectRole,
    },
    paths::NoemaPaths,
    retrieval_policy_fingerprint, sqlite_memory_retrieval,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, named_params, params};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

const CHAT_SOURCE_ID: &str = "source:chat";
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 500;
const BOOTSTRAP_SCHEMA_VERSION: i64 = 0;
const BOOTSTRAP_SCHEMA_NAME: &str = "memory_persistence_bootstrap_v0";

/// SQLite-backed memory repository.
#[derive(Debug)]
pub struct SqliteMemoryRepository {
    conn: Connection,
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

    /// Record a context packet manifest from a memory retrieval result.
    ///
    /// This stores the packet header, memory inclusion edges, audit-only
    /// omissions, redacted agent-visible omission reasons, and memory-use
    /// records. It is the durable audit bridge between deterministic memory
    /// retrieval and later context-packet inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if JSON serialization fails or
    /// SQLite writes fail.
    pub fn record_context_packet(
        &mut self,
        context_packet_id: &str,
        run_id: &str,
        request: &MemoryRetrievalRequest,
        result: &MemoryRetrievalResult,
    ) -> Result<(), MemoryPersistenceError> {
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        ensure_principal(&tx, &request.requesting_principal_id)?;
        for scope_id in &request.trusted.active_scopes {
            ensure_scope(&tx, scope_id)?;
        }

        let active_scopes = json_to_string(&json!(&request.trusted.active_scopes))?;
        let agent_visible_omissions = json_to_string(&json!(agent_visible_omissions_json(result)))?;
        tx.execute(
            r"
            INSERT INTO context_packets (
              context_packet_id,
              run_id,
              requesting_principal_id,
              purpose,
              active_scopes,
              agent_visible_omissions
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                context_packet_id,
                run_id,
                request.requesting_principal_id.as_str(),
                purpose_to_db(request.trusted.purpose),
                active_scopes,
                agent_visible_omissions,
            ],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;

        for included in &result.included {
            let packet_memory_id = allocate_id(&tx, "ctxmem")?;
            let rank_reasons = json_to_string(&json!(
                included
                    .rank_reasons
                    .iter()
                    .map(|reason| rank_reason_to_db(*reason))
                    .collect::<Vec<_>>()
            ))?;
            tx.execute(
                r"
                INSERT INTO context_packet_memories (
                  packet_memory_id,
                  context_packet_id,
                  memory_id,
                  stage,
                  rank_score,
                  eligibility_reason,
                  rank_reasons
                )
                VALUES (?1, ?2, ?3, 'included_in_packet', ?4, ?5, ?6)
                ",
                params![
                    packet_memory_id,
                    context_packet_id,
                    included.memory_id.as_str(),
                    i64::from(included.rank_score),
                    eligibility_reason_to_db(included.eligibility_reason),
                    rank_reasons,
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        let agent_visible_reason = result
            .agent_visible_omissions
            .first()
            .map(|omission| omission.reason)
            .unwrap_or("none");
        for denial in &result.denied_for_audit {
            let omission_id = allocate_id(&tx, "ctxomit")?;
            let omission_sensitivity = denial
                .memory_id
                .as_deref()
                .map(|memory_id| memory_sensitivity_for_tx(&tx, memory_id))
                .transpose()?
                .unwrap_or("normal".to_string());
            tx.execute(
                r"
                INSERT INTO context_packet_omissions (
                  omission_id,
                  context_packet_id,
                  memory_id,
                  relationship_id,
                  omission_sensitivity,
                  agent_visible_reason,
                  audit_reason,
                  details
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    omission_id,
                    context_packet_id,
                    denial.memory_id.as_deref(),
                    denial.relationship_id.as_deref(),
                    omission_sensitivity,
                    agent_visible_reason,
                    denial_reason_to_db(denial.reason),
                    json_to_string(&json!({
                        "run_id": run_id,
                        "purpose": purpose_to_db(request.trusted.purpose),
                    }))?,
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        let event_scope_id = request.trusted.active_scopes.first().map(String::as_str);
        for use_record in &result.use_records {
            insert_memory_use_record(
                &tx,
                ContextMemoryUseInsert {
                    context_packet_id,
                    run_id,
                    memory_id: &use_record.memory_id,
                    stage: use_record.stage,
                    agent_principal_id: &request.requesting_principal_id,
                    scope_id: event_scope_id,
                    purpose: request.trusted.purpose,
                },
            )?;
        }
        for included in &result.included {
            insert_memory_use_record(
                &tx,
                ContextMemoryUseInsert {
                    context_packet_id,
                    run_id,
                    memory_id: &included.memory_id,
                    stage: MemoryUseStage::IncludedInPacket,
                    agent_principal_id: &request.requesting_principal_id,
                    scope_id: event_scope_id,
                    purpose: request.trusted.purpose,
                },
            )?;
        }

        tx.commit().map_err(MemoryPersistenceError::Sqlite)
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

const MEMORY_SUMMARY_BY_ID_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.home_scope_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_type,
  pe.source_id,
  e.raw_ref
FROM memory_items mi
LEFT JOIN memory_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM memory_provenance_edges
  WHERE memory_id = mi.memory_id
  ORDER BY
    CASE source_type WHEN 'episode' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN episodes e
  ON pe.source_type = 'episode'
 AND e.episode_id = pe.source_id
WHERE mi.memory_id = ?1
";

const RECENT_MEMORY_SQL: &str = r"
SELECT
  mi.memory_id,
  mi.status,
  mi.memory_type,
  mi.home_scope_id,
  mi.sensitivity,
  mi.title,
  mi.content,
  mi.created_at,
  pe.source_type,
  pe.source_id,
  e.raw_ref
FROM memory_items mi
LEFT JOIN memory_provenance_edges pe ON pe.edge_id = (
  SELECT edge_id
  FROM memory_provenance_edges
  WHERE memory_id = mi.memory_id
  ORDER BY
    CASE source_type WHEN 'episode' THEN 0 ELSE 1 END,
    created_at ASC,
    edge_id ASC
  LIMIT 1
)
LEFT JOIN episodes e
  ON pe.source_type = 'episode'
 AND e.episode_id = pe.source_id
ORDER BY mi.created_at DESC, mi.rowid DESC
LIMIT ?1
";

/// Memory type stored in `memory_items.memory_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Durable fact.
    Fact,
    /// User or scope preference.
    Preference,
    /// Person-related memory.
    Person,
    /// Organization-related memory.
    Organization,
    /// Project memory.
    Project,
    /// Place memory.
    Place,
    /// Routine or recurring behavior.
    Routine,
    /// Goal memory.
    Goal,
    /// Open loop or follow-up.
    OpenLoop,
    /// Procedure or workflow.
    Procedure,
    /// Constraint.
    Constraint,
    /// Trigger memory.
    Trigger,
    /// Decision.
    Decision,
    /// Agent skill memory.
    Skill,
    /// Policy memory.
    Policy,
    /// General note.
    Note,
    /// Other memory type.
    Other,
}

impl MemoryType {
    /// SQLite representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Place => "place",
            Self::Routine => "routine",
            Self::Goal => "goal",
            Self::OpenLoop => "open_loop",
            Self::Procedure => "procedure",
            Self::Constraint => "constraint",
            Self::Trigger => "trigger",
            Self::Decision => "decision",
            Self::Skill => "skill",
            Self::Policy => "policy",
            Self::Note => "note",
            Self::Other => "other",
        }
    }
}

/// Authority level attached to a created memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryAuthorityLevel {
    /// Human corrected prior state.
    HumanCorrection,
    /// Explicit human statement.
    ExplicitHumanStatement,
    /// Workspace policy.
    WorkspacePolicy,
    /// Project decision.
    ProjectDecision,
    /// Document source.
    DocumentSource,
    /// Repeated observation.
    RepeatedObservation,
    /// Agent inference.
    AgentInference,
    /// Weak inference.
    WeakInference,
    /// System rule.
    SystemRule,
}

impl MemoryAuthorityLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HumanCorrection => "human_correction",
            Self::ExplicitHumanStatement => "explicit_human_statement",
            Self::WorkspacePolicy => "workspace_policy",
            Self::ProjectDecision => "project_decision",
            Self::DocumentSource => "document_source",
            Self::RepeatedObservation => "repeated_observation",
            Self::AgentInference => "agent_inference",
            Self::WeakInference => "weak_inference",
            Self::SystemRule => "system_rule",
        }
    }
}

/// Extraction method attached to a created memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryExtractionMethod {
    /// Direct explicit human instruction.
    ExplicitHuman,
    /// LLM-extracted memory candidate.
    LlmExtracted,
    /// Deterministic rule.
    DeterministicRule,
    /// Imported memory.
    Imported,
    /// Human-edited memory.
    HumanEdited,
    /// Agent summary.
    AgentSummary,
    /// System-generated memory.
    SystemGenerated,
}

impl MemoryExtractionMethod {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitHuman => "explicit_human",
            Self::LlmExtracted => "llm_extracted",
            Self::DeterministicRule => "deterministic_rule",
            Self::Imported => "imported",
            Self::HumanEdited => "human_edited",
            Self::AgentSummary => "agent_summary",
            Self::SystemGenerated => "system_generated",
        }
    }
}

/// Source information for a memory candidate extracted from a chat turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMemorySource {
    /// Conversation id that produced the candidate.
    pub conversation_id: String,
    /// Message id that directly supports the candidate, if known.
    pub message_id: Option<String>,
    /// Short supporting excerpt to show during inspection, if available.
    pub evidence_excerpt: Option<String>,
}

/// Participant to attach to a new memory candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemoryParticipant {
    /// Principal id.
    pub principal_id: PrincipalId,
    /// Participant role.
    pub role: ParticipantRole,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemoryParticipant {
    /// Create a participant with empty metadata.
    #[must_use]
    pub fn new(principal_id: impl Into<PrincipalId>, role: ParticipantRole) -> Self {
        Self {
            principal_id: principal_id.into(),
            role,
            metadata: json!({}),
        }
    }
}

/// Subject entity to bind to a new memory candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemorySubject {
    /// Entity id.
    pub entity_id: String,
    /// Entity type string from the canonical SQLite vocabulary.
    pub entity_type: String,
    /// Canonical entity display name.
    pub canonical_name: String,
    /// Role the entity has in the memory.
    pub role: SubjectRole,
    /// Alternate names for the entity.
    pub aliases: Vec<String>,
    /// Principal linked to the entity, if this entity represents one.
    pub linked_principal_id: Option<PrincipalId>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewMemorySubject {
    /// Create a subject entity binding with empty aliases and metadata.
    #[must_use]
    pub fn new(
        entity_id: impl Into<String>,
        entity_type: impl Into<String>,
        canonical_name: impl Into<String>,
        role: SubjectRole,
    ) -> Self {
        Self {
            entity_id: entity_id.into(),
            entity_type: entity_type.into(),
            canonical_name: canonical_name.into(),
            role,
            aliases: Vec::new(),
            linked_principal_id: None,
            metadata: json!({}),
        }
    }
}

/// New chat turn to persist as message provenance for extracted memories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChatTurn {
    /// Conversation episode id and scope id.
    pub conversation_id: String,
    /// Zero- or one-based turn index assigned by the chat runtime.
    pub turn_index: u64,
    /// Stable user message id.
    pub user_message_id: String,
    /// Stable assistant message id.
    pub assistant_message_id: String,
    /// Principal id for the human/user side of the turn.
    pub user_principal_id: PrincipalId,
    /// Principal id for the assistant side of the turn.
    pub assistant_principal_id: PrincipalId,
    /// User message content.
    pub user_content: String,
    /// Assistant message content.
    pub assistant_content: String,
    /// Optional occurred-at timestamp in canonical text form.
    pub occurred_at: Option<String>,
    /// Additional structured metadata for both message rows.
    pub metadata: Value,
}

impl NewChatTurn {
    /// Create a chat turn with deterministic message ids.
    #[must_use]
    pub fn new(
        conversation_id: impl Into<String>,
        turn_index: u64,
        user_principal_id: impl Into<PrincipalId>,
        assistant_principal_id: impl Into<PrincipalId>,
        user_content: impl Into<String>,
        assistant_content: impl Into<String>,
    ) -> Self {
        let conversation_id = conversation_id.into();
        Self {
            user_message_id: chat_message_id(&conversation_id, "user", turn_index),
            assistant_message_id: chat_message_id(&conversation_id, "assistant", turn_index),
            conversation_id,
            turn_index,
            user_principal_id: user_principal_id.into(),
            assistant_principal_id: assistant_principal_id.into(),
            user_content: user_content.into(),
            assistant_content: assistant_content.into(),
            occurred_at: None,
            metadata: json!({}),
        }
    }
}

/// New memory candidate extracted from a chat turn.
#[derive(Debug, Clone, PartialEq)]
pub struct NewChatMemoryCandidate {
    /// Scope that owns the candidate.
    pub home_scope_id: ScopeId,
    /// Type of memory.
    pub memory_type: MemoryType,
    /// Optional display title. A short title is derived from content when this
    /// is not supplied.
    pub title: Option<String>,
    /// Durable memory content.
    pub content: String,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Initial lifecycle status.
    pub status: MemoryStatus,
    /// Principal that created the candidate.
    pub created_by_principal_id: PrincipalId,
    /// Optional owner principal.
    pub owner_principal_id: Option<PrincipalId>,
    /// Authority level behind the candidate.
    pub authority_level: MemoryAuthorityLevel,
    /// Extraction method.
    pub extraction_method: MemoryExtractionMethod,
    /// Optional confidence score from extraction.
    pub confidence: Option<f64>,
    /// Non-authoritative retrieval hints used for ranking.
    pub retrieval_hints: Value,
    /// Optional observed-at timestamp in canonical text form.
    pub observed_at: Option<String>,
    /// Optional chat provenance.
    pub source: Option<ChatMemorySource>,
    /// Participants in scope when the candidate was formed.
    pub participants: Vec<NewMemoryParticipant>,
    /// Subject entity bindings for the memory.
    pub subjects: Vec<NewMemorySubject>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewChatMemoryCandidate {
    /// Create a default chat memory candidate.
    #[must_use]
    pub fn new(
        home_scope_id: impl Into<ScopeId>,
        content: impl Into<String>,
        created_by_principal_id: impl Into<PrincipalId>,
    ) -> Self {
        Self {
            home_scope_id: home_scope_id.into(),
            memory_type: MemoryType::Note,
            title: None,
            content: content.into(),
            sensitivity: Sensitivity::Normal,
            status: MemoryStatus::Candidate,
            created_by_principal_id: created_by_principal_id.into(),
            owner_principal_id: None,
            authority_level: MemoryAuthorityLevel::AgentInference,
            extraction_method: MemoryExtractionMethod::LlmExtracted,
            confidence: None,
            retrieval_hints: json!({}),
            observed_at: None,
            source: None,
            participants: Vec::new(),
            subjects: Vec::new(),
            metadata: json!({}),
        }
    }
}

/// Recent memory row suitable for CLI inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummary {
    /// Memory id.
    pub id: MemoryId,
    /// Lifecycle status.
    pub status: MemoryStatus,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Scope that owns the memory.
    pub home_scope_id: ScopeId,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
    /// Display title.
    pub title: String,
    /// Memory content.
    pub content: String,
    /// SQLite-created timestamp.
    pub created_at: String,
    /// Provenance source type, if available.
    pub source_type: Option<String>,
    /// Provenance source id, if available.
    pub source_id: Option<String>,
    /// Source conversation id, if available.
    pub conversation_id: Option<String>,
}

/// Relationship claim edge to insert into the persisted context graph.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRelationshipClaim {
    /// Optional stable relationship id. A `rel_` id is allocated when omitted.
    pub relationship_id: Option<String>,
    /// Scope that owns the relationship claim.
    pub home_scope_id: ScopeId,
    /// Subject entity id.
    pub subject_entity_id: String,
    /// Predicate label.
    pub predicate: String,
    /// Object entity id.
    pub object_entity_id: String,
    /// Supporting memory id, required for active or confirmed relationships.
    pub memory_id: Option<MemoryId>,
    /// Relationship lifecycle status.
    pub status: RelationshipStatus,
    /// Optional confidence score from extraction or curation.
    pub confidence: Option<f64>,
    /// Optional start of validity window.
    pub valid_from: Option<String>,
    /// Optional end of validity window.
    pub valid_to: Option<String>,
    /// Additional structured metadata.
    pub metadata: Value,
}

impl NewRelationshipClaim {
    /// Create a candidate relationship claim with no supporting memory yet.
    #[must_use]
    pub fn new(
        home_scope_id: impl Into<ScopeId>,
        subject_entity_id: impl Into<String>,
        predicate: impl Into<String>,
        object_entity_id: impl Into<String>,
    ) -> Self {
        Self {
            relationship_id: None,
            home_scope_id: home_scope_id.into(),
            subject_entity_id: subject_entity_id.into(),
            predicate: predicate.into(),
            object_entity_id: object_entity_id.into(),
            memory_id: None,
            status: RelationshipStatus::Candidate,
            confidence: None,
            valid_from: None,
            valid_to: None,
            metadata: json!({}),
        }
    }
}

/// Errors produced by SQLite memory persistence.
#[derive(Debug, Error)]
pub enum MemoryPersistenceError {
    /// The database directory could not be created.
    #[error("failed to create database directory {}: {source}", path.display())]
    CreateDatabaseDirectory {
        /// Directory path.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },

    /// SQLite could not open the database file.
    #[error("failed to open SQLite database {}: {source}", path.display())]
    Open {
        /// Database path.
        path: PathBuf,
        /// Underlying SQLite error.
        source: rusqlite::Error,
    },

    /// The database file expected for read-only inspection does not exist.
    #[error("noema memory database does not exist: {}", path.display())]
    MissingDatabase {
        /// Missing database path.
        path: PathBuf,
    },

    /// JSON metadata could not be serialized.
    #[error("failed to serialize JSON metadata: {0}")]
    Json(#[from] serde_json::Error),

    /// A database value did not match a closed Noema vocabulary.
    #[error("invalid {kind} value in SQLite: {value}")]
    InvalidEnum {
        /// Vocabulary kind.
        kind: &'static str,
        /// Stored value.
        value: String,
    },

    /// A memory expected to exist was not found.
    #[error("memory not found: {memory_id}")]
    MemoryNotFound {
        /// Missing memory id.
        memory_id: MemoryId,
    },

    /// A relationship expected to exist was not found.
    #[error("relationship not found: {relationship_id}")]
    RelationshipNotFound {
        /// Missing relationship id.
        relationship_id: String,
    },

    /// A current relationship did not include supporting memory.
    #[error("active or confirmed relationship {relationship_id} requires supporting memory")]
    RelationshipRequiresSupportingMemory {
        /// Relationship id or placeholder.
        relationship_id: String,
    },

    /// A relationship's supporting memory lacks provenance.
    #[error("relationship {relationship_id} requires provenance on supporting memory {memory_id}")]
    RelationshipSupportingMemoryMissingProvenance {
        /// Relationship id or placeholder.
        relationship_id: String,
        /// Supporting memory id.
        memory_id: MemoryId,
    },

    /// Stored memory graph rows violated retrieval policy invariants.
    #[error(transparent)]
    MemoryStore(#[from] MemoryStoreError),

    /// SQLite operation failed.
    #[error("SQLite memory persistence failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

fn configure_connection(conn: &Connection) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(
        r"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA synchronous = NORMAL;
        ",
    )?;
    Ok(())
}

fn configure_read_only_connection(conn: &Connection) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(
        r"
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA query_only = ON;
        ",
    )?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), MemoryPersistenceError> {
    conn.execute_batch(MEMORY_SCHEMA_SQL)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
        params![BOOTSTRAP_SCHEMA_VERSION, BOOTSTRAP_SCHEMA_NAME],
    )?;
    Ok(())
}

fn memory_has_provenance(
    tx: &Transaction<'_>,
    memory_id: &str,
) -> Result<bool, MemoryPersistenceError> {
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM memory_provenance_edges WHERE memory_id = ?1",
            params![memory_id],
            |row| row.get(0),
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(count > 0)
}

struct ContextMemoryUseInsert<'a> {
    context_packet_id: &'a str,
    run_id: &'a str,
    memory_id: &'a str,
    stage: MemoryUseStage,
    agent_principal_id: &'a str,
    scope_id: Option<&'a str>,
    purpose: Purpose,
}

fn insert_memory_use_record(
    tx: &Transaction<'_>,
    record: ContextMemoryUseInsert<'_>,
) -> Result<(), MemoryPersistenceError> {
    let memory_use_id = allocate_id(tx, "memuse")?;
    tx.execute(
        r"
        INSERT INTO memory_use_records (
          memory_use_id,
          context_packet_id,
          run_id,
          memory_id,
          stage,
          agent_principal_id,
          scope_id,
          purpose,
          details
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            memory_use_id,
            record.context_packet_id,
            record.run_id,
            record.memory_id,
            memory_use_stage_to_db(record.stage),
            record.agent_principal_id,
            record.scope_id,
            purpose_to_db(record.purpose),
            json_to_string(&json!({
                "context_packet_id": record.context_packet_id,
                "run_id": record.run_id,
                "stage": memory_use_stage_to_db(record.stage),
            }))?,
        ],
    )
    .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(())
}

fn memory_sensitivity_for_tx(
    tx: &Transaction<'_>,
    memory_id: &str,
) -> Result<String, MemoryPersistenceError> {
    tx.query_row(
        "SELECT sensitivity FROM memory_items WHERE memory_id = ?1",
        params![memory_id],
        |row| row.get(0),
    )
    .map_err(MemoryPersistenceError::Sqlite)
}

fn agent_visible_omissions_json(result: &MemoryRetrievalResult) -> Vec<Value> {
    result
        .agent_visible_omissions
        .iter()
        .map(|omission| json!({ "reason": omission.reason }))
        .collect()
}

fn ensure_principal(
    tx: &Transaction<'_>,
    principal_id: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO principals (principal_id, principal_type, display_name, handle)
        VALUES (?1, ?2, ?3, ?1)
        ON CONFLICT(principal_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            principal_id,
            infer_principal_type(principal_id),
            display_name(principal_id),
        ],
    )?;
    Ok(())
}

fn ensure_scope(tx: &Transaction<'_>, scope_id: &str) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO scopes (scope_id, scope_type, name, slug)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(scope_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            scope_id,
            infer_scope_type(scope_id),
            display_name(scope_id),
            slug_for_id(scope_id),
        ],
    )?;
    Ok(())
}

fn ensure_source(tx: &Transaction<'_>) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO sources (source_id, source_type, source_name)
        VALUES (?1, 'chat', 'Noema chat')
        ON CONFLICT(source_id) DO UPDATE SET
          updated_at = CURRENT_TIMESTAMP
        ",
        params![CHAT_SOURCE_ID],
    )?;
    Ok(())
}

fn ensure_conversation_episode(
    tx: &Transaction<'_>,
    source: &ChatMemorySource,
) -> Result<(), MemoryPersistenceError> {
    ensure_conversation_episode_id(tx, &source.conversation_id)
}

fn ensure_conversation_episode_id(
    tx: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO episodes (
          episode_id,
          home_scope_id,
          source_id,
          episode_type,
          title,
          raw_ref,
          occurred_at,
          metadata
        )
        VALUES (?1, ?1, ?2, 'conversation', ?3, ?1, CURRENT_TIMESTAMP, ?4)
        ON CONFLICT(episode_id) DO UPDATE SET
          home_scope_id = COALESCE(episodes.home_scope_id, excluded.home_scope_id),
          source_id = excluded.source_id
        ",
        params![
            conversation_id,
            CHAT_SOURCE_ID,
            display_name(conversation_id),
            json_to_string(&json!({"source": "chat"}))?,
        ],
    )?;
    Ok(())
}

fn upsert_subject_entity(
    tx: &Transaction<'_>,
    home_scope_id: &str,
    subject: &NewMemorySubject,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT INTO entities (
          entity_id,
          home_scope_id,
          entity_type,
          canonical_name,
          aliases,
          linked_principal_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(entity_id) DO UPDATE SET
          home_scope_id = COALESCE(entities.home_scope_id, excluded.home_scope_id),
          entity_type = excluded.entity_type,
          canonical_name = excluded.canonical_name,
          aliases = CASE
            WHEN excluded.aliases = '[]' THEN entities.aliases
            ELSE excluded.aliases
          END,
          linked_principal_id = COALESCE(excluded.linked_principal_id, entities.linked_principal_id),
          metadata = CASE
            WHEN excluded.metadata = '{}' THEN entities.metadata
            ELSE excluded.metadata
          END,
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            subject.entity_id,
            home_scope_id,
            subject.entity_type,
            subject.canonical_name,
            json_to_string(&json!(subject.aliases))?,
            subject.linked_principal_id,
            json_to_string(&subject.metadata)?,
        ],
    )?;
    Ok(())
}

struct ChatMessageRecord<'a> {
    message_id: &'a str,
    episode_id: &'a str,
    author_principal_id: &'a str,
    role: &'a str,
    content: &'a str,
    occurred_at: &'a str,
    metadata: Value,
}

fn insert_chat_message(
    tx: &Transaction<'_>,
    message: ChatMessageRecord<'_>,
) -> Result<(), MemoryPersistenceError> {
    tx.execute(
        r"
        INSERT OR IGNORE INTO messages (
          message_id,
          episode_id,
          author_principal_id,
          role,
          content,
          occurred_at,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            message.message_id,
            message.episode_id,
            message.author_principal_id,
            message.role,
            message.content,
            message.occurred_at,
            json_to_string(&message.metadata)?,
        ],
    )?;
    Ok(())
}

fn insert_provenance_edge(
    tx: &Transaction<'_>,
    memory_id: &str,
    source_type: &str,
    source_id: &str,
    evidence_excerpt: Option<&str>,
    created_by_principal_id: &str,
    metadata: Value,
) -> Result<(), MemoryPersistenceError> {
    let edge_id = allocate_id(tx, "edge")?;
    tx.execute(
        r"
        INSERT INTO memory_provenance_edges (
          edge_id,
          memory_id,
          source_type,
          source_id,
          relation,
          evidence_excerpt,
          created_by_principal_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, 'derived_from', ?5, ?6, ?7)
        ",
        params![
            edge_id,
            memory_id,
            source_type,
            source_id,
            evidence_excerpt,
            created_by_principal_id,
            json_to_string(&metadata)?,
        ],
    )?;
    Ok(())
}

fn allocate_id(conn: &Connection, prefix: &str) -> Result<String, MemoryPersistenceError> {
    let hex = conn.query_row("SELECT lower(hex(randomblob(16)))", [], |row| {
        row.get::<_, String>(0)
    })?;
    Ok(format!("{prefix}_{hex}"))
}

fn row_to_memory_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemorySummary> {
    let status: String = row.get(1)?;
    let memory_type: String = row.get(2)?;
    let sensitivity: String = row.get(4)?;
    Ok(MemorySummary {
        id: row.get(0)?,
        status: parse_memory_status(&status).map_err(enum_to_sql_error)?,
        memory_type: parse_memory_type(&memory_type).map_err(enum_to_sql_error)?,
        home_scope_id: row.get(3)?,
        sensitivity: parse_sensitivity(&sensitivity).map_err(enum_to_sql_error)?,
        title: row.get(5)?,
        content: row.get(6)?,
        created_at: row.get(7)?,
        source_type: row.get(8)?,
        source_id: row.get(9)?,
        conversation_id: row.get(10)?,
    })
}

fn redact_for_list(mut memory: MemorySummary) -> MemorySummary {
    if matches!(
        memory.sensitivity,
        Sensitivity::Sensitive | Sensitivity::Secret
    ) {
        memory.title = "[redacted]".to_string();
        memory.content = "[redacted]".to_string();
    }
    memory
}

fn enum_to_sql_error(error: MemoryPersistenceError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn json_to_string(value: &Value) -> Result<String, MemoryPersistenceError> {
    Ok(serde_json::to_string(value)?)
}

fn chat_message_id(conversation_id: &str, role: &str, turn_index: u64) -> String {
    format!("message:{conversation_id}:{role}:{turn_index}")
}

fn chat_message_metadata(turn: &NewChatTurn, message_role: &str) -> Value {
    json!({
        "source": "chat",
        "turn_index": turn.turn_index,
        "message_role": message_role,
        "turn_metadata": turn.metadata,
    })
}

fn title_from_content(content: &str) -> String {
    let mut title: String = content.trim().chars().take(80).collect();
    if title.is_empty() {
        title.push_str("Untitled memory");
    }
    title
}

fn display_name(id: &str) -> String {
    id.rsplit(':')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(id)
        .replace('_', " ")
}

fn slug_for_id(id: &str) -> String {
    let slug = id.replace([':', '/', ' '], "_");
    if slug.is_empty() {
        "scope".to_string()
    } else {
        slug
    }
}

fn infer_principal_type(principal_id: &str) -> &'static str {
    if principal_id.starts_with("human") {
        "human"
    } else if principal_id.starts_with("agent") {
        "agent"
    } else if principal_id.starts_with("tool") {
        "tool"
    } else {
        "system"
    }
}

fn infer_scope_type(scope_id: &str) -> &'static str {
    if scope_id.starts_with("system") {
        "system"
    } else if scope_id.starts_with("human") {
        "human"
    } else if scope_id.starts_with("workspace") {
        "workspace"
    } else if scope_id.starts_with("project") {
        "project"
    } else if scope_id.starts_with("task") {
        "task"
    } else if scope_id.starts_with("cron") {
        "cron"
    } else if scope_id.starts_with("conversation") {
        "conversation"
    } else if scope_id.starts_with("agent") {
        "agent"
    } else if scope_id.starts_with("relationship") {
        "relationship"
    } else if scope_id.starts_with("tool") {
        "tool"
    } else {
        "custom"
    }
}

fn sensitivity_to_db(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

fn memory_status_to_db(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "candidate",
        MemoryStatus::Active => "active",
        MemoryStatus::Confirmed => "confirmed",
        MemoryStatus::Inferred => "inferred",
        MemoryStatus::Stale => "stale",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Archived => "archived",
        MemoryStatus::Deleted => "deleted",
        MemoryStatus::Disputed => "disputed",
    }
}

fn relationship_status_to_db(status: RelationshipStatus) -> &'static str {
    match status {
        RelationshipStatus::Candidate => "candidate",
        RelationshipStatus::Active => "active",
        RelationshipStatus::Confirmed => "confirmed",
        RelationshipStatus::Superseded => "superseded",
        RelationshipStatus::Archived => "archived",
        RelationshipStatus::Deleted => "deleted",
        RelationshipStatus::Disputed => "disputed",
    }
}

fn purpose_to_db(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::AnswerHumanQuestion => "answer_human_question",
        Purpose::DraftInternalContent => "draft_internal_content",
        Purpose::GeneralPersonalization => "general_personalization",
        Purpose::ManageTask => "manage_task",
        Purpose::ManageCalendar => "manage_calendar",
        Purpose::DraftExternalContent => "draft_external_content",
        Purpose::UseTool => "use_tool",
        Purpose::ProactiveSuggestion => "proactive_suggestion",
        Purpose::ExternalAction => "external_action",
        Purpose::DebugAudit => "debug_audit",
    }
}

fn memory_use_stage_to_db(stage: MemoryUseStage) -> &'static str {
    match stage {
        MemoryUseStage::Retrieved => "retrieved",
        MemoryUseStage::IncludedInPacket => "included_in_packet",
        MemoryUseStage::ShownToAgent => "shown_to_agent",
        MemoryUseStage::UsedInReply => "used_in_reply",
        MemoryUseStage::UsedForAction => "used_for_action",
        MemoryUseStage::UsedForProactivity => "used_for_proactivity",
    }
}

fn eligibility_reason_to_db(reason: EligibilityReason) -> &'static str {
    match reason {
        EligibilityReason::ActiveScope => "active_scope",
        EligibilityReason::ParticipantOverlap => "participant_overlap",
        EligibilityReason::ExplicitGrant => "explicit_grant",
        EligibilityReason::TrustedObjectLink => "trusted_object_link",
        EligibilityReason::PublicHint => "public_hint",
        EligibilityReason::GraphExpansion => "graph_expansion",
    }
}

fn rank_reason_to_db(reason: RankReason) -> &'static str {
    match reason {
        RankReason::ExplicitMemoryRequest => "explicit_memory_request",
        RankReason::TrustedObjectLink => "trusted_object_link",
        RankReason::FuzzyTopic => "fuzzy_topic",
        RankReason::FuzzyKeyword => "fuzzy_keyword",
        RankReason::SameHumanParticipant => "same_human_participant",
        RankReason::GraphExpansion => "graph_expansion",
    }
}

fn denial_reason_to_db(reason: DenialReason) -> &'static str {
    match reason {
        DenialReason::ArchivedOrDeleted => "archived_or_deleted",
        DenialReason::CandidateExcluded => "candidate_excluded",
        DenialReason::DisputedOrStale => "disputed_or_stale",
        DenialReason::ExplicitDenyGrant => "explicit_deny_grant",
        DenialReason::OutsideSearchAperture => "outside_search_aperture",
        DenialReason::SensitivityCeiling => "sensitivity_ceiling",
        DenialReason::RetrievalPolicyInvalid => "retrieval_policy_invalid",
        DenialReason::PurposeDenied => "purpose_denied",
        DenialReason::ParticipantVisibilityDenied => "participant_visibility_denied",
        DenialReason::SensitiveUnlockMissing => "sensitive_unlock_missing",
        DenialReason::SecretApprovalMissing => "secret_approval_missing",
        DenialReason::RelationshipUnsupported => "relationship_unsupported",
        DenialReason::ExternalEgressDenied => "external_egress_denied",
        DenialReason::ExternalEgressApprovalRequired => "external_egress_approval_required",
    }
}

fn parse_sensitivity(value: &str) -> Result<Sensitivity, MemoryPersistenceError> {
    match value {
        "public" => Ok(Sensitivity::Public),
        "normal" => Ok(Sensitivity::Normal),
        "private" => Ok(Sensitivity::Private),
        "sensitive" => Ok(Sensitivity::Sensitive),
        "secret" => Ok(Sensitivity::Secret),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "sensitivity",
            value: value.to_string(),
        }),
    }
}

fn parse_memory_status(value: &str) -> Result<MemoryStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(MemoryStatus::Candidate),
        "active" => Ok(MemoryStatus::Active),
        "confirmed" => Ok(MemoryStatus::Confirmed),
        "inferred" => Ok(MemoryStatus::Inferred),
        "stale" => Ok(MemoryStatus::Stale),
        "superseded" => Ok(MemoryStatus::Superseded),
        "archived" => Ok(MemoryStatus::Archived),
        "deleted" => Ok(MemoryStatus::Deleted),
        "disputed" => Ok(MemoryStatus::Disputed),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "memory status",
            value: value.to_string(),
        }),
    }
}

fn parse_memory_type(value: &str) -> Result<MemoryType, MemoryPersistenceError> {
    match value {
        "fact" => Ok(MemoryType::Fact),
        "preference" => Ok(MemoryType::Preference),
        "person" => Ok(MemoryType::Person),
        "organization" => Ok(MemoryType::Organization),
        "project" => Ok(MemoryType::Project),
        "place" => Ok(MemoryType::Place),
        "routine" => Ok(MemoryType::Routine),
        "goal" => Ok(MemoryType::Goal),
        "open_loop" => Ok(MemoryType::OpenLoop),
        "procedure" => Ok(MemoryType::Procedure),
        "constraint" => Ok(MemoryType::Constraint),
        "trigger" => Ok(MemoryType::Trigger),
        "decision" => Ok(MemoryType::Decision),
        "skill" => Ok(MemoryType::Skill),
        "policy" => Ok(MemoryType::Policy),
        "note" => Ok(MemoryType::Note),
        "other" => Ok(MemoryType::Other),
        _ => Err(MemoryPersistenceError::InvalidEnum {
            kind: "memory type",
            value: value.to_string(),
        }),
    }
}

fn participant_role_to_db(role: ParticipantRole) -> &'static str {
    match role {
        ParticipantRole::HumanInScope => "human_in_scope",
        ParticipantRole::AgentInScope => "agent_in_scope",
        ParticipantRole::Originator => "originator",
        ParticipantRole::Observer => "observer",
    }
}

fn subject_role_to_db(role: SubjectRole) -> &'static str {
    match role {
        SubjectRole::About => "about",
        SubjectRole::Claimant => "claimant",
        SubjectRole::Affected => "affected",
        SubjectRole::Owner => "owner",
        SubjectRole::Assignee => "assignee",
        SubjectRole::Source => "source",
        SubjectRole::Target => "target",
    }
}

const MEMORY_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE TABLE IF NOT EXISTS principals (
  principal_id TEXT PRIMARY KEY,
  principal_type TEXT NOT NULL CHECK (principal_type IN ('human','agent','group','system','tool','service','importer')),
  display_name TEXT NOT NULL,
  handle TEXT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS scopes (
  scope_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL CHECK (scope_type IN ('system','human','workspace','project','task','cron','conversation','agent','relationship','tool','custom')),
  parent_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  description TEXT,
  default_visibility TEXT NOT NULL DEFAULT 'private' CHECK (default_visibility IN ('private','shared','public')),
  default_proactivity_level INTEGER NOT NULL DEFAULT 2 CHECK (default_proactivity_level BETWEEN 0 AND 6),
  is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  UNIQUE(parent_scope_id, slug)
) STRICT;

CREATE TABLE IF NOT EXISTS sources (
  source_id TEXT PRIMARY KEY,
  source_type TEXT NOT NULL CHECK (source_type IN ('chat','email','calendar','file','browser','tool','system','import','human_profile','api','other')),
  source_name TEXT NOT NULL,
  external_ref TEXT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  trust_level INTEGER NOT NULL DEFAULT 3 CHECK (trust_level BETWEEN 0 AND 5),
  retention_policy TEXT NOT NULL DEFAULT 'normal' CHECK (retention_policy IN ('ephemeral','normal','retain','do_not_store')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS episodes (
  episode_id TEXT PRIMARY KEY,
  home_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  source_id TEXT REFERENCES sources(source_id) ON DELETE SET NULL,
  episode_type TEXT NOT NULL CHECK (episode_type IN ('conversation','message','tool_call','file_read','email_seen','calendar_event','task_completed','correction','decision','external_event','system_event','imported_record','other')),
  title TEXT,
  summary TEXT,
  raw_ref TEXT,
  content_hash TEXT,
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS messages (
  message_id TEXT PRIMARY KEY,
  episode_id TEXT NOT NULL REFERENCES episodes(episode_id) ON DELETE CASCADE,
  author_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  role TEXT NOT NULL CHECK (role IN ('human','assistant','agent','tool','system','developer','observer')),
  content TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_items (
  memory_id TEXT PRIMARY KEY,
  home_scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
  memory_type TEXT NOT NULL CHECK (memory_type IN ('fact','preference','person','organization','project','place','routine','goal','open_loop','procedure','constraint','trigger','decision','skill','policy','note','other')),
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  structured_value TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(structured_value)),
  retrieval_hints TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(retrieval_hints)),
  status TEXT NOT NULL DEFAULT 'candidate' CHECK (status IN ('candidate','active','confirmed','inferred','stale','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  sensitivity TEXT NOT NULL DEFAULT 'normal' CHECK (sensitivity IN ('public','normal','private','sensitive','secret')),
  proactivity_level INTEGER NOT NULL DEFAULT 2 CHECK (proactivity_level BETWEEN 0 AND 6),
  retrieval_policy_status TEXT NOT NULL DEFAULT 'needs_review' CHECK (retrieval_policy_status IN ('valid','stale','invalid','needs_review')),
  retrieval_policy_version INTEGER NOT NULL DEFAULT 1 CHECK (retrieval_policy_version >= 1),
  retrieval_policy_fingerprint TEXT,
  retrieval_policy_extractor_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  retrieval_policy_extractor_version TEXT,
  retrieval_policy_validated_at TEXT,
  participant_visibility_policy TEXT NOT NULL DEFAULT 'explicit_grant_only' CHECK (participant_visibility_policy IN ('any_active_human','all_original_humans','owner_only','explicit_grant_only')),
  external_egress_policy TEXT NOT NULL DEFAULT 'approval_required' CHECK (external_egress_policy IN ('allow','approval_required','deny')),
  created_by_principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE RESTRICT,
  owner_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  authority_level TEXT NOT NULL DEFAULT 'agent_inference' CHECK (authority_level IN ('human_correction','explicit_human_statement','workspace_policy','project_decision','document_source','repeated_observation','agent_inference','weak_inference','system_rule')),
  extraction_method TEXT NOT NULL DEFAULT 'llm_extracted' CHECK (extraction_method IN ('explicit_human','llm_extracted','deterministic_rule','imported','human_edited','agent_summary','system_generated')),
  observed_at TEXT,
  valid_from TEXT,
  valid_to TEXT,
  expires_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (
    retrieval_policy_status != 'valid'
    OR (
      retrieval_policy_fingerprint IS NOT NULL
      AND retrieval_policy_extractor_principal_id IS NOT NULL
      AND retrieval_policy_extractor_version IS NOT NULL
      AND retrieval_policy_validated_at IS NOT NULL
    )
  )
) STRICT;

CREATE TABLE IF NOT EXISTS entities (
  entity_id TEXT PRIMARY KEY,
  home_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  entity_type TEXT NOT NULL CHECK (entity_type IN ('human','agent','person','organization','project','workspace','conversation','document','tool','place','task','goal','concept','other')),
  canonical_name TEXT NOT NULL,
  aliases TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(aliases)),
  linked_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_subjects (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('about','claimant','affected','owner','assignee','source','target')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (memory_id, entity_id, role)
) STRICT;

CREATE TABLE IF NOT EXISTS relationships (
  relationship_id TEXT PRIMARY KEY,
  home_scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
  subject_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  predicate TEXT NOT NULL,
  object_entity_id TEXT NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE RESTRICT,
  status TEXT NOT NULL DEFAULT 'candidate' CHECK (status IN ('candidate','active','confirmed','superseded','archived','deleted','disputed')),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  valid_from TEXT,
  valid_to TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (status = 'candidate' OR memory_id IS NOT NULL)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_participants (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('human_in_scope','agent_in_scope','originator','observer')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, principal_id, role)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_retrieval_purpose_rules (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  effect TEXT NOT NULL CHECK (effect IN ('allow','deny')),
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, purpose)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_retrieval_object_links (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  object_type TEXT NOT NULL CHECK (object_type IN ('task','project','workspace','conversation','calendar_event','document','artifact','tool','source','other')),
  object_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('active_context','required_for','relevant_to','open_loop_for','created_from')),
  resolver_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  resolver_version TEXT,
  source_run_id TEXT,
  authorized_scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, object_type, object_id, relation)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_provenance_edges (
  edge_id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  source_type TEXT NOT NULL CHECK (source_type IN ('episode','message','document','tool_result','memory','import','rule','source')),
  source_id TEXT NOT NULL,
  relation TEXT NOT NULL CHECK (relation IN ('derived_from','quoted_from','summarized_from','contradicted_by','supersedes','supports','weakly_supports')),
  evidence_excerpt TEXT,
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_access_grants (
  grant_id TEXT PRIMARY KEY,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE CASCADE,
  principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE CASCADE,
  permission TEXT NOT NULL CHECK (permission IN ('read','write','propose','confirm','delete','use_for_retrieval','use_for_proactivity','use_for_external_action')),
  effect TEXT NOT NULL CHECK (effect IN ('allow','deny')),
  expires_at TEXT,
  created_by_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  CHECK (memory_id IS NOT NULL OR scope_id IS NOT NULL)
) STRICT;

CREATE TABLE IF NOT EXISTS context_packets (
  context_packet_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  requesting_principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE CASCADE,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  active_scopes TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(active_scopes)),
  agent_visible_omissions TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(agent_visible_omissions)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata))
) STRICT;

CREATE TABLE IF NOT EXISTS context_packet_memories (
  packet_memory_id TEXT PRIMARY KEY,
  context_packet_id TEXT NOT NULL REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  stage TEXT NOT NULL CHECK (stage IN ('included_in_packet','shown_to_agent','used_in_reply','used_for_action','used_for_proactivity')),
  rank_score INTEGER CHECK (rank_score IS NULL OR rank_score >= 0),
  eligibility_reason TEXT,
  rank_reasons TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rank_reasons)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS context_packet_omissions (
  omission_id TEXT PRIMARY KEY,
  context_packet_id TEXT NOT NULL REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE SET NULL,
  relationship_id TEXT REFERENCES relationships(relationship_id) ON DELETE SET NULL,
  omission_sensitivity TEXT NOT NULL DEFAULT 'normal' CHECK (omission_sensitivity IN ('public','normal','private','sensitive','secret')),
  agent_visible_reason TEXT NOT NULL,
  audit_reason TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_use_records (
  memory_use_id TEXT PRIMARY KEY,
  context_packet_id TEXT REFERENCES context_packets(context_packet_id) ON DELETE CASCADE,
  run_id TEXT NOT NULL,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  stage TEXT NOT NULL CHECK (stage IN ('retrieved','included_in_packet','shown_to_agent','used_in_reply','used_for_action','used_for_proactivity')),
  agent_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  purpose TEXT NOT NULL CHECK (purpose IN ('answer_human_question','draft_internal_content','general_personalization','manage_task','manage_calendar','draft_external_content','use_tool','proactive_suggestion','external_action','debug_audit')),
  used_for_object_type TEXT,
  used_for_object_id TEXT,
  policy_decision_id TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE TABLE IF NOT EXISTS memory_events (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN ('created','promoted','edited','merged','archived','deleted','retrieved','included_in_packet','shown_to_agent','used_in_reply','used_for_action','used_for_proactivity','exported','confirmed','disputed','superseded','restored')),
  actor_principal_id TEXT REFERENCES principals(principal_id) ON DELETE SET NULL,
  memory_id TEXT REFERENCES memory_items(memory_id) ON DELETE SET NULL,
  scope_id TEXT REFERENCES scopes(scope_id) ON DELETE SET NULL,
  reason TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_messages_episode_time ON messages(episode_id, occurred_at ASC);
CREATE INDEX IF NOT EXISTS idx_memory_items_home_scope ON memory_items(home_scope_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_created_at ON memory_items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_status ON memory_items(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_items_policy_status ON memory_items(retrieval_policy_status, sensitivity);
CREATE INDEX IF NOT EXISTS idx_entities_scope_type ON entities(home_scope_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_canonical_name ON entities(entity_type, canonical_name);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_entity_role ON memory_subjects(entity_id, role, memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_memory_role ON memory_subjects(memory_id, role);
CREATE INDEX IF NOT EXISTS idx_relationships_subject ON relationships(subject_entity_id, predicate);
CREATE INDEX IF NOT EXISTS idx_relationships_object ON relationships(object_entity_id, predicate);
CREATE INDEX IF NOT EXISTS idx_relationships_memory ON relationships(memory_id, status);
CREATE INDEX IF NOT EXISTS idx_memory_participants_principal ON memory_participants(principal_id, role);
CREATE INDEX IF NOT EXISTS idx_memory_retrieval_purpose_rules ON memory_retrieval_purpose_rules(purpose, effect, memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_retrieval_object_links ON memory_retrieval_object_links(object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_memory_provenance_memory ON memory_provenance_edges(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_access_grants_principal ON memory_access_grants(principal_id, permission, effect);
CREATE INDEX IF NOT EXISTS idx_memory_access_grants_memory_scope ON memory_access_grants(memory_id, scope_id);
CREATE INDEX IF NOT EXISTS idx_context_packets_run ON context_packets(run_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_context_packet_memories_packet ON context_packet_memories(context_packet_id, stage);
CREATE INDEX IF NOT EXISTS idx_context_packet_memories_memory ON context_packet_memories(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_context_packet_omissions_packet ON context_packet_omissions(context_packet_id, audit_reason);
CREATE INDEX IF NOT EXISTS idx_context_packet_omissions_memory ON context_packet_omissions(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_run ON memory_use_records(run_id, stage, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_use_records_memory ON memory_use_records(memory_id, stage, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_events_memory_time ON memory_events(memory_id, created_at DESC);
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        DenialReason, Effect, EligibilityReason, ExternalEgressPolicy, MemoryRetrievalRequest,
        MemoryUseStage, ObjectLink, ParticipantVisibilityPolicy, Purpose, RetrievalPolicyStatus,
        TrustedRetrievalContext, UntrustedHints,
    };
    use crate::paths::NoemaPaths;

    #[test]
    fn open_initializes_canonical_database_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");

        let repo = SqliteMemoryRepository::open(&paths).expect("repo");

        assert_eq!(repo.db_path(), &dir.path().join("db").join("noema.sqlite"));
        assert!(repo.db_path().is_file());
    }

    #[test]
    fn readonly_open_missing_database_does_not_create_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let db_dir = dir.path().join("db");
        let db_path = db_dir.join("noema.sqlite");

        let error =
            SqliteMemoryRepository::open_existing_readonly(&paths).expect_err("missing database");

        match error {
            MemoryPersistenceError::MissingDatabase { path } => assert_eq!(path, db_path),
            other => panic!("unexpected error: {other}"),
        }
        assert!(!db_dir.exists());
        assert!(!db_path.exists());
    }

    #[test]
    fn appends_chat_candidate_with_participants_provenance_and_event() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:conv_memory",
            "Kevin prefers persistent chat memory to be inspectable.",
            "agent:primary",
        );
        candidate.memory_type = MemoryType::Preference;
        candidate.title = Some("Inspectable chat memory".to_string());
        candidate.owner_principal_id = Some("human:kevin".to_string());
        candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
        candidate.source = Some(ChatMemorySource {
            conversation_id: "conversation:conv_memory".to_string(),
            message_id: Some("message:user_1".to_string()),
            evidence_excerpt: Some("remember this preference".to_string()),
        });
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];

        let summary = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append");

        assert_eq!(summary.status, MemoryStatus::Candidate);
        assert_eq!(summary.memory_type, MemoryType::Preference);
        assert_eq!(summary.home_scope_id, "conversation:conv_memory");
        assert_eq!(summary.sensitivity, Sensitivity::Normal);
        assert_eq!(summary.title, "Inspectable chat memory");
        assert_eq!(
            summary.conversation_id.as_deref(),
            Some("conversation:conv_memory")
        );

        let participant_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM memory_participants WHERE memory_id = ?1",
                params![summary.id],
                |row| row.get(0),
            )
            .expect("participant count");
        assert_eq!(participant_count, 2);

        let provenance_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM memory_provenance_edges WHERE memory_id = ?1",
                params![summary.id],
                |row| row.get(0),
            )
            .expect("provenance count");
        assert_eq!(provenance_count, 2);

        let event_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM memory_events WHERE memory_id = ?1 AND event_type = 'created'",
                params![summary.id],
                |row| row.get(0),
            )
            .expect("event count");
        assert_eq!(event_count, 1);
    }

    #[test]
    fn appends_confirmed_explicit_chat_memory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:confirmed",
            "Kevin prefers explicit remember commands to be confirmed.",
            "human:local",
        );
        candidate.status = MemoryStatus::Confirmed;
        candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
        candidate.extraction_method = MemoryExtractionMethod::ExplicitHuman;

        let summary = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append");

        assert_eq!(summary.status, MemoryStatus::Confirmed);
    }

    #[test]
    fn appends_chat_candidate_with_confidence_and_retrieval_hints() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:hints",
            "Kevin prefers scoped retrieval hints to stay inspectable.",
            "agent:primary",
        );
        candidate.confidence = Some(0.82);
        candidate.retrieval_hints = json!({
            "topics": ["memory", "retrieval"],
            "keywords": ["inspectable"],
            "summary": "Scoped memory retrieval preference"
        });

        let summary = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append");

        let (confidence, retrieval_hints): (Option<f64>, String) = repo
            .conn
            .query_row(
                "SELECT confidence, retrieval_hints FROM memory_items WHERE memory_id = ?1",
                params![summary.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("memory row");
        assert!((confidence.expect("confidence") - 0.82).abs() < f64::EPSILON);
        assert_eq!(
            serde_json::from_str::<Value>(&retrieval_hints).expect("retrieval hints"),
            candidate.retrieval_hints
        );
    }

    #[test]
    fn appends_chat_candidate_with_subject_entities() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:subjects",
            "Kevin is evaluating Noema memory subjects.",
            "agent:primary",
        );
        let mut human_subject =
            NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::About);
        human_subject.aliases = vec!["KPSuperplane".to_string()];
        human_subject.linked_principal_id = Some("human:kevin".to_string());
        human_subject.metadata = json!({"source": "chat_extraction"});
        candidate.subjects = vec![
            human_subject,
            NewMemorySubject::new(
                "concept:memory_subjects",
                "concept",
                "Memory subjects",
                SubjectRole::About,
            ),
        ];

        let summary = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append");

        let subject_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM memory_subjects WHERE memory_id = ?1",
                params![summary.id],
                |row| row.get(0),
            )
            .expect("subject count");
        assert_eq!(subject_count, 2);

        let (home_scope_id, entity_type, canonical_name, aliases, linked_principal_id, metadata): (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        ) = repo
            .conn
            .query_row(
                r"
                SELECT home_scope_id, entity_type, canonical_name, aliases, linked_principal_id, metadata
                FROM entities
                WHERE entity_id = 'human:kevin'
                ",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("entity row");
        assert_eq!(home_scope_id, "conversation:subjects");
        assert_eq!(entity_type, "human");
        assert_eq!(canonical_name, "Kevin");
        assert_eq!(
            serde_json::from_str::<Value>(&aliases).expect("aliases"),
            json!(["KPSuperplane"])
        );
        assert_eq!(linked_principal_id.as_deref(), Some("human:kevin"));
        assert_eq!(
            serde_json::from_str::<Value>(&metadata).expect("metadata"),
            json!({"source": "chat_extraction"})
        );
    }

    #[test]
    fn appends_relationship_claim_with_supporting_memory_provenance() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:graph",
            "Kevin uses Noema for memory orchestration.",
            "agent:primary",
        );
        candidate.subjects = vec![
            NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
            NewMemorySubject::new("concept:noema", "concept", "Noema", SubjectRole::Target),
        ];
        candidate.source = Some(ChatMemorySource {
            conversation_id: "conversation:graph".to_string(),
            message_id: None,
            evidence_excerpt: Some("Kevin uses Noema".to_string()),
        });
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");

        let mut relationship =
            NewRelationshipClaim::new("conversation:graph", "human:kevin", "uses", "concept:noema");
        relationship.relationship_id = Some("rel_kevin_uses_noema".to_string());
        relationship.status = RelationshipStatus::Active;
        relationship.memory_id = Some(memory.id.clone());
        relationship.confidence = Some(0.86);

        let summary = repo
            .append_relationship_claim(&relationship)
            .expect("relationship");

        assert_eq!(summary.relationship_id, "rel_kevin_uses_noema");
        assert_eq!(summary.status, RelationshipStatus::Active);
        assert_eq!(summary.memory_id.as_deref(), Some(memory.id.as_str()));
        assert_eq!(summary.subject_name.as_deref(), Some("Kevin"));
        assert_eq!(summary.object_name.as_deref(), Some("Noema"));
    }

    #[test]
    fn active_relationship_claim_requires_supporting_provenanced_memory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:graph_policy",
            "Kevin likes policy-aware graph claims.",
            "agent:primary",
        );
        candidate.subjects = vec![
            NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
            NewMemorySubject::new(
                "concept:graph_claims",
                "concept",
                "Graph claims",
                SubjectRole::Target,
            ),
        ];
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");

        let mut no_memory = NewRelationshipClaim::new(
            "conversation:graph_policy",
            "human:kevin",
            "likes",
            "concept:graph_claims",
        );
        no_memory.relationship_id = Some("rel_without_memory".to_string());
        no_memory.status = RelationshipStatus::Active;
        assert!(matches!(
            repo.append_relationship_claim(&no_memory),
            Err(MemoryPersistenceError::RelationshipRequiresSupportingMemory { .. })
        ));

        repo.conn
            .execute(
                "DELETE FROM memory_provenance_edges WHERE memory_id = ?1",
                params![memory.id],
            )
            .expect("delete provenance");
        let mut no_provenance = NewRelationshipClaim::new(
            "conversation:graph_policy",
            "human:kevin",
            "likes",
            "concept:graph_claims",
        );
        no_provenance.relationship_id = Some("rel_without_provenance".to_string());
        no_provenance.status = RelationshipStatus::Confirmed;
        no_provenance.memory_id = Some(memory.id);
        assert!(matches!(
            repo.append_relationship_claim(&no_provenance),
            Err(MemoryPersistenceError::RelationshipSupportingMemoryMissingProvenance { .. })
        ));
    }

    #[test]
    fn inspects_context_graph_from_canonical_tables() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:inspect_graph",
            "Kevin prefers inspectable context graphs.",
            "agent:primary",
        );
        candidate.retrieval_hints = json!({"topics": ["context graph"], "keywords": ["inspect"]});
        candidate.subjects = vec![
            NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
            NewMemorySubject::new(
                "concept:context_graph",
                "concept",
                "Context graph",
                SubjectRole::Target,
            ),
        ];
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        candidate.source = Some(ChatMemorySource {
            conversation_id: "conversation:inspect_graph".to_string(),
            message_id: None,
            evidence_excerpt: Some("inspectable context graphs".to_string()),
        });
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");
        repo.conn
            .execute(
                r"
                UPDATE memory_items
                SET
                  retrieval_policy_status = 'valid',
                  retrieval_policy_version = 2,
                  retrieval_policy_fingerprint = 'sha256:inspect_graph',
                  retrieval_policy_extractor_principal_id = 'agent:primary',
                  retrieval_policy_extractor_version = 'extractor-v1',
                  retrieval_policy_validated_at = '2026-06-24 12:00:00',
                  participant_visibility_policy = 'owner_only',
                  external_egress_policy = 'approval_required'
                WHERE memory_id = ?1
                ",
                params![memory.id],
            )
            .expect("policy metadata");
        let mut relationship = NewRelationshipClaim::new(
            "conversation:inspect_graph",
            "human:kevin",
            "prefers",
            "concept:context_graph",
        );
        relationship.status = RelationshipStatus::Active;
        relationship.memory_id = Some(memory.id.clone());
        repo.append_relationship_claim(&relationship)
            .expect("relationship");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_object_links (
                  memory_id,
                  object_type,
                  object_id,
                  relation,
                  resolver_principal_id,
                  resolver_version,
                  source_run_id,
                  authorized_scope_id,
                  created_by_principal_id
                )
                VALUES (
                  ?1,
                  'project',
                  'project:noema',
                  'active_context',
                  'agent:primary',
                  'resolver-v1',
                  'run:inspect_graph',
                  'conversation:inspect_graph',
                  'agent:primary'
                )
                ",
                params![memory.id],
            )
            .expect("object link");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_purpose_rules (
                  memory_id,
                  purpose,
                  effect,
                  created_by_principal_id
                )
                VALUES
                  (?1, 'answer_human_question', 'allow', 'agent:primary'),
                  (?1, 'external_action', 'deny', 'agent:primary')
                ",
                params![memory.id],
            )
            .expect("purpose rule");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_access_grants (
                  grant_id,
                  memory_id,
                  principal_id,
                  permission,
                  effect,
                  expires_at,
                  created_by_principal_id
                )
                VALUES (
                  'grant_inspect_graph',
                  ?1,
                  'agent:primary',
                  'use_for_retrieval',
                  'allow',
                  '2000-01-01 00:00:00',
                  'human:kevin'
                )
                ",
                params![memory.id],
            )
            .expect("access grant");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_access_grants (
                  grant_id,
                  scope_id,
                  principal_id,
                  permission,
                  effect,
                  created_by_principal_id
                )
                VALUES (
                  'grant_inspect_scope_write',
                  'conversation:inspect_graph',
                  'agent:primary',
                  'write',
                  'allow',
                  'human:kevin'
                )
                ",
                [],
            )
            .expect("scope access grant");
        repo.conn
            .execute(
                r"
                INSERT INTO scopes (scope_id, scope_type, name, slug)
                VALUES (
                  'conversation:unrelated',
                  'conversation',
                  'Unrelated conversation',
                  'unrelated'
                )
                ",
                [],
            )
            .expect("unrelated scope");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_access_grants (
                  grant_id,
                  scope_id,
                  principal_id,
                  permission,
                  effect,
                  created_by_principal_id
                )
                VALUES (
                  'grant_unrelated_scope',
                  'conversation:unrelated',
                  'agent:primary',
                  'use_for_retrieval',
                  'allow',
                  'human:kevin'
                )
                ",
                [],
            )
            .expect("unrelated access grant");
        repo.conn
            .execute(
                r#"
                INSERT INTO memory_events (
                  event_id,
                  event_type,
                  actor_principal_id,
                  memory_id,
                  scope_id,
                  reason,
                  details
                )
                VALUES (
                  'event_inspect_shown',
                  'shown_to_agent',
                  'agent:primary',
                  ?1,
                  'conversation:inspect_graph',
                  'context_packet',
                  '{"run_id":"run:inspect_graph","stage":"shown_to_agent"}'
                )
                "#,
                params![memory.id],
            )
            .expect("memory use event");
        repo.conn
            .execute(
                r#"
                INSERT INTO memory_events (
                  event_id,
                  event_type,
                  actor_principal_id,
                  scope_id,
                  reason,
                  details
                )
                VALUES (
                  'event_inspect_scope_retrieval',
                  'retrieved',
                  'agent:primary',
                  'conversation:inspect_graph',
                  'scope_context',
                  '{"run_id":"run:inspect_graph"}'
                )
                "#,
                [],
            )
            .expect("scope event");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_events (
                  event_id,
                  event_type,
                  actor_principal_id,
                  scope_id,
                  reason,
                  details
                )
                VALUES (
                  'event_unrelated_scope',
                  'retrieved',
                  'agent:primary',
                  'conversation:unrelated',
                  'unrelated',
                  '{}'
                )
                ",
                [],
            )
            .expect("unrelated event");

        let graph = repo.inspect_context_graph(Some(20)).expect("graph");

        assert!(
            graph
                .memories
                .iter()
                .any(|node| node.memory_id == memory.id)
        );
        let memory_node = graph
            .memories
            .iter()
            .find(|node| node.memory_id == memory.id)
            .expect("memory node");
        assert_eq!(
            memory_node.retrieval_policy_status,
            RetrievalPolicyStatus::Valid
        );
        assert_eq!(
            memory_node.retrieval_policy_effective_status,
            RetrievalPolicyStatus::Stale
        );
        assert_eq!(memory_node.retrieval_policy_version, 2);
        assert_eq!(
            memory_node.retrieval_policy_fingerprint.as_deref(),
            Some("sha256:inspect_graph")
        );
        assert_eq!(
            memory_node.participant_visibility_policy,
            ParticipantVisibilityPolicy::OwnerOnly
        );
        assert_eq!(
            memory_node.external_egress_policy,
            ExternalEgressPolicy::ApprovalRequired
        );
        assert!(memory_node.retrieval_hints.contains("context graph"));
        assert!(
            graph
                .entities
                .iter()
                .any(|node| node.entity_id == "concept:context_graph")
        );
        assert!(
            graph
                .subject_edges
                .iter()
                .any(|edge| edge.memory_id == memory.id && edge.entity_id == "human:kevin")
        );
        assert!(
            graph
                .participant_edges
                .iter()
                .any(|edge| edge.memory_id == memory.id && edge.principal_id == "agent:primary")
        );
        assert!(
            graph
                .provenance_edges
                .iter()
                .any(|edge| edge.memory_id == memory.id && edge.source_type == "episode")
        );
        assert!(graph.object_link_edges.iter().any(|edge| {
            edge.memory_id == memory.id
                && edge.object_type == "project"
                && edge.object_id == "project:noema"
                && edge.relation == "active_context"
                && edge.authorized_scope_id.as_deref() == Some("conversation:inspect_graph")
                && edge.resolver_principal_id.as_deref() == Some("agent:primary")
                && edge.resolver_version.as_deref() == Some("resolver-v1")
                && edge.source_run_id.as_deref() == Some("run:inspect_graph")
        }));
        assert!(graph.purpose_rules.iter().any(|rule| {
            rule.memory_id == memory.id
                && rule.purpose == Purpose::AnswerHumanQuestion
                && rule.effect == Effect::Allow
        }));
        assert!(graph.purpose_rules.iter().any(|rule| {
            rule.memory_id == memory.id
                && rule.purpose == Purpose::ExternalAction
                && rule.effect == Effect::Deny
        }));
        assert!(graph.access_grants.iter().any(|grant| {
            grant.grant_id == "grant_inspect_graph"
                && grant.memory_id.as_deref() == Some(memory.id.as_str())
                && grant.principal_id == "agent:primary"
                && grant.permission == "use_for_retrieval"
                && grant.effect == Effect::Allow
                && grant.expires_at.as_deref() == Some("2000-01-01 00:00:00")
        }));
        assert!(graph.access_grants.iter().any(|grant| {
            grant.grant_id == "grant_inspect_scope_write"
                && grant.scope_id.as_deref() == Some("conversation:inspect_graph")
                && grant.principal_id == "agent:primary"
                && grant.permission == "write"
                && grant.effect == Effect::Allow
        }));
        assert!(
            !graph
                .access_grants
                .iter()
                .any(|grant| grant.grant_id == "grant_unrelated_scope")
        );
        assert!(graph.memory_events.iter().any(|event| {
            event.event_id == "event_inspect_shown"
                && event.event_type == "shown_to_agent"
                && event.memory_id.as_deref() == Some(memory.id.as_str())
                && event.memory_sensitivity == Some(Sensitivity::Normal)
                && event.scope_id.as_deref() == Some("conversation:inspect_graph")
                && event.reason.as_deref() == Some("context_packet")
                && event.details.contains("run:inspect_graph")
        }));
        assert!(graph.memory_events.iter().any(|event| {
            event.event_id == "event_inspect_scope_retrieval"
                && event.event_type == "retrieved"
                && event.memory_id.is_none()
                && event.scope_id.as_deref() == Some("conversation:inspect_graph")
        }));
        assert!(
            !graph
                .memory_events
                .iter()
                .any(|event| event.event_id == "event_unrelated_scope")
        );
        assert!(graph.relationships.iter().any(|edge| {
            edge.subject_entity_id == "human:kevin"
                && edge.predicate == "prefers"
                && edge.object_entity_id == "concept:context_graph"
        }));
    }

    #[test]
    fn persisted_retrieval_uses_participant_overlap() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:old_memory",
            "Kevin prefers durable retrieval tests.",
            "agent:primary",
        );
        candidate.status = MemoryStatus::Active;
        candidate.memory_type = MemoryType::Preference;
        candidate.retrieval_hints = json!({"topics": ["memory"], "keywords": ["retrieval"]});
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");

        let mut request = request_for_kevin();
        request.untrusted_hints.fuzzy_topics = vec!["memory".to_string()];
        let result = repo.retrieve_memories(&request).expect("retrieve");

        assert_eq!(included_ids(&result), vec![memory.id.as_str()]);
        assert_eq!(
            result.included[0].eligibility_reason,
            EligibilityReason::ParticipantOverlap
        );
    }

    #[test]
    fn persisted_retrieval_requires_trusted_unlock_for_sensitive_memory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:health",
            "Kevin needs to follow up about a doctor appointment.",
            "agent:primary",
        );
        candidate.status = MemoryStatus::Active;
        candidate.memory_type = MemoryType::OpenLoop;
        candidate.sensitivity = Sensitivity::Sensitive;
        candidate.owner_principal_id = Some("human:kevin".to_string());
        candidate.retrieval_hints =
            json!({"topics": ["health", "doctor"], "keywords": ["appointment"]});
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_purpose_rules (
                  memory_id,
                  purpose,
                  effect,
                  created_by_principal_id
                )
                VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
                ",
                params![memory.id],
            )
            .expect("purpose rule");
        validate_sensitive_policy(&mut repo, &memory.id);

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request.untrusted_hints.fuzzy_topics = vec!["health".to_string(), "doctor".to_string()];
        let denied = repo.retrieve_memories(&request).expect("retrieve denied");

        assert!(denied.included.is_empty());
        assert_eq!(
            denied.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );
        assert_eq!(
            denied.agent_visible_omissions[0].reason,
            "policy_restricted_context"
        );

        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_object_links (
                  memory_id,
                  object_type,
                  object_id,
                  relation,
                  created_by_principal_id
                )
                VALUES (?1, 'task', 'task:schedule_checkup', 'open_loop_for', 'agent:primary')
                ",
                params![memory.id],
            )
            .expect("object link");
        validate_sensitive_policy(&mut repo, &memory.id);
        request
            .trusted
            .active_object_links
            .push(ObjectLink::new("task", "task:schedule_checkup").with_relation("open_loop_for"));

        let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");

        assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
        assert_eq!(
            allowed.included[0].eligibility_reason,
            EligibilityReason::TrustedObjectLink
        );

        repo.conn
            .execute(
                "UPDATE memory_items SET content = 'Changed sensitive content' WHERE memory_id = ?1",
                params![memory.id],
            )
            .expect("change sensitive content");
        let stale = repo.retrieve_memories(&request).expect("retrieve stale");
        assert!(stale.included.is_empty());
        assert_eq!(
            stale.denied_for_audit[0].reason,
            DenialReason::RetrievalPolicyInvalid
        );
    }

    #[test]
    fn persisted_object_link_requires_authorized_scope_when_policy_sets_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:health",
            "Kevin needs to follow up about a doctor appointment.",
            "agent:primary",
        );
        candidate.status = MemoryStatus::Active;
        candidate.memory_type = MemoryType::OpenLoop;
        candidate.sensitivity = Sensitivity::Sensitive;
        candidate.owner_principal_id = Some("human:kevin".to_string());
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_purpose_rules (
                  memory_id,
                  purpose,
                  effect,
                  created_by_principal_id
                )
                VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
                ",
                params![memory.id],
            )
            .expect("purpose rule");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_object_links (
                  memory_id,
                  object_type,
                  object_id,
                  relation,
                  authorized_scope_id,
                  created_by_principal_id
                )
                VALUES (
                  ?1,
                  'task',
                  'task:schedule_checkup',
                  'open_loop_for',
                  'conversation:health',
                  'agent:primary'
                )
                ",
                params![memory.id],
            )
            .expect("object link");
        validate_sensitive_policy(&mut repo, &memory.id);

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Sensitive;
        request.trusted.active_object_links =
            vec![ObjectLink::new("task", "task:schedule_checkup").with_relation("open_loop_for")];
        let denied = repo.retrieve_memories(&request).expect("retrieve denied");
        assert!(denied.included.is_empty());
        assert_eq!(
            denied.denied_for_audit[0].reason,
            DenialReason::SensitiveUnlockMissing
        );

        request
            .trusted
            .active_scopes
            .push("conversation:health".to_string());
        let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");
        assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
    }

    #[test]
    fn persisted_retrieval_honors_explicit_grant_for_private_memory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:private",
            "Kevin keeps a private project preference.",
            "agent:primary",
        );
        candidate.status = MemoryStatus::Active;
        candidate.sensitivity = Sensitivity::Private;
        candidate.owner_principal_id = Some("human:kevin".to_string());
        candidate.participants = vec![
            NewMemoryParticipant::new("human:kevin", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        let memory = repo
            .append_chat_memory_candidate(&candidate)
            .expect("memory");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_retrieval_purpose_rules (
                  memory_id,
                  purpose,
                  effect,
                  created_by_principal_id
                )
                VALUES (?1, 'answer_human_question', 'allow', 'agent:primary')
                ",
                params![memory.id],
            )
            .expect("purpose rule");
        validate_private_policy(&mut repo, &memory.id);

        let mut request = request_for_kevin();
        request.trusted.sensitivity_ceiling = Sensitivity::Private;
        let denied = repo.retrieve_memories(&request).expect("retrieve denied");
        assert!(denied.included.is_empty());

        repo.conn
            .execute(
                r"
                INSERT INTO memory_access_grants (
                  grant_id,
                  memory_id,
                  principal_id,
                  permission,
                  effect,
                  created_by_principal_id
                )
                VALUES ('grant_private_memory', ?1, 'agent:primary', 'use_for_retrieval', 'allow', 'human:kevin')
                ",
                params![memory.id],
            )
            .expect("grant");

        let stale_after_grant = repo
            .retrieve_memories(&request)
            .expect("retrieve stale after grant");
        assert!(stale_after_grant.included.is_empty());
        assert_eq!(
            stale_after_grant.denied_for_audit[0].reason,
            DenialReason::RetrievalPolicyInvalid
        );

        validate_private_policy(&mut repo, &memory.id);
        let allowed = repo.retrieve_memories(&request).expect("retrieve allowed");
        assert_eq!(included_ids(&allowed), vec![memory.id.as_str()]);
        assert_eq!(
            allowed.included[0].eligibility_reason,
            EligibilityReason::ExplicitGrant
        );
    }

    #[test]
    fn persisted_retrieval_expands_one_hop_graph_after_policy() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut anchor = NewChatMemoryCandidate::new(
            "project:noema",
            "Noema uses an inspectable memory system.",
            "agent:primary",
        );
        anchor.status = MemoryStatus::Active;
        anchor.subjects = vec![
            NewMemorySubject::new("project:noema", "project", "Noema", SubjectRole::About),
            NewMemorySubject::new(
                "concept:scoped_graph_claims",
                "concept",
                "Scoped graph claims",
                SubjectRole::About,
            ),
            NewMemorySubject::new(
                "concept:unscoped_graph_claims",
                "concept",
                "Unscoped graph claims",
                SubjectRole::About,
            ),
        ];
        let anchor_memory = repo.append_chat_memory_candidate(&anchor).expect("anchor");

        let mut backing = NewChatMemoryCandidate::new(
            "conversation:graph_evidence",
            "Noema graph claims are scoped and memory-backed.",
            "agent:primary",
        );
        backing.status = MemoryStatus::Active;
        backing.participants = vec![NewMemoryParticipant::new(
            "human:kevin",
            ParticipantRole::HumanInScope,
        )];
        let backing_memory = repo
            .append_chat_memory_candidate(&backing)
            .expect("backing");

        let mut relationship = NewRelationshipClaim::new(
            "project:noema",
            "project:noema",
            "uses",
            "concept:scoped_graph_claims",
        );
        relationship.status = RelationshipStatus::Active;
        relationship.memory_id = Some(backing_memory.id.clone());
        repo.append_relationship_claim(&relationship)
            .expect("relationship");
        let mut unscoped_backing = NewChatMemoryCandidate::new(
            "conversation:unscoped_graph_evidence",
            "This graph claim has no participant aperture.",
            "agent:primary",
        );
        unscoped_backing.status = MemoryStatus::Active;
        let unscoped_memory = repo
            .append_chat_memory_candidate(&unscoped_backing)
            .expect("unscoped backing");
        let mut unscoped_relationship = NewRelationshipClaim::new(
            "project:noema",
            "project:noema",
            "mentions",
            "concept:unscoped_graph_claims",
        );
        unscoped_relationship.status = RelationshipStatus::Active;
        unscoped_relationship.memory_id = Some(unscoped_memory.id.clone());
        repo.append_relationship_claim(&unscoped_relationship)
            .expect("unscoped relationship");

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("project:noema".to_string());
        let result = repo.retrieve_memories(&request).expect("retrieve");

        assert_eq!(
            included_ids(&result),
            vec![anchor_memory.id.as_str(), backing_memory.id.as_str()]
        );
        assert!(
            result
                .included
                .iter()
                .any(|memory| memory.memory_id == backing_memory.id
                    && memory.eligibility_reason == EligibilityReason::ParticipantOverlap)
        );
        assert!(result.denied_for_audit.iter().any(|denial| {
            denial.memory_id.as_deref() == Some(unscoped_memory.id.as_str())
                && denial.reason == DenialReason::OutsideSearchAperture
        }));
    }

    #[test]
    fn records_context_packet_with_omissions_and_memory_use_records() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut included = NewChatMemoryCandidate::new(
            "conversation:packet",
            "Noema should record context packet manifests.",
            "agent:primary",
        );
        included.status = MemoryStatus::Active;
        included.participants = vec![NewMemoryParticipant::new(
            "human:kevin",
            ParticipantRole::HumanInScope,
        )];
        let included_memory = repo
            .append_chat_memory_candidate(&included)
            .expect("included memory");

        let mut denied = NewChatMemoryCandidate::new(
            "conversation:packet",
            "Sensitive packet detail should stay audit-only.",
            "agent:primary",
        );
        denied.status = MemoryStatus::Active;
        denied.sensitivity = Sensitivity::Sensitive;
        denied.participants = vec![NewMemoryParticipant::new(
            "human:kevin",
            ParticipantRole::HumanInScope,
        )];
        let denied_memory = repo
            .append_chat_memory_candidate(&denied)
            .expect("denied memory");

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("conversation:packet".to_string());
        let result = repo.retrieve_memories(&request).expect("retrieve");

        assert_eq!(included_ids(&result), vec![included_memory.id.as_str()]);
        assert!(
            result
                .use_records
                .iter()
                .all(|record| record.stage == MemoryUseStage::Retrieved)
        );
        assert!(result.denied_for_audit.iter().any(|denial| {
            denial.memory_id.as_deref() == Some(denied_memory.id.as_str())
                && denial.reason == DenialReason::SensitivityCeiling
        }));

        repo.record_context_packet("ctx_packet", "run:packet", &request, &result)
            .expect("record packet");
        repo.record_context_packet("ctx_other", "run:other", &request, &result)
            .expect("record other packet");
        let mut unrelated = NewChatMemoryCandidate::new(
            "conversation:unrelated_packet",
            "This memory is newer but not part of the packet.",
            "agent:primary",
        );
        unrelated.status = MemoryStatus::Active;
        unrelated.participants = vec![NewMemoryParticipant::new(
            "human:someone_else",
            ParticipantRole::HumanInScope,
        )];
        let unrelated_memory = repo
            .append_chat_memory_candidate(&unrelated)
            .expect("unrelated memory");

        let mut relationship_backing = NewChatMemoryCandidate::new(
            "conversation:packet",
            "Relationship-only omissions still need supporting memory context.",
            "agent:primary",
        );
        relationship_backing.status = MemoryStatus::Active;
        relationship_backing.participants = vec![NewMemoryParticipant::new(
            "human:kevin",
            ParticipantRole::HumanInScope,
        )];
        relationship_backing.subjects = vec![
            NewMemorySubject::new("human:kevin", "human", "Kevin", SubjectRole::Source),
            NewMemorySubject::new(
                "concept:packet_relationship",
                "concept",
                "Packet Relationship",
                SubjectRole::Target,
            ),
        ];
        let relationship_memory = repo
            .append_chat_memory_candidate(&relationship_backing)
            .expect("relationship memory");
        let mut relationship = NewRelationshipClaim::new(
            "conversation:packet",
            "human:kevin",
            "prefers",
            "concept:packet_relationship",
        );
        relationship.status = RelationshipStatus::Active;
        relationship.memory_id = Some(relationship_memory.id.clone());
        let relationship = repo
            .append_relationship_claim(&relationship)
            .expect("relationship");
        repo.conn
            .execute(
                r#"
                INSERT INTO context_packet_omissions (
                  omission_id,
                  context_packet_id,
                  relationship_id,
                  omission_sensitivity,
                  agent_visible_reason,
                  audit_reason,
                  details
                )
                VALUES (
                  'ctxomit_relationship_only',
                  'ctx_packet',
                  ?1,
                  'normal',
                  'policy_restricted_context',
                  'outside_search_aperture',
                  '{"source":"test"}'
                )
                "#,
                params![relationship.relationship_id],
            )
            .expect("relationship-only omission");
        repo.conn
            .execute(
                r"
                INSERT INTO memory_access_grants (
                  grant_id,
                  scope_id,
                  principal_id,
                  permission,
                  effect
                )
                VALUES (
                  'grant_same_scope_unrelated',
                  'conversation:packet',
                  'agent:primary',
                  'use_for_retrieval',
                  'allow'
                )
                ",
                [],
            )
            .expect("same-scope grant");
        repo.conn
            .execute(
                r#"
                INSERT INTO memory_events (
                  event_id,
                  event_type,
                  actor_principal_id,
                  scope_id,
                  reason,
                  details
                )
                VALUES (
                  'event_same_scope_unrelated',
                  'retrieved',
                  'agent:primary',
                  'conversation:packet',
                  'same_scope_unrelated',
                  '{"run_id":"run:other"}'
                )
                "#,
                [],
            )
            .expect("same-scope event");
        let graph = repo.inspect_context_graph(Some(20)).expect("graph");

        assert!(graph.context_packets.iter().any(|packet| {
            packet.context_packet_id == "ctx_packet"
                && packet.run_id == "run:packet"
                && packet
                    .agent_visible_omissions
                    .contains("policy_restricted_context")
        }));
        assert!(graph.context_packet_memory_edges.iter().any(|edge| {
            edge.context_packet_id == "ctx_packet"
                && edge.memory_id == included_memory.id
                && edge.stage == "included_in_packet"
        }));
        assert!(graph.context_packet_omissions.iter().any(|omission| {
            omission.context_packet_id == "ctx_packet"
                && omission.memory_id.as_deref() == Some(denied_memory.id.as_str())
                && omission.omission_sensitivity == Sensitivity::Sensitive
                && omission.agent_visible_reason == "policy_restricted_context"
                && omission.audit_reason == "sensitivity_ceiling"
        }));
        assert!(graph.memory_use_records.iter().any(|record| {
            record.context_packet_id.as_deref() == Some("ctx_packet")
                && record.run_id == "run:packet"
                && record.memory_id == included_memory.id
                && record.stage == "retrieved"
        }));
        assert!(graph.memory_use_records.iter().any(|record| {
            record.context_packet_id.as_deref() == Some("ctx_packet")
                && record.run_id == "run:packet"
                && record.memory_id == included_memory.id
                && record.stage == "included_in_packet"
        }));

        let filtered = repo
            .inspect_context_graph_with_filter(
                &ContextGraphFilter {
                    run_id: Some("run:packet".to_string()),
                    context_packet_id: None,
                },
                Some(20),
            )
            .expect("filtered graph");
        assert_eq!(filtered.context_packets.len(), 1);
        assert_eq!(filtered.context_packets[0].context_packet_id, "ctx_packet");
        assert!(
            !filtered
                .memories
                .iter()
                .any(|memory| memory.memory_id == unrelated_memory.id)
        );
        assert!(
            filtered
                .memories
                .iter()
                .any(|memory| memory.memory_id == relationship_memory.id)
        );
        assert!(
            filtered
                .relationships
                .iter()
                .any(|edge| edge.relationship_id == relationship.relationship_id)
        );
        assert!(
            !filtered
                .access_grants
                .iter()
                .any(|grant| grant.grant_id == "grant_same_scope_unrelated")
        );
        assert!(
            !filtered
                .memory_events
                .iter()
                .any(|event| event.event_id == "event_same_scope_unrelated")
        );
        assert!(
            filtered
                .memory_use_records
                .iter()
                .all(|record| record.run_id == "run:packet")
        );
        assert!(
            filtered
                .context_packet_omissions
                .iter()
                .all(|omission| omission.context_packet_id == "ctx_packet")
        );

        let packet_filtered = repo
            .inspect_context_graph_with_filter(
                &ContextGraphFilter {
                    run_id: None,
                    context_packet_id: Some("ctx_packet".to_string()),
                },
                Some(20),
            )
            .expect("packet-filtered graph");
        assert_eq!(packet_filtered.context_packets.len(), 1);
        assert_eq!(
            packet_filtered.context_packets[0].context_packet_id,
            "ctx_packet"
        );
        assert!(
            !packet_filtered
                .context_packets
                .iter()
                .any(|packet| packet.context_packet_id == "ctx_other")
        );
    }

    #[test]
    fn persisted_retrieval_respects_validity_windows() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut expired = NewChatMemoryCandidate::new(
            "conversation:expired",
            "Kevin once preferred an expired memory.",
            "agent:primary",
        );
        expired.status = MemoryStatus::Active;
        expired.participants = vec![NewMemoryParticipant::new(
            "human:kevin",
            ParticipantRole::HumanInScope,
        )];
        let expired_memory = repo
            .append_chat_memory_candidate(&expired)
            .expect("expired memory");
        repo.conn
            .execute(
                "UPDATE memory_items SET expires_at = '2000-01-01T00:00:00Z' WHERE memory_id = ?1",
                params![expired_memory.id],
            )
            .expect("expire memory");

        let mut anchor = NewChatMemoryCandidate::new(
            "project:validity",
            "Noema has validity-windowed graph claims.",
            "agent:primary",
        );
        anchor.status = MemoryStatus::Active;
        anchor.subjects = vec![
            NewMemorySubject::new(
                "project:validity",
                "project",
                "Validity",
                SubjectRole::About,
            ),
            NewMemorySubject::new(
                "concept:expired_edge",
                "concept",
                "Expired edge",
                SubjectRole::About,
            ),
        ];
        let anchor_memory = repo.append_chat_memory_candidate(&anchor).expect("anchor");
        let mut graph_backing = NewChatMemoryCandidate::new(
            "conversation:validity_graph",
            "A public graph backing memory should be excluded by an expired edge.",
            "agent:primary",
        );
        graph_backing.status = MemoryStatus::Active;
        graph_backing.sensitivity = Sensitivity::Public;
        let graph_memory = repo
            .append_chat_memory_candidate(&graph_backing)
            .expect("graph backing");
        let mut relationship = NewRelationshipClaim::new(
            "project:validity",
            "project:validity",
            "mentions",
            "concept:expired_edge",
        );
        relationship.relationship_id = Some("rel_expired_validity".to_string());
        relationship.status = RelationshipStatus::Active;
        relationship.memory_id = Some(graph_memory.id.clone());
        repo.append_relationship_claim(&relationship)
            .expect("relationship");
        repo.conn
            .execute(
                "UPDATE relationships SET valid_to = '2000-01-01T00:00:00Z' WHERE relationship_id = 'rel_expired_validity'",
                [],
            )
            .expect("expire relationship");

        let mut request = request_for_kevin();
        request
            .trusted
            .active_scopes
            .push("project:validity".to_string());
        let expired_result = repo.retrieve_memories(&request).expect("retrieve expired");

        assert_eq!(
            included_ids(&expired_result),
            vec![anchor_memory.id.as_str()]
        );

        repo.conn
            .execute(
                "UPDATE relationships SET valid_to = NULL WHERE relationship_id = 'rel_expired_validity'",
                [],
            )
            .expect("restore relationship");
        let active_result = repo.retrieve_memories(&request).expect("retrieve active");

        assert_eq!(
            included_ids(&active_result),
            vec![anchor_memory.id.as_str(), graph_memory.id.as_str()]
        );
        assert!(
            active_result
                .included
                .iter()
                .any(|memory| memory.memory_id == graph_memory.id
                    && memory.eligibility_reason == EligibilityReason::GraphExpansion)
        );
    }

    #[test]
    fn records_chat_turn_idempotently() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut turn = NewChatTurn::new(
            "conversation:turns",
            7,
            "human:kevin",
            "agent:primary",
            "Please remember that I care about provenance.",
            "Noted.",
        );
        turn.occurred_at = Some("2026-06-24T12:00:00Z".to_string());
        turn.metadata = json!({"request_id": "req_123"});

        repo.record_chat_turn(&turn).expect("first record");
        repo.record_chat_turn(&turn).expect("second record");

        let message_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE episode_id = ?1",
                params![turn.conversation_id],
                |row| row.get(0),
            )
            .expect("message count");
        assert_eq!(message_count, 2);

        let user_message: (String, String, String, String) = repo
            .conn
            .query_row(
                r"
                SELECT role, author_principal_id, content, occurred_at
                FROM messages
                WHERE message_id = ?1
                ",
                params![turn.user_message_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("user message");
        assert_eq!(
            user_message,
            (
                "human".to_string(),
                "human:kevin".to_string(),
                "Please remember that I care about provenance.".to_string(),
                "2026-06-24T12:00:00Z".to_string(),
            )
        );

        let assistant_message_id_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE message_id = ?1",
                params![turn.assistant_message_id],
                |row| row.get(0),
            )
            .expect("assistant count");
        assert_eq!(assistant_message_id_count, 1);

        let bootstrap_count: i64 = repo
            .conn
            .query_row(
                r"
                SELECT
                  (SELECT COUNT(*) FROM sources WHERE source_id = ?1)
                  + (SELECT COUNT(*) FROM scopes WHERE scope_id = ?2)
                  + (SELECT COUNT(*) FROM episodes WHERE episode_id = ?2 AND home_scope_id = ?2)
                  + (SELECT COUNT(*) FROM principals WHERE principal_id IN (?3, ?4))
                ",
                params![
                    CHAT_SOURCE_ID,
                    turn.conversation_id,
                    turn.user_principal_id,
                    turn.assistant_principal_id,
                ],
                |row| row.get(0),
            )
            .expect("bootstrap count");
        assert_eq!(bootstrap_count, 5);
    }

    #[test]
    fn lists_recent_memories_with_inspection_fields() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");

        let mut first =
            NewChatMemoryCandidate::new("conversation:older", "Older memory", "agent:primary");
        first.source = Some(ChatMemorySource {
            conversation_id: "conversation:older".to_string(),
            message_id: None,
            evidence_excerpt: None,
        });
        repo.append_chat_memory_candidate(&first).expect("first");

        let mut second =
            NewChatMemoryCandidate::new("conversation:newer", "Newer memory", "agent:primary");
        second.memory_type = MemoryType::Fact;
        second.sensitivity = Sensitivity::Private;
        second.source = Some(ChatMemorySource {
            conversation_id: "conversation:newer".to_string(),
            message_id: None,
            evidence_excerpt: None,
        });
        repo.append_chat_memory_candidate(&second).expect("second");

        let memories = repo.list_recent_memories(Some(10)).expect("list");

        assert_eq!(memories.len(), 2);
        assert_eq!(memories[0].content, "Newer memory");
        assert_eq!(memories[0].memory_type, MemoryType::Fact);
        assert_eq!(memories[0].sensitivity, Sensitivity::Private);
        assert_eq!(
            memories[0].conversation_id.as_deref(),
            Some("conversation:newer")
        );
        assert_eq!(memories[1].content, "Older memory");
    }

    #[test]
    fn list_redacts_sensitive_and_secret_memory_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut sensitive = NewChatMemoryCandidate::new(
            "conversation:sensitive",
            "Sensitive medical detail",
            "agent:primary",
        );
        sensitive.title = Some("Medical detail".to_string());
        sensitive.sensitivity = Sensitivity::Sensitive;
        let sensitive_created = repo
            .append_chat_memory_candidate(&sensitive)
            .expect("sensitive");
        let mut secret = NewChatMemoryCandidate::new(
            "conversation:secret",
            "Secret credential-like detail",
            "agent:primary",
        );
        secret.title = Some("Credential detail".to_string());
        secret.sensitivity = Sensitivity::Secret;
        repo.append_chat_memory_candidate(&secret).expect("secret");

        let memories = repo.list_recent_memories(Some(10)).expect("list");

        assert_eq!(memories.len(), 2);
        assert!(memories.iter().all(|memory| memory.title == "[redacted]"));
        assert!(memories.iter().all(|memory| memory.content == "[redacted]"));
        let fetched = repo
            .get_memory(&sensitive_created.id)
            .expect("get")
            .expect("memory");
        assert_eq!(fetched.title, "Medical detail");
        assert_eq!(fetched.content, "Sensitive medical detail");
    }

    #[test]
    fn fetches_single_memory_by_id() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:show",
            "Memory show should fetch one row by id.",
            "agent:primary",
        );
        candidate.source = Some(ChatMemorySource {
            conversation_id: "conversation:show".to_string(),
            message_id: Some("message:show_1".to_string()),
            evidence_excerpt: None,
        });

        let created = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append");

        let fetched = repo
            .get_memory(&created.id)
            .expect("get memory")
            .expect("created memory");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.content, "Memory show should fetch one row by id.");
        assert_eq!(
            fetched.conversation_id.as_deref(),
            Some("conversation:show")
        );

        let missing = repo.get_memory("mem_missing").expect("missing lookup");
        assert_eq!(missing, None);
    }

    fn request_for_kevin() -> MemoryRetrievalRequest {
        MemoryRetrievalRequest {
            requesting_principal_id: "agent:primary".to_string(),
            trusted: TrustedRetrievalContext::for_human(
                "human:kevin",
                Purpose::AnswerHumanQuestion,
            ),
            untrusted_hints: UntrustedHints::default(),
        }
    }

    fn validate_sensitive_policy(repo: &mut SqliteMemoryRepository, memory_id: &str) {
        repo.conn
            .execute(
                r"
                UPDATE memory_items
                SET
                  participant_visibility_policy = 'owner_only'
                WHERE memory_id = ?1
                ",
                params![memory_id],
            )
            .expect("sensitive participant visibility");
        let fingerprint = repo
            .refresh_retrieval_policy_fingerprint(memory_id, "agent:primary", "test")
            .expect("valid sensitive policy");
        assert!(fingerprint.starts_with("sha256:"));
    }

    fn validate_private_policy(repo: &mut SqliteMemoryRepository, memory_id: &str) {
        repo.conn
            .execute(
                r"
                UPDATE memory_items
                SET
                  participant_visibility_policy = 'explicit_grant_only'
                WHERE memory_id = ?1
                ",
                params![memory_id],
            )
            .expect("private participant visibility");
        let fingerprint = repo
            .refresh_retrieval_policy_fingerprint(memory_id, "agent:primary", "test")
            .expect("valid private policy");
        assert!(fingerprint.starts_with("sha256:"));
    }

    fn included_ids(result: &MemoryRetrievalResult) -> Vec<&str> {
        result
            .included
            .iter()
            .map(|memory| memory.memory_id.as_str())
            .collect()
    }
}
