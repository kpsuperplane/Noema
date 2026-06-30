//! Pure ordinary-chat memory extraction prompt and proposal validation.
//!
//! This module is intentionally side-effect free. It builds the extractor
//! prompt, parses the model's JSON proposal response, validates provenance and
//! risk boundaries, and makes the deterministic initial status decision.

use super::{MemoryStatus, Sensitivity, types::MemoryType};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Minimum extractor confidence accepted for an ordinary-chat memory proposal.
pub const MIN_MEMORY_EXTRACTION_CONFIDENCE: f32 = 0.70;

/// Top-level JSON object returned by the ordinary-chat memory extractor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractorMemoryResponse {
    /// Proposed memories extracted from the chat turn.
    pub proposals: Vec<ExtractorMemoryProposal>,
}

/// A single memory proposal emitted by the extractor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractorMemoryProposal {
    /// Durable memory content proposed by the model.
    pub content: String,
    /// Memory type from Noema's closed memory type vocabulary.
    #[serde(with = "memory_type_json")]
    pub memory_type: MemoryType,
    /// Optional short display title.
    pub title: Option<String>,
    /// Extractor confidence in the inclusive range 0.0 through 1.0.
    pub confidence: f32,
    /// Proposed sensitivity tier.
    #[serde(with = "sensitivity_json")]
    pub sensitivity: Sensitivity,
    /// Subject entities the memory is about or otherwise bound to.
    pub subjects: Vec<MemoryExtractionSubject>,
    /// Non-authoritative search and ranking hints.
    pub retrieval_hints: MemoryExtractionRetrievalHints,
    /// Review risk flags. Low-risk proposals should use an empty array.
    pub risk_flags: Vec<MemoryExtractionRiskFlag>,
    /// Exact quote from the user or assistant message supporting the proposal.
    pub evidence_excerpt: String,
}

/// A validated extractor proposal with its deterministic initial status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatedMemoryProposal {
    /// The validated proposal payload.
    pub proposal: ExtractorMemoryProposal,
    /// Initial status selected by deterministic promotion policy.
    #[serde(with = "memory_status_json")]
    pub status: MemoryStatus,
}

/// One proposal accepted while partitioning a structured extractor response.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedMemoryExtractionProposal {
    /// Zero-based index in the original extractor proposal batch.
    pub proposal_index: usize,
    /// The validated proposal payload.
    pub proposal: ValidatedMemoryProposal,
}

/// One proposal rejected while partitioning a structured extractor response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedMemoryExtractionProposal {
    /// Zero-based index in the original extractor proposal batch.
    pub proposal_index: usize,
    /// Human-readable validation failure.
    pub error: String,
}

/// Accepted and rejected proposals from a structured extractor response.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryExtractionValidationPartition {
    /// Valid proposals that may continue to canonicalization and persistence.
    pub accepted: Vec<AcceptedMemoryExtractionProposal>,
    /// Invalid proposals discarded before persistence.
    pub rejected: Vec<RejectedMemoryExtractionProposal>,
}

/// Subject entity attached to a memory proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryExtractionSubject {
    /// Optional canonical entity id when the extractor has one.
    pub id: Option<String>,
    /// Broad subject kind from the extractor vocabulary.
    pub kind: MemoryExtractionSubjectKind,
    /// Human-readable subject name.
    pub name: String,
    /// Subject role with respect to the proposed memory.
    pub role: MemoryExtractionSubjectRole,
}

impl MemoryExtractionSubject {
    fn implies_human(&self) -> bool {
        if self.kind == MemoryExtractionSubjectKind::Human {
            return true;
        }

        self.id
            .as_deref()
            .is_some_and(|id| id.trim().starts_with("human:"))
            || matches!(
                self.name.trim().to_ascii_lowercase().as_str(),
                "human" | "current human" | "local human" | "user" | "me"
            )
    }

    fn explicitly_implies_local_human(&self) -> bool {
        self.id.as_deref().is_some_and(|id| id == "human:local")
            || matches!(
                self.name.trim().to_ascii_lowercase().as_str(),
                "current human" | "local human" | "user" | "me"
            )
    }

