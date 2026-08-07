use std::{cmp::Ordering, collections::BTreeSet, fmt::Write, fs, path::Path};

use noema_runtime::eval_support::{
    RUNTIME_EVAL_SUITE_VERSION, RuntimeEvalCaseResult, RuntimeEvalRole,
};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};

use crate::{
    manifest::SuiteConfig,
    matrix_manifest::{EvaluationCandidate, ModelPricing},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EvaluationMatrixReport {
    pub schema_version: u32,
    pub run_id: String,
    pub mode: EvaluationRunMode,
    pub status: EvaluationRunStatus,
    pub failure: Option<String>,
    pub environment: EvaluationEnvironment,
    pub runtime_suite_version: u32,
    pub decision_fingerprint: String,
    pub repetitions: u32,
    pub candidates: Vec<EvaluationCandidate>,
    pub entries: Vec<EvaluationMatrixEntry>,
    pub rankings: Vec<RoleRanking>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationRunMode {
    Exploration,
    DefaultDecision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationRunStatus {
    Running,
    Incomplete,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EvaluationEnvironment {
    pub evaluator_version: String,
    pub target_os: String,
    pub target_arch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EvaluationMatrixEntry {
    pub candidate_id: String,
    pub repetition: u32,
    pub cases: Vec<RuntimeEvalCaseResult>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RoleRanking {
    pub role: RuntimeEvalRole,
    pub recommended_candidate_id: Option<String>,
    pub candidates: Vec<RoleCandidateScore>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RoleCandidateScore {
    pub candidate_id: String,
    pub completed_repetitions: u32,
    pub qualified: bool,
    pub identity_matched: bool,
    pub observed_models: Vec<String>,
    pub passed_critical_cases: usize,
    pub total_critical_cases: usize,
    pub passed_cases: usize,
    pub total_cases: usize,
    pub median_latency_ms: Option<u64>,
    pub estimated_cost_usd: Option<f64>,
}

impl EvaluationMatrixReport {
    pub(crate) fn new(
        run_id: String,
        suite: &SuiteConfig,
        candidates: Vec<EvaluationCandidate>,
        mode: EvaluationRunMode,
    ) -> Self {
        let decision_fingerprint = decision_fingerprint(suite, &candidates);
        let mut report = Self {
            schema_version: 1,
            run_id,
            mode,
            status: EvaluationRunStatus::Running,
            failure: None,
            environment: EvaluationEnvironment {
                evaluator_version: env!("CARGO_PKG_VERSION").to_string(),
                target_os: std::env::consts::OS.to_string(),
                target_arch: std::env::consts::ARCH.to_string(),
            },
            runtime_suite_version: RUNTIME_EVAL_SUITE_VERSION,
            decision_fingerprint,
            repetitions: suite.repetitions,
            candidates,
            entries: Vec::new(),
            rankings: Vec::new(),
        };
        report.refresh_rankings();
        report
    }

    pub(crate) fn push(&mut self, entry: EvaluationMatrixEntry) {
        self.entries.push(entry);
        self.refresh_rankings();
    }

    pub(crate) fn finish(&mut self) {
        self.status = match self.mode {
            EvaluationRunMode::Exploration => EvaluationRunStatus::Incomplete,
            EvaluationRunMode::DefaultDecision if self.has_every_repetition() => {
                EvaluationRunStatus::Complete
            }
            EvaluationRunMode::DefaultDecision => EvaluationRunStatus::Incomplete,
        };
        self.refresh_rankings();
    }

    pub(crate) fn fail(&mut self, error: String) {
        self.status = EvaluationRunStatus::Failed;
        self.failure = Some(error);
        self.refresh_rankings();
    }

    pub(crate) fn write(&self, directory: &Path) -> Result<(), String> {
        fs::create_dir_all(directory)
            .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        let json = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("failed to serialize evaluation matrix: {error}"))?;
        fs::write(directory.join("matrix.json"), json)
            .map_err(|error| format!("failed to write evaluation matrix JSON: {error}"))?;
        fs::write(directory.join("summary.md"), self.markdown())
            .map_err(|error| format!("failed to write evaluation matrix summary: {error}"))?;
        Ok(())
    }

    fn refresh_rankings(&mut self) {
        self.rankings = RuntimeEvalRole::ALL
            .iter()
            .copied()
            .map(|role| self.rank_role(role))
            .collect();
    }

    fn has_every_repetition(&self) -> bool {
        self.candidates.iter().all(|candidate| {
            let repetitions = self
                .entries
                .iter()
                .filter(|entry| entry.candidate_id == candidate.id)
                .map(|entry| entry.repetition)
                .collect::<BTreeSet<_>>();
            let entry_count = self
                .entries
                .iter()
                .filter(|entry| entry.candidate_id == candidate.id)
                .count();
            entry_count == usize::try_from(self.repetitions).unwrap_or(usize::MAX)
                && repetitions.len() == entry_count
                && (1..=self.repetitions).all(|repetition| repetitions.contains(&repetition))
        })
    }

    fn rank_role(&self, role: RuntimeEvalRole) -> RoleRanking {
        let mut candidates = self
            .candidates
            .iter()
            .filter(|candidate| candidate.roles.contains(&role))
            .map(|candidate| self.score_candidate(role, candidate))
            .collect::<Vec<_>>();
        candidates.sort_by(compare_scores);
        let recommended_candidate_id = candidates
            .first()
            .filter(|score| {
                self.mode == EvaluationRunMode::DefaultDecision
                    && self.status == EvaluationRunStatus::Complete
                    && score.qualified
            })
            .map(|score| score.candidate_id.clone());
        RoleRanking {
            role,
            recommended_candidate_id,
            candidates,
        }
    }

    fn score_candidate(
        &self,
        role: RuntimeEvalRole,
        candidate: &EvaluationCandidate,
    ) -> RoleCandidateScore {
        let entries = self
            .entries
            .iter()
            .filter(|entry| entry.candidate_id == candidate.id)
            .collect::<Vec<_>>();
        let cases = entries
            .iter()
            .flat_map(|entry| &entry.cases)
            .filter(|case| case.role == role)
            .collect::<Vec<_>>();
        let total_critical_cases = cases.iter().filter(|case| case.critical).count();
        let passed_critical_cases = cases
            .iter()
            .filter(|case| case.critical && case.passed)
            .count();
        let mut latencies = cases.iter().map(|case| case.latency_ms).collect::<Vec<_>>();
        latencies.sort_unstable();
        let observed_models = cases
            .iter()
            .filter_map(|case| case.response_model.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let identity_matched = !cases.is_empty()
            && cases.iter().all(|case| {
                case.response_provider.as_deref() == Some("openrouter")
                    && case.response_model.as_deref().is_some_and(|model| {
                        model == candidate.model
                            || candidate
                                .accepted_response_models
                                .iter()
                                .any(|accepted| accepted == model)
                    })
            });
        let completed_repetitions = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        let qualified = completed_repetitions == self.repetitions
            && total_critical_cases > 0
            && passed_critical_cases == total_critical_cases
            && identity_matched
            && entries.iter().all(|entry| entry.error.is_none());
        RoleCandidateScore {
            candidate_id: candidate.id.clone(),
            completed_repetitions,
            qualified,
            identity_matched,
            observed_models,
            passed_critical_cases,
            total_critical_cases,
            passed_cases: cases.iter().filter(|case| case.passed).count(),
            total_cases: cases.len(),
            median_latency_ms: median(&latencies),
            estimated_cost_usd: estimated_cost(candidate.pricing, &cases),
        }
    }

    fn markdown(&self) -> String {
        let mut output = format!(
            "# Noema OpenRouter model evaluation {}\n\nMode: `{:?}`. Status: `{:?}`. Repetitions: {}. Suite version: {}. Decision fingerprint: `{}`.\n",
            self.run_id,
            self.mode,
            self.status,
            self.repetitions,
            self.runtime_suite_version,
            self.decision_fingerprint,
        );
        if let Some(failure) = &self.failure {
            let _ = writeln!(output, "\nRun failure: {}", failure.replace('|', "\\|"));
        }
        for ranking in &self.rankings {
            let recommendation = match ranking.recommended_candidate_id.as_deref() {
                Some(candidate) => candidate,
                None if self.mode == EvaluationRunMode::DefaultDecision
                    && self.status == EvaluationRunStatus::Complete =>
                {
                    "none; no candidate qualified"
                }
                None => "none until a default decision completes",
            };
            let _ = write!(
                output,
                "\n## {}\n\nFinal recommendation: **{}**\n\n| Candidate | Qualified | Identity | Critical | All cases | Estimated cost | Median latency | Runs |\n|---|---:|---:|---:|---:|---:|---:|---:|\n",
                ranking.role.as_str(),
                recommendation,
            );
            for score in &ranking.candidates {
                let cost = score
                    .estimated_cost_usd
                    .map_or_else(|| "-".to_string(), |cost| format!("${cost:.6}"));
                let latency = score
                    .median_latency_ms
                    .map_or_else(|| "-".to_string(), |latency| format!("{latency} ms"));
                let _ = writeln!(
                    output,
                    "| {} | {} | {} | {}/{} | {}/{} | {} | {} | {}/{} |",
                    score.candidate_id,
                    if score.qualified { "yes" } else { "no" },
                    if score.identity_matched { "yes" } else { "no" },
                    score.passed_critical_cases,
                    score.total_critical_cases,
                    score.passed_cases,
                    score.total_cases,
                    cost,
                    latency,
                    score.completed_repetitions,
                    self.repetitions,
                );
            }
        }
        output.push_str("\nPrices are dated manifest snapshots and costs are estimates from OpenRouter-reported usage, not billing records. Final recommendations appear only for a completed default-decision run.\n");
        output
    }
}

fn decision_fingerprint(suite: &SuiteConfig, candidates: &[EvaluationCandidate]) -> String {
    let bytes = serde_json::to_vec(&(RUNTIME_EVAL_SUITE_VERSION, suite, candidates))
        .expect("evaluation decision metadata serializes");
    digest(&SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn compare_scores(left: &RoleCandidateScore, right: &RoleCandidateScore) -> Ordering {
    right
        .qualified
        .cmp(&left.qualified)
        .then_with(|| {
            compare_ratio(
                right.passed_cases,
                right.total_cases,
                left.passed_cases,
                left.total_cases,
            )
        })
        .then_with(|| compare_optional_f64(left.estimated_cost_usd, right.estimated_cost_usd))
        .then_with(|| {
            left.median_latency_ms
                .unwrap_or(u64::MAX)
                .cmp(&right.median_latency_ms.unwrap_or(u64::MAX))
        })
        .then_with(|| left.candidate_id.cmp(&right.candidate_id))
}

fn compare_ratio(
    left_numerator: usize,
    left_denominator: usize,
    right_numerator: usize,
    right_denominator: usize,
) -> Ordering {
    left_numerator
        .saturating_mul(right_denominator.max(1))
        .cmp(&right_numerator.saturating_mul(left_denominator.max(1)))
}

fn compare_optional_f64(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn estimated_cost(pricing: Option<ModelPricing>, cases: &[&RuntimeEvalCaseResult]) -> Option<f64> {
    if cases.is_empty() {
        return None;
    }
    let pricing = pricing?;
    cases.iter().try_fold(0.0, |total, case| {
        let input = case.input_tokens?;
        let cached = case.cached_input_tokens.unwrap_or(0).min(input);
        let uncached = input.saturating_sub(cached);
        let output = case.output_tokens?;
        let cached_rate = pricing
            .cached_input_usd_per_million
            .unwrap_or(pricing.input_usd_per_million);
        Some(
            total
                + uncached as f64 * pricing.input_usd_per_million / 1_000_000.0
                + cached as f64 * cached_rate / 1_000_000.0
                + output as f64 * pricing.output_usd_per_million / 1_000_000.0,
        )
    })
}

fn median(values: &[u64]) -> Option<u64> {
    values.get(values.len().saturating_sub(1) / 2).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix_manifest::RecommendationTarget;

    #[test]
    fn default_decision_suppresses_recommendations_until_finished() {
        let mut cheap = candidate("cheap", 1.0);
        cheap.enabled = false;
        let expensive = candidate("expensive", 2.0);
        let mut report = EvaluationMatrixReport::new(
            "test".to_string(),
            &SuiteConfig {
                context_window_tokens: 8_192,
                generation_timeout_seconds: 120,
                startup_timeout_seconds: 180,
                worker_timeout_seconds: 300,
                repetitions: 2,
            },
            vec![expensive, cheap],
            EvaluationRunMode::DefaultDecision,
        );
        for id in ["expensive", "cheap"] {
            report.push(EvaluationMatrixEntry {
                candidate_id: id.to_string(),
                repetition: 1,
                cases: vec![case(RuntimeEvalRole::Primary)],
                error: None,
            });
        }
        let primary = report
            .rankings
            .iter()
            .find(|ranking| ranking.role == RuntimeEvalRole::Primary)
            .expect("primary ranking");
        assert!(primary.recommended_candidate_id.is_none());
        assert!(primary.candidates.iter().all(|score| !score.qualified));
        report.finish();
        assert_eq!(report.status, EvaluationRunStatus::Incomplete);

        for id in ["expensive", "cheap"] {
            report.push(EvaluationMatrixEntry {
                candidate_id: id.to_string(),
                repetition: 2,
                cases: vec![case(RuntimeEvalRole::Primary)],
                error: None,
            });
        }
        let primary = report
            .rankings
            .iter()
            .find(|ranking| ranking.role == RuntimeEvalRole::Primary)
            .expect("primary ranking");
        assert!(primary.recommended_candidate_id.is_none());

        report.finish();
        let primary = report
            .rankings
            .iter()
            .find(|ranking| ranking.role == RuntimeEvalRole::Primary)
            .expect("primary ranking");
        assert_eq!(primary.recommended_candidate_id.as_deref(), Some("cheap"));
    }

    #[test]
    fn exploration_never_emits_a_final_recommendation() {
        let mut report = EvaluationMatrixReport::new(
            "test".to_string(),
            &suite(1),
            vec![candidate("candidate", 1.0)],
            EvaluationRunMode::Exploration,
        );
        report.push(EvaluationMatrixEntry {
            candidate_id: "candidate".to_string(),
            repetition: 1,
            cases: vec![case(RuntimeEvalRole::Primary)],
            error: None,
        });
        report.finish();

        assert_eq!(report.status, EvaluationRunStatus::Incomplete);
        assert!(report.rankings[0].recommended_candidate_id.is_none());
    }

    #[test]
    fn returned_model_identity_is_a_qualification_gate() {
        let mut report = EvaluationMatrixReport::new(
            "test".to_string(),
            &suite(1),
            vec![candidate("requested/model", 1.0)],
            EvaluationRunMode::DefaultDecision,
        );
        let mut mismatched = case(RuntimeEvalRole::Primary);
        mismatched.response_model = Some("other/model".to_string());
        report.push(EvaluationMatrixEntry {
            candidate_id: "requested/model".to_string(),
            repetition: 1,
            cases: vec![mismatched],
            error: None,
        });
        report.finish();

        let score = &report.rankings[0].candidates[0];
        assert!(!score.identity_matched);
        assert!(!score.qualified);
        assert!(report.rankings[0].recommended_candidate_id.is_none());
    }

    fn candidate(id: &str, input_price: f64) -> EvaluationCandidate {
        EvaluationCandidate {
            id: id.to_string(),
            name: id.to_string(),
            model: id.to_string(),
            roles: vec![RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            accepted_response_models: vec!["case-model".to_string()],
            targets: vec![RecommendationTarget {
                provider: "openrouter".to_string(),
                model_profile: id.to_string(),
                reasoning_effort: None,
            }],
            pricing: Some(ModelPricing {
                input_usd_per_million: input_price,
                cached_input_usd_per_million: None,
                output_usd_per_million: 1.0,
            }),
            enabled: true,
            notes: String::new(),
        }
    }

    fn case(role: RuntimeEvalRole) -> RuntimeEvalCaseResult {
        RuntimeEvalCaseResult {
            case_id: "case".to_string(),
            category: "category".to_string(),
            role,
            critical: true,
            passed: true,
            response_provider: Some("openrouter".to_string()),
            response_model: Some("case-model".to_string()),
            latency_ms: 10,
            first_visible_delta_ms: Some(5),
            streamed_chars: 1,
            input_tokens: Some(1_000),
            cached_input_tokens: Some(0),
            output_tokens: Some(100),
            assistant_text: String::new(),
            tool_calls: Vec::new(),
            failure: None,
        }
    }

    fn suite(repetitions: u32) -> SuiteConfig {
        SuiteConfig {
            context_window_tokens: 8_192,
            generation_timeout_seconds: 120,
            startup_timeout_seconds: 180,
            worker_timeout_seconds: 300,
            repetitions,
        }
    }
}
