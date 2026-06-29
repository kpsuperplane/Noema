//! Pure future-write memory consolidation types, prompts, and parsers.

use crate::memory::Sensitivity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Raw write input prepared for canonical graph-memory consolidation.
pub struct MemoryWriteProposal {
    /// Category of source that produced the write proposal.
    pub source_kind: MemoryWriteSourceKind,
    /// Stable id for the source item that carries the evidence.
    pub source_item_id: String,
    /// Actor id responsible for the source evidence.
    pub source_actor_id: String,
    /// Short evidence excerpt used for audit and provider prompts.
    pub source_excerpt: String,
    /// Concrete owner object type for the proposed memory.
    pub owner_object_type: String,
    /// Concrete owner object id for the proposed memory.
    pub owner_object_id: String,
    /// Raw text to canonicalize into graph-shaped memory.
    pub raw_text: String,
    /// Coarse memory category supplied by the extraction layer.
    pub memory_type: String,
    /// Sensitivity tier for the proposed memory.
    #[serde(with = "sensitivity_json")]
    pub sensitivity: Sensitivity,
    /// Risk labels carried from extraction and policy checks.
    pub risk_flags: Vec<String>,
    /// Provider or deterministic retrieval hints for future search.
    pub retrieval_hints: Value,
    /// Additional source metadata preserved for downstream policy.
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Source category for a future memory write.
pub enum MemoryWriteSourceKind {
    /// User explicitly asked Noema to remember something.
    ExplicitRemember,
    /// Ordinary chat content produced a memory candidate.
    OrdinaryChat,
    /// Imported document content produced the proposal.
    DocumentImport,
    /// Tool output produced the proposal.
    ToolOutput,
    /// Task execution output produced the proposal.
    TaskOutput,
    /// System-owned source produced the proposal.
    SystemSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Strict provider response containing canonical claim candidates.
pub struct CanonicalizationResponse {
    /// Candidate graph claims proposed from the write input.
    pub candidates: Vec<CanonicalClaimCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Graph-shaped claim candidate returned by canonicalization.
pub struct CanonicalClaimCandidate {
    /// Canonical subject entity for the claim.
    pub subject: CanonicalEntity,
    /// Canonical object entity for the claim.
    pub object: CanonicalEntity,
    /// Predicate resolution selected for the claim.
    pub predicate: PredicateResolution,
    /// Human-readable claim fact.
    pub fact: String,
    /// Sensitivity tier assigned to the claim.
    #[serde(with = "sensitivity_json")]
    pub sensitivity: Sensitivity,
    /// Proposed lifecycle status for the claim.
    pub status: CanonicalClaimStatus,
    /// Provider confidence between 0.0 and 1.0.
    pub confidence: f64,
    /// Retrieval hints to store alongside the claim.
    #[serde(default)]
    pub retrieval_hints: Value,
    /// Provider rationale for the canonicalization decision.
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Canonical entity endpoint for a graph-memory claim.
pub struct CanonicalEntity {
    /// Stable entity id.
    pub entity_id: String,
    /// Entity type vocabulary value.
    pub entity_type: String,
    /// Human-readable canonical entity name.
    pub canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
/// Provider resolution for a claim predicate.
pub enum PredicateResolution {
    /// Use an existing promoted predicate.
    PromotedPredicate {
        /// Promoted predicate id.
        predicate_id: String,
    },
    /// Propose a new predicate for review or candidate persistence.
    PredicateProposal {
        /// Proposed predicate metadata.
        proposal: ProposedPredicate,
    },
    /// Store the content as an unstructured fallback note.
    FallbackNote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// New predicate proposed when no promoted predicate clearly fits.
pub struct ProposedPredicate {
    /// Human-readable predicate label.
    pub label: String,
    /// Description of the relationship represented by the predicate.
    pub description: String,
    /// Entity types allowed for claim subjects.
    pub allowed_subject_types: Vec<String>,
    /// Entity types allowed for claim objects.
    pub allowed_object_types: Vec<String>,
    /// Retrieval use modes allowed for this predicate.
    pub allowed_use_modes: Vec<String>,
    /// Default sensitivity for claims using this predicate.
    #[serde(with = "sensitivity_json")]
    pub default_sensitivity: Sensitivity,
    /// Conflict handling policy id.
    pub conflict_policy: String,
    /// Review handling policy id.
    pub review_policy: String,
    /// Inverse predicate behavior policy id.
    pub inverse_behavior: String,
    /// Optional promoted inverse predicate id.
    pub inverse_predicate_id: Option<String>,
    /// Default proactivity tier for claims using this predicate.
    pub proactivity_default: i64,
    /// Predicate-specific merge guidance.
    #[serde(default)]
    pub merge_hints: Value,
    /// Synonym hints for extraction and matching.
    #[serde(default)]
    pub synonym_hints: Vec<String>,
    /// Extraction examples or other provider hints.
    #[serde(default)]
    pub extraction_hints: Value,
    /// Provider rationale for proposing the predicate.
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Lifecycle status requested for a canonical claim candidate.
pub enum CanonicalClaimStatus {
    /// Claim should enter review or candidate state.
    Candidate,
    /// Claim can be active immediately.
    Active,
    /// Claim is confirmed by strong evidence or policy.
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Provider decision comparing one candidate against existing memories.
pub struct ConsolidationDecision {
    /// Consolidation action to apply.
    pub decision: ConsolidationDecisionKind,
    /// Existing claim id for decisions that operate on a stored claim.
    pub existing_claim_id: Option<String>,
    /// Provider confidence between 0.0 and 1.0.
    pub confidence: f64,
    /// Provider rationale for the decision.
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Consolidation action selected for a canonical claim candidate.
pub enum ConsolidationDecisionKind {
    /// Create a new claim.
    Create,
    /// Reinforce an existing claim with new evidence.
    Reinforce,
    /// Supersede an existing claim.
    Supersede,
    /// Mark or route conflicting evidence.
    Dispute,
    /// Relate the candidate to an existing claim.
    Relate,
    /// Defer persistence to review.
    NeedsReview,
}

#[derive(Debug, Error)]
/// Error returned by strict consolidation parsers and validators.
pub enum MemoryConsolidationError {
    /// JSON parsing or serde validation failed.
    #[error("invalid consolidation JSON: {source}")]
    InvalidJson {
        /// Underlying serde JSON error.
        #[from]
        source: serde_json::Error,
    },
    /// Canonicalization response failed deterministic validation.
    #[error("invalid canonicalization response: {0}")]
    InvalidCanonicalization(&'static str),
    /// Consolidation decision failed deterministic validation.
    #[error("invalid consolidation decision: {0}")]
    InvalidDecision(&'static str),
}

/// Parse and validate strict canonicalization JSON.
///
/// # Errors
///
/// Returns an error when the text is not valid strict JSON for a
/// canonicalization response, or when deterministic validation rejects a
/// candidate.
pub fn parse_canonicalization_response(
    text: &str,
) -> Result<CanonicalizationResponse, MemoryConsolidationError> {
    let mut response: CanonicalizationResponse = serde_json::from_str(text.trim())?;
    if response.candidates.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(
            "candidates must not be empty",
        ));
    }
    for candidate in &mut response.candidates {
        validate_entity(&mut candidate.subject, "subject")?;
        validate_entity(&mut candidate.object, "object")?;
        validate_predicate_resolution(&mut candidate.predicate)?;
        candidate.fact = candidate.fact.trim().to_string();
        candidate.rationale = candidate.rationale.trim().to_string();
        if candidate.fact.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "fact must not be empty",
            ));
        }
        if !candidate.confidence.is_finite() || !(0.0..=1.0).contains(&candidate.confidence) {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "confidence must be between 0.0 and 1.0",
            ));
        }
        if candidate.rationale.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                "rationale must not be empty",
            ));
        }
    }
    Ok(response)
}

fn validate_entity(
    entity: &mut CanonicalEntity,
    prefix: &str,
) -> Result<(), MemoryConsolidationError> {
    entity.entity_id = entity.entity_id.trim().to_string();
    entity.entity_type = entity.entity_type.trim().to_string();
    entity.canonical_name = entity.canonical_name.trim().to_string();
    if entity.entity_id.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(
            if prefix == "subject" {
                "subject.entity_id must not be empty"
            } else {
                "object.entity_id must not be empty"
            },
        ));
    }
    if entity.entity_type.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(
            if prefix == "subject" {
                "subject.entity_type must not be empty"
            } else {
                "object.entity_type must not be empty"
            },
        ));
    }
    if entity.canonical_name.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(
            if prefix == "subject" {
                "subject.canonical_name must not be empty"
            } else {
                "object.canonical_name must not be empty"
            },
        ));
    }
    Ok(())
}

