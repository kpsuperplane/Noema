//! Postgres loader for the deterministic memory retrieval policy engine.

use std::collections::HashSet;

use crate::{
    memory::{
        AccessGrant, Effect, ExternalEgressPolicy, MemoryItem, MemoryRetrievalRequest,
        MemoryRetrievalResult, MemoryStore, ObjectLink, ParticipantRole,
        ParticipantVisibilityPolicy, Permission, Purpose, Relationship, RelationshipStatus,
        RetrievalHints, RetrievalPolicyStatus, Sensitivity, SubjectRole,
    },
    memory_persistence::MemoryPersistenceError,
    postgres_retrieval_policy_fingerprint,
};
use serde_json::Value;
use sqlx::PgPool;

const MAX_PUBLIC_HINT_CANDIDATES: i64 = 128;
const MAX_FTS_TERMS: usize = 24;

pub(crate) async fn retrieve(
    pool: &PgPool,
    request: &MemoryRetrievalRequest,
) -> Result<MemoryRetrievalResult, MemoryPersistenceError> {
    let mut store = MemoryStore::default();
    let public_hint_candidates = public_hint_candidate_ids(pool, request).await?;
    load_memories(pool, &mut store).await?;
    load_participants(pool, &mut store).await?;
    load_subjects(pool, &mut store).await?;
    load_purpose_rules(pool, &mut store).await?;
    load_object_links(pool, &mut store).await?;
    load_access_grants(pool, &mut store).await?;
    load_provenance(pool, &mut store).await?;
    load_relationships(pool, &mut store).await?;
    Ok(store.retrieve_with_public_hint_candidates(request, &public_hint_candidates))
}

async fn public_hint_candidate_ids(
    pool: &PgPool,
    request: &MemoryRetrievalRequest,
) -> Result<HashSet<String>, MemoryPersistenceError> {
    let Some(query) = fts_query_for_request(request) else {
        return Ok(HashSet::new());
    };

    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT memory_id
        FROM memory_items
        WHERE search_vector @@ to_tsquery('simple', $1)
        ORDER BY ts_rank_cd(search_vector, to_tsquery('simple', $1)) DESC, memory_id ASC
        LIMIT $2
        ",
    )
    .bind(query)
    .bind(MAX_PUBLIC_HINT_CANDIDATES)
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;
    Ok(rows.into_iter().collect())
}

fn fts_query_for_request(request: &MemoryRetrievalRequest) -> Option<String> {
    let mut terms = Vec::new();
    add_fts_terms(&request.untrusted_hints.query_text, &mut terms);
    for topic in &request.untrusted_hints.fuzzy_topics {
        add_fts_terms(topic, &mut terms);
    }
    for entity in &request.untrusted_hints.fuzzy_entities {
        add_fts_terms(entity, &mut terms);
    }
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" | "))
    }
}

