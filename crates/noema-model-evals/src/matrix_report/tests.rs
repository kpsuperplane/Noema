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
            decision_repetitions: None,
            exploration_repetitions: None,
        },
        vec![expensive, cheap],
        policies("cheap", 0.0),
        EvaluationRunMode::DefaultDecision,
    );
    for id in ["expensive", "cheap"] {
        report.push(EvaluationMatrixEntry {
            candidate_id: id.to_string(),
            repetition: 1,
            cases: cases(RuntimeEvalRole::Primary),
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
            cases: cases(RuntimeEvalRole::Primary),
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
        policies("candidate", 0.0),
        EvaluationRunMode::Exploration,
    );
    report.push(EvaluationMatrixEntry {
        candidate_id: "candidate".to_string(),
        repetition: 1,
        cases: cases(RuntimeEvalRole::Primary),
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
        policies("requested/model", 0.0),
        EvaluationRunMode::DefaultDecision,
    );
    let mut mismatched = cases(RuntimeEvalRole::Primary);
    mismatched[0].response_model = Some("other/model".to_string());
    report.push(EvaluationMatrixEntry {
        candidate_id: "requested/model".to_string(),
        repetition: 1,
        cases: mismatched,
        error: None,
    });
    report.finish();

    let score = &report.rankings[0].candidates[0];
    assert!(!score.identity_matched);
    assert!(!score.qualified);
    assert!(report.rankings[0].recommended_candidate_id.is_none());
}

#[test]
fn one_failed_stateful_action_blocks_primary_qualification() {
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("candidate", 1.0)],
        policies("candidate", 0.0),
        EvaluationRunMode::DefaultDecision,
    );
    let mut results = cases(RuntimeEvalRole::Primary);
    let failed = results
        .iter_mut()
        .find(|case| case.case_id == "primary_stateful_email_meeting_to_calendar")
        .expect("stateful action case");
    failed.passed = false;
    failed.failure = Some("did not complete the grounded action".to_string());
    report.push(EvaluationMatrixEntry {
        candidate_id: "candidate".to_string(),
        repetition: 1,
        cases: results,
        error: None,
    });
    report.finish();

    let score = &report.rankings[0].candidates[0];
    assert!(score.deterministic_score.expect("score") > 0.90);
    assert!(!score.qualified);
    assert!(report.rankings[0].recommended_candidate_id.is_none());
}

#[test]
fn markdown_exposes_each_stateful_primary_gap() {
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("candidate", 1.0)],
        policies("candidate", 0.0),
        EvaluationRunMode::DefaultDecision,
    );
    let mut results = cases(RuntimeEvalRole::Primary);
    let failed = results
        .iter_mut()
        .find(|case| case.case_id == "primary_stateful_package_delivery")
        .expect("package delivery case");
    failed.passed = false;
    failed.failure = Some("answer omitted the delivery date".to_string());
    report.push(EvaluationMatrixEntry {
        candidate_id: "candidate".to_string(),
        repetition: 1,
        cases: results,
        error: None,
    });

    let markdown = report.markdown();
    assert!(markdown.contains("### Stateful Primary diagnostics"));
    assert!(markdown.contains("| Candidate | flight to calendar |"));
    assert!(markdown.contains("| candidate | 1/1 | 1/1 | 1/1 | 1/1 | 0/1 | 1/1 | 1/1 |"));
    assert!(markdown.contains("candidate · package_delivery · repetition 1"));
    assert!(markdown.contains("answer omitted the delivery date"));
}

#[test]
fn replacement_margin_retains_a_qualified_incumbent() {
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("challenger", 0.5), candidate("incumbent", 1.0)],
        policies("incumbent", 0.1),
        EvaluationRunMode::DefaultDecision,
    );
    for id in ["challenger", "incumbent"] {
        report.push(EvaluationMatrixEntry {
            candidate_id: id.to_string(),
            repetition: 1,
            cases: cases(RuntimeEvalRole::Primary),
            error: None,
        });
    }
    report.finish();

    assert_eq!(
        report.rankings[0].recommended_candidate_id.as_deref(),
        Some("incumbent")
    );
    assert!(report.rankings[0].selection_reason.contains("below margin"));
}

