use std::path::Path;

use crate::{
    MemoryType,
    memory::consolidation::{
        CanonicalClaimCandidate, CanonicalClaimStatus, MemoryWriteProposal, MemoryWriteSourceKind,
        PredicateResolution,
    },
    memory::extraction::{
        MemoryExtractionSubject, MemoryExtractionSubjectKind, ValidatedMemoryProposal,
        infer_memory_text_sensitivity, memory_extraction_subject_implies_local_human,
    },
    memory::{MemoryStatus, Sensitivity},
    store::{
        ClaimStatus, EntityCandidate, EntityType, EvidenceAuthority, EvidenceCandidate,
        NewClaimCandidate, PredicateProposalCandidate,
    },
};
use serde_json::json;

use super::protocol::{TurnActivityStatus, TurnTranscriptItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CanonicalClaimConversionError {
    UnsupportedPredicateResolution,
    InvalidSubjectEntityMetadata,
    InvalidObjectEntityMetadata,
}

impl std::fmt::Display for CanonicalClaimConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPredicateResolution => {
                formatter.write_str("canonical claim candidate is not a promoted predicate")
            }
            Self::InvalidSubjectEntityMetadata => formatter
                .write_str("invalid canonical entity metadata for promoted predicate subject"),
            Self::InvalidObjectEntityMetadata => formatter
                .write_str("invalid canonical entity metadata for promoted predicate object"),
        }
    }
}

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
    let parsed = parse_explicit_claim(content);
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
    let write_proposal =
        provider_memory_write_proposal(validated, context, proposal_index, trigger);
    deterministic_canonical_claim(
        &write_proposal,
        claim_status_from_memory_status(validated.status),
        Some(f64::from(validated.proposal.confidence)),
        EvidenceAuthority::AgentInference,
    )
}

pub(super) fn provider_memory_write_proposal(
    validated: &ValidatedMemoryProposal,
    context: &ConversationMemoryContext,
    proposal_index: usize,
    trigger: &str,
) -> MemoryWriteProposal {
    let proposal = &validated.proposal;
    let evidence_source = evidence_source_for_excerpt(&proposal.evidence_excerpt, context);
    let canonical_subject = provider_subject_entity(&proposal.subjects, &proposal.evidence_excerpt);
    let risk_flags: Vec<String> = proposal
        .risk_flags
        .iter()
        .filter_map(stable_risk_flag_label)
        .collect();
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
        risk_flags: risk_flags.clone(),
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
            "validated_status": memory_status_label(validated.status),
            "risk_flags": risk_flags,
            "evidence_source": evidence_source.source.as_str(),
            "cwd_project_hint": project_scope_from_cwd(context.cwd.as_deref()),
            "canonical_subject": entity_candidate_metadata(&canonical_subject),
        }),
    }
}

