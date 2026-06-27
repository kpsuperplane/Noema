use std::path::Path;

use crate::{
    memory::{ParticipantRole, Sensitivity, SubjectRole},
    memory_extraction::{
        MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
        ValidatedMemoryProposal,
    },
    memory_persistence::{
        ActorRef, MemoryAuthorityLevel, MemoryExtractionMethod, MemoryType, NewMemoryCandidate,
        NewMemoryParticipant, NewMemorySubject, ObjectProvenanceSource, ObjectRef,
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
    pub user_content: String,
    pub assistant_content: String,
    pub cwd: Option<String>,
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
    let lowered = content.to_ascii_lowercase();
    if contains_any(
        &lowered,
        &[
            "api key",
            "access key",
            "access token",
            "auth token",
            "bearer token",
            "client secret",
            "password",
            "passphrase",
            "private key",
            "secret key",
            "ssh key",
            "ssn",
            "social security number",
            "recovery code",
        ],
    ) || looks_like_secret_token(content)
    {
        return Sensitivity::Secret;
    }

    if contains_any(
        &lowered,
        &[
            "bank account",
            "compensation",
            "credit card",
            "diagnosed",
            "diagnosis",
            "doctor",
            "driver license",
            "health insurance",
            "lawyer",
            "legal matter",
            "medical",
            "medication",
            "passport",
            "routing number",
            "salary",
            "tax return",
            "therapist",
            "therapy",
        ],
    ) {
        return Sensitivity::Sensitive;
    }

    Sensitivity::Normal
}

pub(super) fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub(super) fn looks_like_secret_token(content: &str) -> bool {
    content.split_whitespace().any(|token| {
        let token =
            token.trim_matches(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')));
        let lowered = token.to_ascii_lowercase();
        lowered.starts_with("sk-")
            || lowered.starts_with("ghp_")
            || lowered.starts_with("xoxb-")
            || (lowered.starts_with("akia") && lowered.len() >= 16)
    })
}

pub(super) fn title_from_memory_content(content: &str) -> String {
    content.trim().chars().take(80).collect()
}

pub(super) fn extracted_proposal_to_candidate(
    validated: &ValidatedMemoryProposal,
    context: &ConversationMemoryContext,
    project_scope_id: Option<&str>,
    user_input: &str,
    trigger: &str,
) -> Result<NewMemoryCandidate, String> {
    let proposal = &validated.proposal;
    let owner = owner_for_extracted_proposal(proposal, context, project_scope_id)?;
    let mut candidate = NewMemoryCandidate::confirmed_note(
        owner,
        proposal.content.clone(),
        ActorRef::agent("agent:primary"),
        source_item_ref_for_evidence(&proposal.evidence_excerpt, user_input, context),
    );
    candidate.memory_type = proposal.memory_type;
    candidate.title = proposal.title.clone();
    candidate.sensitivity = proposal.sensitivity;
    candidate.status = validated.status;
    candidate.confidence = Some(f64::from(proposal.confidence));
    candidate.retrieval_hints =
        serde_json::to_value(&proposal.retrieval_hints).map_err(|error| error.to_string())?;
    candidate.owner_actor = Some(ActorRef::human("human:local"));
    candidate.authority_level = MemoryAuthorityLevel::AgentInference;
    candidate.extraction_method = MemoryExtractionMethod::LlmExtracted;
    candidate.source = Some(ObjectProvenanceSource {
        source: source_item_ref_for_evidence(&proposal.evidence_excerpt, user_input, context),
        evidence_excerpt: Some(proposal.evidence_excerpt.clone()),
    });
    candidate.participants = vec![
        NewMemoryParticipant::new("human:local", ParticipantRole::HumanInScope),
        NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
    ];
    candidate.subjects = proposal
        .subjects
        .iter()
        .map(memory_extraction_subject_to_persistence)
        .collect();
    candidate.metadata = json!({
        "trigger": trigger,
        "turn_id": context.turn_id,
        "turn_index": context.turn_index,
        "risk_flags": proposal.risk_flags,
    });

    Ok(candidate)
}

pub(super) fn owner_for_extracted_proposal(
    proposal: &crate::memory_extraction::ExtractorMemoryProposal,
    context: &ConversationMemoryContext,
    _project_scope_id: Option<&str>,
) -> Result<ObjectRef, String> {
    if proposal
        .subjects
        .iter()
        .any(memory_extraction_subject_is_local_human)
        && matches!(
            proposal.memory_type,
            MemoryType::Fact | MemoryType::Preference
        )
    {
        return Ok(ObjectRef::human("human:local"));
    }

    ObjectRef::new(
        crate::memory_persistence::ObjectType::Conversation,
        context.conversation_id.clone(),
    )
    .map_err(|error| error.to_string())
}

pub(super) fn source_item_ref_for_evidence(
    evidence_excerpt: &str,
    user_input: &str,
    context: &ConversationMemoryContext,
) -> ObjectRef {
    if user_input.contains(evidence_excerpt) {
        ObjectRef::conversation_item(context.user_item_id.clone())
    } else {
        ObjectRef::conversation_item(
            context
                .assistant_item_id
                .clone()
                .unwrap_or_else(|| context.user_item_id.clone()),
        )
    }
}

pub(super) fn memory_extraction_subject_to_persistence(
    subject: &MemoryExtractionSubject,
) -> NewMemorySubject {
    let entity_id = subject
        .id
        .clone()
        .unwrap_or_else(|| generated_entity_id(subject.kind, &subject.name));
    let mut stored = NewMemorySubject::new(
        entity_id,
        subject_kind_to_entity_type(subject.kind),
        subject.name.clone(),
        subject_role_to_memory_role(subject.role),
    );
    if memory_extraction_subject_is_local_human(subject) {
        stored.linked_object = Some(ObjectRef::human("human:local"));
    }
    stored
}

pub(super) fn memory_extraction_subject_is_local_human(subject: &MemoryExtractionSubject) -> bool {
    subject.id.as_deref().is_some_and(|id| id == "human:local")
        || matches!(
            subject.name.trim().to_ascii_lowercase().as_str(),
            "current human" | "user" | "me"
        )
}

pub(super) fn generated_entity_id(kind: MemoryExtractionSubjectKind, name: &str) -> String {
    format!(
        "{}:{}",
        subject_kind_to_entity_type(kind),
        slug_fragment(name)
    )
}

pub(super) fn subject_kind_to_entity_type(kind: MemoryExtractionSubjectKind) -> &'static str {
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

pub(super) fn subject_role_to_memory_role(role: MemoryExtractionSubjectRole) -> SubjectRole {
    match role {
        MemoryExtractionSubjectRole::About | MemoryExtractionSubjectRole::Participant => {
            SubjectRole::About
        }
        MemoryExtractionSubjectRole::Owner => SubjectRole::Owner,
        MemoryExtractionSubjectRole::Affected => SubjectRole::Affected,
        MemoryExtractionSubjectRole::Assignee => SubjectRole::Assignee,
        MemoryExtractionSubjectRole::Source => SubjectRole::Source,
        MemoryExtractionSubjectRole::Target => SubjectRole::Target,
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
