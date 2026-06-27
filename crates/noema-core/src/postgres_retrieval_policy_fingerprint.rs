//! Deterministic retrieval-policy fingerprints for Postgres memory rows.

use crate::{
    memory_persistence::MemoryPersistenceError,
    retrieval_policy_fingerprint::{self, FINGERPRINT_VERSION},
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

pub(crate) async fn current_fingerprint(
    pool: &PgPool,
    memory_id: &str,
) -> Result<String, MemoryPersistenceError> {
    let basis = fingerprint_basis(pool, memory_id).await?;
    retrieval_policy_fingerprint::fingerprint_for_basis(&basis)
}

async fn fingerprint_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Value, MemoryPersistenceError> {
    let memory = memory_basis(pool, memory_id).await?;
    Ok(json!({
        "version": FINGERPRINT_VERSION,
        "memory": memory,
        "subjects": subjects_basis(pool, memory_id).await?,
        "participants": participants_basis(pool, memory_id).await?,
        "purpose_rules": purpose_rules_basis(pool, memory_id).await?,
        "trusted_object_links": object_links_basis(pool, memory_id).await?,
        "access_grants": access_grants_basis(pool, memory_id).await?,
        "relationships": relationships_basis(pool, memory_id).await?,
        "provenance": provenance_basis(pool, memory_id).await?,
    }))
}

async fn memory_basis(pool: &PgPool, memory_id: &str) -> Result<Value, MemoryPersistenceError> {
    let row = sqlx::query(
        r"
        SELECT
          memory_id,
          owner_object_type,
          owner_object_id,
          memory_type,
          title,
          content,
          structured_value,
          status,
          sensitivity,
          proactivity_level,
          retrieval_policy_version,
          participant_visibility_policy,
          external_egress_policy,
          created_by_actor_id,
          authority_level,
          extraction_method,
          valid_from::text AS valid_from,
          valid_to::text AS valid_to,
          expires_at::text AS expires_at
        FROM memory_items
        WHERE memory_id = $1
        ",
    )
    .bind(memory_id)
    .fetch_optional(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?
    .ok_or_else(|| MemoryPersistenceError::MemoryNotFound {
        memory_id: memory_id.to_string(),
    })?;

    Ok(json!({
        "memory_id": row.try_get::<String, _>("memory_id")?,
        "owner_object_type": row.try_get::<String, _>("owner_object_type")?,
        "owner_object_id": row.try_get::<String, _>("owner_object_id")?,
        "memory_type": row.try_get::<String, _>("memory_type")?,
        "title": row.try_get::<String, _>("title")?,
        "content": row.try_get::<String, _>("content")?,
        "structured_value": row.try_get::<Value, _>("structured_value")?,
        "status": row.try_get::<String, _>("status")?,
        "sensitivity": row.try_get::<String, _>("sensitivity")?,
        "proactivity_level": row.try_get::<i32, _>("proactivity_level")?,
        "retrieval_policy_version": row.try_get::<i32, _>("retrieval_policy_version")?,
        "participant_visibility_policy": row.try_get::<String, _>("participant_visibility_policy")?,
        "external_egress_policy": row.try_get::<String, _>("external_egress_policy")?,
        "created_by_actor_id": row.try_get::<String, _>("created_by_actor_id")?,
        "authority_level": row.try_get::<String, _>("authority_level")?,
        "extraction_method": row.try_get::<String, _>("extraction_method")?,
        "valid_from": row.try_get::<Option<String>, _>("valid_from")?,
        "valid_to": row.try_get::<Option<String>, _>("valid_to")?,
        "expires_at": row.try_get::<Option<String>, _>("expires_at")?,
    }))
}

async fn subjects_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r"
        SELECT entity_id, role
        FROM memory_subjects
        WHERE memory_id = $1
        ORDER BY entity_id ASC, role ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    Ok(rows
        .into_iter()
        .map(|(entity_id, role)| json!({ "entity_id": entity_id, "role": role }))
        .collect())
}

async fn participants_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r"
        SELECT participant_actor_id, role
        FROM memory_participants
        WHERE memory_id = $1
        ORDER BY participant_actor_id ASC, role ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    Ok(rows
        .into_iter()
        .map(|(participant_actor_id, role)| {
            json!({
                "participant_actor_id": participant_actor_id,
                "role": role,
            })
        })
        .collect())
}

