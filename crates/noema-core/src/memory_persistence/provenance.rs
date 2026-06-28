use crate::memory::{MemoryStatus, Sensitivity};
use serde_json::{Value, json};

use super::{
    ActorRef, MemoryPersistenceError, MemorySummary, NewMemoryCandidate, NewMemorySubject,
    ObjectRef, ObjectType, PostgresMemoryRepository,
    helpers::{
        memory_status_to_db, parse_memory_status, parse_sensitivity, participant_role_to_db,
        sensitivity_to_db, subject_role_to_db, title_from_content,
    },
    memory_candidate_dedupe_fingerprint,
    objects::{validate_actor_ref_for_pool, validate_object_ref_for_pool},
    postgres_helpers::{allocate_id as allocate_postgres_id, json_value},
    queries::{
        POSTGRES_MEMORY_SUMMARY_BY_DEDUPE_FINGERPRINT_SQL, POSTGRES_MEMORY_SUMMARY_BY_ID_SQL,
    },
    repository::{MemorySummaryRow, postgres_row_to_memory_summary},
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
    /// Actor that created the edge.
    pub created_by: ActorRef,
    /// Additional structured metadata.
    pub metadata: Value,
}

/// Request to soft-delete and redact one conversation item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConversationItem {
    /// Conversation item to delete.
    pub item_id: String,
    /// Actor requesting the deletion.
    pub deleted_by: ActorRef,
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
        let dedupe_fingerprint = memory_candidate_dedupe_fingerprint(&candidate);

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;

        if let Some(existing) = reuse_memory_candidate_for_dedupe_fingerprint_tx(
            &mut tx,
            &candidate,
            &dedupe_fingerprint,
        )
        .await?
        {
            tx.commit()
                .await
                .map_err(MemoryPersistenceError::Database)?;
            return Ok(existing);
        }

        let memory_id = allocate_postgres_id(&mut *tx, "mem").await?;
        let event_id = allocate_postgres_id(&mut *tx, "evt").await?;
        let title = candidate
            .title
            .clone()
            .unwrap_or_else(|| title_from_content(&candidate.content));

        let created_at_result = sqlx::query_scalar::<_, String>(
            r"
            INSERT INTO memory_items (
              memory_id,
              owner_object_type,
              owner_object_id,
              memory_type,
              title,
              content,
              memory_dedupe_fingerprint,
              retrieval_hints,
              status,
              confidence,
              sensitivity,
              created_by_actor_id,
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
        .bind(dedupe_fingerprint.as_str())
        .bind(json_value(candidate.retrieval_hints.clone()))
        .bind(memory_status_to_db(candidate.status))
        .bind(candidate.confidence)
        .bind(sensitivity_to_db(candidate.sensitivity))
        .bind(candidate.created_by.actor_id.as_str())
        .bind(candidate.authority_level.as_str())
        .bind(candidate.extraction_method.as_str())
        .bind(candidate.observed_at.as_deref())
        .bind(json_value(candidate.metadata.clone()))
        .fetch_one(&mut *tx)
        .await;
        let created_at = match created_at_result {
            Ok(created_at) => created_at,
            Err(error) if is_memory_dedupe_unique_violation(&error) => {
                tx.rollback()
                    .await
                    .map_err(MemoryPersistenceError::Database)?;
                if let Some(existing) = self
                    .reuse_memory_candidate_after_dedupe_conflict(
                        &candidate,
                        dedupe_fingerprint.as_str(),
                    )
                    .await?
                {
                    return Ok(existing);
                }
                return Err(MemoryPersistenceError::Database(error));
            }
            Err(error) => return Err(MemoryPersistenceError::Database(error)),
        };

        for participant in &candidate.participants {
            sqlx::query(
                r"
                INSERT INTO memory_participants (
                  memory_id,
                  participant_actor_id,
                  role,
                  metadata
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (memory_id, participant_actor_id, role)
                DO NOTHING
                ",
            )
            .bind(memory_id.as_str())
            .bind(participant.participant.actor_id.as_str())
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
                Some(source.source.object_id.to_string()),
            )
        } else {
            (None, None)
        };

        sqlx::query(
            r"
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES ($1, 'memory_created', $2, 'memory_item', $3, $4, $5)
            ",
        )
        .bind(event_id.as_str())
        .bind(candidate.created_by.actor_id.as_str())
        .bind(memory_id.as_str())
        .bind("memory_candidate")
        .bind(json_value(json!({
            "owner": {
                "object_type": candidate.owner.object_type.as_str(),
                "object_id": candidate.owner.object_id.as_str(),
            },
            "source": candidate.source.as_ref().map(|source| json!({
                "object_type": source.source.object_type.as_str(),
                "object_id": source.source.object_id.as_str(),
            })),
            "dedupe_fingerprint": dedupe_fingerprint,
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
            owner_object_id: candidate.owner.object_id.to_string(),
            home_scope_id: format!(
                "{}:{}",
                candidate.owner.object_type.as_str(),
                candidate.owner.object_id.as_str()
            ),
            sensitivity: candidate.sensitivity,
            title,
            content: candidate.content.clone(),
            subject_entity_ids: candidate
                .subjects
                .iter()
                .map(|subject| subject.entity_id.clone())
                .collect(),
            dedupe_fingerprint: Some(dedupe_fingerprint),
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

    async fn reuse_memory_candidate_after_dedupe_conflict(
        &self,
        candidate: &NewMemoryCandidate,
        dedupe_fingerprint: &str,
    ) -> Result<Option<MemorySummary>, MemoryPersistenceError> {
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        let existing = reuse_memory_candidate_for_dedupe_fingerprint_tx(
            &mut tx,
            candidate,
            dedupe_fingerprint,
        )
        .await?;
        tx.commit()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        Ok(existing)
    }

    /// Reinforce an existing memory with candidate provenance.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] when the memory or referenced
    /// candidate objects are missing, or Postgres writes fail.
    pub async fn reinforce_memory_with_candidate(
        &self,
        memory_id: &str,
        candidate: &NewMemoryCandidate,
        reason: &str,
    ) -> Result<MemorySummary, MemoryPersistenceError> {
        validate_postgres_memory_candidate_refs(self.pool(), candidate).await?;
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;

        let Some(locked_target) = locked_memory_reinforcement_target_tx(&mut tx, memory_id).await?
        else {
            return Err(MemoryPersistenceError::MemoryNotFound {
                memory_id: memory_id.to_string(),
            });
        };
        validate_memory_reinforcement_target(&locked_target, candidate)?;
        let Some(existing) = existing_memory_summary_by_id_tx(&mut tx, memory_id).await? else {
            return Err(MemoryPersistenceError::MemoryNotFound {
                memory_id: memory_id.to_string(),
            });
        };

        if let Some(source) = &candidate.source {
            let target = ObjectRef::new(ObjectType::MemoryItem, memory_id.to_string())?;
            insert_postgres_object_provenance_edge_tx(
                &mut tx,
                &NewObjectProvenanceEdge {
                    target,
                    source: source.source.clone(),
                    relation: "supports".to_string(),
                    evidence_excerpt: source.evidence_excerpt.clone(),
                    created_by: candidate.created_by.clone(),
                    metadata: json!({"reason": reason}),
                },
            )
            .await?;
        }

        let event_id = allocate_postgres_id(&mut *tx, "evt").await?;
        sqlx::query(
            r"
            INSERT INTO object_events (
              event_id,
              event_type,
              actor_id,
              target_object_type,
              target_object_id,
              reason,
              details
            )
            VALUES ($1, 'memory_reinforced', $2, 'memory_item', $3, $4, $5)
            ",
        )
        .bind(event_id.as_str())
        .bind(candidate.created_by.actor_id.as_str())
        .bind(memory_id)
        .bind(reason)
        .bind(json_value(json!({"candidate_content": candidate.content})))
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        tx.commit()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        Ok(existing)
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
        validate_actor_ref_for_pool(self.pool(), &edge.created_by).await?;

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
        validate_actor_ref_for_pool(self.pool(), &deletion.deleted_by).await?;

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
                deleted_by_actor_id = $2,
                redacted_at = now(),
                redaction_reason = $3,
                content_text = '[redacted]',
                payload_json = '{}'::jsonb,
                updated_at = now()
            WHERE item_id = $1
            ",
        )
        .bind(deletion.item_id.as_str())
        .bind(deletion.deleted_by.actor_id.as_str())
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
    validate_actor_ref_for_pool(pool, &candidate.created_by).await?;
    if let Some(owner_actor) = &candidate.owner_actor {
        validate_actor_ref_for_pool(pool, owner_actor).await?;
    }
    if let Some(source) = &candidate.source {
        validate_object_ref_for_pool(pool, &source.source).await?;
    }
    for participant in &candidate.participants {
        validate_actor_ref_for_pool(pool, &participant.participant).await?;
    }
    for subject in &candidate.subjects {
        if let Some(linked_object) = &subject.linked_object {
            validate_object_ref_for_pool(pool, linked_object).await?;
        }
    }
    Ok(())
}

async fn reuse_memory_candidate_for_dedupe_fingerprint_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    candidate: &NewMemoryCandidate,
    dedupe_fingerprint: &str,
) -> Result<Option<MemorySummary>, MemoryPersistenceError> {
    let Some(existing) =
        existing_memory_summary_for_dedupe_fingerprint_tx(tx, dedupe_fingerprint).await?
    else {
        return Ok(None);
    };

    if let Some(source) = &candidate.source {
        insert_postgres_object_provenance_edge_tx(
            tx,
            &NewObjectProvenanceEdge {
                target: ObjectRef::new(ObjectType::MemoryItem, existing.id.as_str())?,
                source: source.source.clone(),
                relation: "supports".to_string(),
                evidence_excerpt: source.evidence_excerpt.clone(),
                created_by: candidate.created_by.clone(),
                metadata: json!({
                    "reason": "exact_dedupe_fingerprint",
                    "dedupe_fingerprint": dedupe_fingerprint,
                }),
            },
        )
        .await?;
    }

    let event_id = allocate_postgres_id(&mut **tx, "evt").await?;
    sqlx::query(
        r"
        INSERT INTO object_events (
          event_id,
          event_type,
          actor_id,
          target_object_type,
          target_object_id,
          reason,
          details
        )
        VALUES ($1, 'memory_reused', $2, 'memory_item', $3, $4, $5)
        ",
    )
    .bind(event_id.as_str())
    .bind(candidate.created_by.actor_id.as_str())
    .bind(existing.id.as_str())
    .bind("exact_dedupe_fingerprint")
    .bind(json_value(json!({
        "dedupe_fingerprint": dedupe_fingerprint,
        "source": candidate.source.as_ref().map(|source| json!({
            "object_type": source.source.object_type.as_str(),
            "object_id": source.source.object_id.as_str(),
        })),
    })))
    .execute(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    Ok(Some(existing))
}

async fn existing_memory_summary_by_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
            Vec<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(POSTGRES_MEMORY_SUMMARY_BY_ID_SQL)
    .bind(memory_id)
    .fetch_optional(&mut **tx)
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
            subject_entity_ids,
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
                subject_entity_ids,
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

struct LockedMemoryReinforcementTarget {
    memory_id: String,
    status: MemoryStatus,
    memory_type: String,
    owner_object_type: String,
    owner_object_id: String,
    sensitivity: Sensitivity,
    deleted_at: Option<String>,
    redacted_at: Option<String>,
}

async fn locked_memory_reinforcement_target_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    memory_id: &str,
) -> Result<Option<LockedMemoryReinforcementTarget>, MemoryPersistenceError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
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
          deleted_at::text,
          redacted_at::text
        FROM memory_items
        WHERE memory_id = $1
        FOR UPDATE
        ",
    )
    .bind(memory_id)
    .fetch_optional(&mut **tx)
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
            deleted_at,
            redacted_at,
        )| {
            Ok(LockedMemoryReinforcementTarget {
                memory_id,
                status: parse_memory_status(&status)?,
                memory_type,
                owner_object_type,
                owner_object_id,
                sensitivity: parse_sensitivity(&sensitivity)?,
                deleted_at,
                redacted_at,
            })
        },
    )
    .transpose()
}

