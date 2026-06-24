//! SQLite loader for the deterministic memory retrieval policy engine.

use crate::{
    memory::{
        AccessGrant, Effect, ExternalEgressPolicy, MemoryItem, MemoryRetrievalRequest,
        MemoryRetrievalResult, MemoryStore, ObjectLink, ParticipantRole,
        ParticipantVisibilityPolicy, Permission, Purpose, Relationship, RelationshipStatus,
        RetrievalHints, RetrievalPolicyStatus, Sensitivity, SubjectRole,
    },
    memory_persistence::MemoryPersistenceError,
    retrieval_policy_fingerprint,
};
use rusqlite::Connection;
use serde_json::Value;

pub(crate) fn retrieve(
    conn: &Connection,
    request: &MemoryRetrievalRequest,
) -> Result<MemoryRetrievalResult, MemoryPersistenceError> {
    let mut store = MemoryStore::default();
    load_memories(conn, &mut store)?;
    load_participants(conn, &mut store)?;
    load_subjects(conn, &mut store)?;
    load_purpose_rules(conn, &mut store)?;
    load_object_links(conn, &mut store)?;
    load_access_grants(conn, &mut store)?;
    load_provenance(conn, &mut store)?;
    load_relationships(conn, &mut store)?;
    Ok(store.retrieve(request))
}

fn load_memories(conn: &Connection, store: &mut MemoryStore) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT
              memory_id,
              home_scope_id,
              title,
              content,
              status,
              sensitivity,
              retrieval_hints,
              retrieval_policy_status,
              retrieval_policy_fingerprint,
              participant_visibility_policy,
              external_egress_policy,
              owner_principal_id
            FROM memory_items
            WHERE (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
              AND (valid_from IS NULL OR valid_from <= CURRENT_TIMESTAMP)
              AND (valid_to IS NULL OR valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (
            memory_id,
            home_scope_id,
            title,
            content,
            status,
            sensitivity,
            retrieval_hints,
            retrieval_policy_status,
            retrieval_policy_fingerprint,
            participant_visibility_policy,
            external_egress_policy,
            owner_principal_id,
        ) = row.map_err(MemoryPersistenceError::Sqlite)?;
        let mut memory = MemoryItem::new(memory_id, home_scope_id, title, content);
        memory.status = parse_memory_status(&status)?;
        memory.sensitivity = parse_sensitivity(&sensitivity)?;
        memory.retrieval_hints = parse_retrieval_hints(&retrieval_hints)?;
        memory.retrieval_policy_status = effective_retrieval_policy_status(
            conn,
            &memory.memory_id,
            &retrieval_policy_status,
            retrieval_policy_fingerprint.as_deref(),
        )?;
        memory.participant_visibility_policy =
            parse_participant_visibility_policy(&participant_visibility_policy)?;
        memory.external_egress_policy = parse_external_egress_policy(&external_egress_policy)?;
        memory.owner_principal_id = owner_principal_id;
        store.insert_memory(memory);
    }

    Ok(())
}

fn load_participants(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT mp.memory_id, mp.principal_id, mp.role
            FROM memory_participants mp
            JOIN memory_items mi ON mi.memory_id = mp.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, principal_id, role) = row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_participant(memory_id, principal_id, parse_participant_role(&role)?)?;
    }

    Ok(())
}

fn load_subjects(conn: &Connection, store: &mut MemoryStore) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT ms.memory_id, ms.entity_id, ms.role
            FROM memory_subjects ms
            JOIN memory_items mi ON mi.memory_id = ms.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, entity_id, role) = row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_subject(memory_id, entity_id, parse_subject_role(&role)?)?;
    }

    Ok(())
}

fn load_purpose_rules(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT rule.memory_id, rule.purpose, rule.effect
            FROM memory_retrieval_purpose_rules rule
            JOIN memory_items mi ON mi.memory_id = rule.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, purpose, effect) = row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_purpose_rule(memory_id, parse_purpose(&purpose)?, parse_effect(&effect)?)?;
    }

    Ok(())
}

fn load_object_links(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT
              link.memory_id,
              link.object_type,
              link.object_id,
              link.relation,
              link.authorized_scope_id
            FROM memory_retrieval_object_links link
            JOIN memory_items mi ON mi.memory_id = link.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, object_type, object_id, relation, authorized_scope_id) =
            row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_object_link(
            memory_id,
            ObjectLink::new(object_type, object_id),
            relation,
            authorized_scope_id,
        )?;
    }

    Ok(())
}

