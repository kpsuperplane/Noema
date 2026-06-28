use crate::{memory::Sensitivity, memory_persistence::NewMemoryCandidate};
use serde::Deserialize;

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
