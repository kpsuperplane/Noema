use std::{fmt::Write, fs, path::Path};

use noema_providers::{
    NoemaModelRecommendation, NoemaModelUseCase, ProviderKind, ReasoningEffort,
    noema_model_recommendation,
};
use noema_runtime::eval_support::RuntimeEvalRole;

use crate::{
    decision_plan::DecisionPlan,
    matrix_manifest::{EvaluationCandidate, RecommendationTarget},
    matrix_report::{EvaluationMatrixReport, EvaluationRunMode, RoleCandidateScore, RoleRanking},
};

struct CellDecision {
    provider: ProviderKind,
    role: RuntimeEvalRole,
    use_case: NoemaModelUseCase,
    current: NoemaModelRecommendation,
    selected_candidate_id: Option<String>,
    selected: Option<RecommendationTarget>,
    reason: String,
}

pub(crate) fn propose(run_root: &Path) -> Result<(String, String), String> {
    let (plan, report) = load_completed_decision(run_root)?;
    let decisions = decide_cells(&report)?;
    let source_path =
        crate::orchestrator::workspace_root().join("crates/noema-providers/src/recommendations.rs");
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;
    let patch = render_patch(&source, &decisions)?;
    let rationale = render_rationale(&plan, &decisions);
    fs::write(run_root.join("recommendations.patch"), &patch)
        .map_err(|error| format!("failed to write recommendation patch: {error}"))?;
    fs::write(run_root.join("proposal.md"), &rationale)
        .map_err(|error| format!("failed to write recommendation rationale: {error}"))?;
    Ok((patch, rationale))
}

