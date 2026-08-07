use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use noema_runtime::eval_support::run_runtime_suite_for_roles;

use crate::{
    comparative_judge::run_required_comparisons,
    hosted_provider::{HostedProviderContext, HostedProviderKind, HostedProviderSpec},
    manifest::SuiteConfig,
    matrix_manifest::EvaluationCandidate,
    matrix_report::{EvaluationMatrixEntry, EvaluationMatrixReport, EvaluationRunMode},
    orchestrator::workspace_root,
    role_policy::RolePolicyManifest,
};

pub(crate) fn select_evaluation_candidates(
    available: &[EvaluationCandidate],
    requested: &[String],
) -> Result<Vec<EvaluationCandidate>, String> {
    if requested.is_empty() {
        return Ok(available
            .iter()
            .filter(|candidate| candidate.enabled)
            .cloned()
            .collect());
    }
    let mut selected = Vec::with_capacity(requested.len());
    for id in requested {
        let candidate = available
            .iter()
            .find(|candidate| candidate.id == *id)
            .ok_or_else(|| format!("unknown evaluation candidate id: {id}"))?;
        if selected
            .iter()
            .any(|existing: &EvaluationCandidate| existing.id == candidate.id)
        {
            return Err(format!(
                "evaluation candidate selected more than once: {id}"
            ));
        }
        selected.push(candidate.clone());
    }
    Ok(selected)
}

pub(crate) async fn run_evaluation_matrix(
    candidates: Vec<EvaluationCandidate>,
    suite: SuiteConfig,
    policies: RolePolicyManifest,
    mode: EvaluationRunMode,
) -> Result<PathBuf, String> {
    if candidates.is_empty() {
        return Err("no enabled evaluation candidates were selected".to_string());
    }
    let workspace = workspace_root();
    let run_root = workspace
        .join("target/noema-model-evals/model-matrix")
        .join(run_id());
    fs::create_dir_all(&run_root)
        .map_err(|error| format!("failed to create {}: {error}", run_root.display()))?;
    if mode == EvaluationRunMode::DefaultDecision {
        validate_decision_candidates(&candidates, &policies)?;
    }
    let mut report = EvaluationMatrixReport::new(
        run_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("run")
            .to_string(),
        &suite,
        candidates.clone(),
        policies,
        mode,
    );
    report.write(&run_root)?;

    let hosted = match HostedProviderContext::from_process_env().await {
        Ok(hosted) => hosted,
        Err(error) => {
            report.fail(error.clone());
            report.write(&run_root)?;
            return Err(error);
        }
    };

    let run_result = async {
        for candidate in &candidates {
            println!(
                "evaluating {} (OpenRouter, {})",
                candidate.name, candidate.model
            );
            run_openrouter_candidate(candidate, &hosted, &suite, &run_root, &mut report).await?;
        }
        if mode == EvaluationRunMode::DefaultDecision {
            println!("running blinded incumbent comparisons");
            run_required_comparisons(&mut report, &hosted, &suite).await;
            report.write(&run_root)?;
        }
        Ok::<(), String>(())
    }
    .await;

    hosted.shutdown().await;
    match run_result {
        Ok(()) => {
            report.finish();
            report.write(&run_root)?;
            Ok(run_root)
        }
        Err(error) => {
            report.fail(error.clone());
            let _ = report.write(&run_root);
            Err(error)
        }
    }
}

async fn run_openrouter_candidate(
    candidate: &EvaluationCandidate,
    context: &HostedProviderContext,
    suite: &SuiteConfig,
    run_root: &std::path::Path,
    report: &mut EvaluationMatrixReport,
) -> Result<(), String> {
    let spec = HostedProviderSpec {
        kind: HostedProviderKind::OpenRouter,
        model: candidate.model.clone(),
        reasoning_effort: candidate.reasoning_effort,
        timeout_seconds: suite.generation_timeout_seconds,
        base_url: candidate.base_url.clone(),
        bridge_path: None,
    };
    let provider = context.build_provider(&spec).map(|(_, provider)| provider);
    for repetition in 1..=report.repetitions {
        let entry = match &provider {
            Ok(provider) => {
                match run_runtime_suite_for_roles(
                    provider,
                    &candidate.model,
                    &candidate.roles,
                    candidate.reasoning_effort,
                )
                .await
                {
                    Ok(cases) => EvaluationMatrixEntry {
                        candidate_id: candidate.id.clone(),
                        repetition,
                        cases,
                        error: None,
                    },
                    Err(error) => failed_entry(candidate, repetition, error),
                }
            }
            Err(error) => failed_entry(candidate, repetition, error.clone()),
        };
        print_entry(&entry);
        report.push(entry);
        report.write(run_root)?;
    }
    Ok(())
}