#[test]
fn quality_and_tail_latency_thresholds_are_qualification_gates() {
    let mut role_policies = policies("candidate", 0.0);
    let primary = role_policies
        .policies
        .iter_mut()
        .find(|policy| policy.role == RuntimeEvalRole::Primary)
        .expect("primary policy");
    primary.minimum_quality_score = 0.75;
    primary.maximum_p95_latency_ms = 50;
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("candidate", 1.0)],
        role_policies,
        EvaluationRunMode::DefaultDecision,
    );
    let mut passing = case(RuntimeEvalRole::Primary);
    passing.case_id = "fast".to_string();
    let mut slow = case(RuntimeEvalRole::Primary);
    slow.case_id = "slow".to_string();
    slow.latency_ms = 51;
    report.push(EvaluationMatrixEntry {
        candidate_id: "candidate".to_string(),
        repetition: 1,
        cases: vec![passing, slow],
        error: None,
    });
    report.finish();

    let score = &report.rankings[0].candidates[0];
    assert_eq!(score.quality_score, Some(1.0));
    assert_eq!(score.p95_latency_ms, Some(51));
    assert!(!score.qualified);
}

#[test]
fn required_comparison_keeps_decision_incomplete_until_recorded() {
    let mut role_policies = policies("incumbent", 0.0);
    let primary = role_policies
        .policies
        .iter_mut()
        .find(|policy| policy.role == RuntimeEvalRole::Primary)
        .expect("primary policy");
    primary.deterministic_weight = 0.5;
    primary.judge_weight = 0.5;
    primary.judge_case_ids = vec!["case".to_string()];
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("challenger", 1.0), candidate("incumbent", 1.0)],
        role_policies,
        EvaluationRunMode::DefaultDecision,
    );
    for id in ["challenger", "incumbent"] {
        report.push(EvaluationMatrixEntry {
            candidate_id: id.to_string(),
            repetition: 1,
            cases: cases(RuntimeEvalRole::Primary),
            error: None,
        });
    }
    report.finish();
    assert_eq!(report.status, EvaluationRunStatus::Incomplete);

    report.push_comparison(RoleComparison {
        role: RuntimeEvalRole::Primary,
        challenger_candidate_id: "challenger".to_string(),
        incumbent_candidate_id: "incumbent".to_string(),
        judge_model: "judge/model".to_string(),
        candidate_a_id: "challenger".to_string(),
        candidate_b_id: "incumbent".to_string(),
        winner_candidate_id: Some("challenger".to_string()),
        challenger_score: Some(90),
        incumbent_score: Some(80),
        rationale: Some("Challenger was more complete.".to_string()),
        response_provider: Some("openrouter".to_string()),
        response_model: Some("judge/model".to_string()),
        error: None,
    });
    report.finish();
    assert_eq!(report.status, EvaluationRunStatus::Complete);
}

#[test]
fn checkpoint_rejects_a_duplicate_case_without_overwriting() {
    let mut report = EvaluationMatrixReport::new(
        "test".to_string(),
        &suite(1),
        vec![candidate("candidate", 1.0)],
        policies("candidate", 0.0),
        EvaluationRunMode::DefaultDecision,
    );
    report
        .record_case("candidate", 1, case(RuntimeEvalRole::Primary))
        .expect("first checkpoint");
    let error = report
        .record_case("candidate", 1, case(RuntimeEvalRole::Primary))
        .expect_err("duplicate checkpoint");

    assert!(error.contains("already checkpointed"));
    assert_eq!(report.entries[0].cases.len(), 1);

    report.fail("provider unavailable".to_string());
    report.resume();
    assert_eq!(report.status, EvaluationRunStatus::Running);
    assert!(report.failure.is_none());
    assert!(report.has_case("candidate", 1, "case"));
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
        judge_rubric: None,
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

fn cases(role: RuntimeEvalRole) -> Vec<RuntimeEvalCaseResult> {
    runtime_eval_case_descriptors_for_roles("test/model", &[role], None)
        .expect("case descriptors")
        .into_iter()
        .map(|descriptor| {
            let mut result = case(descriptor.role);
            result.case_id = descriptor.case_id;
            result.category = descriptor.category;
            result
        })
        .collect()
}

fn suite(repetitions: u32) -> SuiteConfig {
    SuiteConfig {
        context_window_tokens: 8_192,
        generation_timeout_seconds: 120,
        startup_timeout_seconds: 180,
        worker_timeout_seconds: 300,
        repetitions,
        decision_repetitions: None,
        exploration_repetitions: None,
    }
}

fn policies(incumbent: &str, margin: f64) -> RolePolicyManifest {
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
            .map(|role| RolePolicy {
                role,
                incumbent_candidate_id: incumbent.to_string(),
                minimum_cases: 1,
                minimum_quality_score: 1.0,
                maximum_error_rate: 0.0,
                maximum_p95_latency_ms: 100,
                replacement_quality_margin: margin,
                deterministic_weight: 1.0,
                judge_weight: 0.0,
                judge_case_ids: Vec::new(),
            })
            .collect(),
    }
}
