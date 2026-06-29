use std::path::Path;

use crate::{
    MemoryType,
    memory::{MemoryStatus, Sensitivity},
    memory_consolidation::{MemoryWriteProposal, MemoryWriteSourceKind},
    memory_extraction::{
        MemoryExtractionSubject, MemoryExtractionSubjectKind, ValidatedMemoryProposal,
        infer_memory_text_sensitivity, memory_extraction_subject_implies_local_human,
    },
    store::{
        ClaimStatus, EntityCandidate, EntityType, EvidenceAuthority, EvidenceCandidate,
        NewClaimCandidate,
    },
};
use serde_json::json;

use super::protocol::{TurnActivityStatus, TurnTranscriptItem};

#[derive(Debug, Clone)]
pub(super) struct ConversationMemoryContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub turn_index: u64,
    pub user_item_id: String,
    pub assistant_item_id: Option<String>,
    pub assistant_items: Vec<AssistantEvidenceItem>,
    pub user_content: String,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct AssistantEvidenceItem {
    pub item_id: String,
    pub text: String,
}

pub(super) fn explicit_memory_content(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let lowered = trimmed.to_ascii_lowercase();
    for prefix in ["remember this:", "remember that:", "remember:"] {
        if lowered.starts_with(prefix) {
            let content = trimmed[prefix.len()..].trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }
    if lowered.starts_with("/remember ") {
        let content = trimmed["/remember ".len()..].trim();
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    if lowered.starts_with("/remember:") {
        let content = trimmed["/remember:".len()..].trim();
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    None
}

#[allow(
    dead_code,
    reason = "legacy daemon tests exercise this wrapper while runtime writes use proposal routing"
)]
pub(super) fn explicit_memory_claim_candidate(
    content: &str,
    source_item_id: String,
) -> NewClaimCandidate {
    let context = ConversationMemoryContext {
        conversation_id: String::new(),
        turn_id: String::new(),
        turn_index: 0,
        user_item_id: source_item_id,
        assistant_item_id: None,
        assistant_items: Vec::new(),
        user_content: content.to_string(),
        cwd: None,
    };
    let proposal = explicit_memory_write_proposal(content, &context);
    deterministic_canonical_claim(
        &proposal,
        ClaimStatus::Confirmed,
        Some(1.0),
        EvidenceAuthority::ExplicitHumanStatement,
    )
}

pub(super) fn explicit_memory_write_proposal(
    content: &str,
    context: &ConversationMemoryContext,
) -> MemoryWriteProposal {
    let parsed = parse_provider_claim(content, None);
    MemoryWriteProposal {
        source_kind: MemoryWriteSourceKind::ExplicitRemember,
        source_item_id: context.user_item_id.clone(),
        source_actor_id: "human:local".to_string(),
        source_excerpt: content.to_string(),
        owner_object_type: "human".to_string(),
        owner_object_id: "human:local".to_string(),
        raw_text: content.to_string(),
        memory_type: "note".to_string(),
        sensitivity: infer_chat_sensitivity(content),
        risk_flags: Vec::new(),
        retrieval_hints: json!({
            "keywords": [parsed.object_phrase],
            "summary": parsed.fact,
            "source": "explicit_remember",
        }),
        metadata: json!({
            "trigger": "explicit_remember",
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
        }),
    }
}

#[allow(
    dead_code,
    reason = "legacy daemon tests exercise this wrapper while runtime writes use proposal routing"
)]
pub(super) fn provider_memory_claim_candidate(
    validated: &ValidatedMemoryProposal,
    context: &ConversationMemoryContext,
    proposal_index: usize,
    trigger: &str,
) -> NewClaimCandidate {
    let proposal = &validated.proposal;
    let parsed = parse_provider_claim(&proposal.content, proposal.title.as_deref());
    let evidence_source = evidence_source_for_excerpt(&proposal.evidence_excerpt, context);
    let source_item_id = evidence_source.source_item_id.clone();

    NewClaimCandidate {
        subject: provider_subject_entity(&proposal.subjects, &proposal.evidence_excerpt),
        object: claim_object_entity(parsed.predicate_id, &parsed.object_phrase),
        predicate_id: parsed.predicate_id.to_string(),
        fact: parsed.fact,
        sensitivity: proposal.sensitivity,
        status: claim_status_from_memory_status(validated.status),
        confidence: Some(f64::from(proposal.confidence)),
        evidence: EvidenceCandidate {
            source_item_id,
            authority: EvidenceAuthority::AgentInference,
            excerpt: Some(proposal.evidence_excerpt.clone()),
        },
        retrieval_hints: serde_json::to_value(&proposal.retrieval_hints).unwrap_or_else(|_| {
            json!({
                "keywords": [parsed.object_phrase],
                "summary": proposal.content,
            })
        }),
        metadata: json!({
            "trigger": trigger,
            "source": "provider_structured_output",
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
            "proposal_index": proposal_index,
            "memory_type": memory_type_label(proposal.memory_type),
            "title": proposal.title,
            "risk_flags": proposal.risk_flags,
            "evidence_source": evidence_source.source.as_str(),
            "cwd_project_hint": project_scope_from_cwd(context.cwd.as_deref()),
        }),
    }
}