    fn implies_local_human_for_evidence(&self, evidence_excerpt: &str) -> bool {
        self.explicitly_implies_local_human()
            || (self.kind == MemoryExtractionSubjectKind::Human
                && self.name.trim().eq_ignore_ascii_case("kevin")
                && !evidence_mentions_subject_name(evidence_excerpt, &self.name)
                && !has_third_party_human_signal(evidence_excerpt))
    }

    fn implies_third_party_human_for_evidence(&self, evidence_excerpt: &str) -> bool {
        self.kind == MemoryExtractionSubjectKind::Human
            && !self.implies_local_human_for_evidence(evidence_excerpt)
    }
}

pub(crate) fn memory_extraction_subject_implies_local_human(
    subject: &MemoryExtractionSubject,
    evidence_excerpt: &str,
) -> bool {
    subject.implies_local_human_for_evidence(evidence_excerpt)
}

/// Broad kind for a proposal subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryExtractionSubjectKind {
    /// Human subject.
    Human,
    /// Agent subject.
    Agent,
    /// Conversation subject.
    Conversation,
    /// Workspace subject.
    Workspace,
    /// Project subject.
    Project,
    /// Task subject.
    Task,
    /// Cron or schedule subject.
    Cron,
    /// Relationship subject.
    Relationship,
    /// Tool subject.
    Tool,
    /// Organization subject.
    Organization,
    /// Place subject.
    Place,
    /// Concept subject.
    Concept,
    /// Other supported subject kind.
    Other,
}

/// Role a subject has in a memory proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryExtractionSubjectRole {
    /// Entity the memory is about.
    About,
    /// Entity that owns the fact or preference.
    Owner,
    /// Entity affected by the memory.
    Affected,
    /// Entity assigned to the memory.
    Assignee,
    /// Source side of a relationship-like proposal.
    Source,
    /// Target side of a relationship-like proposal.
    Target,
    /// Human or agent participant implied by the proposal.
    Participant,
}

/// Non-authoritative retrieval hints emitted by the extractor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryExtractionRetrievalHints {
    /// Fuzzy topic labels.
    #[serde(default)]
    pub topics: Vec<String>,
    /// Keyword hints for search and ranking.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Optional short summary for display or ranking.
    #[serde(default)]
    pub summary: Option<String>,
}

/// Review risk flag emitted by the extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryExtractionRiskFlag {
    /// Proposal depends on an inference rather than a direct statement.
    Inferred,
    /// Proposal could trigger a reminder, task, tool call, or external action.
    ActionTriggering,
    /// Proposal may conflict with existing memory or the current turn.
    Contradiction,
    /// Proposal includes third-party personal data.
    ThirdParty,
    /// Proposal carries a general review risk.
    Risk,
    /// Proposal carries a project risk.
    ProjectRisk,
    /// Proposal carries a security risk.
    SecurityRisk,
    /// Proposal carries a safety risk.
    SafetyRisk,
    /// Proposal contains sensitive information.
    Sensitive,
    /// Proposal contains secret information.
    Secret,
    /// Proposal is likely temporary task or conversation context.
    TemporaryContext,
    /// Proposal is associated with external egress.
    ExternalEgress,
    /// Proposal came from an explicit memory command handled elsewhere.
    ExplicitMemory,
}

/// Error returned while parsing or validating extractor output.
#[derive(Debug, Error)]
pub enum MemoryExtractionError {
    /// Extractor output was not the strict JSON response shape.
    #[error("invalid extractor JSON: {source}")]
    InvalidJson {
        /// Underlying JSON or serde error.
        #[from]
        source: serde_json::Error,
    },

    /// One extractor proposal failed validation.
    #[error("invalid memory extraction proposal at index {index}: {reason}")]
    InvalidProposal {
        /// Zero-based proposal index.
        index: usize,
        /// Human-readable validation failure.
        reason: &'static str,
    },
}

