use crate::{
    memory::{MemoryRetrievalRequest, MemoryRetrievalResult, MemoryUseStage},
    memory_persistence::ActorRef,
};
use serde_json::json;

use super::{
    error::MemoryPersistenceError,
    helpers::{
        ContextMemoryUseInsert, agent_visible_omissions_json, denial_reason_to_db,
        eligibility_reason_to_db, memory_use_stage_to_db, purpose_to_db, rank_reason_to_db,
    },
    postgres_helpers::allocate_id as allocate_postgres_id,
    repository::PostgresMemoryRepository,
};

impl PostgresMemoryRepository {
    /// Record a context packet manifest from a memory retrieval result.
    ///
    /// This stores the packet header, memory inclusion edges, audit-only
    /// omissions, redacted agent-visible omission reasons, and memory-use
    /// records. It is the durable audit bridge between deterministic memory
    /// retrieval and later context-packet inspection.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres writes fail.
    pub async fn record_context_packet(
        &self,
        context_packet_id: &str,
        run_id: &str,
        request: &MemoryRetrievalRequest,
        result: &MemoryRetrievalResult,
    ) -> Result<(), MemoryPersistenceError> {
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(MemoryPersistenceError::Database)?;
        let requesting_actor = requesting_actor_ref(&request.requesting_principal_id)?;
        let requesting_actor_id = requesting_actor.actor_id.as_str();
        let active_objects = request
            .trusted
            .active_scopes
            .iter()
            .map(|scope_id| {
                let (object_type, object_id) = scope_object_parts(scope_id);
                json!({
                    "object_type": object_type,
                    "object_id": object_id,
                })
            })
            .collect::<Vec<_>>();
        sqlx::query(
            r"
            INSERT INTO context_packets (
              context_packet_id,
              run_id,
              requesting_actor_id,
              purpose,
              active_objects,
              agent_visible_omissions
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ",
        )
        .bind(context_packet_id)
        .bind(run_id)
        .bind(requesting_actor_id)
        .bind(purpose_to_db(request.trusted.purpose))
        .bind(json!(active_objects))
        .bind(json!(agent_visible_omissions_json(result)))
        .execute(&mut *tx)
        .await
        .map_err(MemoryPersistenceError::Database)?;

        for included in &result.included {
            let packet_memory_id = allocate_postgres_id(&mut *tx, "ctxmem").await?;
            let rank_score = i32::try_from(included.rank_score).map_err(|_| {
                MemoryPersistenceError::InvalidEnum {
                    kind: "rank score",
                    value: included.rank_score.to_string(),
                }
            })?;
            let rank_reasons = included
                .rank_reasons
                .iter()
                .map(|reason| rank_reason_to_db(*reason))
                .collect::<Vec<_>>();
            sqlx::query(
                r"
                INSERT INTO context_packet_memory_edges (
                  packet_memory_id,
                  context_packet_id,
                  memory_id,
                  stage,
                  rank_score,
                  eligibility_reason,
                  rank_reasons
                )
                VALUES ($1, $2, $3, 'included_in_packet', $4, $5, $6)
                ",
            )
            .bind(packet_memory_id)
            .bind(context_packet_id)
            .bind(included.memory_id.as_str())
            .bind(rank_score)
            .bind(eligibility_reason_to_db(included.eligibility_reason))
            .bind(json!(rank_reasons))
            .execute(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        }

        let agent_visible_reason = result
            .agent_visible_omissions
            .first()
            .map(|omission| omission.reason)
            .unwrap_or("none");
        for denial in &result.denied_for_audit {
            let omission_id = allocate_postgres_id(&mut *tx, "ctxomit").await?;
            let omission_sensitivity = match denial.memory_id.as_deref() {
                Some(memory_id) => postgres_memory_sensitivity_for_tx(&mut tx, memory_id).await?,
                None => "normal".to_string(),
            };
            sqlx::query(
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
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ",
            )
            .bind(omission_id)
            .bind(context_packet_id)
            .bind(denial.memory_id.as_deref())
            .bind(denial.relationship_id.as_deref())
            .bind(omission_sensitivity)
            .bind(agent_visible_reason)
            .bind(denial_reason_to_db(denial.reason))
            .bind(json!({
                "run_id": run_id,
                "purpose": purpose_to_db(request.trusted.purpose),
            }))
            .execute(&mut *tx)
            .await
            .map_err(MemoryPersistenceError::Database)?;
        }

        let context_object = request
            .trusted
            .active_scopes
            .first()
            .map(|scope_id| scope_object_parts(scope_id));
        for use_record in &result.use_records {
            insert_postgres_memory_use_record(
                &mut tx,
                ContextMemoryUseInsert {
                    context_packet_id,
                    run_id,
                    memory_id: &use_record.memory_id,
                    stage: use_record.stage,
                    agent_actor_id: requesting_actor_id,
                    context_object_type: context_object
                        .as_ref()
                        .map(|(object_type, _)| *object_type),
                    context_object_id: context_object.as_ref().map(|(_, object_id)| *object_id),
                    purpose: request.trusted.purpose,
                },
            )
            .await?;
        }
        for included in &result.included {
            insert_postgres_memory_use_record(
                &mut tx,
                ContextMemoryUseInsert {
                    context_packet_id,
                    run_id,
                    memory_id: &included.memory_id,
                    stage: MemoryUseStage::IncludedInPacket,
                    agent_actor_id: requesting_actor_id,
                    context_object_type: context_object
                        .as_ref()
                        .map(|(object_type, _)| *object_type),
                    context_object_id: context_object.as_ref().map(|(_, object_id)| *object_id),
                    purpose: request.trusted.purpose,
                },
            )
            .await?;
        }

        tx.commit().await.map_err(MemoryPersistenceError::Database)
    }
}

async fn postgres_memory_sensitivity_for_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    memory_id: &str,
) -> Result<String, MemoryPersistenceError> {
    sqlx::query_scalar::<_, String>("SELECT sensitivity FROM memory_items WHERE memory_id = $1")
        .bind(memory_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(MemoryPersistenceError::Database)
}

async fn insert_postgres_memory_use_record(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    record: ContextMemoryUseInsert<'_>,
) -> Result<(), MemoryPersistenceError> {
    let memory_use_id = allocate_postgres_id(&mut **tx, "memuse").await?;
    sqlx::query(
        r"
        INSERT INTO memory_use_records (
          memory_use_id,
          context_packet_id,
          run_id,
          memory_id,
          stage,
          agent_actor_id,
          context_object_type,
          context_object_id,
          purpose,
          details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ",
    )
    .bind(memory_use_id)
    .bind(record.context_packet_id)
    .bind(record.run_id)
    .bind(record.memory_id)
    .bind(memory_use_stage_to_db(record.stage))
    .bind(record.agent_actor_id)
    .bind(record.context_object_type)
    .bind(record.context_object_id)
    .bind(purpose_to_db(record.purpose))
    .bind(json!({
        "context_packet_id": record.context_packet_id,
        "run_id": record.run_id,
        "stage": memory_use_stage_to_db(record.stage),
    }))
    .execute(&mut **tx)
    .await
    .map_err(MemoryPersistenceError::Database)?;
    Ok(())
}

fn requesting_actor_ref(actor_id: &str) -> Result<ActorRef, MemoryPersistenceError> {
    if actor_id.starts_with("agent:") {
        Ok(ActorRef::agent(actor_id))
    } else if actor_id.starts_with("human:") {
        Ok(ActorRef::human(actor_id))
    } else if actor_id.starts_with("system:") {
        Ok(ActorRef::system(actor_id))
    } else {
        Err(MemoryPersistenceError::InvalidEnum {
            kind: "requesting actor principal",
            value: actor_id.to_string(),
        })
    }
}

fn scope_object_parts(scope_id: &str) -> (&'static str, &str) {
    if let Some(conversation_id) = scope_id.strip_prefix("conversation:") {
        ("conversation", conversation_id)
    } else {
        ("entity", scope_id)
    }
}