fn validate_predicate_resolution(
    predicate: &mut PredicateResolution,
) -> Result<(), MemoryConsolidationError> {
    match predicate {
        PredicateResolution::PromotedPredicate { predicate_id } => {
            *predicate_id = predicate_id.trim().to_string();
            if predicate_id.is_empty() {
                return Err(MemoryConsolidationError::InvalidCanonicalization(
                    "predicate_id must not be empty",
                ));
            }
        }
        PredicateResolution::PredicateProposal { proposal } => {
            validate_predicate_proposal(proposal)?
        }
        PredicateResolution::FallbackNote => {}
    }
    Ok(())
}

fn validate_predicate_proposal(
    proposal: &mut ProposedPredicate,
) -> Result<(), MemoryConsolidationError> {
    proposal.label = proposal.label.trim().to_string();
    proposal.description = proposal.description.trim().to_string();
    proposal.conflict_policy = proposal.conflict_policy.trim().to_string();
    proposal.review_policy = proposal.review_policy.trim().to_string();
    proposal.inverse_behavior = proposal.inverse_behavior.trim().to_string();
    proposal.rationale = proposal.rationale.trim().to_string();
    proposal.synonym_hints = proposal
        .synonym_hints
        .iter()
        .map(|hint| hint.trim())
        .filter(|hint| !hint.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    require_non_empty_field(
        &proposal.label,
        "predicate proposal label must not be empty",
    )?;
    require_non_empty_field(
        &proposal.description,
        "predicate proposal description must not be empty",
    )?;
    require_non_empty_field(
        &proposal.conflict_policy,
        "predicate proposal conflict_policy must not be empty",
    )?;
    require_non_empty_field(
        &proposal.review_policy,
        "predicate proposal review_policy must not be empty",
    )?;
    require_non_empty_field(
        &proposal.inverse_behavior,
        "predicate proposal inverse_behavior must not be empty",
    )?;
    require_non_empty_field(
        &proposal.rationale,
        "predicate proposal rationale must not be empty",
    )?;
    trim_non_empty_vector(
        &mut proposal.allowed_subject_types,
        "predicate proposal allowed_subject_types must not be empty",
        "predicate proposal allowed_subject_types entries must not be empty",
    )?;
    trim_non_empty_vector(
        &mut proposal.allowed_object_types,
        "predicate proposal allowed_object_types must not be empty",
        "predicate proposal allowed_object_types entries must not be empty",
    )?;
    trim_non_empty_vector(
        &mut proposal.allowed_use_modes,
        "predicate proposal allowed_use_modes must not be empty",
        "predicate proposal allowed_use_modes entries must not be empty",
    )?;
    Ok(())
}

fn require_non_empty_field(
    value: &str,
    message: &'static str,
) -> Result<(), MemoryConsolidationError> {
    if value.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(message));
    }
    Ok(())
}