/// Build the strict JSON-only prompt for ordinary-chat memory extraction.
#[must_use]
pub fn build_memory_extraction_prompt(
    user_input: &str,
    assistant_response: &str,
    conversation_id: &str,
    turn_index: u64,
    cwd_project_hint: Option<&str>,
) -> String {
    let project_hint = cwd_project_hint
        .map(str::trim)
        .filter(|hint| !hint.is_empty())
        .unwrap_or("none");

    format!(
        r#"You are Noema's ordinary-chat memory proposal extractor.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose.
If there are no durable ordinary-chat memories, return {{"proposals":[]}}.

Extract proposals only from the user and assistant messages below. This extractor
does not handle explicit "remember this" commands; those are processed elsewhere.

Return exactly this JSON shape:
{{
  "proposals": [
    {{
      "content": "durable memory content",
      "memory_type": "fact|preference|person|organization|project|place|routine|goal|open_loop|procedure|constraint|trigger|decision|skill|policy|note|other",
      "title": "short title or null",
      "confidence": 0.0,
      "sensitivity": "public|normal|private|sensitive|secret",
      "subjects": [
        {{
          "id": "optional canonical id or null",
          "kind": "human|agent|conversation|workspace|project|task|cron|relationship|tool|organization|place|concept|other",
          "name": "subject name",
          "role": "about|owner|affected|assignee|source|target|participant"
        }}
      ],
      "retrieval_hints": {{
        "topics": [],
        "keywords": [],
        "summary": null
      }},
      "risk_flags": [],
      "evidence_excerpt": "exact contiguous quote from one source message"
    }}
  ]
}}

Rules:
- Propose only durable facts, preferences, constraints, decisions, routines,
  goals, procedures, or notes that could matter later.
- Do not propose jokes, speculation, transient task chatter, or generic world facts.
- Do not propose memories from assistant acknowledgements, status commentary,
  celebratory/meta commentary, or statements that something was saved, recorded,
  remembered, updated, or available in memory.
- Assistant evidence may support durable assistant, conversation, project, or
  workspace notes, but human-subject memories require direct user evidence.
- evidence_excerpt must be an exact contiguous quote from the user or assistant
  message and must directly support the proposal.
- subjects must be non-empty and must show a human subject or participant when
  the memory affects a person.
- Use id "human:local" only for the current human/user/me. Do not use it for
  third-party people.
- confidence must be between 0.0 and 1.0. Use at least 0.70 only when evidence
  directly supports the proposal.
- Use an empty risk_flags array only for low-risk direct ordinary facts and
  preferences.
- Add risk_flags for inferred, sensitive, secret, action-triggering,
  contradiction-prone, third-party, risk-bearing, temporary, or external-egress
  proposals.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}

User message:
{user_input}

Assistant message:
{assistant_response}"#
    )
}

/// Parse and validate strict extractor JSON into accepted memory proposals.
///
/// # Errors
///
/// Returns [`MemoryExtractionError::InvalidJson`] when the extractor text is
/// not valid JSON or uses unsupported enum values, and
/// [`MemoryExtractionError::InvalidProposal`] when any proposal violates the
/// ordinary-chat validation policy.
pub fn parse_memory_extraction_proposals(
    extractor_text: &str,
    user_input: &str,
    assistant_response: &str,
) -> Result<Vec<ValidatedMemoryProposal>, MemoryExtractionError> {
    let response: ExtractorMemoryResponse = serde_json::from_str(extractor_text.trim())?;
    validate_memory_extraction_response(response, user_input, assistant_response)
}

/// Parse and validate strict extractor JSON with item-bounded assistant sources.
///
/// # Errors
///
/// Returns [`MemoryExtractionError::InvalidJson`] when the extractor text is
/// not valid JSON or uses unsupported enum values, and
/// [`MemoryExtractionError::InvalidProposal`] when any proposal violates the
/// ordinary-chat validation policy.
pub fn parse_memory_extraction_proposals_with_assistant_items(
    extractor_text: &str,
    user_input: &str,
    assistant_items: &[&str],
) -> Result<Vec<ValidatedMemoryProposal>, MemoryExtractionError> {
    let response: ExtractorMemoryResponse = serde_json::from_str(extractor_text.trim())?;
    validate_memory_extraction_response_with_assistant_items(response, user_input, assistant_items)
}

/// Validate an already-structured extractor response.
///
/// # Errors
///
/// Returns [`MemoryExtractionError::InvalidProposal`] when any proposal violates
/// the ordinary-chat validation policy.
pub fn validate_memory_extraction_response(
    response: ExtractorMemoryResponse,
    user_input: &str,
    assistant_response: &str,
) -> Result<Vec<ValidatedMemoryProposal>, MemoryExtractionError> {
    validate_memory_extraction_response_with_assistant_items(
        response,
        user_input,
        &[assistant_response],
    )
}