async fn purpose_rules_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        r"
        SELECT purpose, effect, created_by_actor_id
        FROM memory_retrieval_purpose_rules
        WHERE memory_id = $1
        ORDER BY purpose ASC, effect ASC, created_by_actor_id ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    Ok(rows
        .into_iter()
        .map(|(purpose, effect, created_by_actor_id)| {
            json!({
                "purpose": purpose,
                "effect": effect,
                "created_by_actor_id": created_by_actor_id,
            })
        })
        .collect())
}

async fn object_links_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query(
        r"
        SELECT
          object_type,
          object_id,
          relation,
          resolver_actor_id,
          resolver_version,
          source_run_id,
          authorized_actor_id,
          created_by_actor_id
        FROM memory_retrieval_object_links
        WHERE memory_id = $1
        ORDER BY object_type ASC, object_id ASC, relation ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "object_type": row.try_get::<String, _>("object_type")?,
                "object_id": row.try_get::<String, _>("object_id")?,
                "relation": row.try_get::<String, _>("relation")?,
                "resolver_actor_id": row.try_get::<Option<String>, _>("resolver_actor_id")?,
                "resolver_version": row.try_get::<Option<String>, _>("resolver_version")?,
                "source_run_id": row.try_get::<Option<String>, _>("source_run_id")?,
                "authorized_actor_id": row.try_get::<Option<String>, _>("authorized_actor_id")?,
                "created_by_actor_id": row.try_get::<Option<String>, _>("created_by_actor_id")?,
            }))
        })
        .collect()
}

async fn access_grants_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query(
        r"
        SELECT
          grant_id,
          target_object_type,
          target_object_id,
          grantee_object_type,
          grantee_object_id,
          permission,
          effect,
          expires_at::text AS expires_at
        FROM object_access_grants
        WHERE (
            target_object_type = 'memory_item'
            AND target_object_id = $1
          )
           OR (
             target_object_type = (
               SELECT owner_object_type FROM memory_items WHERE memory_id = $1
             )
             AND target_object_id = (
               SELECT owner_object_id FROM memory_items WHERE memory_id = $1
             )
           )
        ORDER BY grantee_object_type ASC, grantee_object_id ASC, permission ASC, effect ASC, grant_id ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "grant_id": row.try_get::<String, _>("grant_id")?,
                "target_object_type": row.try_get::<String, _>("target_object_type")?,
                "target_object_id": row.try_get::<String, _>("target_object_id")?,
                "grantee_object_type": row.try_get::<String, _>("grantee_object_type")?,
                "grantee_object_id": row.try_get::<String, _>("grantee_object_id")?,
                "permission": row.try_get::<String, _>("permission")?,
                "effect": row.try_get::<String, _>("effect")?,
                "expires_at": row.try_get::<Option<String>, _>("expires_at")?,
            }))
        })
        .collect()
}

async fn relationships_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query(
        r"
        SELECT
          relationship_id,
          owner_object_type,
          owner_object_id,
          subject_entity_id,
          predicate,
          object_entity_id,
          status,
          confidence,
          valid_from::text AS valid_from,
          valid_to::text AS valid_to,
          metadata
        FROM relationships
        WHERE memory_id = $1
        ORDER BY relationship_id ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "relationship_id": row.try_get::<String, _>("relationship_id")?,
                "owner_object_type": row.try_get::<String, _>("owner_object_type")?,
                "owner_object_id": row.try_get::<String, _>("owner_object_id")?,
                "subject_entity_id": row.try_get::<String, _>("subject_entity_id")?,
                "predicate": row.try_get::<String, _>("predicate")?,
                "object_entity_id": row.try_get::<String, _>("object_entity_id")?,
                "status": row.try_get::<String, _>("status")?,
                "confidence": row.try_get::<Option<f64>, _>("confidence")?,
                "valid_from": row.try_get::<Option<String>, _>("valid_from")?,
                "valid_to": row.try_get::<Option<String>, _>("valid_to")?,
                "metadata": row.try_get::<Value, _>("metadata")?,
            }))
        })
        .collect()
}

async fn provenance_basis(
    pool: &PgPool,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        r"
        SELECT source_object_type, source_object_id, relation, evidence_excerpt
        FROM object_provenance_edges
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
          AND deleted_at IS NULL
        ORDER BY source_object_type ASC, source_object_id ASC, relation ASC, edge_id ASC
        ",
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    Ok(rows
        .into_iter()
        .map(
            |(source_object_type, source_object_id, relation, evidence_excerpt)| {
                json!({
                    "source_object_type": source_object_type,
                    "source_object_id": source_object_id,
                    "relation": relation,
                    "evidence_excerpt": evidence_excerpt,
                })
            },
        )
        .collect())
}
