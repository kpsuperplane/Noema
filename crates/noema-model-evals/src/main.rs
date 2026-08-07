//! Production-derived model qualification across Noema's provider adapters.

mod comparative_judge;
mod decision_plan;
mod download;
mod hosted_provider;
mod manifest;
mod matrix_manifest;
mod matrix_report;
mod matrix_runner;
mod memory;
mod model_report;
mod orchestrator;
mod provider_suite;
mod report;
mod resource_probe;
mod role_policy;

use std::{fs, path::PathBuf};

use crate::{
    decision_plan::DecisionPlan,
    manifest::{load_candidates, load_suite},
    matrix_manifest::load_evaluation_candidates,
    matrix_report::EvaluationRunMode,
    matrix_runner::{
        run_evaluation_matrix, run_planned_evaluation_matrix, select_evaluation_candidates,
    },
    model_report::ModelEvalConfig,
    orchestrator::{prepare_candidates, run_matrix, select_candidates, workspace_root},
    provider_suite::run_provider_suite,
    role_policy::load_role_policies,
};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("noema-model-evals: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "list".to_string());
    let remaining = arguments.collect::<Vec<_>>();
    if command == "worker" {
        return run_worker(&remaining).await;
    }

    let root = workspace_root();
    if command == "matrix" {
        return run_openrouter_matrix(&root, &remaining).await;
    }
    if command == "defaults" {
        return run_defaults_workflow(&root, &remaining).await;
    }
    let manifest = load_candidates(&root.join("evals/local-models/candidates.toml"))?;
    match command.as_str() {
        "list" => {
            for candidate in manifest.candidates {
                println!(
                    "{}\t{}\t{:.2} GB\t{}",
                    candidate.id,
                    candidate.name,
                    candidate.bytes as f64 / 1_000_000_000.0,
                    candidate.notes
                );
            }
            Ok(())
        }
        "prepare" => {
            let selected = select_candidates(&manifest.candidates, &remaining)?;
            prepare_candidates(&selected).await
        }
        "run" | "soak" => {
            let soak = command == "soak";
            if soak && remaining.is_empty() {
                return Err("soak requires at least one candidate id".to_string());
            }
            let selected = select_candidates(&manifest.candidates, &remaining)?;
            let suite = load_suite(&root.join("evals/local-models/suite.toml"))?;
            let report_root = run_matrix(selected, suite, soak).await?;
            println!("reports written to {}", report_root.display());
            Ok(())
        }
        _ => Err(usage()),
    }
}

async fn run_defaults_workflow(root: &std::path::Path, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "plan" => {
            let manifest =
                load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))?;
            let candidates = select_evaluation_candidates(&manifest.candidates, &arguments[1..])?;
            let suite = load_suite(&root.join("evals/model-matrix/suite.toml"))?;
            let policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))?;
            let plan = DecisionPlan::create(suite, policies, candidates)?;
            let path = plan.write_new()?;
            println!("decision plan: {}", path.display());
            println!(
                "maximum estimated OpenRouter cost: ${:.4}",
                plan.estimated_max_cost_usd
            );
            if plan.git_dirty {
                println!("warning: the plan records a dirty Git worktree");
            }
            Ok(())
        }
        "estimate" | "run" => {
            if arguments.len() != 2 {
                return Err(format!(
                    "defaults {command} requires one decision plan path"
                ));
            }
            let plan_path = PathBuf::from(&arguments[1]);
            let plan = DecisionPlan::load(&plan_path)?;
            if command == "estimate" {
                println!(
                    "{}: at most ${:.4} across {} candidates and {} repetitions",
                    plan.decision_id,
                    plan.estimated_max_cost_usd,
                    plan.candidates.len(),
                    plan.suite.decision_repetitions(),
                );
                return Ok(());
            }
            plan.validate_execution_git_state()?;
            require_explicit_evaluation_home()?;
            let run_root = root
                .join("target/noema-model-evals/decisions")
                .join(&plan.decision_id);
            fs::create_dir_all(&run_root)
                .map_err(|error| format!("failed to create {}: {error}", run_root.display()))?;
            let plan_copy = run_root.join("plan.json");
            if !plan_copy.exists() {
                fs::copy(&plan_path, &plan_copy).map_err(|error| {
                    format!("failed to copy plan to {}: {error}", plan_copy.display())
                })?;
            }
            let report_root = run_planned_evaluation_matrix(
                &plan.decision_id,
                plan.candidates,
                plan.suite,
                plan.policies,
                run_root,
            )
            .await?;
            println!("decision evidence: {}", report_root.display());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn require_explicit_evaluation_home() -> Result<(), String> {
    let home = std::env::var_os("NOEMA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "defaults run requires an explicit NOEMA_HOME".to_string())?;
    if !home.is_absolute() {
        return Err("defaults run requires an absolute NOEMA_HOME".to_string());
    }
    Ok(())
}

async fn run_openrouter_matrix(root: &std::path::Path, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage());
    };
    let manifest = load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))?;
    match command {
        "list" => {
            for candidate in manifest.candidates {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    candidate.id,
                    candidate.model,
                    if candidate.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    },
                    candidate
                        .roles
                        .iter()
                        .map(|role| role.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    candidate
                        .targets
                        .iter()
                        .map(|target| format!("{}:{}", target.provider, target.model_profile))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            Ok(())
        }
        "run" => {
            let requested = &arguments[1..];
            let mode = if requested.is_empty() {
                EvaluationRunMode::DefaultDecision
            } else {
                EvaluationRunMode::Exploration
            };
            if mode == EvaluationRunMode::DefaultDecision {
                require_explicit_evaluation_home()?;
            }
            let selected = select_evaluation_candidates(&manifest.candidates, requested)?;
            let suite = load_suite(&root.join("evals/model-matrix/suite.toml"))?;
            let policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))?;
            let report_root = run_evaluation_matrix(selected, suite, policies, mode).await?;
            println!("reports written to {}", report_root.display());
            Ok(())
        }
        _ => Err(usage()),
    }
}

async fn run_worker(arguments: &[String]) -> Result<(), String> {
    if arguments.len() != 9 {
        return Err("internal worker expected 9 arguments".to_string());
    }
    let context_window_tokens = parse(&arguments[4], "context window")?;
    let timeout_seconds = parse(&arguments[5], "generation timeout")?;
    let startup_timeout_seconds = parse(&arguments[6], "startup timeout")?;
    let run_resource_probe = arguments[7]
        .parse::<bool>()
        .map_err(|error| format!("invalid resource probe flag: {error}"))?;
    let report = run_provider_suite(ModelEvalConfig {
        model_id: arguments[0].clone(),
        model_path: PathBuf::from(&arguments[1]),
        runtime_root: PathBuf::from(&arguments[2]),
        context_window_tokens,
        timeout_seconds,
        startup_timeout_seconds,
        run_resource_probe,
        roles: serde_json::from_str(&arguments[8])
            .map_err(|error| format!("invalid evaluation roles: {error}"))?,
    })
    .await?;
    let report_path = PathBuf::from(&arguments[3]);
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize worker report: {error}"))?;
    fs::write(&report_path, json)
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;
    Ok(())
}

fn parse<T: std::str::FromStr>(value: &str, label: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn usage() -> String {
    "usage: noema-model-evals list | prepare [candidate-id ...] | run [candidate-id ...] | soak [candidate-id ...] | matrix list | matrix run [candidate-id ...] | defaults plan [candidate-id ...] | defaults estimate <plan> | defaults run <plan>".to_string()
}
