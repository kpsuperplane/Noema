use super::helpers::CHAT_SOURCE_ID;
use super::models::{ChatMemorySource, NewChatMemoryCandidate, NewChatTurn};
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
        .refresh_retrieval_policy_fingerprint(memory_id, "agent:primary", "test")
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
        .refresh_retrieval_policy_fingerprint(memory_id, "agent:primary", "test")
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