pub(crate) fn verify(run_root: &Path) -> Result<(), String> {
    let (_, report) = load_completed_decision(run_root)?;
    let decisions = decide_cells(&report)?;
    let mismatches = decisions
        .iter()
        .filter_map(|decision| {
            let selected = decision.selected.as_ref()?;
            (selected.model_profile != decision.current.model_profile
                || selected.reasoning_effort != decision.current.reasoning_effort)
                .then(|| {
                    format!(
                        "{}/{} ships {:?}/{:?}, decision selects {:?}/{:?}",
                        decision.provider,
                        decision.role,
                        decision.current.model_profile,
                        decision.current.reasoning_effort,
                        selected.model_profile,
                        selected.reasoning_effort,
                    )
                })
        })
        .collect::<Vec<_>>();
    let missing = decisions
        .iter()
        .filter(|decision| decision.selected.is_none())
        .map(|decision| {
            format!(
                "{}/{}: {}",
                decision.provider, decision.role, decision.reason
            )
        })
        .collect::<Vec<_>>();
    if mismatches.is_empty() && missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "shipped recommendations do not match completed evidence:\n{}",
            mismatches
                .into_iter()
                .chain(missing)
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn load_completed_decision(
    run_root: &Path,
) -> Result<(DecisionPlan, EvaluationMatrixReport), String> {
    let plan = DecisionPlan::load(&run_root.join("plan.json"))?;
    let report = EvaluationMatrixReport::read(run_root)?;
    if !report.is_complete_decision() || report.run_id != plan.decision_id {
        return Err("proposal requires a complete planned default decision".to_string());
    }
    let expected = EvaluationMatrixReport::new(
        plan.decision_id.clone(),
        &plan.suite,
        plan.candidates.clone(),
        plan.policies.clone(),
        EvaluationRunMode::DefaultDecision,
    );
    if expected.decision_fingerprint != report.decision_fingerprint {
        return Err("decision evidence does not match its immutable plan".to_string());
    }
    Ok((plan, report))
}

fn decide_cells(report: &EvaluationMatrixReport) -> Result<Vec<CellDecision>, String> {
    let mut decisions = Vec::new();
    for provider in [
        ProviderKind::Codex,
        ProviderKind::OpenAi,
        ProviderKind::OpenRouter,
    ] {
        for role in RuntimeEvalRole::ALL {
            let use_case = use_case(*role);
            let current = noema_model_recommendation(provider.clone(), use_case)
                .ok_or_else(|| format!("missing shipped recommendation for {provider}/{role}"))?;
            let ranking = report
                .rankings
                .iter()
                .find(|ranking| ranking.role == *role)
                .ok_or_else(|| format!("decision has no ranking for {role}"))?;
            let (selected_candidate_id, selected, reason) =
                select_for_provider(report, ranking, &provider, current);
            decisions.push(CellDecision {
                provider: provider.clone(),
                role: *role,
                use_case,
                current,
                selected_candidate_id,
                selected,
                reason,
            });
        }
    }
    Ok(decisions)
}

fn select_for_provider(
    report: &EvaluationMatrixReport,
    ranking: &RoleRanking,
    provider: &ProviderKind,
    current: NoemaModelRecommendation,
) -> (Option<String>, Option<RecommendationTarget>, String) {
    let eligible = ranking.candidates.iter().filter_map(|score| {
        let candidate = candidate(report, &score.candidate_id)?;
        let target = target(candidate, provider)?;
        score.qualified.then_some((score, target))
    });
    let Some((challenger_score, challenger_target)) = eligible.into_iter().next() else {
        return (
            None,
            None,
            "no qualified candidate has an explicit mapping".to_string(),
        );
    };
    let incumbent = ranking.candidates.iter().find_map(|score| {
        let candidate = candidate(report, &score.candidate_id)?;
        let target = target(candidate, provider)?;
        (target.model_profile == current.model_profile
            && target.reasoning_effort == current.reasoning_effort)
            .then_some((score, target))
    });
    let margin = report
        .policies
        .policy(ranking.role)
        .replacement_quality_margin;
    if let Some((incumbent_score, incumbent_target)) =
        incumbent.filter(|(score, _)| score.qualified)
        && challenger_score.candidate_id != incumbent_score.candidate_id
    {
        let improvement = score(challenger_score) - score(incumbent_score);
        if improvement + f64::EPSILON < margin {
            return (
                Some(incumbent_score.candidate_id.clone()),
                Some(incumbent_target.clone()),
                format!(
                    "retained mapped incumbent; improvement {improvement:.3} was below margin {margin:.3}"
                ),
            );
        }
    }
    (
        Some(challenger_score.candidate_id.clone()),
        Some(challenger_target.clone()),
        if challenger_target.model_profile == current.model_profile
            && challenger_target.reasoning_effort == current.reasoning_effort
        {
            "shipped recommendation remains selected".to_string()
        } else {
            "highest-ranked qualified mapped candidate cleared policy".to_string()
        },
    )
}

fn candidate<'a>(
    report: &'a EvaluationMatrixReport,
    candidate_id: &str,
) -> Option<&'a EvaluationCandidate> {
    report
        .candidates
        .iter()
        .find(|candidate| candidate.id == candidate_id)
}

fn target<'a>(
    candidate: &'a EvaluationCandidate,
    provider: &ProviderKind,
) -> Option<&'a RecommendationTarget> {
    candidate
        .targets
        .iter()
        .find(|target| target.provider == provider.as_str())
}

fn score(score: &RoleCandidateScore) -> f64 {
    score.quality_score.unwrap_or(0.0)
}