pub(super) fn provider_memory_write_proposal(
    validated: &ValidatedMemoryProposal,
    context: &ConversationMemoryContext,
    proposal_index: usize,
    trigger: &str,
) -> MemoryWriteProposal {
    let proposal = &validated.proposal;
    let evidence_source = evidence_source_for_excerpt(&proposal.evidence_excerpt, context);
    MemoryWriteProposal {
        source_kind: MemoryWriteSourceKind::OrdinaryChat,
        source_item_id: evidence_source.source_item_id,
        source_actor_id: match evidence_source.source {
            ProviderEvidenceSource::User => "human:local",
            ProviderEvidenceSource::Assistant => "agent:primary",
        }
        .to_string(),
        source_excerpt: proposal.evidence_excerpt.clone(),
        owner_object_type: "human".to_string(),
        owner_object_id: "human:local".to_string(),
        raw_text: proposal.content.clone(),
        memory_type: memory_type_label(proposal.memory_type).to_string(),
        sensitivity: proposal.sensitivity,
        risk_flags: proposal
            .risk_flags
            .iter()
            .map(|flag| format!("{flag:?}"))
            .collect(),
        retrieval_hints: serde_json::to_value(&proposal.retrieval_hints)
            .unwrap_or_else(|_| json!({})),
        metadata: json!({
            "trigger": trigger,
            "source": "provider_structured_output",
            "turn_id": context.turn_id,
            "turn_index": context.turn_index,
            "proposal_index": proposal_index,
            "memory_type": memory_type_label(proposal.memory_type),
            "title": proposal.title,
            "risk_flags": proposal.risk_flags,
            "evidence_source": evidence_source.source.as_str(),
            "cwd_project_hint": project_scope_from_cwd(context.cwd.as_deref()),
        }),
    }
}

pub(super) fn deterministic_canonical_claim(
    proposal: &MemoryWriteProposal,
    status: ClaimStatus,
    confidence: Option<f64>,
    authority: EvidenceAuthority,
) -> NewClaimCandidate {
    let parsed = parse_provider_claim(&proposal.raw_text, None);
    NewClaimCandidate {
        subject: EntityCandidate::local_human(),
        object: claim_object_entity(parsed.predicate_id, &parsed.object_phrase),
        predicate_id: parsed.predicate_id.to_string(),
        fact: parsed.fact.clone(),
        sensitivity: proposal.sensitivity,
        status,
        confidence,
        evidence: EvidenceCandidate {
            source_item_id: proposal.source_item_id.clone(),
            authority,
            excerpt: Some(proposal.source_excerpt.clone()),
        },
        retrieval_hints: if proposal.retrieval_hints.is_object() {
            proposal.retrieval_hints.clone()
        } else {
            json!({
                "keywords": [parsed.object_phrase],
                "summary": parsed.fact,
            })
        },
        metadata: proposal.metadata.clone(),
    }
}

struct ParsedExplicitClaim {
    predicate_id: &'static str,
    object_phrase: String,
    fact: String,
}

