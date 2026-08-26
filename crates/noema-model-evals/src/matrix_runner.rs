use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use noema_runtime::eval_support::{
    run_runtime_case_for_roles, runtime_eval_case_descriptors_for_roles,
};

use crate::{
    comparative_judge::run_required_comparisons,
    hosted_provider::{HostedProviderContext, OpenRouterProviderSpec},
    manifest::SuiteConfig,
    matrix_manifest::EvaluationCandidate,
    matrix_report::{EvaluationMatrixReport, EvaluationRunMode},
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
    let report = EvaluationMatrixReport::new(
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
    execute_evaluation_matrix(report, suite, run_root).await
}

pub(crate) async fn run_planned_evaluation_matrix(
    decision_id: &str,
    candidates: Vec<EvaluationCandidate>,
    suite: SuiteConfig,
    policies: RolePolicyManifest,
    run_root: PathBuf,
) -> Result<PathBuf, String> {
    validate_decision_candidates(&candidates, &policies)?;
    fs::create_dir_all(&run_root)
        .map_err(|error| format!("failed to create {}: {error}", run_root.display()))?;
    let expected = EvaluationMatrixReport::new(
        decision_id.to_string(),
        &suite,
        candidates,
        policies,
        EvaluationRunMode::DefaultDecision,
    );
    let report = if run_root.join("matrix.json").is_file() {
        let mut report = EvaluationMatrixReport::read(&run_root)?;
        if report.decision_fingerprint != expected.decision_fingerprint
            || report.run_id != decision_id
        {
            return Err("checkpoint does not match the immutable decision plan".to_string());
        }
        if report.status == crate::matrix_report::EvaluationRunStatus::Complete {
            return Ok(run_root);
        }
        report.resume();
        report.write(&run_root)?;
        report
    } else {
        expected.write(&run_root)?;
        expected
    };
    execute_evaluation_matrix(report, suite, run_root).await
}

async fn execute_evaluation_matrix(
    mut report: EvaluationMatrixReport,
    suite: SuiteConfig,
    run_root: PathBuf,
) -> Result<PathBuf, String> {
    let candidates = report.candidates.clone();
    let mode = report.mode;

    let hosted = match HostedProviderContext::from_process_env(&run_root) {
        Ok(hosted) => hosted,
        Err(error) => {
            report.fail(error.clone());
            report.write(&run_root)?;
            return Err(error);
        }
    };
    let mut planned_models = candidates
        .iter()
        .map(|candidate| candidate.model.clone())
        .collect::<Vec<_>>();
    if mode == EvaluationRunMode::DefaultDecision {
        planned_models.push(report.policies.judge.model.clone());
    }
    planned_models.sort();
    planned_models.dedup();
    if let Err(error) = hosted.preflight_openrouter_models(&planned_models).await {
        report.fail(error.clone());
        report.write(&run_root)?;
        return Err(error);
    }

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
    let spec = OpenRouterProviderSpec {
        model: candidate.model.clone(),
        reasoning_effort: candidate.reasoning_effort,
        timeout_seconds: suite.generation_timeout_seconds,
        base_url: candidate.base_url.clone(),
    };
    let provider = context.build_provider(&spec);
    let descriptors = runtime_eval_case_descriptors_for_roles(
        &candidate.model,
        &candidate.roles,
        candidate.reasoning_effort,
    )?;
    for repetition in 1..=report.repetitions {
        for descriptor in &descriptors {
            if report.has_case(&candidate.id, repetition, &descriptor.case_id) {
                continue;
            }
            let case = match &provider {
                Ok(provider) => run_runtime_case_for_roles(
                    provider,
                    &candidate.model,
                    &candidate.roles,
                    candidate.reasoning_effort,
                    &descriptor.case_id,
                )
                .await
                .map_err(|error| format!("{}: {error}", descriptor.case_id))?,
                Err(error) => return Err(error.clone()),
            };
            println!(
                "  run {repetition}: {} {}",
                descriptor.case_id,
                if case.passed { "passed" } else { "failed" }
            );
            report.record_case(&candidate.id, repetition, case)?;
            report.write(run_root)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_decision_candidates(
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