fn validate_memory_reinforcement_target(
    target: &LockedMemoryReinforcementTarget,
    candidate: &NewMemoryCandidate,
) -> Result<(), MemoryPersistenceError> {
    if target.owner_object_type != candidate.owner.object_type.as_str()
        || target.owner_object_id != candidate.owner.object_id.as_str()
        || target.memory_type != candidate.memory_type.as_str()
        || !is_reinforceable_memory_status(target.status)
        || target.deleted_at.is_some()
        || target.redacted_at.is_some()
        || !candidate_sensitivity_can_reinforce_target(candidate.sensitivity, target.sensitivity)
    {
        return Err(MemoryPersistenceError::IncompatibleMemoryReinforcement {
            memory_id: target.memory_id.clone(),
        });
    }

    Ok(())
}

fn candidate_sensitivity_can_reinforce_target(
    candidate_sensitivity: Sensitivity,
    target_sensitivity: Sensitivity,
) -> bool {
    candidate_sensitivity <= target_sensitivity
}

fn is_reinforceable_memory_status(status: MemoryStatus) -> bool {
    matches!(
        status,
        MemoryStatus::Candidate
            | MemoryStatus::Active
            | MemoryStatus::Confirmed
            | MemoryStatus::Inferred
    )
}

fn is_memory_dedupe_unique_violation(error: &sqlx::Error) -> bool {
    let Some(database_error) = error.as_database_error() else {
        return false;
    };
    if database_error.code().as_deref() != Some("23505") {
        return false;
    }

    database_error.constraint() == Some("idx_memory_items_live_dedupe_fingerprint")
        || database_error
            .message()
            .contains("idx_memory_items_live_dedupe_fingerprint")
}

