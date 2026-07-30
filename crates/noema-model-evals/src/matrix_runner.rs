use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use noema_runtime::eval_support::run_runtime_suite_for_roles;

use crate::{
    hosted_provider::{HostedProviderContext, HostedProviderKind, HostedProviderSpec},
    manifest::{ModelCandidate, SuiteConfig},
    matrix_manifest::{EvaluationCandidate, EvaluationProvider},
    matrix_report::{EvaluationMatrixEntry, EvaluationMatrixReport},
    orchestrator::{run_worker, workspace_root},
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
    local_candidates: &[ModelCandidate],
    suite: SuiteConfig,
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
    let mut report = EvaluationMatrixReport::new(
        run_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("run")
            .to_string(),
        &suite,
        candidates.clone(),
    );
    report.write(&run_root)?;

    let needs_hosted = candidates
        .iter()
        .any(|candidate| candidate.provider != EvaluationProvider::LocalModels);
    let hosted = if needs_hosted {
        Some(HostedProviderContext::from_process_env().await?)
    } else {
        None
    };

    let run_result = async {
        for candidate in &candidates {
            println!(
                "evaluating {} ({:?}, {})",
                candidate.name, candidate.provider, candidate.model
            );
            match candidate.provider {
                EvaluationProvider::LocalModels => {
                    run_local_candidate(
                        candidate,
                        local_candidates,
                        &suite,
                        &run_root,
                        &mut report,
                    )
                    .await?;
                }
                _ => {
                    let context = hosted
                        .as_ref()
                        .ok_or_else(|| "hosted provider context was not initialized".to_string())?;
                    run_hosted_candidate(candidate, context, &suite, &run_root, &mut report)
                        .await?;
                }
            }
        }
        Ok::<(), String>(())
    }
    .await;

    if let Some(hosted) = hosted {
        hosted.shutdown().await;
    }
    run_result?;
    Ok(run_root)
}

async fn run_hosted_candidate(
    candidate: &EvaluationCandidate,
    context: &HostedProviderContext,
    suite: &SuiteConfig,
    run_root: &std::path::Path,
    report: &mut EvaluationMatrixReport,
) -> Result<(), String> {
    let spec = HostedProviderSpec {
        kind: hosted_kind(candidate.provider)?,
        model: candidate.model.clone(),
        reasoning_effort: candidate.reasoning_effort,
        timeout_seconds: suite.generation_timeout_seconds,
        base_url: candidate.base_url.clone(),
        bridge_path: candidate.bridge_path.as_ref().map(PathBuf::from),
    };
    let provider = context.build_provider(&spec).map(|(_, provider)| provider);
    for repetition in 1..=suite.repetitions {
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

async fn run_local_candidate(
    candidate: &EvaluationCandidate,
    local_candidates: &[ModelCandidate],
    suite: &SuiteConfig,
    run_root: &std::path::Path,
    report: &mut EvaluationMatrixReport,
) -> Result<(), String> {
    let source = local_candidates
        .iter()
        .find(|local| local.id == candidate.model)
        .ok_or_else(|| {
            format!(
                "local evaluation candidate {} references unknown GGUF candidate {}",
                candidate.id, candidate.model
            )
        })?;
    let workspace = workspace_root();
    let runtime_root = workspace.join("crates/noema-desktop/binaries/runtime");
    if !runtime_root.is_dir() {
        return Err(format!(
            "pinned runtime is missing at {}",
            runtime_root.display()
        ));
    }
    let eval_home = workspace.join("target/noema-model-evals/cache/noema");
    let model_path = crate::download::resolve_candidate(source, &eval_home).await?;
    let mut worker_candidate = source.clone();
    worker_candidate.id.clone_from(&candidate.id);
    worker_candidate.name.clone_from(&candidate.name);
    for repetition in 1..=suite.repetitions {
        let local = run_worker(
            &worker_candidate,
            repetition,
            &model_path,
            &runtime_root,
            run_root,
            suite,
            (false, &candidate.roles),
        )
        .await;
        let entry = match local.report {
            Some(local_report) => {
                let runtime_error = local_report.runtime_error;
                EvaluationMatrixEntry {
                    candidate_id: candidate.id.clone(),
                    repetition,
                    cases: local_report.cases,
                    error: runtime_error,
                }
            }
            None => failed_entry(
                candidate,
                repetition,
                local
                    .worker_error
                    .unwrap_or_else(|| "local worker failed".to_string()),
            ),
        };
        print_entry(&entry);
        report.push(entry);
        report.write(run_root)?;
    }
    Ok(())
}

fn hosted_kind(provider: EvaluationProvider) -> Result<HostedProviderKind, String> {
    match provider {
        EvaluationProvider::Codex => Ok(HostedProviderKind::Codex),
        EvaluationProvider::OpenAi => Ok(HostedProviderKind::OpenAi),
        EvaluationProvider::OpenRouter => Ok(HostedProviderKind::OpenRouter),
        EvaluationProvider::FoundationLocal => Ok(HostedProviderKind::FoundationLocal),
        EvaluationProvider::LocalModels => Err("local_models is not hosted".to_string()),
    }
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

    fn candidate(id: &str) -> EvaluationCandidate {
        EvaluationCandidate {
            id: id.to_string(),
            name: id.to_string(),
            provider: EvaluationProvider::OpenRouter,
            model: "vendor/model".to_string(),
            roles: vec![RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            bridge_path: None,
            pricing: None,
            enabled: true,
            notes: String::new(),
        }
    }
}