pub(super) fn deterministic_canonical_claim(
    proposal: &MemoryWriteProposal,
    status: ClaimStatus,
    confidence: Option<f64>,
    authority: EvidenceAuthority,
) -> NewClaimCandidate {
    let parsed = match proposal.source_kind {
        MemoryWriteSourceKind::ExplicitRemember => parse_explicit_claim(&proposal.raw_text),
        _ => parse_provider_claim(&proposal.raw_text, None),
    };
    NewClaimCandidate {
        subject: canonical_subject_from_metadata(&proposal.metadata)
            .unwrap_or_else(EntityCandidate::local_human),
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
        retrieval_hints: if retrieval_hints_have_useful_content(&proposal.retrieval_hints) {
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

pub(super) fn new_claim_from_canonical(
    candidate: &CanonicalClaimCandidate,
    proposal: &MemoryWriteProposal,
    authority: EvidenceAuthority,
) -> Result<NewClaimCandidate, CanonicalClaimConversionError> {
    let PredicateResolution::PromotedPredicate { predicate_id } = &candidate.predicate else {
        return Err(CanonicalClaimConversionError::UnsupportedPredicateResolution);
    };
    let subject = canonical_entity_candidate(
        &candidate.subject.entity_id,
        &candidate.subject.entity_type,
        &candidate.subject.canonical_name,
    )
    .ok_or(CanonicalClaimConversionError::InvalidSubjectEntityMetadata)?;
    let object = canonical_entity_candidate(
        &candidate.object.entity_id,
        &candidate.object.entity_type,
        &candidate.object.canonical_name,
    )
    .ok_or(CanonicalClaimConversionError::InvalidObjectEntityMetadata)?;
    Ok(NewClaimCandidate {
        subject,
        object,
        predicate_id: predicate_id.clone(),
        fact: candidate.fact.clone(),
        sensitivity: candidate.sensitivity.max(proposal.sensitivity),
        status: bounded_canonical_claim_status(candidate.status, proposal),
        confidence: Some(candidate.confidence),
        evidence: EvidenceCandidate {
            source_item_id: proposal.source_item_id.clone(),
            authority,
            excerpt: Some(proposal.source_excerpt.clone()),
        },
        retrieval_hints: candidate.retrieval_hints.clone(),
        metadata: proposal.metadata.clone(),
    })
}

pub(super) fn predicate_proposal_candidate_from_canonical(
    candidate: &CanonicalClaimCandidate,
    proposal: &MemoryWriteProposal,
) -> Option<PredicateProposalCandidate> {
    let PredicateResolution::PredicateProposal {
        proposal: predicate,
    } = &candidate.predicate
    else {
        return None;
    };
    Some(PredicateProposalCandidate {
        label: predicate.label.clone(),
        description: predicate.description.clone(),
        proposed_predicate: serde_json::to_value(predicate).expect("predicate proposal value"),
        source_item_id: Some(proposal.source_item_id.clone()),
        proposed_claim: json!({
            "subject": candidate.subject,
            "object": candidate.object,
            "fact": candidate.fact,
            "sensitivity": canonical_sensitivity_label(candidate.sensitivity),
            "status": "candidate",
            "confidence": candidate.confidence,
            "retrieval_hints": candidate.retrieval_hints,
            "rationale": candidate.rationale,
        }),
    })
}

struct ParsedExplicitClaim {
    predicate_id: &'static str,
    object_phrase: String,
    fact: String,
}

fn parse_explicit_claim(content: &str) -> ParsedExplicitClaim {
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

fn entity_candidate_metadata(entity: &EntityCandidate) -> serde_json::Value {
    json!({
        "entity_id": entity.entity_id,
        "entity_type": entity_type_label(entity.entity_type),
        "canonical_name": entity.canonical_name,
    })
}

fn canonical_subject_from_metadata(metadata: &serde_json::Value) -> Option<EntityCandidate> {
    let subject = metadata.get("canonical_subject")?;
    let entity_id = non_empty_json_string(subject.get("entity_id")?)?;
    let entity_type = entity_type_from_label(non_empty_json_string(subject.get("entity_type")?)?)?;
    let canonical_name = non_empty_json_string(subject.get("canonical_name")?)?;
    Some(EntityCandidate {
        entity_id: entity_id.to_string(),
        entity_type,
        canonical_name: canonical_name.to_string(),
    })
}

fn non_empty_json_string(value: &serde_json::Value) -> Option<&str> {
    let value = value.as_str()?.trim();
    if value.is_empty() { None } else { Some(value) }
}

fn entity_type_label(entity_type: EntityType) -> &'static str {
    match entity_type {
        EntityType::Human => "human",
        EntityType::Agent => "agent",
        EntityType::Person => "person",
        EntityType::Organization => "organization",
        EntityType::Project => "project",
        EntityType::Workspace => "workspace",
        EntityType::Conversation => "conversation",
        EntityType::Document => "document",
        EntityType::Tool => "tool",
        EntityType::Place => "place",
        EntityType::Task => "task",
        EntityType::Goal => "goal",
        EntityType::Concept => "concept",
        EntityType::Other => "other",
    }
}

fn entity_type_from_label(value: &str) -> Option<EntityType> {
    match value {
        "human" => Some(EntityType::Human),
        "agent" => Some(EntityType::Agent),
        "person" => Some(EntityType::Person),
        "organization" => Some(EntityType::Organization),
        "project" => Some(EntityType::Project),
        "workspace" => Some(EntityType::Workspace),
        "conversation" => Some(EntityType::Conversation),
        "document" => Some(EntityType::Document),
        "tool" => Some(EntityType::Tool),
        "place" => Some(EntityType::Place),
        "task" => Some(EntityType::Task),
        "goal" => Some(EntityType::Goal),
        "concept" => Some(EntityType::Concept),
        "other" => Some(EntityType::Other),
        _ => None,
    }
}

fn retrieval_hints_have_useful_content(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(value) => !value.trim().is_empty(),
        serde_json::Value::Array(values) => values.iter().any(retrieval_hints_have_useful_content),
        serde_json::Value::Object(values) => {
            values.values().any(retrieval_hints_have_useful_content)
        }
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    }
}

fn stable_risk_flag_label(
    flag: &crate::memory::extraction::MemoryExtractionRiskFlag,
) -> Option<String> {
    serde_json::to_value(flag)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
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

fn canonical_entity_candidate(
    entity_id: &str,
    entity_type: &str,
    canonical_name: &str,
) -> Option<EntityCandidate> {
    if entity_id == "human:local" {
        return (canonical_entity_type(entity_type)? == EntityType::Human)
            .then(EntityCandidate::local_human);
    }

    let entity_type = canonical_entity_type(entity_type)?;
    if stable_entity_prefix_type(entity_id).is_some_and(|expected| expected != entity_type) {
        return None;
    }

    Some(EntityCandidate {
        entity_id: entity_id.to_string(),
        entity_type,
        canonical_name: canonical_name.to_string(),
    })
}

fn canonical_entity_type(entity_type: &str) -> Option<EntityType> {
    entity_type_from_label(&entity_type.trim().to_ascii_lowercase())
}

fn stable_entity_prefix_type(entity_id: &str) -> Option<EntityType> {
    let (prefix, _) = entity_id.split_once(':')?;
    match prefix {
        "human" => Some(EntityType::Human),
        "agent" => Some(EntityType::Agent),
        "person" => Some(EntityType::Person),
        "organization" => Some(EntityType::Organization),
        "project" => Some(EntityType::Project),
        "workspace" => Some(EntityType::Workspace),
        "conversation" => Some(EntityType::Conversation),
        "document" => Some(EntityType::Document),
        "tool" => Some(EntityType::Tool),
        "place" => Some(EntityType::Place),
        "task" => Some(EntityType::Task),
        "goal" => Some(EntityType::Goal),
        "concept" => Some(EntityType::Concept),
        "other" => Some(EntityType::Other),
        _ => None,
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

fn claim_status_from_canonical(status: CanonicalClaimStatus) -> ClaimStatus {
    match status {
        CanonicalClaimStatus::Candidate => ClaimStatus::Candidate,
        CanonicalClaimStatus::Active => ClaimStatus::Active,
        CanonicalClaimStatus::Confirmed => ClaimStatus::Confirmed,
    }
}

fn bounded_canonical_claim_status(
    status: CanonicalClaimStatus,
    proposal: &MemoryWriteProposal,
) -> ClaimStatus {
    let canonical = claim_status_from_canonical(status);
    let Some(validated) = proposal.metadata["validated_status"]
        .as_str()
        .and_then(memory_status_from_label)
        .map(claim_status_from_memory_status)
    else {
        return canonical;
    };
    if claim_status_rank(canonical) <= claim_status_rank(validated) {
        canonical
    } else {
        validated
    }
}

const fn claim_status_rank(status: ClaimStatus) -> u8 {
    match status {
        ClaimStatus::Candidate => 0,
        ClaimStatus::Active => 1,
        ClaimStatus::Confirmed => 2,
        ClaimStatus::Disputed
        | ClaimStatus::Superseded
        | ClaimStatus::Archived
        | ClaimStatus::Deleted => 0,
    }
}

fn memory_status_from_label(label: &str) -> Option<MemoryStatus> {
    match label {
        "candidate" => Some(MemoryStatus::Candidate),
        "active" => Some(MemoryStatus::Active),
        "confirmed" => Some(MemoryStatus::Confirmed),
        "inferred" => Some(MemoryStatus::Inferred),
        "stale" => Some(MemoryStatus::Stale),
        "superseded" => Some(MemoryStatus::Superseded),
        "archived" => Some(MemoryStatus::Archived),
        "deleted" => Some(MemoryStatus::Deleted),
        "disputed" => Some(MemoryStatus::Disputed),
        _ => None,
    }
}

fn memory_status_label(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "candidate",
        MemoryStatus::Active => "active",
        MemoryStatus::Confirmed => "confirmed",
        MemoryStatus::Inferred => "inferred",
        MemoryStatus::Stale => "stale",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Archived => "archived",
        MemoryStatus::Deleted => "deleted",
        MemoryStatus::Disputed => "disputed",
    }
}

fn canonical_sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
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
