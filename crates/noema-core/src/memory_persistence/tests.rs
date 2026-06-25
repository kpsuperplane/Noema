use super::*;
use crate::context_graph::ContextGraphFilter;
use crate::memory::{
    DenialReason, Effect, EligibilityReason, ExternalEgressPolicy, MemoryRetrievalRequest,
    MemoryRetrievalResult, MemoryStatus, MemoryUseStage, ObjectLink, ParticipantRole,
    ParticipantVisibilityPolicy, Purpose, RelationshipStatus, RetrievalPolicyStatus, Sensitivity,
    SubjectRole, TrustedRetrievalContext, UntrustedHints,
};
use crate::paths::NoemaPaths;
use rusqlite::params;
use serde_json::{Value, json};

mod chat_listing;
mod context_packets;
mod conversations;
mod core_writes;
mod graph_inspection;
mod object_refs;
mod provenance_deletion;
mod retrieval;
mod schema_contract;

fn request_for_kevin() -> MemoryRetrievalRequest {
    MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted: TrustedRetrievalContext::for_human("human:kevin", Purpose::AnswerHumanQuestion),
        untrusted_hints: UntrustedHints::default(),
    }
}

fn ensure_test_actors(repo: &mut SqliteMemoryRepository) {
    repo.ensure_default_actors().expect("default actors");
    repo.conn
        .execute(
            r"
            INSERT INTO humans (human_id, display_name, handle)
            VALUES
              ('human:kevin', 'Kevin', 'kevin'),
              ('human:someone_else', 'Someone Else', 'someone_else')
            ON CONFLICT(human_id) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
            ",
            [],
        )
        .expect("test humans");
}

fn ensure_entity_object(
    repo: &mut SqliteMemoryRepository,
    entity_id: &str,
    entity_type: &str,
    canonical_name: &str,
) -> ObjectRef {
    ensure_test_actors(repo);
    repo.conn
        .execute(
            r"
            INSERT INTO entities (
              entity_id,
              owner_object_type,
              owner_object_id,
              entity_type,
              canonical_name
            )
            VALUES (?1, 'human', 'human:local', ?2, ?3)
            ON CONFLICT(entity_id) DO UPDATE SET
              owner_object_type = excluded.owner_object_type,
              owner_object_id = excluded.owner_object_id,
              entity_type = excluded.entity_type,
              canonical_name = excluded.canonical_name,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![entity_id, entity_type, canonical_name],
        )
        .expect("entity object");
    ObjectRef::new(ObjectType::Entity, entity_id).expect("entity ref")
}

fn create_source_item(repo: &mut SqliteMemoryRepository, content: &str) -> ConversationItemRecord {
    ensure_test_actors(repo);
    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: json!({}),
        })
        .expect("turn");
    repo.append_conversation_item(NewConversationItem {
        conversation_id: conversation.conversation_id,
        turn_id: Some(turn.turn_id),
        parent_item_id: None,
        kind: ConversationItemKind::UserText,
        status: ConversationItemStatus::Completed,
        author: ObjectRef::human("human:local"),
        content_text: Some(content.to_string()),
        payload_json: json!({}),
        metadata: json!({}),
    })
    .expect("source item")
}

fn new_memory_candidate_for_owner(
    repo: &mut SqliteMemoryRepository,
    owner: ObjectRef,
    content: &str,
) -> NewMemoryCandidate {
    let source_item = create_source_item(repo, content);
    NewMemoryCandidate {
        owner,
        memory_type: MemoryType::Note,
        title: None,
        content: content.to_string(),
        sensitivity: Sensitivity::Normal,
        status: MemoryStatus::Candidate,
        created_by: ObjectRef::agent("agent:primary"),
        owner_actor: None,
        authority_level: MemoryAuthorityLevel::AgentInference,
        extraction_method: MemoryExtractionMethod::LlmExtracted,
        confidence: None,
        retrieval_hints: json!({}),
        observed_at: None,
        source: Some(ObjectProvenanceSource {
            source: ObjectRef::conversation_item(source_item.item_id),
            evidence_excerpt: None,
        }),
        participants: Vec::new(),
        subjects: Vec::new(),
        metadata: json!({}),
    }
}

fn new_conversation_memory_candidate(
    repo: &mut SqliteMemoryRepository,
    content: &str,
) -> NewMemoryCandidate {
    let source_item = create_source_item(repo, content);
    let owner = ObjectRef::new(
        ObjectType::Conversation,
        source_item.conversation_id.clone(),
    )
    .expect("conversation owner");
    NewMemoryCandidate {
        owner,
        memory_type: MemoryType::Note,
        title: None,
        content: content.to_string(),
        sensitivity: Sensitivity::Normal,
        status: MemoryStatus::Candidate,
        created_by: ObjectRef::agent("agent:primary"),
        owner_actor: None,
        authority_level: MemoryAuthorityLevel::AgentInference,
        extraction_method: MemoryExtractionMethod::LlmExtracted,
        confidence: None,
        retrieval_hints: json!({}),
        observed_at: None,
        source: Some(ObjectProvenanceSource {
            source: ObjectRef::conversation_item(source_item.item_id),
            evidence_excerpt: None,
        }),
        participants: Vec::new(),
        subjects: Vec::new(),
        metadata: json!({}),
    }
}

fn object_scope_key(object_ref: &ObjectRef) -> String {
    format!(
        "{}:{}",
        object_ref.object_type.as_str(),
        object_ref.object_id.as_str()
    )
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
        .refresh_retrieval_policy_fingerprint(memory_id, ObjectRef::agent("agent:primary"), "test")
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
        .refresh_retrieval_policy_fingerprint(memory_id, ObjectRef::agent("agent:primary"), "test")
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
