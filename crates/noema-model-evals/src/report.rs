use std::{fmt::Write, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    manifest::{ModelCandidate, SuiteConfig},
    model_report::ModelEvalReport,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MatrixReport {
    pub run_id: String,
    pub suite: MatrixSuiteMetadata,
    pub candidates: Vec<ModelCandidate>,
    pub entries: Vec<MatrixEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MatrixSuiteMetadata {
    pub context_window_tokens: u32,
    pub generation_timeout_seconds: u64,
    pub startup_timeout_seconds: u64,
    pub repetitions: u32,
    pub resource_probe: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MatrixEntry {
    pub candidate_id: String,
    pub repetition: u32,
    pub report: Option<ModelEvalReport>,
    pub worker_error: Option<String>,
}

impl MatrixReport {
    pub(crate) fn new(
        run_id: String,
        suite: &SuiteConfig,
        candidates: Vec<ModelCandidate>,
        resource_probe: bool,
    ) -> Self {
        Self {
            run_id,
            suite: MatrixSuiteMetadata {
                context_window_tokens: suite.context_window_tokens,
                generation_timeout_seconds: suite.generation_timeout_seconds,
                startup_timeout_seconds: suite.startup_timeout_seconds,
                repetitions: suite.repetitions,
                resource_probe,
            },
            candidates,
            entries: Vec::new(),
        }
    }

    pub(crate) fn write(&self, directory: &Path) -> Result<(), String> {
        fs::create_dir_all(directory)
            .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        let json = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("failed to serialize matrix report: {error}"))?;
        fs::write(directory.join("matrix.json"), json)
            .map_err(|error| format!("failed to write matrix JSON: {error}"))?;
        fs::write(directory.join("summary.md"), self.markdown())
            .map_err(|error| format!("failed to write matrix summary: {error}"))?;
        Ok(())
    }

    fn markdown(&self) -> String {
        let mut output = format!(
            "# Noema local-model evaluation {}\n\nPinned context: {} tokens. Repetitions: {}. Resource probe: {}.\n\n",
            self.run_id,
            self.suite.context_window_tokens,
            self.suite.repetitions,
            if self.suite.resource_probe {
                "yes"
            } else {
                "no"
            }
        );
        output.push_str(
            "| Candidate | Runtime | Critical | All cases | Load | Peak memory | Median case | Resource probe |\n|---|---:|---:|---:|---:|---:|---:|---|\n",
        );
        for candidate in &self.candidates {
            for entry in self
                .entries
                .iter()
                .filter(|entry| entry.candidate_id == candidate.id)
            {
                let columns = match &entry.report {
                    Some(report) if report.runtime_error.is_none() => {
                        let mut latencies = report
                            .cases
                            .iter()
                            .map(|case| case.latency_ms)
                            .collect::<Vec<_>>();
                        latencies.sort_unstable();
                        let median = latencies
                            .get(latencies.len().saturating_sub(1) / 2)
                            .copied()
                            .unwrap_or(0);
                        format!(
                            "{} | {}/{} | {}/{} | {:.2}s | {} | {:.2}s | {}",
                            report.backend.as_deref().unwrap_or("unknown"),
                            report.passed_critical_cases,
                            report.total_critical_cases,
                            report.passed_cases,
                            report.total_cases,
                            report.runtime_load_ms as f64 / 1_000.0,
                            report.runtime_memory.as_ref().map_or_else(
                                || "-".to_string(),
                                |memory| format!(
                                    "{:.2} GiB",
                                    memory.peak_bytes as f64 / 1_073_741_824.0
                                )
                            ),
                            median as f64 / 1_000.0,
                            resource_probe_label(report),
                        )
                    }
                    Some(report) => {
                        let error = report
                            .runtime_error
                            .as_deref()
                            .unwrap_or("unknown runtime failure")
                            .replace('|', "\\|");
                        format!(
                            "incompatible: {error} | 0/0 | 0/0 | {:.2}s | - | - | -",
                            report.runtime_load_ms as f64 / 1_000.0
                        )
                    }
                    None => {
                        let error = entry
                            .worker_error
                            .as_deref()
                            .unwrap_or("worker failed")
                            .replace('|', "\\|");
                        format!("worker error: {error} | 0/0 | 0/0 | - | - | - | -")
                    }
                };
                let _ = writeln!(
                    output,
                    "| {} (run {}) | {columns} |",
                    candidate.name, entry.repetition
                );
            }
        }
        output.push_str("\nCorrectness gates are deterministic typed/sentinel predicates. Speed is reported separately and does not raise a model's correctness score.\n");
        output
    }
}

fn resource_probe_label(report: &ModelEvalReport) -> String {
    let Some(probe) = report.resource_probe.as_ref() else {
        return "-".to_string();
    };
    if let Some(failure) = probe.failure.as_deref() {
        return format!("failed: {}", failure.replace('|', "\\|"));
    }
    format!(
        "{} tokens; {}/{} turns",
        probe.observed_input_tokens.unwrap_or_default(),
        probe.steady_turns_completed,
        probe.steady_turns_requested,
    )
}
