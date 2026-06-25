use rusqlite::{OptionalExtension, Transaction, named_params, params};
use serde_json::{Value, json};

use super::{
    MemoryPersistenceError, MemorySummary, NewMemoryCandidate, NewMemorySubject, ObjectRef,
    ObjectType, SqliteMemoryRepository,
    helpers::{
        allocate_id, json_to_string, memory_status_to_db, participant_role_to_db,
        sensitivity_to_db, subject_role_to_db, title_from_content, upsert_memory_fts,
    },
    objects::validate_object_ref_for_conn,
};

/// New typed provenance edge between concrete objects.
#[derive(Debug, Clone, PartialEq)]
pub struct NewObjectProvenanceEdge {
    /// Object that is supported by the source.
    pub target: ObjectRef,
    /// Concrete evidence/source object.
    pub source: ObjectRef,
    /// Provenance relation.
    pub relation: String,
    /// Short supporting excerpt to show during inspection, if available.
    pub evidence_excerpt: Option<String>,
    /// Object that created the edge.
    pub created_by: ObjectRef,
    /// Additional structured metadata.
    pub metadata: Value,
}

/// Request to soft-delete and redact one conversation item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConversationItem {
    /// Conversation item to delete.
    pub item_id: String,
    /// Object requesting the deletion.
    pub deleted_by: ObjectRef,
    /// Optional human-readable deletion/redaction reason.
    pub reason: Option<String>,
}

