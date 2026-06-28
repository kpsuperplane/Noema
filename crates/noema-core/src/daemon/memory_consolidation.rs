use crate::{
    memory::Sensitivity,
    memory_persistence::{MemorySummary, NewMemoryCandidate},
    provider::{GenerateRequest, ProviderError},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::runtime::RuntimeModelProvider;

// Task 3 defines outcomes before runtime integration lands in Tasks 5/6.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MemoryConsolidationOutcome {
    Created {
        memory_id: String,
    },
    Reused {
        memory_id: String,
        reason: &'static str,
    },
    Reinforced {
        memory_id: String,
        reason: &'static str,
    },
    Conflict {
        memory_id: String,
        conflicting_memory_id: String,
        reason: String,
    },
}

// Task 3 parser is tested here; runtime consumers are added in later tasks.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SemanticConsolidationDecision {
    Create,
    Reuse {
        existing_memory_id: String,
    },
    Reinforce {
        existing_memory_id: String,
    },
    Conflict {
        existing_memory_id: String,
        rationale: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticDecisionJson {
    decision: String,
    existing_memory_id: Option<String>,
    confidence: f64,
    rationale: String,
}

// Used by this module's unit tests until semantic consolidation is wired into runtime.
#[allow(dead_code)]
pub(super) fn parse_semantic_decision(text: &str) -> Result<SemanticConsolidationDecision, String> {
    let parsed: SemanticDecisionJson =
        serde_json::from_str(text.trim()).map_err(|error| format!("invalid JSON: {error}"))?;
    if !(0.0..=1.0).contains(&parsed.confidence) {
        return Err("confidence must be between 0.0 and 1.0".to_string());
    }
    match parsed.decision.as_str() {
        "create" => {
            if parsed
                .existing_memory_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty())
            {
                return Err("existing_memory_id must be empty for create".to_string());
            }
            Ok(SemanticConsolidationDecision::Create)
        }
        "reuse" => Ok(SemanticConsolidationDecision::Reuse {
            existing_memory_id: required_existing_memory_id(&parsed)?,
        }),
        "reinforce" => Ok(SemanticConsolidationDecision::Reinforce {
            existing_memory_id: required_existing_memory_id(&parsed)?,
        }),
        "conflict" => {
            if parsed.rationale.trim().is_empty() {
                return Err("rationale is required for conflict".to_string());
            }
            Ok(SemanticConsolidationDecision::Conflict {
                existing_memory_id: required_existing_memory_id(&parsed)?,
                rationale: parsed.rationale,
            })
        }
        other => Err(format!("unsupported semantic decision: {other}")),
    }
}

fn required_existing_memory_id(parsed: &SemanticDecisionJson) -> Result<String, String> {
    parsed
        .existing_memory_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| "existing_memory_id is required".to_string())
}

// Task 3 exposes the policy gate before the semantic comparator calls it.
#[allow(dead_code)]
pub(super) fn semantic_consolidation_allowed(candidate: &NewMemoryCandidate) -> bool {
    matches!(
        candidate.sensitivity,
        Sensitivity::Public | Sensitivity::Normal
    )
}

#[allow(dead_code)]
pub(super) fn build_semantic_consolidation_prompt(
    proposal_content: &str,
    matches: &[MemorySummary],
) -> String {
    let existing = Value::Array(
        matches
            .iter()
            .map(|memory| {
                json!({
                    "memory_id": memory.id,
                    "memory_type": memory.memory_type.as_str(),
                    "owner": {
                        "object_type": memory.owner_object_type,
                        "object_id": memory.owner_object_id,
                    },
                    "sensitivity": sensitivity_label(memory.sensitivity),
                    "content": memory.content,
                })
            })
            .collect(),
    );

    format!(
        r#"You are Noema's memory consolidation comparator.

Return strict JSON only. Do not include Markdown, prose, or comments.

Choose one decision:
- create: the proposal is distinct from every existing memory.
- reuse: an existing memory fully covers the proposal and no new support is useful.
- reinforce: the proposal restates existing truth and should add supporting provenance.
- conflict: the proposal and an existing memory cannot both be true.

Return exactly this shape:
{{"decision":"create|reuse|reinforce|conflict","existing_memory_id":null,"confidence":0.0,"rationale":"short reason"}}

Rules:
- Use reuse, reinforce, or conflict only when the existing memory has the same subject and owner.
- Prefer create when uncertain.
- Prefer reinforce for paraphrases of the same preference or fact.
- Prefer conflict for direct contradiction.

Proposal:
{proposal_content}

Existing memories:
{existing}"#
    )
}