fn parse_provider_claim(content: &str, _title: Option<&str>) -> ParsedExplicitClaim {
    let normalized = collapse_whitespace(content);
    let lowered = normalized.to_ascii_lowercase();

    for prefix in [
        "i like ",
        "i love ",
        "i'm a big fan of ",
        "i am a big fan of ",
        "kevin likes ",
        "kevin loves ",
        "kevin is a big fan of ",
        "local human likes ",
        "local human loves ",
        "the user likes ",
        "the user loves ",
        "user likes ",
        "user loves ",
        "current human likes ",
        "current human loves ",
        "current user likes ",
        "current user loves ",
    ] {
        if lowered.starts_with(prefix) {
            let object_phrase = normalize_object_phrase(&normalized[prefix.len()..]);
            if is_substantive_object_phrase(&object_phrase) {
                return ParsedExplicitClaim {
                    predicate_id: "likes",
                    fact: format!("Kevin likes {}.", object_phrase),
                    object_phrase,
                };
            }
        }
    }

    for prefix in [
        "i dislike ",
        "i don't like ",
        "i do not like ",
        "i hate ",
        "kevin dislikes ",
        "kevin doesn't like ",
        "kevin does not like ",
        "kevin hates ",
        "local human dislikes ",
        "local human doesn't like ",
        "local human does not like ",
        "local human hates ",
        "the user dislikes ",
        "the user doesn't like ",
        "the user does not like ",
        "the user hates ",
        "user dislikes ",
        "user doesn't like ",
        "user does not like ",
        "user hates ",
        "current human dislikes ",
        "current human doesn't like ",
        "current human does not like ",
        "current human hates ",
        "current user dislikes ",
        "current user doesn't like ",
        "current user does not like ",
        "current user hates ",
    ] {
        if lowered.starts_with(prefix) {
            let object_phrase = normalize_object_phrase(&normalized[prefix.len()..]);
            if is_substantive_object_phrase(&object_phrase) {
                return ParsedExplicitClaim {
                    predicate_id: "dislikes",
                    fact: format!("Kevin dislikes {}.", object_phrase),
                    object_phrase,
                };
            }
        }
    }

    for prefix in [
        "i prefer ",
        "i want ",
        "kevin prefers ",
        "kevin wants ",
        "local human prefers ",
        "local human wants ",
        "the user prefers ",
        "the user wants ",
        "user prefers ",
        "user wants ",
        "current human prefers ",
        "current human wants ",
        "current user prefers ",
        "current user wants ",
    ] {
        if lowered.starts_with(prefix) {
            let object_phrase = normalize_object_phrase(&normalized[prefix.len()..]);
            if is_substantive_object_phrase(&object_phrase) {
                return ParsedExplicitClaim {
                    predicate_id: "prefers",
                    fact: format!("Kevin prefers {}.", object_phrase),
                    object_phrase,
                };
            }
        }
    }

    fallback_note_claim(&normalized)
}

fn fallback_note_claim(normalized: &str) -> ParsedExplicitClaim {
    ParsedExplicitClaim {
        predicate_id: "has_note",
        object_phrase: normalize_note_object_phrase(normalized),
        fact: ensure_final_punctuation(normalized),
    }
}

fn normalize_note_object_phrase(value: &str) -> String {
    let normalized = collapse_whitespace(value);
    let terminal_punctuation_count = normalized
        .chars()
        .rev()
        .take_while(|ch| matches!(ch, '.' | '!' | '?'))
        .count();

    if terminal_punctuation_count == 1 {
        let without_terminal = normalized.trim_end_matches(['.', '!', '?']).trim_end();
        if !without_terminal.is_empty() {
            return without_terminal.to_string();
        }
    }

    normalized
}

fn claim_object_entity(predicate_id: &str, object_phrase: &str) -> EntityCandidate {
    let normalized_object = object_phrase
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if predicate_id == "has_note" {
        let fingerprint = stable_hex_fingerprint(normalized_object.as_bytes());
        return EntityCandidate::concept(
            &format!("claim_object:{predicate_id}:note:v1:{fingerprint}"),
            object_phrase,
        );
    }

    EntityCandidate::concept(
        &format!("claim_object:{predicate_id}:{normalized_object}"),
        object_phrase,
    )
}

#[allow(
    dead_code,
    reason = "legacy provider candidate wrapper preserves subject behavior for daemon tests"
)]
fn provider_subject_entity(
    subjects: &[MemoryExtractionSubject],
    evidence_excerpt: &str,
) -> EntityCandidate {
    let Some(subject) = subjects.first() else {
        return EntityCandidate::local_human();
    };
    if memory_extraction_subject_is_local_human(subject, evidence_excerpt) {
        return EntityCandidate::local_human();
    }

    let entity_type = entity_type_for_subject_kind(subject.kind);
    let entity_id = subject
        .id
        .clone()
        .unwrap_or_else(|| generated_entity_id(subject.kind, &subject.name));
    EntityCandidate {
        entity_id,
        entity_type,
        canonical_name: subject.name.clone(),
    }
}

