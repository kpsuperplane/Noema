use serde_json::{Value, json};

use super::{
    MemoryPersistenceError, MemorySummary, NewMemoryCandidate, NewMemorySubject, ObjectRef,
    ObjectType, PostgresMemoryRepository,
    helpers::{
        memory_status_to_db, participant_role_to_db, sensitivity_to_db, subject_role_to_db,
        title_from_content,
    },
    objects::validate_object_ref_for_pool,
    postgres_helpers::{allocate_id as allocate_postgres_id, json_value},
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

impl PostgresMemoryRepository {
    /// Append an object-owned memory with optional concrete provenance.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when referenced objects are missing,
    /// generated IDs cannot be allocated, or Postgres writes fail.
    pub async fn append_memory_candidate(
        &self,
        candidate: NewMemoryCandidate,
    ) -> Result<MemorySummary, MemoryPersistenceError> {
        validate_postgres_memory_candidate_refs(self.pool(), &candidate).await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        let memory_id = allocate_postgres_id(&mut *tx, "mem").await?;
        let event_id = allocate_postgres_id(&mut *tx, "evt").await?;
        let title = candidate
            .title
            .clone()
            .unwrap_or_else(|| title_from_content(&candidate.content));

        let created_at = sqlx::query_scalar::<_, String>(
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
              $1, $2, $3, $4, $5, $6, $7, $8,
              $9, $10, $11, $12, $13, $14, $15::timestamptz, $16
            )
            RETURNING created_at::text
            ",
        )
        .bind(memory_id.as_str())
        .bind(candidate.owner.object_type.as_str())
        .bind(candidate.owner.object_id.as_str())
        .bind(candidate.memory_type.as_str())
        .bind(title.as_str())
        .bind(candidate.content.as_str())
        .bind(json_value(candidate.retrieval_hints.clone()))
        .bind(memory_status_to_db(candidate.status))
        .bind(candidate.confidence)
        .bind(sensitivity_to_db(candidate.sensitivity))
        .bind(candidate.created_by.object_type.as_str())
        .bind(candidate.created_by.object_id.as_str())
        .bind(candidate.authority_level.as_str())
        .bind(candidate.extraction_method.as_str())
        .bind(candidate.observed_at.as_deref())
        .bind(json_value(candidate.metadata.clone()))
        .fetch_one(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        for participant in &candidate.participants {
            sqlx::query(
                r"
                INSERT INTO memory_participants (
                  memory_id,
                  participant_object_type,
                  participant_object_id,
                  role,
                  metadata
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (memory_id, participant_object_type, participant_object_id, role)
                DO NOTHING
                ",
            )
            .bind(memory_id.as_str())
            .bind(participant.participant.object_type.as_str())
            .bind(participant.participant.object_id.as_str())
            .bind(participant_role_to_db(participant.role))
            .bind(json_value(participant.metadata.clone()))
            .execute(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        }

        for subject in &candidate.subjects {
            upsert_postgres_object_subject_entity(&mut tx, &candidate.owner, subject).await?;
            sqlx::query(
                r"
                INSERT INTO memory_subjects (memory_id, entity_id, role)
                VALUES ($1, $2, $3)
                ON CONFLICT (memory_id, entity_id, role) DO NOTHING
                ",
            )
            .bind(memory_id.as_str())
            .bind(subject.entity_id.as_str())
            .bind(subject_role_to_db(subject.role))
            .execute(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        }

        let (source_object_type, source_object_id) = if let Some(source) = &candidate.source {
            let target = ObjectRef::new(ObjectType::MemoryItem, memory_id.clone())?;
            insert_postgres_object_provenance_edge_tx(
                &mut tx,
                &NewObjectProvenanceEdge {
                    target,
                    source: source.source.clone(),
                    relation: "derived_from".to_string(),
                    evidence_excerpt: source.evidence_excerpt.clone(),
                    created_by: candidate.created_by.clone(),
                    metadata: json!({}),
                },
            )
            .await?;
            (
                Some(source.source.object_type.as_str().to_string()),
                Some(source.source.object_id.clone()),
            )
        } else {
            (None, None)
        };

        sqlx::query(
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
            VALUES ($1, 'memory_created', $2, $3, 'memory_item', $4, $5, $6)
            ",
        )
        .bind(event_id.as_str())
        .bind(candidate.created_by.object_type.as_str())
        .bind(candidate.created_by.object_id.as_str())
        .bind(memory_id.as_str())
        .bind("memory_candidate")
        .bind(json_value(json!({
            "owner": {
                "object_type": candidate.owner.object_type.as_str(),
                "object_id": candidate.owner.object_id,
            },
            "source": candidate.source.as_ref().map(|source| json!({
                "object_type": source.source.object_type.as_str(),
                "object_id": source.source.object_id,
            })),
        })))
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        let conversation_id = postgres_conversation_id_for_summary(&mut tx, &candidate).await?;
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

        tx.commit()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        Ok(summary)
    }

    /// Add a typed provenance edge between two concrete objects.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when any referenced object is
    /// missing or Postgres writes fail.
    pub async fn add_object_provenance_edge(
        &self,
        edge: NewObjectProvenanceEdge,
    ) -> Result<String, MemoryPersistenceError> {
        validate_object_ref_for_pool(self.pool(), &edge.target).await?;
        validate_object_ref_for_pool(self.pool(), &edge.source).await?;
        validate_object_ref_for_pool(self.pool(), &edge.created_by).await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        let edge_id = insert_postgres_object_provenance_edge_tx(&mut tx, &edge).await?;
        tx.commit()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        Ok(edge_id)
    }

    /// Soft-delete a conversation item and cascade redaction to sole-source memories.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the item or deleting actor is
    /// missing, or Postgres writes fail.
    pub async fn soft_delete_conversation_item(
        &self,
        deletion: DeleteConversationItem,
    ) -> Result<(), MemoryPersistenceError> {
        validate_object_ref_for_pool(
            self.pool(),
            &ObjectRef::conversation_item(deletion.item_id.as_str()),
        )
        .await?;
        validate_object_ref_for_pool(self.pool(), &deletion.deleted_by).await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;

        let affected_memory_ids = sqlx::query_scalar::<_, String>(
            r"
            SELECT DISTINCT target_object_id
            FROM object_provenance_edges
            WHERE target_object_type = 'memory_item'
              AND source_object_type = 'conversation_item'
              AND source_object_id = $1
              AND deleted_at IS NULL
            ",
        )
        .bind(deletion.item_id.as_str())
        .fetch_all(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        sqlx::query(
            r"
            UPDATE conversation_items
            SET deleted_at = now(),
                deleted_by_object_type = $2,
                deleted_by_object_id = $3,
                redacted_at = now(),
                redaction_reason = $4,
                content_text = '[redacted]',
                payload_json = '{}'::jsonb,
                updated_at = now()
            WHERE item_id = $1
            ",
        )
        .bind(deletion.item_id.as_str())
        .bind(deletion.deleted_by.object_type.as_str())
        .bind(deletion.deleted_by.object_id.as_str())
        .bind(deletion.reason.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        sqlx::query(
            r"
            UPDATE object_provenance_edges
            SET deleted_at = now(),
                evidence_excerpt = '[redacted]'
            WHERE source_object_type = 'conversation_item'
              AND source_object_id = $1
              AND deleted_at IS NULL
            ",
        )
        .bind(deletion.item_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        for memory_id in affected_memory_ids {
            let remaining_count = sqlx::query_scalar::<_, i64>(
                r"
                SELECT COUNT(*)
                FROM object_provenance_edges
                WHERE target_object_type = 'memory_item'
                  AND target_object_id = $1
                  AND deleted_at IS NULL
                ",
            )
            .bind(memory_id.as_str())
            .fetch_one(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?;

            if remaining_count == 0 {
                sqlx::query(
                    r"
                    UPDATE memory_items
                    SET status = 'deleted',
                        title = '[redacted]',
                        content = '[redacted]',
                        structured_value = '{}'::jsonb,
                        retrieval_hints = '{}'::jsonb,
                        updated_at = now(),
                        deleted_at = now(),
                        redacted_at = now()
                    WHERE memory_id = $1
                    ",
                )
                .bind(memory_id.as_str())
                .execute(&mut *tx)
                .await
                .map_err(MemoryPersistenceError::Database)?;
            }
        }

        tx.commit().await.map_err(MemoryPersistenceError::Database)
    }
}

async fn validate_postgres_memory_candidate_refs(
    pool: &sqlx::PgPool,
    candidate: &NewMemoryCandidate,
) -> Result<(), MemoryPersistenceError> {
    validate_object_ref_for_pool(pool, &candidate.owner).await?;
    validate_object_ref_for_pool(pool, &candidate.created_by).await?;
    if let Some(owner_actor) = &candidate.owner_actor {
        validate_object_ref_for_pool(pool, owner_actor).await?;
    }
    if let Some(source) = &candidate.source {
        validate_object_ref_for_pool(pool, &source.source).await?;
    }
    for participant in &candidate.participants {
        validate_object_ref_for_pool(pool, &participant.participant).await?;
    }
    for subject in &candidate.subjects {
        if let Some(linked_object) = &subject.linked_object {
            validate_object_ref_for_pool(pool, linked_object).await?;
        }
    }
    Ok(())
}

async fn insert_postgres_object_provenance_edge_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    edge: &NewObjectProvenanceEdge,
) -> Result<String, MemoryPersistenceError> {
    let edge_id = allocate_postgres_id(&mut **tx, "edge").await?;
    sqlx::query(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ",
    )
    .bind(edge_id.as_str())
    .bind(edge.target.object_type.as_str())
    .bind(edge.target.object_id.as_str())
    .bind(edge.source.object_type.as_str())
    .bind(edge.source.object_id.as_str())
    .bind(edge.relation.as_str())
    .bind(edge.evidence_excerpt.as_deref())
    .bind(edge.created_by.object_type.as_str())
    .bind(edge.created_by.object_id.as_str())
    .bind(json_value(edge.metadata.clone()))
    .execute(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;
    Ok(edge_id)
}

async fn upsert_postgres_object_subject_entity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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

    sqlx::query(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (entity_id) DO UPDATE SET
          owner_object_type = excluded.owner_object_type,
          owner_object_id = excluded.owner_object_id,
          entity_type = excluded.entity_type,
          canonical_name = excluded.canonical_name,
          aliases = CASE
            WHEN excluded.aliases = '[]'::jsonb THEN entities.aliases
            ELSE excluded.aliases
          END,
          linked_object_type = COALESCE(excluded.linked_object_type, entities.linked_object_type),
          linked_object_id = COALESCE(excluded.linked_object_id, entities.linked_object_id),
          metadata = CASE
            WHEN excluded.metadata = '{}'::jsonb THEN entities.metadata
            ELSE excluded.metadata
          END,
          updated_at = now()
        ",
    )
    .bind(subject.entity_id.as_str())
    .bind(owner.object_type.as_str())
    .bind(owner.object_id.as_str())
    .bind(subject.entity_type.as_str())
    .bind(subject.canonical_name.as_str())
    .bind(json_value(json!(subject.aliases)))
    .bind(linked_object_type.as_deref())
    .bind(linked_object_id.as_deref())
    .bind(json_value(subject.metadata.clone()))
    .execute(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;
    Ok(())
}

async fn postgres_conversation_id_for_summary(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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

    sqlx::query_scalar::<_, String>(
        "SELECT conversation_id FROM conversation_items WHERE item_id = $1",
    )
    .bind(source.source.object_id.as_str())
    .fetch_optional(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)
}
