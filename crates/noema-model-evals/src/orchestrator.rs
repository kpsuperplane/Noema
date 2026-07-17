use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::process::Command;

use crate::{
    download::resolve_candidate,
    manifest::{ModelCandidate, SuiteConfig},
    model_report::ModelEvalReport,
    report::{MatrixEntry, MatrixReport},
};

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn select_candidates(
    available: &[ModelCandidate],
    requested: &[String],
) -> Result<Vec<ModelCandidate>, String> {
    if requested.is_empty() {
        return Ok(available.to_vec());
    }
    let mut selected = Vec::with_capacity(requested.len());
    for id in requested {
        let candidate = available
            .iter()
            .find(|candidate| candidate.id == *id)
            .ok_or_else(|| format!("unknown candidate id: {id}"))?;
        if selected
            .iter()
            .any(|existing: &ModelCandidate| existing.id == candidate.id)
        {
            return Err(format!("candidate selected more than once: {id}"));
        }
        selected.push(candidate.clone());
    }
    Ok(selected)
}

pub(crate) async fn prepare_candidates(candidates: &[ModelCandidate]) -> Result<(), String> {
    let eval_home = workspace_root().join("target/noema-model-evals/cache/noema");
    for candidate in candidates {
        let path = resolve_candidate(candidate, &eval_home).await?;
        println!("ready {} at {}", candidate.name, path.display());
    }
    Ok(())
}

pub(crate) async fn run_matrix(
    candidates: Vec<ModelCandidate>,
    suite: SuiteConfig,
    run_resource_probe: bool,
) -> Result<PathBuf, String> {
    let workspace = workspace_root();
    let runtime_root = workspace.join("crates/noema-desktop/binaries/runtime");
    if !runtime_root.is_dir() {
        return Err(format!(
            "pinned runtime is missing at {}; run the desktop runtime preparation first",
            runtime_root.display()
        ));
    }
    let eval_home = workspace.join("target/noema-model-evals/cache/noema");
    let run_id = run_id();
    let run_root = workspace
        .join("target/noema-model-evals/runs")
        .join(&run_id);
    fs::create_dir_all(&run_root)
        .map_err(|error| format!("failed to create {}: {error}", run_root.display()))?;
    let mut matrix = MatrixReport::new(run_id, &suite, candidates.clone(), run_resource_probe);

    for candidate in &candidates {
        let model_path = match resolve_candidate(candidate, &eval_home).await {
            Ok(path) => path,
            Err(error) => {
                for repetition in 1..=suite.repetitions {
                    matrix.entries.push(MatrixEntry {
                        candidate_id: candidate.id.clone(),
                        repetition,
                        report: None,
                        worker_error: Some(error.clone()),
                    });
                }
                matrix.write(&run_root)?;
                continue;
            }
        };
        for repetition in 1..=suite.repetitions {
            println!(
                "evaluating {} (run {}/{})",
                candidate.name, repetition, suite.repetitions
            );
            let entry = run_worker(
                candidate,
                repetition,
                &model_path,
                &runtime_root,
                &run_root,
                &suite,
                run_resource_probe,
            )
            .await;
            if let Some(report) = &entry.report {
                println!(
                    "finished {}: critical {}/{}, all {}/{}",
                    candidate.name,
                    report.passed_critical_cases,
                    report.total_critical_cases,
                    report.passed_cases,
                    report.total_cases
                );
            } else if let Some(error) = &entry.worker_error {
                println!("failed {}: {error}", candidate.name);
            }
            matrix.entries.push(entry);
            matrix.write(&run_root)?;
        }
    }
    Ok(run_root)
}

async fn run_worker(
    candidate: &ModelCandidate,
    repetition: u32,
    model_path: &Path,
    runtime_root: &Path,
    run_root: &Path,
    suite: &SuiteConfig,
    run_resource_probe: bool,
) -> MatrixEntry {
    let report_path = run_root.join(format!("{}-{repetition}.json", candidate.id));
    let worker_home = run_root
        .join("workers")
        .join(format!("{}-{repetition}", candidate.id));
    let executable = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            return worker_failure(candidate, repetition, error.to_string());
        }
    };
    let mut child = match Command::new(executable)
        .arg("worker")
        .arg(&candidate.id)
        .arg(model_path)
        .arg(runtime_root)
        .arg(&report_path)
        .arg(suite.context_window_tokens.to_string())
        .arg(suite.generation_timeout_seconds.to_string())
        .arg(suite.startup_timeout_seconds.to_string())
        .arg(run_resource_probe.to_string())
        .env("NOEMA_HOME", worker_home)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return worker_failure(candidate, repetition, error.to_string()),
    };
    let timeout = Duration::from_secs(suite.worker_timeout_seconds);
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => return worker_failure(candidate, repetition, error.to_string()),
        Err(_) => {
            let _ = child.kill().await;
            return worker_failure(
                candidate,
                repetition,
                format!(
                    "worker exceeded {} seconds and was terminated",
                    suite.worker_timeout_seconds
                ),
            );
        }
    };
    if !status.success() {
        return worker_failure(
            candidate,
            repetition,
            format!("worker exited with {status}"),
        );
    }
    let report = match fs::read(&report_path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            serde_json::from_slice::<ModelEvalReport>(&bytes).map_err(|e| e.to_string())
        }) {
        Ok(report) => report,
        Err(error) => return worker_failure(candidate, repetition, error),
    };
    MatrixEntry {
        candidate_id: candidate.id.clone(),
        repetition,
        report: Some(report),
        worker_error: None,
    }
}

fn worker_failure(candidate: &ModelCandidate, repetition: u32, error: String) -> MatrixEntry {
    MatrixEntry {
        candidate_id: candidate.id.clone(),
        repetition,
        report: None,
        worker_error: Some(error),
    }
}

fn run_id() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("run-{seconds}")
}