#[allow(
    dead_code,
    reason = "legacy provider candidate wrapper preserves subject behavior for daemon tests"
)]
fn entity_type_for_subject_kind(kind: MemoryExtractionSubjectKind) -> EntityType {
    match kind {
        MemoryExtractionSubjectKind::Human => EntityType::Person,
        MemoryExtractionSubjectKind::Agent => EntityType::Agent,
        MemoryExtractionSubjectKind::Conversation => EntityType::Conversation,
        MemoryExtractionSubjectKind::Workspace => EntityType::Workspace,
        MemoryExtractionSubjectKind::Project => EntityType::Project,
        MemoryExtractionSubjectKind::Task => EntityType::Task,
        MemoryExtractionSubjectKind::Cron => EntityType::Other,
        MemoryExtractionSubjectKind::Relationship => EntityType::Other,
        MemoryExtractionSubjectKind::Tool => EntityType::Tool,
        MemoryExtractionSubjectKind::Organization => EntityType::Organization,
        MemoryExtractionSubjectKind::Place => EntityType::Place,
        MemoryExtractionSubjectKind::Concept => EntityType::Concept,
        MemoryExtractionSubjectKind::Other => EntityType::Other,
    }
}

pub(super) fn claim_status_from_memory_status(status: MemoryStatus) -> ClaimStatus {
    match status {
        MemoryStatus::Candidate => ClaimStatus::Candidate,
        MemoryStatus::Active | MemoryStatus::Inferred => ClaimStatus::Active,
        MemoryStatus::Confirmed => ClaimStatus::Confirmed,
        MemoryStatus::Disputed => ClaimStatus::Disputed,
        MemoryStatus::Superseded => ClaimStatus::Superseded,
        MemoryStatus::Stale | MemoryStatus::Archived => ClaimStatus::Archived,
        MemoryStatus::Deleted => ClaimStatus::Deleted,
    }
}

#[derive(Debug, Clone, Copy)]
enum ProviderEvidenceSource {
    User,
    Assistant,
}

impl ProviderEvidenceSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug)]
struct ProviderEvidenceSelection {
    source: ProviderEvidenceSource,
    source_item_id: String,
}

fn evidence_source_for_excerpt(
    evidence_excerpt: &str,
    context: &ConversationMemoryContext,
) -> ProviderEvidenceSelection {
    if context.user_content.contains(evidence_excerpt) {
        ProviderEvidenceSelection {
            source: ProviderEvidenceSource::User,
            source_item_id: context.user_item_id.clone(),
        }
    } else {
        let source_item_id = context
            .assistant_items
            .iter()
            .find(|item| item.text.contains(evidence_excerpt))
            .map(|item| item.item_id.clone())
            .or_else(|| context.assistant_item_id.clone())
            .unwrap_or_else(|| context.user_item_id.clone());
        ProviderEvidenceSelection {
            source: ProviderEvidenceSource::Assistant,
            source_item_id,
        }
    }
}

fn memory_type_label(memory_type: MemoryType) -> &'static str {
    match memory_type {
        MemoryType::Fact => "fact",
        MemoryType::Preference => "preference",
        MemoryType::Person => "person",
        MemoryType::Organization => "organization",
        MemoryType::Project => "project",
        MemoryType::Place => "place",
        MemoryType::Routine => "routine",
        MemoryType::Goal => "goal",
        MemoryType::OpenLoop => "open_loop",
        MemoryType::Procedure => "procedure",
        MemoryType::Constraint => "constraint",
        MemoryType::Trigger => "trigger",
        MemoryType::Decision => "decision",
        MemoryType::Skill => "skill",
        MemoryType::Policy => "policy",
        MemoryType::Note => "note",
        MemoryType::Other => "other",
    }
}