async fn existing_memory_summary_for_dedupe_fingerprint_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    dedupe_fingerprint: &str,
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
            Vec<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(POSTGRES_MEMORY_SUMMARY_BY_DEDUPE_FINGERPRINT_SQL)
    .bind(dedupe_fingerprint)
    .fetch_optional(&mut **tx)
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
            subject_entity_ids,
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
                subject_entity_ids,
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
          created_by_actor_id,
          metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ",
    )
    .bind(edge_id.as_str())
    .bind(edge.target.object_type.as_str())
    .bind(edge.target.object_id.as_str())
    .bind(edge.source.object_type.as_str())
    .bind(edge.source.object_id.as_str())
    .bind(edge.relation.as_str())
    .bind(edge.evidence_excerpt.as_deref())
    .bind(edge.created_by.actor_id.as_str())
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
        .map(|object_ref| object_ref.object_id.to_string());

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
        return Ok(Some(candidate.owner.object_id.to_string()));
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

#[cfg(test)]
mod tests {
    use crate::memory::Sensitivity;

    use super::candidate_sensitivity_can_reinforce_target;

    #[test]
    fn candidate_sensitivity_must_not_exceed_target_sensitivity() {
        assert!(candidate_sensitivity_can_reinforce_target(
            Sensitivity::Public,
            Sensitivity::Public
        ));
        assert!(candidate_sensitivity_can_reinforce_target(
            Sensitivity::Private,
            Sensitivity::Secret
        ));
        assert!(!candidate_sensitivity_can_reinforce_target(
            Sensitivity::Private,
            Sensitivity::Public
        ));
        assert!(!candidate_sensitivity_can_reinforce_target(
            Sensitivity::Secret,
            Sensitivity::Normal
        ));
    }
}