fn trim_non_empty_vector(
    values: &mut Vec<String>,
    empty_message: &'static str,
    blank_entry_message: &'static str,
) -> Result<(), MemoryConsolidationError> {
    if values.is_empty() {
        return Err(MemoryConsolidationError::InvalidCanonicalization(
            empty_message,
        ));
    }
    for value in values {
        *value = value.trim().to_string();
        if value.is_empty() {
            return Err(MemoryConsolidationError::InvalidCanonicalization(
                blank_entry_message,
            ));
        }
    }
    Ok(())
}

/// Parse and validate strict consolidation decision JSON.
///
/// # Errors
///
/// Returns an error when the text is not valid strict JSON for a consolidation
/// decision, or when deterministic validation rejects the decision.
pub fn parse_consolidation_decision(
    text: &str,
) -> Result<ConsolidationDecision, MemoryConsolidationError> {
    let mut decision: ConsolidationDecision = serde_json::from_str(text.trim())?;
    decision.rationale = decision.rationale.trim().to_string();
    if !decision.confidence.is_finite() || !(0.0..=1.0).contains(&decision.confidence) {
        return Err(MemoryConsolidationError::InvalidDecision(
            "confidence must be between 0.0 and 1.0",
        ));
    }
    if !matches!(
        decision.decision,
        ConsolidationDecisionKind::Create | ConsolidationDecisionKind::NeedsReview
    ) && decision
        .existing_claim_id
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err(MemoryConsolidationError::InvalidDecision(
            "existing_claim_id is required for this decision",
        ));
    }
    if decision.rationale.is_empty() {
        return Err(MemoryConsolidationError::InvalidDecision(
            "rationale must not be empty",
        ));
    }
    Ok(decision)
}