/// Validate an already-structured extractor response with item-bounded
/// assistant sources.
///
/// User evidence is valid when the excerpt appears in the user item. Assistant
/// evidence is valid only when the excerpt appears in at least one assistant
/// item supplied here.
///
/// # Errors
///
/// Returns [`MemoryExtractionError::InvalidProposal`] when any proposal violates
/// the ordinary-chat validation policy.
pub fn validate_memory_extraction_response_with_assistant_items(
    response: ExtractorMemoryResponse,
    user_input: &str,
    assistant_items: &[&str],
) -> Result<Vec<ValidatedMemoryProposal>, MemoryExtractionError> {
    if is_explicit_memory_command(user_input) && !response.proposals.is_empty() {
        return invalid_proposal(0, "explicit memory commands are out of scope");
    }

    response
        .proposals
        .into_iter()
        .enumerate()
        .map(|(index, proposal)| validate_proposal(index, proposal, user_input, assistant_items))
        .collect()
}

/// Partition an already-structured extractor response into valid proposals and
/// invalid drafts that should be discarded.
#[must_use]
pub fn partition_memory_extraction_response_with_assistant_items(
    response: ExtractorMemoryResponse,
    user_input: &str,
    assistant_items: &[&str],
) -> MemoryExtractionValidationPartition {
    if is_explicit_memory_command(user_input) && !response.proposals.is_empty() {
        let rejected = response
            .proposals
            .into_iter()
            .enumerate()
            .map(|(proposal_index, _)| RejectedMemoryExtractionProposal {
                proposal_index,
                error: MemoryExtractionError::InvalidProposal {
                    index: proposal_index,
                    reason: "explicit memory commands are out of scope",
                }
                .to_string(),
            })
            .collect();
        return MemoryExtractionValidationPartition {
            accepted: Vec::new(),
            rejected,
        };
    }

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (proposal_index, proposal) in response.proposals.into_iter().enumerate() {
        match validate_proposal(proposal_index, proposal, user_input, assistant_items) {
            Ok(proposal) => accepted.push(AcceptedMemoryExtractionProposal {
                proposal_index,
                proposal,
            }),
            Err(error) => rejected.push(RejectedMemoryExtractionProposal {
                proposal_index,
                error: error.to_string(),
            }),
        }
    }

    MemoryExtractionValidationPartition { accepted, rejected }
}

