use std::{cmp::Ordering, fmt::Write, fs, path::Path};

use noema_runtime::eval_support::{RuntimeEvalCaseResult, RuntimeEvalRole};
use serde::{Deserialize, Serialize};

use crate::{
    manifest::SuiteConfig,
    matrix_manifest::{EvaluationCandidate, ModelPricing},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EvaluationMatrixReport {
    pub run_id: String,
    pub repetitions: u32,
    pub candidates: Vec<EvaluationCandidate>,
    pub entries: Vec<EvaluationMatrixEntry>,
    pub rankings: Vec<RoleRanking>,
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
    ) -> Self {
        let mut report = Self {
            run_id,
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
            .filter(|score| score.qualified && score.completed_repetitions == self.repetitions)
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
        let completed_repetitions = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        let qualified = completed_repetitions == self.repetitions
            && total_critical_cases > 0
            && passed_critical_cases == total_critical_cases
            && entries.iter().all(|entry| entry.error.is_none());
        RoleCandidateScore {
            candidate_id: candidate.id.clone(),
            completed_repetitions,
            qualified,
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
            "# Noema cross-provider model evaluation {}\n\nRepetitions: {}. Correctness is ranked before estimated cost and latency.\n",
            self.run_id, self.repetitions
        );
        for ranking in &self.rankings {
            let _ = write!(
                output,
                "\n## {}\n\nRecommended: **{}**\n\n| Candidate | Qualified | Critical | All cases | Estimated cost | Median latency | Runs |\n|---|---:|---:|---:|---:|---:|---:|\n",
                ranking.role.as_str(),
                ranking
                    .recommended_candidate_id
                    .as_deref()
                    .unwrap_or("none")
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
                    "| {} | {} | {}/{} | {}/{} | {} | {} | {}/{} |",
                    score.candidate_id,
                    if score.qualified { "yes" } else { "no" },
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
        output.push_str("\nPrices are manifest snapshots and costs are estimates from provider-reported token usage; they are not billing records. A recommendation is emitted only after every configured repetition completes and every critical case passes.\n");
        output
    }
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

    #[test]
    fn ranking_requires_complete_critical_passes_then_prefers_lower_cost() {
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
        assert_eq!(primary.recommended_candidate_id.as_deref(), Some("cheap"));
    }

    fn candidate(id: &str, input_price: f64) -> EvaluationCandidate {
        EvaluationCandidate {
            id: id.to_string(),
            name: id.to_string(),
            provider: crate::matrix_manifest::EvaluationProvider::OpenRouter,
            model: id.to_string(),
            roles: vec![RuntimeEvalRole::Primary],
            reasoning_effort: None,
            base_url: None,
            bridge_path: None,
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
}