/// Build the provider prompt for canonicalizing a memory write proposal.
#[must_use]
pub fn build_claim_canonicalization_prompt(
    proposal: &MemoryWriteProposal,
    promoted_predicates_json: &Value,
) -> String {
    let promoted_predicates =
        serde_json::to_string_pretty(promoted_predicates_json).expect("serialize predicates");
    format!(
        r#"You are Noema's memory claim canonicalizer.

Return strict JSON only. Do not include Markdown, comments, or prose.

Required output JSON shape:
{{
  "candidates": [
    {{
      "subject": {{
        "entity_id": "human:local",
        "entity_type": "human",
        "canonical_name": "Local human"
      }},
      "object": {{
        "entity_id": "concept:stable_object_id",
        "entity_type": "concept",
        "canonical_name": "stable object name"
      }},
      "predicate": {{
        "kind": "promoted_predicate",
        "predicate_id": "exact_catalog_predicate_id"
      }},
      "fact": "Local human has a durable relationship to stable object.",
      "sensitivity": "normal",
      "status": "active",
      "confidence": 0.9,
      "retrieval_hints": {{
        "keywords": ["stable object"],
        "summary": "Local human has a durable relationship to stable object."
      }},
      "rationale": "The source states a durable relationship."
    }}
  ]
}}

Every candidate must include subject, object, predicate, fact, sensitivity,
status, confidence, retrieval_hints, and rationale. Use only these field names.
Entity fields are exactly entity_id, entity_type, and canonical_name.
Sensitivity must be one of public, normal, private, sensitive, or secret.
Status must be active, confirmed, or candidate. Use candidate when the memory
needs review. Use active for memories that can be saved immediately.

Predicate resolution variants:
- Use {{"kind": "promoted_predicate", "predicate_id": "..."}} only when the
  predicate_id is copied exactly from the promoted predicate catalog below.
- Use {{"kind": "predicate_proposal", "proposal": {{...}}}} when no catalog
  predicate clearly fits. A predicate proposal must include label, description,
  allowed_subject_types, allowed_object_types, allowed_use_modes,
  default_sensitivity, conflict_policy, review_policy, inverse_behavior,
  inverse_predicate_id, proactivity_default, merge_hints, synonym_hints,
  extraction_hints, and rationale.
- Use {{"kind": "fallback_note"}} only for genuinely unstructured notes that do
  not express a durable relationship.

Map the proposal to promoted predicates when one clearly fits. If none fits,
return a predicate_proposal. Use fallback_note only for genuinely unstructured
notes that do not express a durable relationship.

Promoted predicate catalog JSON:
{promoted_predicates}

Input JSON payload:
{}"#,
        serde_json::to_string_pretty(proposal).expect("serialize proposal")
    )
}