fn render_patch(source: &str, decisions: &[CellDecision]) -> Result<String, String> {
    let mut patch = String::new();
    let mut changed = false;
    for decision in decisions {
        let Some(selected) = &decision.selected else {
            continue;
        };
        if selected.model_profile == decision.current.model_profile
            && selected.reasoning_effort == decision.current.reasoning_effort
        {
            continue;
        }
        let old = cell_line(
            &decision.provider,
            decision.use_case,
            decision.current.model_profile,
            decision.current.reasoning_effort,
        );
        let new = cell_line(
            &decision.provider,
            decision.use_case,
            &selected.model_profile,
            selected.reasoning_effort,
        );
        let offset = source.find(&old).ok_or_else(|| {
            format!(
                "recommendation source block changed for {}/{}",
                decision.provider, decision.role
            )
        })?;
        let line = source[..offset].lines().count() + 1;
        if !changed {
            patch.push_str("--- a/crates/noema-providers/src/recommendations.rs\n+++ b/crates/noema-providers/src/recommendations.rs\n");
            changed = true;
        }
        let count = old.lines().count();
        let _ = writeln!(patch, "@@ -{line},{count} +{line},{count} @@");
        for line in old.lines() {
            let _ = writeln!(patch, "-{line}");
        }
        for line in new.lines() {
            let _ = writeln!(patch, "+{line}");
        }
    }
    Ok(patch)
}

fn cell_line(
    provider: &ProviderKind,
    use_case: NoemaModelUseCase,
    model: &str,
    effort: Option<ReasoningEffort>,
) -> String {
    let effort = match effort {
        None => "none",
        Some(ReasoningEffort::None) => "off",
        Some(ReasoningEffort::Minimal) => "minimal",
        Some(ReasoningEffort::Low) => "low",
        Some(ReasoningEffort::Medium) => "medium",
        Some(ReasoningEffort::High) => "high",
        Some(ReasoningEffort::XHigh) => "xhigh",
    };
    format!("    ({provider:?}, {use_case:?}, \"{model}\", {effort}),")
}

fn render_rationale(plan: &DecisionPlan, decisions: &[CellDecision]) -> String {
    let mut output = format!(
        "# Recommendation proposal {}\n\nDecision fingerprint: `{}`.\n\n| Provider | Role | Candidate | Selected profile | Reason |\n|---|---|---|---|---|\n",
        plan.decision_id, plan.content_fingerprint
    );
    for decision in decisions {
        let profile = decision
            .selected
            .as_ref()
            .map_or("none", |target| target.model_profile.as_str());
        let _ = writeln!(
            output,
            "| {} | {} | {} | {} | {} |",
            decision.provider,
            decision.role,
            decision.selected_candidate_id.as_deref().unwrap_or("none"),
            profile,
            decision.reason.replace('|', "\\|")
        );
    }
    output
}