fn load_access_grants(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT memory_id, scope_id, principal_id, permission, effect
            FROM memory_access_grants
            WHERE (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
              AND permission IN (
                'read',
                'use_for_retrieval',
                'use_for_proactivity',
                'use_for_external_action'
              )
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, scope_id, principal_id, permission, effect) =
            row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_access_grant(AccessGrant {
            memory_id,
            scope_id,
            principal_id,
            permission: parse_permission(&permission)?,
            effect: parse_effect(&effect)?,
        });
    }

    Ok(())
}

fn load_provenance(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT provenance.memory_id, provenance.source_id
            FROM memory_provenance_edges provenance
            JOIN memory_items mi ON mi.memory_id = provenance.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (memory_id, source_id) = row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_provenance(memory_id, source_id)?;
    }

    Ok(())
}

fn load_relationships(
    conn: &Connection,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let mut stmt = conn
        .prepare(
            r"
            SELECT
              r.relationship_id,
              r.home_scope_id,
              r.subject_entity_id,
              r.predicate,
              r.object_entity_id,
              r.memory_id,
              r.status
            FROM relationships r
            JOIN memory_items mi ON mi.memory_id = r.memory_id
            WHERE (mi.expires_at IS NULL OR mi.expires_at > CURRENT_TIMESTAMP)
              AND (mi.valid_from IS NULL OR mi.valid_from <= CURRENT_TIMESTAMP)
              AND (mi.valid_to IS NULL OR mi.valid_to > CURRENT_TIMESTAMP)
              AND (r.valid_from IS NULL OR r.valid_from <= CURRENT_TIMESTAMP)
              AND (r.valid_to IS NULL OR r.valid_to > CURRENT_TIMESTAMP)
            ",
        )
        .map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(MemoryPersistenceError::Sqlite)?;

    for row in rows {
        let (
            relationship_id,
            home_scope_id,
            subject_entity_id,
            predicate,
            object_entity_id,
            memory_id,
            status,
        ) = row.map_err(MemoryPersistenceError::Sqlite)?;
        store.add_relationship(Relationship {
            relationship_id,
            home_scope_id,
            subject_entity_id,
            predicate,
            object_entity_id,
            memory_id,
            status: parse_relationship_status(&status)?,
        })?;
    }

    Ok(())
}

fn parse_retrieval_hints(value: &str) -> Result<RetrievalHints, MemoryPersistenceError> {
    let value: Value = serde_json::from_str(value)?;
    Ok(RetrievalHints {
        topics: string_array(&value, "topics"),
        keywords: string_array(&value, "keywords"),
        summary: value
            .get("summary")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn effective_retrieval_policy_status(
    conn: &Connection,
    memory_id: &str,
    status: &str,
    stored_fingerprint: Option<&str>,
) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
    let status = parse_retrieval_policy_status(status)?;
    if status != RetrievalPolicyStatus::Valid {
        return Ok(status);
    }

    let Some(stored_fingerprint) = stored_fingerprint else {
        return Ok(RetrievalPolicyStatus::Stale);
    };
    let current_fingerprint = retrieval_policy_fingerprint::current_fingerprint(conn, memory_id)?;
    if stored_fingerprint == current_fingerprint {
        Ok(RetrievalPolicyStatus::Valid)
    } else {
        Ok(RetrievalPolicyStatus::Stale)
    }
}

fn parse_memory_status(value: &str) -> Result<crate::memory::MemoryStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(crate::memory::MemoryStatus::Candidate),
        "active" => Ok(crate::memory::MemoryStatus::Active),
        "confirmed" => Ok(crate::memory::MemoryStatus::Confirmed),
        "inferred" => Ok(crate::memory::MemoryStatus::Inferred),
        "stale" => Ok(crate::memory::MemoryStatus::Stale),
        "superseded" => Ok(crate::memory::MemoryStatus::Superseded),
        "archived" => Ok(crate::memory::MemoryStatus::Archived),
        "deleted" => Ok(crate::memory::MemoryStatus::Deleted),
        "disputed" => Ok(crate::memory::MemoryStatus::Disputed),
        _ => invalid_enum("memory status", value),
    }
}

fn parse_sensitivity(value: &str) -> Result<Sensitivity, MemoryPersistenceError> {
    match value {
        "public" => Ok(Sensitivity::Public),
        "normal" => Ok(Sensitivity::Normal),
        "private" => Ok(Sensitivity::Private),
        "sensitive" => Ok(Sensitivity::Sensitive),
        "secret" => Ok(Sensitivity::Secret),
        _ => invalid_enum("sensitivity", value),
    }
}