#[allow(dead_code)]
pub(super) async fn semantic_consolidation_decision(
    provider: &dyn RuntimeModelProvider,
    candidate: &NewMemoryCandidate,
    matches: &[MemorySummary],
) -> Result<SemanticConsolidationDecision, String> {
    if matches.is_empty() || !semantic_consolidation_allowed(candidate) {
        return Ok(SemanticConsolidationDecision::Create);
    }

    let prompt = build_semantic_consolidation_prompt(&candidate.content, matches);
    let response = provider
        .generate(GenerateRequest::text(prompt))
        .await
        .map_err(|error: ProviderError| format!("semantic consolidation model failed: {error}"))?;
    parse_semantic_decision(&response.assistant_text())
}

#[allow(dead_code)]
fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

#[cfg(test)]
mod tests {
    use super::{SemanticConsolidationDecision, parse_semantic_decision};

    #[test]
    fn parse_semantic_reinforce_decision() {
        let parsed = parse_semantic_decision(
            r#"{"decision":"reinforce","existing_memory_id":"mem_1","confidence":0.91,"rationale":"same preference"}"#,
        )
        .expect("decision");

        assert_eq!(
            parsed,
            SemanticConsolidationDecision::Reinforce {
                existing_memory_id: "mem_1".to_string()
            }
        );
    }

    #[test]
    fn parse_semantic_decision_requires_existing_memory_for_reuse() {
        let error = parse_semantic_decision(
            r#"{"decision":"reuse","existing_memory_id":null,"confidence":0.91,"rationale":"same preference"}"#,
        )
        .expect_err("missing id");

        assert_eq!(error, "existing_memory_id is required");
    }

    #[test]
    fn parse_semantic_decision_rejects_unknown_fields() {
        let error = parse_semantic_decision(
            r#"{"decision":"create","existing_memory_id":null,"confidence":0.91,"rationale":"distinct","extra":"nope"}"#,
        )
        .expect_err("unknown field");

        assert!(error.starts_with("invalid JSON:"), "{error}");
    }

    #[test]
    fn parse_semantic_create_rejects_existing_memory_id() {
        let error = parse_semantic_decision(
            r#"{"decision":"create","existing_memory_id":"mem_1","confidence":0.91,"rationale":"distinct"}"#,
        )
        .expect_err("existing id");

        assert_eq!(error, "existing_memory_id must be empty for create");
    }

    #[test]
    fn parse_semantic_conflict_requires_non_empty_rationale() {
        let error = parse_semantic_decision(
            r#"{"decision":"conflict","existing_memory_id":"mem_1","confidence":0.91,"rationale":"   "}"#,
        )
        .expect_err("empty rationale");

        assert_eq!(error, "rationale is required for conflict");
    }
}

#[cfg(test)]
mod prompt_tests {
    use crate::{
        memory::{MemoryStatus, Sensitivity},
        memory_persistence::{MemorySummary, MemoryType},
    };

    use super::build_semantic_consolidation_prompt;

    fn summary(id: &str, content: &str) -> MemorySummary {
        MemorySummary {
            id: id.to_string(),
            status: MemoryStatus::Active,
            memory_type: MemoryType::Preference,
            home_scope_id: "human:human:local".to_string(),
            owner_object_type: "human".to_string(),
            owner_object_id: "human:local".to_string(),
            sensitivity: Sensitivity::Normal,
            title: content.to_string(),
            content: content.to_string(),
            created_at: "2026-06-28 00:00:00+00".to_string(),
            dedupe_fingerprint: Some("sha256:abc".to_string()),
            source_object_type: None,
            source_object_id: None,
            source_type: None,
            source_id: None,
            conversation_id: None,
        }
    }

    #[test]
    fn semantic_prompt_contains_strict_decision_contract() {
        let prompt = build_semantic_consolidation_prompt(
            "Kevin likes ice cream.",
            &[summary("mem_1", "Kevin enjoys ice cream.")],
        );

        assert!(prompt.contains("\"decision\":\"create|reuse|reinforce|conflict\""));
        assert!(prompt.contains("mem_1"));
        assert!(prompt.contains("Kevin likes ice cream."));
    }
}
