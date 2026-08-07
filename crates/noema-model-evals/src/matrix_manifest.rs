use std::{collections::HashSet, fs, path::Path};

use noema_providers::{ProviderKind, ReasoningEffort};
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
    pub model: String,
    pub roles: Vec<RuntimeEvalRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing)]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_response_models: Vec<String>,
    pub targets: Vec<RecommendationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecommendationTarget {
    pub provider: String,
    pub model_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
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
        if candidate.targets.is_empty() {
            return Err(format!(
                "candidate {} must map at least one recommendation target",
                candidate.id
            ));
        }
        let mut target_providers = HashSet::with_capacity(candidate.targets.len());
        for target in &candidate.targets {
            let provider = target.provider.parse::<ProviderKind>().map_err(|_| {
                format!(
                    "candidate {} has unsupported recommendation target {}",
                    candidate.id, target.provider
                )
            })?;
            if provider.as_str() != target.provider {
                return Err(format!(
                    "candidate {} recommendation target must use canonical provider id {}",
                    candidate.id,
                    provider.as_str()
                ));
            }
            if !target_providers.insert(target.provider.as_str()) {
                return Err(format!(
                    "candidate {} repeats recommendation target {}",
                    candidate.id, target.provider
                ));
            }
            if matches!(
                provider,
                ProviderKind::FoundationLocal | ProviderKind::LocalModels
            ) {
                return Err(format!(
                    "candidate {} cannot map local recommendation target {}",
                    candidate.id, target.provider
                ));
            }
            if target.model_profile.trim().is_empty() {
                return Err(format!(
                    "candidate {} has an empty {} model profile",
                    candidate.id, target.provider
                ));
            }
        }
        let valid_openrouter_target = candidate.targets.iter().any(|target| {
            target.provider == ProviderKind::OpenRouter.as_str()
                && target.model_profile == candidate.model
                && target.reasoning_effort == candidate.reasoning_effort
        });
        if !valid_openrouter_target {
            return Err(format!(
                "candidate {} must map its exact OpenRouter model and effort",
                candidate.id
            ));
        }
        let accepted_response_models = candidate
            .accepted_response_models
            .iter()
            .map(|model| model.trim())
            .collect::<HashSet<_>>();
        if accepted_response_models.len() != candidate.accepted_response_models.len()
            || accepted_response_models.contains("")
        {
            return Err(format!(
                "candidate {} has blank or duplicate accepted response models",
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
        } else if candidate.enabled {
            return Err(format!(
                "enabled candidate {} must include an OpenRouter price snapshot",
                candidate.id
            ));
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
    fn manifest_rejects_duplicate_roles_and_hides_private_base_url() {
        let candidate = EvaluationCandidate {
            id: "candidate".to_string(),
            name: "Candidate".to_string(),
            model: "vendor/model".to_string(),
            roles: vec![RuntimeEvalRole::Primary, RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            accepted_response_models: Vec::new(),
            targets: vec![RecommendationTarget {
                provider: "openrouter".to_string(),
                model_profile: "vendor/model".to_string(),
                reasoning_effort: None,
            }],
            pricing: Some(ModelPricing {
                input_usd_per_million: 1.0,
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

        let mut unsafe_candidate = report_candidate;
        unsafe_candidate.id = "../escape".to_string();
        assert!(
            validate_evaluation_candidates(&EvaluationCandidateManifest {
                candidates: vec![unsafe_candidate],
            })
            .is_err()
        );
    }

    #[test]
    fn manifest_requires_exact_openrouter_mapping_and_rejects_local_targets() {
        let mut candidate = candidate();
        candidate.targets[0].model_profile = "vendor/other".to_string();
        let error = validate_evaluation_candidates(&EvaluationCandidateManifest {
            candidates: vec![candidate.clone()],
        })
        .expect_err("mismatched OpenRouter target");
        assert!(error.contains("exact OpenRouter model and effort"));

        candidate.targets = vec![RecommendationTarget {
            provider: "local_models".to_string(),
            model_profile: "local-model".to_string(),
            reasoning_effort: None,
        }];
        let error = validate_evaluation_candidates(&EvaluationCandidateManifest {
            candidates: vec![candidate],
        })
        .expect_err("local target");
        assert!(error.contains("cannot map local recommendation target"));
    }

    #[test]
    fn bundled_luna_effort_candidates_are_primary_only() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/model-matrix/candidates.toml");
        let manifest = load_evaluation_candidates(&path).expect("candidate manifest");
        for (id, effort) in [
            ("openrouter-luna-medium-primary", ReasoningEffort::Medium),
            ("openrouter-luna-high-primary", ReasoningEffort::High),
        ] {
            let candidate = manifest
                .candidates
                .iter()
                .find(|candidate| candidate.id == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(candidate.model, "openai/gpt-5.6-luna");
            assert_eq!(candidate.reasoning_effort, Some(effort));
            assert_eq!(candidate.roles, [RuntimeEvalRole::Primary]);
        }
    }

    fn candidate() -> EvaluationCandidate {
        EvaluationCandidate {
            id: "candidate".to_string(),
            name: "Candidate".to_string(),
            model: "vendor/model".to_string(),
            roles: vec![RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            accepted_response_models: Vec::new(),
            targets: vec![RecommendationTarget {
                provider: "openrouter".to_string(),
                model_profile: "vendor/model".to_string(),
                reasoning_effort: None,
            }],
            pricing: Some(ModelPricing {
                input_usd_per_million: 1.0,
                cached_input_usd_per_million: None,
                output_usd_per_million: 1.0,
            }),
            enabled: true,
            notes: String::new(),
        }
    }
}