/// Choose the initial status for a validated ordinary-chat memory proposal.
#[must_use]
pub fn decide_memory_proposal_status(
    proposal: &ExtractorMemoryProposal,
    has_direct_human_evidence: bool,
) -> MemoryStatus {
    if !has_direct_human_evidence {
        return MemoryStatus::Candidate;
    }

    if !valid_confidence(proposal.confidence) {
        return MemoryStatus::Candidate;
    }

    if !matches!(
        proposal.sensitivity,
        Sensitivity::Public | Sensitivity::Normal
    ) {
        return MemoryStatus::Candidate;
    }

    if !proposal.risk_flags.is_empty() {
        return MemoryStatus::Candidate;
    }

    if proposal
        .subjects
        .iter()
        .any(|subject| subject.implies_third_party_human_for_evidence(&proposal.evidence_excerpt))
    {
        return MemoryStatus::Candidate;
    }

    if matches!(
        proposal.memory_type,
        MemoryType::Fact | MemoryType::Preference
    ) {
        MemoryStatus::Active
    } else {
        MemoryStatus::Candidate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceSource {
    User,
    Assistant,
}

fn validate_proposal(
    index: usize,
    mut proposal: ExtractorMemoryProposal,
    user_input: &str,
    assistant_items: &[&str],
) -> Result<ValidatedMemoryProposal, MemoryExtractionError> {
    proposal.content = proposal.content.trim().to_string();
    if proposal.content.is_empty() {
        return invalid_proposal(index, "content must not be empty");
    }

    proposal.title = trim_optional_string(proposal.title);

    if !valid_confidence(proposal.confidence) {
        return invalid_proposal(index, "confidence must be between 0.70 and 1.0");
    }

    if proposal.subjects.is_empty() {
        return invalid_proposal(index, "subjects must not be empty");
    }

    sanitize_subjects(index, &mut proposal.subjects)?;
    sanitize_retrieval_hints(&mut proposal.retrieval_hints);
    if proposal
        .risk_flags
        .contains(&MemoryExtractionRiskFlag::ExplicitMemory)
    {
        return invalid_proposal(index, "explicit memory commands are out of scope");
    }

    proposal.evidence_excerpt = proposal.evidence_excerpt.trim().to_string();
    if proposal.evidence_excerpt.is_empty() {
        return invalid_proposal(index, "evidence_excerpt must not be empty");
    }

    proposal.sensitivity = effective_proposal_sensitivity(&proposal);

    let Some(evidence_source) =
        evidence_source(&proposal.evidence_excerpt, user_input, assistant_items)
    else {
        return invalid_proposal(
            index,
            "evidence_excerpt must exactly quote the original turn",
        );
    };

    if matches!(evidence_source, EvidenceSource::Assistant) && has_human_subject(&proposal) {
        return invalid_proposal(index, "human-subject memories require direct user evidence");
    }

    let status =
        decide_memory_proposal_status(&proposal, matches!(evidence_source, EvidenceSource::User));

    Ok(ValidatedMemoryProposal { proposal, status })
}

fn sanitize_subjects(
    index: usize,
    subjects: &mut [MemoryExtractionSubject],
) -> Result<(), MemoryExtractionError> {
    for subject in subjects {
        subject.id = trim_optional_string(subject.id.take());
        subject.name = subject.name.trim().to_string();
        if subject.id.is_none() && subject.name.is_empty() {
            return invalid_proposal(index, "subject must include id or name");
        }
    }

    Ok(())
}

fn sanitize_retrieval_hints(hints: &mut MemoryExtractionRetrievalHints) {
    hints.topics = trim_string_vec(std::mem::take(&mut hints.topics));
    hints.keywords = trim_string_vec(std::mem::take(&mut hints.keywords));
    hints.summary = trim_optional_string(hints.summary.take());
}

fn trim_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .collect()
}

fn trim_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn valid_confidence(confidence: f32) -> bool {
    confidence.is_finite() && (MIN_MEMORY_EXTRACTION_CONFIDENCE..=1.0).contains(&confidence)
}

fn is_explicit_memory_command(input: &str) -> bool {
    let trimmed = input.trim_start();
    if trimmed.starts_with('>') {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("/remember")
        || lower.starts_with("remember this:")
        || lower.starts_with("remember that:")
}

fn evidence_source(
    evidence_excerpt: &str,
    user_input: &str,
    assistant_items: &[&str],
) -> Option<EvidenceSource> {
    if user_input.contains(evidence_excerpt) {
        Some(EvidenceSource::User)
    } else if assistant_items
        .iter()
        .any(|assistant_item| assistant_item.contains(evidence_excerpt))
    {
        Some(EvidenceSource::Assistant)
    } else {
        None
    }
}

fn effective_proposal_sensitivity(proposal: &ExtractorMemoryProposal) -> Sensitivity {
    proposal.sensitivity.max(infer_memory_text_sensitivity([
        proposal.content.as_str(),
        proposal.title.as_deref().unwrap_or_default(),
        proposal.evidence_excerpt.as_str(),
    ]))
}

pub(crate) fn infer_memory_text_sensitivity<'a>(
    parts: impl IntoIterator<Item = &'a str>,
) -> Sensitivity {
    let mut sensitivity = Sensitivity::Normal;
    for part in parts {
        sensitivity = sensitivity.max(infer_single_text_sensitivity(part));
        if sensitivity == Sensitivity::Secret {
            break;
        }
    }
    sensitivity
}

fn infer_single_text_sensitivity(content: &str) -> Sensitivity {
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

pub(crate) fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub(crate) fn looks_like_secret_token(content: &str) -> bool {
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

fn has_third_party_human_signal(evidence_excerpt: &str) -> bool {
    [
        "friend",
        "coworker",
        "colleague",
        "partner",
        "brother",
        "sister",
        "mother",
        "father",
        "dad",
        "mom",
        "wife",
        "husband",
        "manager",
        "boss",
        "neighbor",
        "neighbour",
        "roommate",
        "teammate",
        "client",
    ]
    .iter()
    .any(|relation| contains_word_ascii(evidence_excerpt, relation))
}

fn evidence_mentions_subject_name(evidence_excerpt: &str, name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    !name.is_empty() && contains_word_ascii(evidence_excerpt, &name)
}

fn contains_word_ascii(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '\''))
        .any(|word| word == needle)
}

fn has_human_subject(proposal: &ExtractorMemoryProposal) -> bool {
    proposal
        .subjects
        .iter()
        .any(MemoryExtractionSubject::implies_human)
}

fn invalid_proposal<T>(index: usize, reason: &'static str) -> Result<T, MemoryExtractionError> {
    Err(MemoryExtractionError::InvalidProposal { index, reason })
}

mod memory_type_json {
    use super::*;
    use serde::{Deserializer, Serializer, de};

    pub(super) fn serialize<S>(memory_type: &MemoryType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(memory_type.as_str())
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<MemoryType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_memory_type(&value)
            .ok_or_else(|| de::Error::unknown_variant(&value, SUPPORTED_MEMORY_TYPES))
    }

    const SUPPORTED_MEMORY_TYPES: &[&str] = &[
        "fact",
        "preference",
        "person",
        "organization",
        "project",
        "place",
        "routine",
        "goal",
        "open_loop",
        "procedure",
        "constraint",
        "trigger",
        "decision",
        "skill",
        "policy",
        "note",
        "other",
    ];

    fn parse_memory_type(value: &str) -> Option<MemoryType> {
        match value {
            "fact" => Some(MemoryType::Fact),
            "preference" => Some(MemoryType::Preference),
            "person" => Some(MemoryType::Person),
            "organization" => Some(MemoryType::Organization),
            "project" => Some(MemoryType::Project),
            "place" => Some(MemoryType::Place),
            "routine" => Some(MemoryType::Routine),
            "goal" => Some(MemoryType::Goal),
            "open_loop" => Some(MemoryType::OpenLoop),
            "procedure" => Some(MemoryType::Procedure),
            "constraint" => Some(MemoryType::Constraint),
            "trigger" => Some(MemoryType::Trigger),
            "decision" => Some(MemoryType::Decision),
            "skill" => Some(MemoryType::Skill),
            "policy" => Some(MemoryType::Policy),
            "note" => Some(MemoryType::Note),
            "other" => Some(MemoryType::Other),
            _ => None,
        }
    }
}

mod sensitivity_json {
    use super::*;
    use serde::{Deserializer, Serializer, de};

    pub(super) fn serialize<S>(sensitivity: &Sensitivity, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(sensitivity_as_str(*sensitivity))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Sensitivity, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_sensitivity(&value)
            .ok_or_else(|| de::Error::unknown_variant(&value, SUPPORTED_SENSITIVITIES))
    }

    const SUPPORTED_SENSITIVITIES: &[&str] =
        &["public", "normal", "private", "sensitive", "secret"];

    fn sensitivity_as_str(sensitivity: Sensitivity) -> &'static str {
        match sensitivity {
            Sensitivity::Public => "public",
            Sensitivity::Normal => "normal",
            Sensitivity::Private => "private",
            Sensitivity::Sensitive => "sensitive",
            Sensitivity::Secret => "secret",
        }
    }

    fn parse_sensitivity(value: &str) -> Option<Sensitivity> {
        match value {
            "public" => Some(Sensitivity::Public),
            "normal" => Some(Sensitivity::Normal),
            "private" => Some(Sensitivity::Private),
            "sensitive" => Some(Sensitivity::Sensitive),
            "secret" => Some(Sensitivity::Secret),
            _ => None,
        }
    }
}

mod memory_status_json {
    use super::*;
    use serde::{Deserializer, Serializer, de};

    pub(super) fn serialize<S>(status: &MemoryStatus, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(memory_status_as_str(*status))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<MemoryStatus, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_memory_status(&value)
            .ok_or_else(|| de::Error::unknown_variant(&value, SUPPORTED_MEMORY_STATUSES))
    }

    const SUPPORTED_MEMORY_STATUSES: &[&str] = &[
        "candidate",
        "active",
        "confirmed",
        "inferred",
        "stale",
        "superseded",
        "archived",
        "deleted",
        "disputed",
    ];

    fn memory_status_as_str(status: MemoryStatus) -> &'static str {
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

    fn parse_memory_status(value: &str) -> Option<MemoryStatus> {
        match value {
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
}

#[cfg(test)]
mod tests;
