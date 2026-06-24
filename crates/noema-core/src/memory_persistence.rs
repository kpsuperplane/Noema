//! SQLite-backed durable memory repository.
//!
//! This module is the small persistence slice used by chat integration. It
//! initializes `db/noema.sqlite`, creates the canonical memory tables needed
//! for chat-created memories, writes provenance and participants, and exposes
//! a recent-memory listing for CLI inspection.

use crate::{
    memory::{
        MemoryId, MemoryStatus, ParticipantRole, PrincipalId, ScopeId, Sensitivity, SubjectRole,
    },
    paths::NoemaPaths,
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
    ensure_memory_items_column(
        conn,
        "retrieval_hints",
        "retrieval_hints TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(retrieval_hints))",
    )?;
    ensure_memory_items_column(
        conn,
        "confidence",
        "confidence REAL CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0))",
    )?;
    conn.execute(
        "DELETE FROM schema_migrations WHERE version = 1 AND name = 'memory_persistence_v1'",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
        params![BOOTSTRAP_SCHEMA_VERSION, BOOTSTRAP_SCHEMA_NAME],
    )?;
    Ok(())
}

fn ensure_memory_items_column(
    conn: &Connection,
    column_name: &str,
    column_definition: &str,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(memory_items)")
        .map_err(MemoryPersistenceError::Sqlite)?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(MemoryPersistenceError::Sqlite)?;

    for column in columns {
        if column.map_err(MemoryPersistenceError::Sqlite)? == column_name {
            return Ok(());
        }
    }

    conn.execute_batch(&format!(
        "ALTER TABLE memory_items ADD COLUMN {column_definition};"
    ))?;
    Ok(())
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

CREATE TABLE IF NOT EXISTS memory_participants (
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id) ON DELETE CASCADE,
  principal_id TEXT NOT NULL REFERENCES principals(principal_id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('human_in_scope','agent_in_scope','originator','observer')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
  PRIMARY KEY (memory_id, principal_id, role)
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

CREATE TABLE IF NOT EXISTS memory_events (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN ('created','promoted','edited','merged','archived','deleted','retrieved','shown_to_model','used_in_reply','used_for_action','exported','confirmed','disputed','superseded','restored')),
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
CREATE INDEX IF NOT EXISTS idx_entities_scope_type ON entities(home_scope_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_canonical_name ON entities(entity_type, canonical_name);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_entity_role ON memory_subjects(entity_id, role, memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_subjects_memory_role ON memory_subjects(memory_id, role);
CREATE INDEX IF NOT EXISTS idx_memory_participants_principal ON memory_participants(principal_id, role);
CREATE INDEX IF NOT EXISTS idx_memory_provenance_memory ON memory_provenance_edges(memory_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_events_memory_time ON memory_events(memory_id, created_at DESC);
";

#[cfg(test)]
mod tests {
    use super::*;
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
    fn open_removes_legacy_bootstrap_migration_row() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        std::fs::create_dir_all(db_path.parent().expect("parent")).expect("db dir");
        {
            let conn = Connection::open(&db_path).expect("raw conn");
            conn.execute_batch(
                r"
                CREATE TABLE schema_migrations (
                  version INTEGER PRIMARY KEY,
                  name TEXT NOT NULL,
                  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                ) STRICT;
                INSERT INTO schema_migrations (version, name)
                VALUES (1, 'memory_persistence_v1');
                ",
            )
            .expect("legacy migration row");
        }

        let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");

        let legacy_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 1 AND name = 'memory_persistence_v1'",
                [],
                |row| row.get(0),
            )
            .expect("legacy count");
        assert_eq!(legacy_count, 0);

        let bootstrap_count: i64 = repo
            .conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1 AND name = ?2",
                params![BOOTSTRAP_SCHEMA_VERSION, BOOTSTRAP_SCHEMA_NAME],
                |row| row.get(0),
            )
            .expect("bootstrap count");
        assert_eq!(bootstrap_count, 1);
    }

    #[test]
    fn open_migrates_old_memory_items_columns_for_extraction_metadata() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("db").join("noema.sqlite");
        std::fs::create_dir_all(db_path.parent().expect("parent")).expect("db dir");
        {
            let conn = Connection::open(&db_path).expect("raw conn");
            conn.execute_batch(
                r"
                CREATE TABLE memory_items (
                  memory_id TEXT PRIMARY KEY,
                  home_scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
                  memory_type TEXT NOT NULL CHECK (memory_type IN ('fact','preference','person','organization','project','place','routine','goal','open_loop','procedure','constraint','trigger','decision','skill','policy','note','other')),
                  title TEXT NOT NULL,
                  content TEXT NOT NULL,
                  structured_value TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(structured_value)),
                  status TEXT NOT NULL DEFAULT 'candidate' CHECK (status IN ('candidate','active','confirmed','inferred','stale','superseded','archived','deleted','disputed')),
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
                ",
            )
            .expect("old memory_items table");
        }

        let mut repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
        let mut candidate = NewChatMemoryCandidate::new(
            "conversation:old_schema",
            "Kevin prefers old-schema migrations to be automatic.",
            "agent:primary",
        );
        candidate.confidence = Some(0.88);
        candidate.retrieval_hints = json!({"topics": ["migrations"]});

        let summary = repo
            .append_chat_memory_candidate(&candidate)
            .expect("append after migration");

        let (confidence, retrieval_hints): (Option<f64>, String) = repo
            .conn
            .query_row(
                "SELECT confidence, retrieval_hints FROM memory_items WHERE memory_id = ?1",
                params![summary.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("memory row");
        assert_eq!(confidence, Some(0.88));
        assert_eq!(
            serde_json::from_str::<Value>(&retrieval_hints).expect("retrieval hints"),
            json!({"topics": ["migrations"]})
        );
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
}
