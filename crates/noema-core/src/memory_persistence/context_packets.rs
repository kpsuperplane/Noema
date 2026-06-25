use crate::memory::{MemoryRetrievalRequest, MemoryRetrievalResult, MemoryUseStage};
use rusqlite::params;
use serde_json::json;

use super::{
    error::MemoryPersistenceError,
    helpers::{
        ContextMemoryUseInsert, agent_visible_omissions_json, allocate_id, denial_reason_to_db,
        eligibility_reason_to_db, ensure_principal, ensure_scope, insert_memory_use_record,
        json_to_string, memory_sensitivity_for_tx, purpose_to_db, rank_reason_to_db,
    },
    repository::SqliteMemoryRepository,
};

impl SqliteMemoryRepository {
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
}