const fn use_case(role: RuntimeEvalRole) -> NoemaModelUseCase {
    match role {
        RuntimeEvalRole::Primary => NoemaModelUseCase::Primary,
        RuntimeEvalRole::TaskSimple => NoemaModelUseCase::TaskSimple,
        RuntimeEvalRole::TaskMedium => NoemaModelUseCase::TaskMedium,
        RuntimeEvalRole::TaskDifficult => NoemaModelUseCase::TaskDifficult,
        RuntimeEvalRole::TaskReviewer => NoemaModelUseCase::TaskReviewer,
        RuntimeEvalRole::WebFetchSummarizer => NoemaModelUseCase::WebFetchSummarizer,
        RuntimeEvalRole::ToolProgressAudit => NoemaModelUseCase::ToolProgressAudit,
        RuntimeEvalRole::ActionReviewer => NoemaModelUseCase::ActionReviewer,
        RuntimeEvalRole::MemoryConsolidation => NoemaModelUseCase::MemoryConsolidation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        manifest::load_suite, matrix_manifest::load_evaluation_candidates,
        role_policy::load_role_policies,
    };

    #[test]
    fn rendered_cell_line_matches_the_table_authority() {
        let source = include_str!("../../noema-providers/src/recommendations.rs");
        let block = cell_line(
            &ProviderKind::OpenRouter,
            NoemaModelUseCase::ActionReviewer,
            "openai/gpt-5.6-luna",
            Some(ReasoningEffort::Low),
        );
        assert!(source.contains(&block));
    }

    #[test]
    fn unmapped_winner_does_not_change_a_direct_provider() {
        let report = report();
        let ranking = ranking(&[
            ("openrouter-deepseek-v4-flash", 0.99),
            ("openrouter-luna-high-primary", 0.90),
        ]);
        let current = noema_model_recommendation(ProviderKind::OpenAi, NoemaModelUseCase::Primary)
            .expect("current recommendation");

        let (candidate, target, _) =
            select_for_provider(&report, &ranking, &ProviderKind::OpenAi, current);
        assert_eq!(candidate.as_deref(), Some("openrouter-luna-high-primary"));
        assert_eq!(target.expect("mapped target").model_profile, "gpt-5.6-luna");
    }

    #[test]
    fn provider_selection_retains_incumbent_below_margin() {
        let report = report();
        let ranking = ranking(&[
            ("openrouter-terra-medium", 0.92),
            ("openrouter-luna-high-primary", 0.90),
        ]);
        let current = noema_model_recommendation(ProviderKind::OpenAi, NoemaModelUseCase::Primary)
            .expect("current recommendation");

        let (candidate, _, reason) =
            select_for_provider(&report, &ranking, &ProviderKind::OpenAi, current);
        assert_eq!(candidate.as_deref(), Some("openrouter-luna-high-primary"));
        assert!(reason.contains("below margin"));
    }

    #[test]
    fn patch_changes_only_the_exact_provider_role_cell() {
        let source = include_str!("../../noema-providers/src/recommendations.rs");
        let decision = CellDecision {
            provider: ProviderKind::OpenAi,
            role: RuntimeEvalRole::Primary,
            use_case: NoemaModelUseCase::Primary,
            current: noema_model_recommendation(ProviderKind::OpenAi, NoemaModelUseCase::Primary)
                .expect("current recommendation"),
            selected_candidate_id: Some("openrouter-terra-medium".to_string()),
            selected: Some(RecommendationTarget {
                provider: "openai".to_string(),
                model_profile: "gpt-5.6-terra".to_string(),
                reasoning_effort: Some(ReasoningEffort::Medium),
            }),
            reason: "test".to_string(),
        };

        let patch = render_patch(source, &[decision]).expect("patch");
        assert_eq!(patch.matches("@@ ").count(), 1);
        assert!(patch.contains("-    (OpenAi, Primary, \"gpt-5.6-luna\", high),"));
        assert!(patch.contains("+    (OpenAi, Primary, \"gpt-5.6-terra\", medium),"));
    }

    fn report() -> EvaluationMatrixReport {
        let root = crate::orchestrator::workspace_root();
        let candidates =
            load_evaluation_candidates(&root.join("evals/model-matrix/candidates.toml"))
                .expect("candidates")
                .candidates;
        let policies = load_role_policies(&root.join("evals/model-matrix/role-policies.toml"))
            .expect("policies");
        let suite = load_suite(&root.join("evals/model-matrix/suite.toml")).expect("suite");
        EvaluationMatrixReport::new(
            "test".to_string(),
            &suite,
            candidates,
            policies,
            EvaluationRunMode::DefaultDecision,
        )
    }

    fn ranking(candidates: &[(&str, f64)]) -> RoleRanking {
        RoleRanking {
            role: RuntimeEvalRole::Primary,
            recommended_candidate_id: None,
            selection_reason: String::new(),
            candidates: candidates
                .iter()
                .map(|(candidate_id, quality)| RoleCandidateScore {
                    candidate_id: (*candidate_id).to_string(),
                    completed_repetitions: 3,
                    qualified: true,
                    identity_matched: true,
                    observed_models: Vec::new(),
                    passed_critical_cases: 1,
                    total_critical_cases: 1,
                    passed_cases: 1,
                    total_cases: 1,
                    deterministic_score: Some(*quality),
                    judge_score: Some(*quality),
                    quality_score: Some(*quality),
                    error_rate: Some(0.0),
                    median_latency_ms: Some(1),
                    p95_latency_ms: Some(1),
                    estimated_cost_usd: Some(1.0),
                })
                .collect(),
        }
    }
}
