//! Isolated qualification runner for local GGUF candidates.

mod download;
mod manifest;
mod memory;
mod model_report;
mod orchestrator;
mod provider_suite;
mod report;
mod resource_probe;

use std::{fs, path::PathBuf};

use crate::{
    manifest::{load_candidates, load_suite},
    model_report::ModelEvalConfig,
    orchestrator::{prepare_candidates, run_matrix, select_candidates, workspace_root},
    provider_suite::run_provider_suite,
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
        "run" => {
            let selected = select_candidates(&manifest.candidates, &remaining)?;
            let suite = load_suite(&root.join("evals/local-models/suite.toml"))?;
            let report_root = run_matrix(selected, suite, false).await?;
            println!("reports written to {}", report_root.display());
            Ok(())
        }
        "soak" => {
            if remaining.is_empty() {
                return Err("soak requires at least one candidate id".to_string());
            }
            let selected = select_candidates(&manifest.candidates, &remaining)?;
            let suite = load_suite(&root.join("evals/local-models/suite.toml"))?;
            let report_root = run_matrix(selected, suite, true).await?;
            println!("reports written to {}", report_root.display());
            Ok(())
        }
        _ => Err(usage()),
    }
}

async fn run_worker(arguments: &[String]) -> Result<(), String> {
    if arguments.len() != 8 {
        return Err("internal worker expected 8 arguments".to_string());
    }
    let context_window_tokens = parse_u32(&arguments[4], "context window")?;
    let timeout_seconds = parse_u64(&arguments[5], "generation timeout")?;
    let startup_timeout_seconds = parse_u64(&arguments[6], "startup timeout")?;
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

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn parse_u64(value: &str, label: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn usage() -> String {
    "usage: noema-model-evals list | prepare [candidate-id ...] | run [candidate-id ...] | soak [candidate-id ...]".to_string()
}