fn validate_decision_candidates(
    candidates: &[EvaluationCandidate],
    policies: &RolePolicyManifest,
) -> Result<(), String> {
    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.base_url.is_some())
    {
        return Err(format!(
            "default decision candidate {} cannot override the OpenRouter base URL",
            candidate.id
        ));
    }
    for role in noema_runtime::eval_support::RuntimeEvalRole::ALL {
        if !candidates
            .iter()
            .any(|candidate| candidate.roles.contains(role))
        {
            return Err(format!(
                "default decision has no candidate for role {}",
                role.as_str()
            ));
        }
        let incumbent = &policies.policy(*role).incumbent_candidate_id;
        if !candidates
            .iter()
            .any(|candidate| candidate.id == *incumbent && candidate.roles.contains(role))
        {
            return Err(format!(
                "default decision is missing incumbent {incumbent} for role {}",
                role.as_str()
            ));
        }
    }
    Ok(())
}

fn failed_entry(
    candidate: &EvaluationCandidate,
    repetition: u32,
    error: String,
) -> EvaluationMatrixEntry {
    EvaluationMatrixEntry {
        candidate_id: candidate.id.clone(),
        repetition,
        cases: Vec::new(),
        error: Some(error),
    }
}

fn print_entry(entry: &EvaluationMatrixEntry) {
    if let Some(error) = &entry.error {
        println!("  run {} failed: {error}", entry.repetition);
    } else {
        let passed = entry.cases.iter().filter(|case| case.passed).count();
        println!(
            "  run {}: {passed}/{} cases",
            entry.repetition,
            entry.cases.len()
        );
    }
}

fn run_id() -> String {
    let nanoseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("run-{nanoseconds}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_runtime::eval_support::RuntimeEvalRole;

    use crate::matrix_manifest::{ModelPricing, RecommendationTarget};

    #[test]
    fn default_selection_skips_disabled_candidates_but_explicit_selection_keeps_them() {
        let mut enabled = candidate("enabled");
        let mut disabled = candidate("disabled");
        enabled.enabled = true;
        disabled.enabled = false;
        let candidates = vec![enabled, disabled];
        assert_eq!(
            select_evaluation_candidates(&candidates, &[])
                .expect("default selection")
                .len(),
            1
        );
        assert_eq!(
            select_evaluation_candidates(&candidates, &["disabled".to_string()])
                .expect("explicit selection")[0]
                .id,
            "disabled"
        );
    }

    #[test]
    fn default_decision_requires_every_runtime_role() {
        let error = validate_decision_candidates(
            &[candidate("primary-only")],
            &test_policies("primary-only"),
        )
        .expect_err("missing decision roles");
        assert!(error.contains("task_simple"));

        let mut custom_endpoint = candidate("custom-endpoint");
        custom_endpoint.base_url = Some("https://example.test/v1".to_string());
        let error =
            validate_decision_candidates(&[custom_endpoint], &test_policies("custom-endpoint"))
                .expect_err("custom decision endpoint");
        assert!(error.contains("cannot override the OpenRouter base URL"));
    }

    fn candidate(id: &str) -> EvaluationCandidate {
        EvaluationCandidate {
            id: id.to_string(),
            name: id.to_string(),
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

    fn test_policies(incumbent: &str) -> RolePolicyManifest {
        RolePolicyManifest {
            schema_version: 1,
            judge: crate::role_policy::JudgePolicy {
                model: "judge/model".to_string(),
                reasoning_effort: None,
                maximum_output_tokens: 256,
            },
            policies: RuntimeEvalRole::ALL
                .iter()
                .copied()
                .map(|role| crate::role_policy::RolePolicy {
                    role,
                    incumbent_candidate_id: incumbent.to_string(),
                    minimum_cases: 5,
                    minimum_quality_score: 0.8,
                    maximum_error_rate: 0.1,
                    maximum_p95_latency_ms: 120_000,
                    replacement_quality_margin: 0.02,
                    deterministic_weight: 1.0,
                    judge_weight: 0.0,
                    judge_case_ids: Vec::new(),
                })
                .collect(),
        }
    }
}