fn add_fts_terms(input: &str, terms: &mut Vec<String>) {
    for term in input
        .split(|character: char| !character.is_alphanumeric())
        .filter_map(normalized_fts_term)
    {
        if terms.len() >= MAX_FTS_TERMS {
            return;
        }
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
}

fn normalized_fts_term(term: &str) -> Option<String> {
    let term = term.trim().to_ascii_lowercase();
    if term.len() < 2 { None } else { Some(term) }
}

async fn load_memories(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Value,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
        ),
    >(
        r"
        SELECT
          memory_id,
          owner_object_type,
          owner_object_id,
          title,
          content,
          status,
          sensitivity,
          retrieval_hints,
          retrieval_policy_status,
          retrieval_policy_fingerprint,
          participant_visibility_policy,
          external_egress_policy,
          CASE
            WHEN owner_object_type = 'human' THEN owner_object_id
            ELSE (
              SELECT participant_object_id
              FROM memory_participants
              WHERE memory_participants.memory_id = memory_items.memory_id
                AND role = 'human_in_scope'
              ORDER BY participant_object_id ASC
              LIMIT 1
            )
          END AS owner_principal_id
        FROM memory_items
        WHERE (expires_at IS NULL OR expires_at > now())
          AND (valid_from IS NULL OR valid_from <= now())
          AND (valid_to IS NULL OR valid_to > now())
          AND status != 'deleted'
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (
        memory_id,
        owner_object_type,
        owner_object_id,
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
    ) in rows
    {
        let home_scope_id = object_ref_key(&owner_object_type, &owner_object_id);
        let mut memory = MemoryItem::new(memory_id, home_scope_id, title, content);
        memory.status = parse_memory_status(&status)?;
        memory.sensitivity = parse_sensitivity(&sensitivity)?;
        memory.retrieval_hints = parse_retrieval_hints(&retrieval_hints);
        memory.retrieval_policy_status = effective_retrieval_policy_status(
            pool,
            &memory.memory_id,
            &retrieval_policy_status,
            retrieval_policy_fingerprint.as_deref(),
        )
        .await?;
        memory.participant_visibility_policy =
            parse_participant_visibility_policy(&participant_visibility_policy)?;
        memory.external_egress_policy = parse_external_egress_policy(&external_egress_policy)?;
        memory.owner_principal_id = owner_principal_id;
        store.insert_memory(memory);
    }

    Ok(())
}

async fn load_participants(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        r"
        SELECT mp.memory_id, mp.participant_object_type, mp.participant_object_id, mp.role
        FROM memory_participants mp
        JOIN memory_items mi ON mi.memory_id = mp.memory_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (memory_id, participant_object_type, participant_object_id, role) in rows {
        store.add_participant(
            memory_id,
            object_ref_key(&participant_object_type, &participant_object_id),
            parse_participant_role(&role)?,
        )?;
    }

    Ok(())
}

async fn load_subjects(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r"
        SELECT ms.memory_id, ms.entity_id, ms.role
        FROM memory_subjects ms
        JOIN memory_items mi ON mi.memory_id = ms.memory_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (memory_id, entity_id, role) in rows {
        store.add_subject(memory_id, entity_id, parse_subject_role(&role)?)?;
    }

    Ok(())
}

async fn load_purpose_rules(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r"
        SELECT rule.memory_id, rule.purpose, rule.effect
        FROM memory_retrieval_purpose_rules rule
        JOIN memory_items mi ON mi.memory_id = rule.memory_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (memory_id, purpose, effect) in rows {
        store.add_purpose_rule(memory_id, parse_purpose(&purpose)?, parse_effect(&effect)?)?;
    }

    Ok(())
}

async fn load_object_links(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<
        _,
        (
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
          link.memory_id,
          link.object_type,
          link.object_id,
          link.relation,
          link.authorized_object_type,
          link.authorized_object_id
        FROM memory_retrieval_object_links link
        JOIN memory_items mi ON mi.memory_id = link.memory_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (
        memory_id,
        object_type,
        object_id,
        relation,
        authorized_object_type,
        authorized_object_id,
    ) in rows
    {
        let authorized_scope_id = authorized_object_type
            .zip(authorized_object_id)
            .map(|(object_type, object_id)| object_ref_key(&object_type, &object_id));
        store.add_object_link(
            memory_id,
            ObjectLink::new(object_type, object_id),
            relation,
            authorized_scope_id,
        )?;
    }

    Ok(())
}

async fn load_access_grants(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
            String,
            String,
        ),
    >(
        r"
        SELECT
          CASE WHEN target_object_type = 'memory_item' THEN target_object_id END AS memory_id,
          CASE WHEN target_object_type != 'memory_item' THEN target_object_type END AS scope_object_type,
          CASE WHEN target_object_type != 'memory_item' THEN target_object_id END AS scope_object_id,
          grantee_object_type,
          grantee_object_id,
          permission,
          effect
        FROM object_access_grants
        WHERE (expires_at IS NULL OR expires_at > now())
          AND permission IN (
            'read',
            'use_for_retrieval',
            'use_for_proactivity',
            'use_for_external_action'
          )
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (
        memory_id,
        scope_object_type,
        scope_object_id,
        grantee_object_type,
        grantee_object_id,
        permission,
        effect,
    ) in rows
    {
        let scope_id = scope_object_type
            .zip(scope_object_id)
            .map(|(object_type, object_id)| object_ref_key(&object_type, &object_id));
        store.add_access_grant(AccessGrant {
            memory_id,
            scope_id,
            principal_id: object_ref_key(&grantee_object_type, &grantee_object_id),
            permission: parse_permission(&permission)?,
            effect: parse_effect(&effect)?,
        });
    }

    Ok(())
}

async fn load_provenance(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r"
        SELECT provenance.target_object_id, provenance.source_object_type, provenance.source_object_id
        FROM object_provenance_edges provenance
        JOIN memory_items mi
          ON provenance.target_object_type = 'memory_item'
         AND mi.memory_id = provenance.target_object_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
          AND provenance.deleted_at IS NULL
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (memory_id, source_object_type, source_object_id) in rows {
        store.add_provenance(
            memory_id,
            object_ref_key(&source_object_type, &source_object_id),
        )?;
    }

    Ok(())
}

async fn load_relationships(
    pool: &PgPool,
    store: &mut MemoryStore,
) -> Result<(), MemoryPersistenceError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        ),
    >(
        r"
        SELECT
          r.relationship_id,
          r.owner_object_type,
          r.owner_object_id,
          r.subject_entity_id,
          r.predicate,
          r.object_entity_id,
          r.memory_id,
          r.status
        FROM relationships r
        JOIN memory_items mi ON mi.memory_id = r.memory_id
        WHERE (mi.expires_at IS NULL OR mi.expires_at > now())
          AND (mi.valid_from IS NULL OR mi.valid_from <= now())
          AND (mi.valid_to IS NULL OR mi.valid_to > now())
          AND mi.status != 'deleted'
          AND (r.valid_from IS NULL OR r.valid_from <= now())
          AND (r.valid_to IS NULL OR r.valid_to > now())
        ",
    )
    .fetch_all(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?;

    for (
        relationship_id,
        owner_object_type,
        owner_object_id,
        subject_entity_id,
        predicate,
        object_entity_id,
        memory_id,
        status,
    ) in rows
    {
        store.add_relationship(Relationship {
            relationship_id,
            home_scope_id: object_ref_key(&owner_object_type, &owner_object_id),
            subject_entity_id,
            predicate,
            object_entity_id,
            memory_id,
            status: parse_relationship_status(&status)?,
        })?;
    }

    Ok(())
}

fn parse_retrieval_hints(value: &Value) -> RetrievalHints {
    RetrievalHints {
        topics: string_array(value, "topics"),
        keywords: string_array(value, "keywords"),
        summary: value
            .get("summary")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
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

fn object_ref_key(object_type: &str, object_id: &str) -> String {
    let prefix = format!("{object_type}:");
    if object_id.starts_with(&prefix) {
        object_id.to_string()
    } else {
        format!("{object_type}:{object_id}")
    }
}

async fn effective_retrieval_policy_status(
    pool: &PgPool,
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
    let current_fingerprint =
        postgres_retrieval_policy_fingerprint::current_fingerprint(pool, memory_id).await?;
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