impl SqliteMemoryRepository {
    /// Append an object-owned memory with optional concrete provenance.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when referenced objects are missing,
    /// input JSON cannot be serialized, generated IDs cannot be allocated, or
    /// SQLite writes fail.
    pub fn append_memory_candidate(
        &mut self,
        candidate: &NewMemoryCandidate,
    ) -> Result<MemorySummary, MemoryPersistenceError> {
        validate_object_ref_for_conn(&self.conn, &candidate.owner)?;
        validate_object_ref_for_conn(&self.conn, &candidate.created_by)?;
        if let Some(owner_actor) = &candidate.owner_actor {
            validate_object_ref_for_conn(&self.conn, owner_actor)?;
        }
        if let Some(source) = &candidate.source {
            validate_object_ref_for_conn(&self.conn, &source.source)?;
        }
        for participant in &candidate.participants {
            validate_object_ref_for_conn(&self.conn, &participant.participant)?;
        }
        for subject in &candidate.subjects {
            if let Some(linked_object) = &subject.linked_object {
                validate_object_ref_for_conn(&self.conn, linked_object)?;
            }
        }

        let metadata = json_to_string(&candidate.metadata)?;
        let retrieval_hints = json_to_string(&candidate.retrieval_hints)?;
        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        let memory_id = allocate_id(&tx, "mem")?;
        let event_id = allocate_id(&tx, "evt")?;
        let title = candidate
            .title
            .clone()
            .unwrap_or_else(|| title_from_content(&candidate.content));
        let observed_at = candidate.observed_at.as_deref();

        tx.execute(
            r"
            INSERT INTO memory_items (
              memory_id,
              owner_object_type,
              owner_object_id,
              memory_type,
              title,
              content,
              retrieval_hints,
              status,
              confidence,
              sensitivity,
              created_by_object_type,
              created_by_object_id,
              authority_level,
              extraction_method,
              observed_at,
              metadata
            )
            VALUES (
              :memory_id,
              :owner_object_type,
              :owner_object_id,
              :memory_type,
              :title,
              :content,
              :retrieval_hints,
              :status,
              :confidence,
              :sensitivity,
              :created_by_object_type,
              :created_by_object_id,
              :authority_level,
              :extraction_method,
              :observed_at,
              :metadata
            )
            ",
            named_params! {
                ":memory_id": memory_id.as_str(),
                ":owner_object_type": candidate.owner.object_type.as_str(),
                ":owner_object_id": candidate.owner.object_id.as_str(),
                ":memory_type": candidate.memory_type.as_str(),
                ":title": title.as_str(),
                ":content": candidate.content.as_str(),
                ":retrieval_hints": retrieval_hints.as_str(),
                ":status": memory_status_to_db(candidate.status),
                ":confidence": candidate.confidence,
                ":sensitivity": sensitivity_to_db(candidate.sensitivity),
                ":created_by_object_type": candidate.created_by.object_type.as_str(),
                ":created_by_object_id": candidate.created_by.object_id.as_str(),
                ":authority_level": candidate.authority_level.as_str(),
                ":extraction_method": candidate.extraction_method.as_str(),
                ":observed_at": observed_at,
                ":metadata": metadata.as_str(),
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
                  participant_object_type,
                  participant_object_id,
                  role,
                  metadata
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    memory_id.as_str(),
                    participant.participant.object_type.as_str(),
                    participant.participant.object_id.as_str(),
                    participant_role_to_db(participant.role),
                    participant_metadata.as_str(),
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        for subject in &candidate.subjects {
            upsert_object_subject_entity(&tx, &candidate.owner, subject)?;
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
                    memory_id.as_str(),
                    subject.entity_id.as_str(),
                    subject_role_to_db(subject.role)
                ],
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        }

        let (source_object_type, source_object_id) = if let Some(source) = &candidate.source {
            let target = ObjectRef::new(ObjectType::MemoryItem, memory_id.clone())?;
            insert_object_provenance_edge_tx(
                &tx,
                &NewObjectProvenanceEdge {
                    target,
                    source: source.source.clone(),
                    relation: "derived_from".to_string(),
                    evidence_excerpt: source.evidence_excerpt.clone(),
                    created_by: candidate.created_by.clone(),
                    metadata: json!({}),
                },
            )?;
            (
                Some(source.source.object_type.as_str().to_string()),
                Some(source.source.object_id.clone()),
            )
        } else {
            (None, None)
        };

        tx.execute(
            r"
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_object_type,
              actor_object_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES (?1, 'memory_created', ?2, ?3, 'memory_item', ?4, ?5, ?6)
            ",
            params![
                event_id.as_str(),
                candidate.created_by.object_type.as_str(),
                candidate.created_by.object_id.as_str(),
                memory_id.as_str(),
                "memory_candidate",
                json_to_string(&json!({
                    "owner": {
                        "object_type": candidate.owner.object_type.as_str(),
                        "object_id": candidate.owner.object_id,
                    },
                    "source": candidate.source.as_ref().map(|source| json!({
                        "object_type": source.source.object_type.as_str(),
                        "object_id": source.source.object_id,
                    })),
                }))?,
            ],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;

        let created_at = tx
            .query_row(
                "SELECT created_at FROM memory_items WHERE memory_id = ?1",
                params![memory_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .map_err(MemoryPersistenceError::Sqlite)?;
        let conversation_id = conversation_id_for_summary(&tx, candidate)?;
        let summary = MemorySummary {
            id: memory_id,
            status: candidate.status,
            memory_type: candidate.memory_type,
            owner_object_type: candidate.owner.object_type.as_str().to_string(),
            owner_object_id: candidate.owner.object_id.clone(),
            home_scope_id: format!(
                "{}:{}",
                candidate.owner.object_type.as_str(),
                candidate.owner.object_id
            ),
            sensitivity: candidate.sensitivity,
            title,
            content: candidate.content.clone(),
            created_at,
            source_object_type: source_object_type.clone(),
            source_object_id: source_object_id.clone(),
            source_type: source_object_type,
            source_id: source_object_id,
            conversation_id,
        };

        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;
        Ok(summary)
    }

    /// Add a typed provenance edge between two concrete objects.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when any referenced object is
    /// missing, metadata cannot serialize, or SQLite writes fail.
    pub fn add_object_provenance_edge(
        &mut self,
        edge: NewObjectProvenanceEdge,
    ) -> Result<String, MemoryPersistenceError> {
        validate_object_ref_for_conn(&self.conn, &edge.target)?;
        validate_object_ref_for_conn(&self.conn, &edge.source)?;
        validate_object_ref_for_conn(&self.conn, &edge.created_by)?;

        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        let edge_id = insert_object_provenance_edge_tx(&tx, &edge)?;
        tx.commit().map_err(MemoryPersistenceError::Sqlite)?;
        Ok(edge_id)
    }

    /// Soft-delete a conversation item and cascade redaction to sole-source memories.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the item or deleting actor is
    /// missing, or SQLite writes fail.
    pub fn soft_delete_conversation_item(
        &mut self,
        delete: DeleteConversationItem,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_conn(
            &self.conn,
            &ObjectRef::conversation_item(delete.item_id.as_str()),
        )?;
        validate_object_ref_for_conn(&self.conn, &delete.deleted_by)?;

        let tx = self
            .conn
            .transaction()
            .map_err(MemoryPersistenceError::Sqlite)?;
        tx.execute(
            r"
            UPDATE conversation_items
            SET deleted_at = CURRENT_TIMESTAMP,
                deleted_by_object_type = ?2,
                deleted_by_object_id = ?3,
                redacted_at = CURRENT_TIMESTAMP,
                redaction_reason = ?4,
                content_text = '[redacted]',
                payload_json = '{}',
                updated_at = CURRENT_TIMESTAMP
            WHERE item_id = ?1
            ",
            params![
                delete.item_id.as_str(),
                delete.deleted_by.object_type.as_str(),
                delete.deleted_by.object_id.as_str(),
                delete.reason.as_deref(),
            ],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;

        let affected_memory_ids = {
            let mut stmt = tx.prepare(
                r"
                SELECT DISTINCT target_object_id
                FROM object_provenance_edges
                WHERE target_object_type = 'memory_item'
                  AND source_object_type = 'conversation_item'
                  AND source_object_id = ?1
                  AND deleted_at IS NULL
                ",
            )?;
            let rows = stmt.query_map(params![delete.item_id.as_str()], |row| row.get(0))?;
            let mut memory_ids: Vec<String> = Vec::new();
            for row in rows {
                memory_ids.push(row?);
            }
            memory_ids
        };

        tx.execute(
            r"
            UPDATE object_provenance_edges
            SET deleted_at = CURRENT_TIMESTAMP,
                evidence_excerpt = '[redacted]'
            WHERE source_object_type = 'conversation_item'
              AND source_object_id = ?1
              AND deleted_at IS NULL
            ",
            params![delete.item_id.as_str()],
        )
        .map_err(MemoryPersistenceError::Sqlite)?;

        for memory_id in affected_memory_ids {
            let remaining_count: i64 = tx
                .query_row(
                    r"
                    SELECT COUNT(*)
                    FROM object_provenance_edges
                    WHERE target_object_type = 'memory_item'
                      AND target_object_id = ?1
                      AND deleted_at IS NULL
                    ",
                    params![memory_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(MemoryPersistenceError::Sqlite)?;
            if remaining_count == 0 {
                tx.execute(
                    r"
                    UPDATE memory_items
                    SET status = 'deleted',
                        title = '[redacted]',
                        content = '[redacted]',
                        structured_value = '{}',
                        retrieval_hints = '{}',
                        updated_at = CURRENT_TIMESTAMP,
                        deleted_at = CURRENT_TIMESTAMP,
                        redacted_at = CURRENT_TIMESTAMP
                    WHERE memory_id = ?1
                    ",
                    params![memory_id.as_str()],
                )
                .map_err(MemoryPersistenceError::Sqlite)?;
                tx.execute(
                    "DELETE FROM memory_fts WHERE memory_id = ?1",
                    params![memory_id.as_str()],
                )
                .map_err(MemoryPersistenceError::Sqlite)?;
            }
        }

        tx.commit().map_err(MemoryPersistenceError::Sqlite)
    }
}

pub(super) fn insert_object_provenance_edge_tx(
    tx: &Transaction<'_>,
    edge: &NewObjectProvenanceEdge,
) -> Result<String, MemoryPersistenceError> {
    let edge_id = allocate_id(tx, "edge")?;
    tx.execute(
        r"
        INSERT INTO object_provenance_edges (
          edge_id,
          target_object_type,
          target_object_id,
          source_object_type,
          source_object_id,
          relation,
          evidence_excerpt,
          created_by_object_type,
          created_by_object_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ",
        params![
            edge_id.as_str(),
            edge.target.object_type.as_str(),
            edge.target.object_id.as_str(),
            edge.source.object_type.as_str(),
            edge.source.object_id.as_str(),
            edge.relation.as_str(),
            edge.evidence_excerpt.as_deref(),
            edge.created_by.object_type.as_str(),
            edge.created_by.object_id.as_str(),
            json_to_string(&edge.metadata)?,
        ],
    )
    .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(edge_id)
}

fn upsert_object_subject_entity(
    tx: &Transaction<'_>,
    owner: &ObjectRef,
    subject: &NewMemorySubject,
) -> Result<(), MemoryPersistenceError> {
    let linked_object_type = subject
        .linked_object
        .as_ref()
        .map(|object_ref| object_ref.object_type.as_str().to_string());
    let linked_object_id = subject
        .linked_object
        .as_ref()
        .map(|object_ref| object_ref.object_id.clone());

    tx.execute(
        r"
        INSERT INTO entities (
          entity_id,
          owner_object_type,
          owner_object_id,
          entity_type,
          canonical_name,
          aliases,
          linked_object_type,
          linked_object_id,
          metadata
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(entity_id) DO UPDATE SET
          owner_object_type = excluded.owner_object_type,
          owner_object_id = excluded.owner_object_id,
          entity_type = excluded.entity_type,
          canonical_name = excluded.canonical_name,
          aliases = CASE
            WHEN excluded.aliases = '[]' THEN entities.aliases
            ELSE excluded.aliases
          END,
          linked_object_type = COALESCE(excluded.linked_object_type, entities.linked_object_type),
          linked_object_id = COALESCE(excluded.linked_object_id, entities.linked_object_id),
          metadata = CASE
            WHEN excluded.metadata = '{}' THEN entities.metadata
            ELSE excluded.metadata
          END,
          updated_at = CURRENT_TIMESTAMP
        ",
        params![
            subject.entity_id.as_str(),
            owner.object_type.as_str(),
            owner.object_id.as_str(),
            subject.entity_type.as_str(),
            subject.canonical_name.as_str(),
            json_to_string(&json!(subject.aliases))?,
            linked_object_type.as_deref(),
            linked_object_id.as_deref(),
            json_to_string(&subject.metadata)?,
        ],
    )
    .map_err(MemoryPersistenceError::Sqlite)?;
    Ok(())
}

fn conversation_id_for_summary(
    tx: &Transaction<'_>,
    candidate: &NewMemoryCandidate,
) -> Result<Option<String>, MemoryPersistenceError> {
    if candidate.owner.object_type == ObjectType::Conversation {
        return Ok(Some(candidate.owner.object_id.clone()));
    }

    let Some(source) = &candidate.source else {
        return Ok(None);
    };
    if source.source.object_type != ObjectType::ConversationItem {
        return Ok(None);
    }

    tx.query_row(
        "SELECT conversation_id FROM conversation_items WHERE item_id = ?1",
        params![source.source.object_id.as_str()],
        |row| row.get(0),
    )
    .optional()
    .map_err(MemoryPersistenceError::Sqlite)
}
