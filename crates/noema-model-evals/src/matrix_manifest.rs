use std::{collections::HashSet, fs, path::Path};

use noema_providers::ReasoningEffort;
use noema_runtime::eval_support::RuntimeEvalRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluationCandidateManifest {
    pub candidates: Vec<EvaluationCandidate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvaluationCandidate {
    pub id: String,
    pub name: String,
    pub provider: EvaluationProvider,
    pub model: String,
    pub roles: Vec<RuntimeEvalRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing)]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing)]
    pub bridge_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub notes: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationProvider {
    Codex,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openrouter")]
    OpenRouter,
    FoundationLocal,
    LocalModels,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelPricing {
    pub input_usd_per_million: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_usd_per_million: Option<f64>,
    pub output_usd_per_million: f64,
}

pub(crate) fn load_evaluation_candidates(
    path: &Path,
) -> Result<EvaluationCandidateManifest, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest: EvaluationCandidateManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    validate_evaluation_candidates(&manifest)?;
    Ok(manifest)
}

fn validate_evaluation_candidates(manifest: &EvaluationCandidateManifest) -> Result<(), String> {
    if manifest.candidates.is_empty() {
        return Err("evaluation manifest must contain at least one candidate".to_string());
    }
    let mut ids = HashSet::with_capacity(manifest.candidates.len());
    for candidate in &manifest.candidates {
        if !ids.insert(candidate.id.as_str()) {
            return Err(format!(
                "duplicate evaluation candidate id: {}",
                candidate.id
            ));
        }
        if !valid_candidate_id(&candidate.id) {
            return Err(format!(
                "candidate id {:?} must be a lowercase ASCII path-safe id",
                candidate.id,
            ));
        }
        if candidate.name.trim().is_empty() || candidate.model.trim().is_empty() {
            return Err("evaluation candidate name and model must be non-empty".to_string());
        }
        if candidate.roles.is_empty() {
            return Err(format!(
                "candidate {} must target at least one role",
                candidate.id
            ));
        }
        let unique_roles = candidate.roles.iter().copied().collect::<HashSet<_>>();
        if unique_roles.len() != candidate.roles.len() {
            return Err(format!(
                "candidate {} repeats an evaluation role",
                candidate.id
            ));
        }
        if candidate.provider == EvaluationProvider::LocalModels
            && (candidate.reasoning_effort.is_some()
                || candidate.base_url.is_some()
                || candidate.bridge_path.is_some())
        {
            return Err(format!(
                "local candidate {} cannot set hosted-provider options",
                candidate.id
            ));
        }
        if candidate.provider != EvaluationProvider::FoundationLocal
            && candidate.bridge_path.is_some()
        {
            return Err(format!(
                "candidate {} bridge_path is only valid for foundation_local",
                candidate.id
            ));
        }
        if let Some(pricing) = candidate.pricing {
            for value in [
                pricing.input_usd_per_million,
                pricing.cached_input_usd_per_million.unwrap_or(0.0),
                pricing.output_usd_per_million,
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!("candidate {} has invalid pricing", candidate.id));
                }
            }
        }
    }
    Ok(())
}

fn valid_candidate_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 100
        && !id.contains("..")
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'-' | b'.')
        })
}

const fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_duplicate_roles_and_invalid_prices() {
        let candidate = EvaluationCandidate {
            id: "candidate".to_string(),
            name: "Candidate".to_string(),
            provider: EvaluationProvider::OpenRouter,
            model: "vendor/model".to_string(),
            roles: vec![RuntimeEvalRole::Primary, RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            bridge_path: None,
            pricing: Some(ModelPricing {
                input_usd_per_million: -1.0,
                cached_input_usd_per_million: None,
                output_usd_per_million: 1.0,
            }),
            enabled: true,
            notes: String::new(),
        };
        let error = validate_evaluation_candidates(&EvaluationCandidateManifest {
            candidates: vec![candidate.clone()],
        })
        .expect_err("invalid manifest");
        assert!(error.contains("repeats an evaluation role"));

        let mut report_candidate = candidate;
        report_candidate.roles = vec![RuntimeEvalRole::Primary];
        report_candidate.base_url = Some("https://user:secret@example.test/v1".to_string());
        report_candidate.pricing = None;
        let report_value = serde_json::to_value(&report_candidate).expect("report metadata");
        assert!(report_value.get("base_url").is_none());
        assert!(report_value.get("bridge_path").is_none());

        let mut unsafe_candidate = report_candidate;
        unsafe_candidate.id = "../escape".to_string();
        assert!(
            validate_evaluation_candidates(&EvaluationCandidateManifest {
                candidates: vec![unsafe_candidate],
            })
            .is_err()
        );
    }
}