fn collapse_whitespace(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_object_phrase(value: &str) -> String {
    let phrase = value
        .trim()
        .trim_matches(|ch: char| matches!(ch, '.' | ',' | ';' | ':' | '!' | '?'))
        .trim();
    if phrase.chars().any(|ch| ch.is_ascii_alphabetic())
        && phrase
            .chars()
            .filter(|ch| ch.is_ascii_alphabetic())
            .all(|ch| ch.is_ascii_uppercase())
    {
        phrase.to_ascii_lowercase()
    } else {
        phrase.to_string()
    }
}

fn is_substantive_object_phrase(value: &str) -> bool {
    value.chars().any(char::is_alphanumeric)
}

fn ensure_final_punctuation(value: &str) -> String {
    let value = value.trim();
    if value.ends_with(['.', '!', '?']) {
        value.to_string()
    } else {
        format!("{value}.")
    }
}

#[expect(
    dead_code,
    reason = "graph-claim memory write bridge keeps classification helpers staged for the next slice"
)]
pub(super) fn infer_chat_memory_type(content: &str) -> MemoryType {
    let lowered = content.to_ascii_lowercase();
    if lowered.contains("prefer") || lowered.contains("preference") {
        MemoryType::Preference
    } else if lowered.contains("decided") || lowered.contains("decision") {
        MemoryType::Decision
    } else {
        MemoryType::Note
    }
}

pub(super) fn infer_chat_sensitivity(content: &str) -> Sensitivity {
    infer_memory_text_sensitivity([content])
}

#[expect(
    dead_code,
    reason = "graph-claim memory write bridge keeps title derivation staged for the next slice"
)]
pub(super) fn title_from_memory_content(content: &str) -> String {
    content.trim().chars().take(80).collect()
}

#[allow(
    dead_code,
    reason = "legacy provider candidate wrapper preserves subject behavior for daemon tests"
)]
pub(super) fn memory_extraction_subject_is_local_human(
    subject: &MemoryExtractionSubject,
    evidence_excerpt: &str,
) -> bool {
    memory_extraction_subject_implies_local_human(subject, evidence_excerpt)
}

fn stable_hex_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[allow(
    dead_code,
    reason = "legacy provider candidate wrapper preserves subject behavior for daemon tests"
)]
pub(super) fn generated_entity_id(kind: MemoryExtractionSubjectKind, name: &str) -> String {
    format!("{}:{}", subject_kind_id_prefix(kind), slug_fragment(name))
}

#[allow(
    dead_code,
    reason = "legacy provider candidate wrapper preserves subject behavior for daemon tests"
)]
fn subject_kind_id_prefix(kind: MemoryExtractionSubjectKind) -> &'static str {
    match kind {
        MemoryExtractionSubjectKind::Human => "human",
        MemoryExtractionSubjectKind::Agent => "agent",
        MemoryExtractionSubjectKind::Conversation => "conversation",
        MemoryExtractionSubjectKind::Workspace => "workspace",
        MemoryExtractionSubjectKind::Project => "project",
        MemoryExtractionSubjectKind::Task => "task",
        MemoryExtractionSubjectKind::Cron => "other",
        MemoryExtractionSubjectKind::Relationship => "other",
        MemoryExtractionSubjectKind::Tool => "tool",
        MemoryExtractionSubjectKind::Organization => "organization",
        MemoryExtractionSubjectKind::Place => "place",
        MemoryExtractionSubjectKind::Concept => "concept",
        MemoryExtractionSubjectKind::Other => "other",
    }
}

pub(super) fn project_scope_from_cwd(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?.trim();
    if cwd.is_empty() {
        return None;
    }

    let path = Path::new(cwd);
    if !path.join(".git").exists() {
        return None;
    }

    let name = path.file_name()?.to_string_lossy();
    Some(format!("project:{}", slug_fragment(&name)))
}

pub(super) fn slug_fragment(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}

pub(super) fn memory_activity(
    id: &str,
    status: TurnActivityStatus,
    title: &str,
    summary: Option<&str>,
    metadata: serde_json::Value,
) -> TurnTranscriptItem {
    typed_memory_activity(id, "memory_extraction", status, title, summary, metadata)
}

pub(super) fn typed_memory_activity(
    id: &str,
    activity_kind: &str,
    status: TurnActivityStatus,
    title: &str,
    summary: Option<&str>,
    metadata: serde_json::Value,
) -> TurnTranscriptItem {
    TurnTranscriptItem::Activity {
        id: id.to_string(),
        activity_kind: activity_kind.to_string(),
        status,
        title: title.to_string(),
        summary: summary.map(ToOwned::to_owned),
        metadata,
    }
}

pub(super) fn memory_activity_failed(id: &str, message: String) -> TurnTranscriptItem {
    memory_activity(
        id,
        TurnActivityStatus::Failed,
        "Memory extraction failed",
        Some(&message),
        json!({}),
    )
}