/// Build the provider prompt for comparing a candidate to existing memories.
#[must_use]
pub fn build_consolidation_prompt(candidate: &CanonicalClaimCandidate, existing: &Value) -> String {
    let payload = serde_json::json!({
        "candidate": candidate,
        "existing_memories": existing,
    });
    format!(
        r#"You are Noema's memory consolidation comparator.

Return strict JSON only with fields decision, existing_claim_id, confidence, and rationale.
Use create, reinforce, supersede, dispute, relate, or needs_review.
Prefer needs_review when uncertain.

Input JSON payload:
{}"#,
        serde_json::to_string_pretty(&payload).expect("serialize comparator payload")
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Sensitivity;

    fn proposal_fixture() -> MemoryWriteProposal {
        MemoryWriteProposal {
            source_kind: MemoryWriteSourceKind::ExplicitRemember,
            source_item_id: "conversation_item:source-1".to_string(),
            source_actor_id: "human:local".to_string(),
            source_excerpt: "Remember that I adore trains.".to_string(),
            owner_object_type: "human".to_string(),
            owner_object_id: "human:local".to_string(),
            raw_text: "I adore trains.".to_string(),
            memory_type: "preference".to_string(),
            sensitivity: Sensitivity::Normal,
            risk_flags: vec!["user_asserted".to_string()],
            retrieval_hints: serde_json::json!({
                "keywords": ["trains"],
                "summary": "Kevin likes trains."
            }),
            metadata: serde_json::json!({
                "conversation_id": "conversation:abc"
            }),
        }
    }

    fn candidate_fixture() -> CanonicalClaimCandidate {
        CanonicalClaimCandidate {
            subject: CanonicalEntity {
                entity_id: "human:local".to_string(),
                entity_type: "human".to_string(),
                canonical_name: "Local human".to_string(),
            },
            object: CanonicalEntity {
                entity_id: "concept:trains".to_string(),
                entity_type: "concept".to_string(),
                canonical_name: "trains".to_string(),
            },
            predicate: PredicateResolution::PromotedPredicate {
                predicate_id: "likes".to_string(),
            },
            fact: "Kevin likes trains.".to_string(),
            sensitivity: Sensitivity::Normal,
            status: CanonicalClaimStatus::Active,
            confidence: 0.94,
            retrieval_hints: serde_json::json!({
                "keywords": ["trains"],
                "summary": "Kevin likes trains."
            }),
            rationale: "The source directly states a durable preference.".to_string(),
        }
    }

    fn predicate_proposal_fixture() -> ProposedPredicate {
        ProposedPredicate {
            label: "collects".to_string(),
            description: "The subject collects the object.".to_string(),
            allowed_subject_types: vec!["human".to_string()],
            allowed_object_types: vec!["concept".to_string()],
            allowed_use_modes: vec!["answer".to_string()],
            default_sensitivity: Sensitivity::Normal,
            conflict_policy: "allow_many".to_string(),
            review_policy: "auto_candidate".to_string(),
            inverse_behavior: "none".to_string(),
            inverse_predicate_id: None,
            proactivity_default: 1,
            merge_hints: serde_json::json!({}),
            synonym_hints: vec!["  gathers  ".to_string(), "   ".to_string()],
            extraction_hints: serde_json::json!({}),
            rationale: "No promoted predicate represents collecting.".to_string(),
        }
    }

    fn canonicalization_json(candidate: CanonicalClaimCandidate) -> String {
        serde_json::to_string(&CanonicalizationResponse {
            candidates: vec![candidate],
        })
        .expect("serialize canonicalization")
    }

    #[test]
    fn claim_canonicalization_prompt_includes_strict_json_input_and_predicates() {
        let promoted_predicates = serde_json::json!([{
            "predicate_id": "likes",
            "label": "likes",
            "allowed_subject_types": ["human"],
            "allowed_object_types": ["concept"],
            "merge_hints": {"equivalent_phrases": ["adore", "love"]}
        }]);

        let prompt = build_claim_canonicalization_prompt(&proposal_fixture(), &promoted_predicates);

        assert!(prompt.contains("You are Noema's memory claim canonicalizer."));
        assert!(prompt.contains("Return strict JSON only."));
        assert!(prompt.contains("Do not include Markdown, comments, or prose."));
        assert!(prompt.contains("Required output JSON shape:"));
        assert!(prompt.contains(r#""kind": "promoted_predicate""#));
        assert!(prompt.contains(r#""kind": "predicate_proposal""#));
        assert!(prompt.contains(r#""kind": "fallback_note""#));
        assert!(prompt.contains("Use active for memories that can be saved immediately."));
        assert!(prompt.contains("Map the proposal to promoted predicates when one clearly fits."));
        assert!(prompt.contains("Use fallback_note only for genuinely unstructured"));
        assert!(prompt.contains("Promoted predicate catalog JSON:"));
        assert!(prompt.contains("\"predicate_id\": \"likes\""));
        assert!(prompt.contains("\"equivalent_phrases\""));
        assert!(prompt.contains("Input JSON payload:"));
        assert!(prompt.contains("\"source_item_id\": \"conversation_item:source-1\""));
        assert!(prompt.contains("\"raw_text\": \"I adore trains.\""));
        assert!(prompt.contains("\"summary\": \"Kevin likes trains.\""));
    }

    #[test]
    fn consolidation_prompt_includes_strict_json_candidate_and_existing_memories() {
        let existing = serde_json::json!([{
            "claim_id": "claim:likes-trains",
            "predicate_id": "likes",
            "fact": "Kevin likes trains.",
            "status": "active",
            "retrieval_hints": {"keywords": ["rail", "trains"]}
        }]);

        let prompt = build_consolidation_prompt(&candidate_fixture(), &existing);

        assert!(prompt.contains("You are Noema's memory consolidation comparator."));
        assert!(prompt.contains("Return strict JSON only with fields decision, existing_claim_id, confidence, and rationale."));
        assert!(
            prompt.contains("Use create, reinforce, supersede, dispute, relate, or needs_review.")
        );
        assert!(prompt.contains("Prefer needs_review when uncertain."));
        assert!(prompt.contains("Input JSON payload:"));
        assert!(prompt.contains("\"candidate\""));
        assert!(prompt.contains("\"existing_memories\""));
        assert!(prompt.contains("\"fact\": \"Kevin likes trains.\""));
        assert!(prompt.contains("\"claim_id\": \"claim:likes-trains\""));
        assert!(prompt.contains("\"keywords\""));
    }

    #[test]
    fn parses_promoted_predicate_canonicalization() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:planes", "entity_type": "concept", "canonical_name": "planes"},
            "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
            "fact": "Kevin likes planes.",
            "sensitivity": "normal",
            "status": "active",
            "confidence": 0.94,
            "retrieval_hints": {"keywords": ["planes"], "summary": "Kevin likes planes."},
            "rationale": "The user directly said they love planes."
          }]
        }"#;

        let parsed = parse_canonicalization_response(json).expect("canonicalization");
        assert_eq!(parsed.candidates.len(), 1);
        let candidate = &parsed.candidates[0];
        assert_eq!(candidate.subject.entity_id, "human:local");
        assert_eq!(candidate.object.entity_id, "concept:planes");
        assert_eq!(
            candidate.predicate,
            PredicateResolution::PromotedPredicate {
                predicate_id: "likes".to_string()
            }
        );
        assert_eq!(candidate.fact, "Kevin likes planes.");
        assert_eq!(candidate.sensitivity, Sensitivity::Normal);
        assert_eq!(candidate.status, CanonicalClaimStatus::Active);
    }

    #[test]
    fn parses_predicate_proposal_canonicalization() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:model_aircraft", "entity_type": "concept", "canonical_name": "model aircraft"},
            "predicate": {
              "kind": "predicate_proposal",
              "proposal": {
                "label": "collects",
                "description": "The subject collects the object.",
                "allowed_subject_types": ["human", "person"],
                "allowed_object_types": ["concept", "other"],
                "allowed_use_modes": ["answer", "personalize"],
                "default_sensitivity": "normal",
                "conflict_policy": "allow_many",
                "review_policy": "auto_candidate",
                "inverse_behavior": "none",
                "inverse_predicate_id": null,
                "proactivity_default": 1,
                "merge_hints": {"strategy": "object_identity"},
                "synonym_hints": ["gathers", "keeps a collection of"],
                "extraction_hints": {"examples": ["I collect model aircraft"]},
                "rationale": "No promoted predicate represents collecting."
              }
            },
            "fact": "Kevin collects model aircraft.",
            "sensitivity": "normal",
            "status": "candidate",
            "confidence": 0.88,
            "retrieval_hints": {"keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
            "rationale": "The source states a durable collecting relationship."
          }]
        }"#;

        let parsed = parse_canonicalization_response(json).expect("canonicalization");
        let PredicateResolution::PredicateProposal { proposal } = &parsed.candidates[0].predicate
        else {
            panic!("expected predicate proposal");
        };
        assert_eq!(proposal.label, "collects");
        assert_eq!(proposal.allowed_subject_types, vec!["human", "person"]);
        assert_eq!(parsed.candidates[0].status, CanonicalClaimStatus::Candidate);
    }

    #[test]
    fn rejects_invalid_canonicalization_confidence() {
        let json = r#"{
          "candidates": [{
            "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
            "object": {"entity_id": "concept:planes", "entity_type": "concept", "canonical_name": "planes"},
            "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
            "fact": "Kevin likes planes.",
            "sensitivity": "normal",
            "status": "active",
            "confidence": 1.7,
            "retrieval_hints": {},
            "rationale": "bad confidence"
          }]
        }"#;

        let error = parse_canonicalization_response(json).expect_err("invalid confidence");
        assert!(
            error
                .to_string()
                .contains("confidence must be between 0.0 and 1.0")
        );
    }

    #[test]
    fn rejects_empty_canonicalization_candidates() {
        let json = serde_json::to_string(&CanonicalizationResponse { candidates: vec![] })
            .expect("serialize empty canonicalization");

        let error = parse_canonicalization_response(&json).expect_err("empty candidates");

        assert!(
            error.to_string().contains("candidates must not be empty"),
            "{error}"
        );
    }

    #[test]
    fn rejects_blank_entity_or_promoted_predicate_id() {
        let mut blank_subject_id = candidate_fixture();
        blank_subject_id.subject.entity_id = "   ".to_string();

        let mut blank_object_name = candidate_fixture();
        blank_object_name.object.canonical_name = "   ".to_string();

        let mut blank_predicate_id = candidate_fixture();
        blank_predicate_id.predicate = PredicateResolution::PromotedPredicate {
            predicate_id: "   ".to_string(),
        };

        let cases = [
            (
                "blank_subject_entity_id",
                blank_subject_id,
                "subject.entity_id must not be empty",
            ),
            (
                "blank_object_canonical_name",
                blank_object_name,
                "object.canonical_name must not be empty",
            ),
            (
                "blank_promoted_predicate_id",
                blank_predicate_id,
                "predicate_id must not be empty",
            ),
        ];

        for (case, candidate, expected) in cases {
            let json = canonicalization_json(candidate);
            let error = parse_canonicalization_response(&json).expect_err(case);
            assert!(error.to_string().contains(expected), "{case}: {error}");
        }
    }

    #[test]
    fn rejects_unusable_predicate_proposal() {
        let mut blank_label = predicate_proposal_fixture();
        blank_label.label = "   ".to_string();

        let mut empty_use_modes = predicate_proposal_fixture();
        empty_use_modes.allowed_use_modes = vec![];

        let mut blank_subject_type = predicate_proposal_fixture();
        blank_subject_type.allowed_subject_types = vec!["human".to_string(), "   ".to_string()];

        let cases = [
            (
                "blank_label",
                blank_label,
                "predicate proposal label must not be empty",
            ),
            (
                "empty_allowed_use_modes",
                empty_use_modes,
                "predicate proposal allowed_use_modes must not be empty",
            ),
            (
                "blank_allowed_subject_type",
                blank_subject_type,
                "predicate proposal allowed_subject_types entries must not be empty",
            ),
        ];

        for (case, proposal, expected) in cases {
            let mut candidate = candidate_fixture();
            candidate.status = CanonicalClaimStatus::Candidate;
            candidate.predicate = PredicateResolution::PredicateProposal { proposal };
            let json = canonicalization_json(candidate);
            let error = parse_canonicalization_response(&json).expect_err(case);
            assert!(error.to_string().contains(expected), "{case}: {error}");
        }
    }

    #[test]
    fn parses_consolidation_decision() {
        let json = r#"{
          "decision": "reinforce",
          "existing_claim_id": "claim:abc",
          "confidence": 0.92,
          "rationale": "same preference"
        }"#;

        let decision = parse_consolidation_decision(json).expect("decision");
        assert_eq!(decision.decision, ConsolidationDecisionKind::Reinforce);
        assert_eq!(decision.existing_claim_id.as_deref(), Some("claim:abc"));
        assert_eq!(decision.confidence, 0.92);
    }

    #[test]
    fn rejects_consolidation_decisions_missing_existing_claim_id() {
        for (case, json) in [
            (
                "missing",
                r#"{
                  "decision": "reinforce",
                  "confidence": 0.92,
                  "rationale": "same preference"
                }"#,
            ),
            (
                "blank",
                r#"{
                  "decision": "supersede",
                  "existing_claim_id": "   ",
                  "confidence": 0.86,
                  "rationale": "newer correction replaces older truth"
                }"#,
            ),
        ] {
            let error = parse_consolidation_decision(json).expect_err(case);
            assert!(
                error
                    .to_string()
                    .contains("existing_claim_id is required for this decision"),
                "{case}: {error}"
            );
        }
    }
}