fn parse_retrieval_policy_status(
    value: &str,
) -> Result<RetrievalPolicyStatus, MemoryPersistenceError> {
    match value {
        "valid" => Ok(RetrievalPolicyStatus::Valid),
        "stale" => Ok(RetrievalPolicyStatus::Stale),
        "invalid" => Ok(RetrievalPolicyStatus::Invalid),
        "needs_review" => Ok(RetrievalPolicyStatus::NeedsReview),
        _ => invalid_enum("retrieval policy status", value),
    }
}

fn parse_participant_visibility_policy(
    value: &str,
) -> Result<ParticipantVisibilityPolicy, MemoryPersistenceError> {
    match value {
        "any_active_human" => Ok(ParticipantVisibilityPolicy::AnyActiveHuman),
        "all_original_humans" => Ok(ParticipantVisibilityPolicy::AllOriginalHumans),
        "owner_only" => Ok(ParticipantVisibilityPolicy::OwnerOnly),
        "explicit_grant_only" => Ok(ParticipantVisibilityPolicy::ExplicitGrantOnly),
        _ => invalid_enum("participant visibility policy", value),
    }
}

fn parse_external_egress_policy(
    value: &str,
) -> Result<ExternalEgressPolicy, MemoryPersistenceError> {
    match value {
        "allow" => Ok(ExternalEgressPolicy::Allow),
        "approval_required" => Ok(ExternalEgressPolicy::ApprovalRequired),
        "deny" => Ok(ExternalEgressPolicy::Deny),
        _ => invalid_enum("external egress policy", value),
    }
}

fn parse_participant_role(value: &str) -> Result<ParticipantRole, MemoryPersistenceError> {
    match value {
        "human_in_scope" => Ok(ParticipantRole::HumanInScope),
        "agent_in_scope" => Ok(ParticipantRole::AgentInScope),
        "originator" => Ok(ParticipantRole::Originator),
        "observer" => Ok(ParticipantRole::Observer),
        _ => invalid_enum("participant role", value),
    }
}

fn parse_subject_role(value: &str) -> Result<SubjectRole, MemoryPersistenceError> {
    match value {
        "about" => Ok(SubjectRole::About),
        "claimant" => Ok(SubjectRole::Claimant),
        "affected" => Ok(SubjectRole::Affected),
        "owner" => Ok(SubjectRole::Owner),
        "assignee" => Ok(SubjectRole::Assignee),
        "source" => Ok(SubjectRole::Source),
        "target" => Ok(SubjectRole::Target),
        _ => invalid_enum("subject role", value),
    }
}

fn parse_purpose(value: &str) -> Result<Purpose, MemoryPersistenceError> {
    match value {
        "answer_human_question" => Ok(Purpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(Purpose::DraftInternalContent),
        "general_personalization" => Ok(Purpose::GeneralPersonalization),
        "manage_task" => Ok(Purpose::ManageTask),
        "manage_calendar" => Ok(Purpose::ManageCalendar),
        "draft_external_content" => Ok(Purpose::DraftExternalContent),
        "use_tool" => Ok(Purpose::UseTool),
        "proactive_suggestion" => Ok(Purpose::ProactiveSuggestion),
        "external_action" => Ok(Purpose::ExternalAction),
        "debug_audit" => Ok(Purpose::DebugAudit),
        _ => invalid_enum("retrieval purpose", value),
    }
}

fn parse_effect(value: &str) -> Result<Effect, MemoryPersistenceError> {
    match value {
        "allow" => Ok(Effect::Allow),
        "deny" => Ok(Effect::Deny),
        _ => invalid_enum("policy effect", value),
    }
}

fn parse_permission(value: &str) -> Result<Permission, MemoryPersistenceError> {
    match value {
        "read" => Ok(Permission::Read),
        "use_for_retrieval" => Ok(Permission::UseForRetrieval),
        "use_for_proactivity" => Ok(Permission::UseForProactivity),
        "use_for_external_action" => Ok(Permission::UseForExternalAction),
        _ => invalid_enum("memory access permission", value),
    }
}

fn parse_relationship_status(value: &str) -> Result<RelationshipStatus, MemoryPersistenceError> {
    match value {
        "candidate" => Ok(RelationshipStatus::Candidate),
        "active" => Ok(RelationshipStatus::Active),
        "confirmed" => Ok(RelationshipStatus::Confirmed),
        "superseded" => Ok(RelationshipStatus::Superseded),
        "archived" => Ok(RelationshipStatus::Archived),
        "deleted" => Ok(RelationshipStatus::Deleted),
        "disputed" => Ok(RelationshipStatus::Disputed),
        _ => invalid_enum("relationship status", value),
    }
}

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, MemoryPersistenceError> {
    Err(MemoryPersistenceError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}
