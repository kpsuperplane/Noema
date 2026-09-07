package modeleval

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
)

// Rust source: crates/noema-model-evals/src/comparative_judge.rs::blinded_order_is_stable_and_not_constant
func TestRustModelEvals_BlindedOrderIsStableAndNotConstant(t *testing.T) {
	if challengerIsA("run", "primary", "challenger") != challengerIsA("run", "primary", "challenger") {
		t.Fatal("blinded order was not stable")
	}
	outcomes := map[bool]struct{}{}
	for index := 0; index < 100; index++ {
		outcomes[challengerIsA("run", "primary", fmt.Sprintf("candidate-%d", index))] = struct{}{}
	}
	if len(outcomes) != 2 {
		t.Fatalf("blinded order outcomes = %d, want both orders", len(outcomes))
	}
}

// Rust source: crates/noema-model-evals/src/comparative_judge.rs::judge_decision_parser_is_strict_and_bounded
func TestRustModelEvals_JudgeDecisionParserIsStrictAndBounded(t *testing.T) {
	if _, err := parseJudge(`{"winner":"tie","a_score":80,"b_score":80,"rationale":"Equivalent."}`); err != nil {
		t.Fatal(err)
	}
	if _, err := parseJudge(`{"winner":"a","a_score":101,"b_score":80,"rationale":"A","extra":true}`); err == nil {
		t.Fatal("out-of-bounds or unknown judge decision was accepted")
	}
}

// Rust source: crates/noema-model-evals/src/decision_plan.rs::plan_round_trip_detects_mutation_and_has_a_cost_bound
func TestRustModelEvals_PlanRoundTripDetectsMutationAndHasACostBound(t *testing.T) {
	root := filepath.Join("..", "..")
	suite, policies, candidates, err := loadMatrix(root, nil)
	if err != nil {
		t.Fatal(err)
	}
	plan, err := createDecisionPlan(root, suite, policies, candidates)
	if err != nil {
		t.Fatal(err)
	}
	if plan.EstimatedMaxCostUSD <= 0 {
		t.Fatalf("estimated cost = %f, want positive", plan.EstimatedMaxCostUSD)
	}
	if plan.GitCommit == "" || plan.CreatedAtUnixSeconds <= 0 || plan.ContentFingerprint == "" {
		t.Fatalf("plan metadata was not populated: %#v", plan)
	}
	if err := plan.validate(); err != nil {
		t.Fatal(err)
	}
	plan.SpendCeilingUSD /= 2
	if err := plan.validate(); err == nil {
		t.Fatal("mutated spend ceiling was accepted")
	}
}

// Rust source: crates/noema-model-evals/src/decision_plan.rs::estimate_requires_a_priced_candidate_for_the_pinned_judge
func TestRustModelEvals_EstimateRequiresAPricedCandidateForThePinnedJudge(t *testing.T) {
	root := filepath.Join("..", "..")
	suite, policies, candidates, err := loadMatrix(root, nil)
	if err != nil {
		t.Fatal(err)
	}
	policies.Judge.Model = "missing/judge"
	if _, err := estimate(suite, policies, candidates); err == nil {
		t.Fatal("estimate accepted a missing priced pinned judge")
	}
}

// Rust source: crates/noema-model-evals/src/decision_plan.rs::dirty_plan_cannot_be_executed
func TestRustModelEvals_DirtyPlanCannotBeExecuted(t *testing.T) {
	root := filepath.Join("..", "..")
	suite, policies, candidates, err := loadMatrix(root, nil)
	if err != nil {
		t.Fatal(err)
	}
	plan, err := createDecisionPlan(root, suite, policies, candidates)
	if err != nil {
		t.Fatal(err)
	}
	plan.GitDirty = true
	if err := plan.validateExecutionGitState(root); err == nil {
		t.Fatal("dirty plan was executable")
	}
}

// Rust source: crates/noema-model-evals/src/hosted_provider.rs::openrouter_api_key_requires_a_non_blank_environment_value
func TestRustModelEvals_OpenrouterAPIKeyRequiresANonBlankEnvironmentValue(t *testing.T) {
	if _, err := openrouterAPIKey(nil); err == nil {
		t.Fatal("unset API key was accepted")
	}
	blank := "  "
	if _, err := openrouterAPIKey(&blank); err == nil {
		t.Fatal("blank API key was accepted")
	}
	value := "  secret  "
	key, err := openrouterAPIKey(&value)
	if err != nil || key != "secret" {
		t.Fatalf("API key = %q, err = %v", key, err)
	}
}

// Rust source: crates/noema-model-evals/src/hosted_provider.rs::environment_credentials_are_scoped_to_the_openrouter_default_account
func TestRustModelEvals_EnvironmentCredentialsAreScopedToTheOpenrouterDefaultAccount(t *testing.T) {
	credentials := openRouterEnvCredentials{apiKey: "secret"}
	key, err := credentials.apiKeyFor("openrouter", openRouterDefaultAccountID)
	if err != nil || key != "secret" {
		t.Fatalf("OpenRouter credential = %q, err = %v", key, err)
	}
	if _, err := credentials.apiKeyFor("openrouter", "provider_account:openrouter:other"); err == nil {
		t.Fatal("credential access for another account was accepted")
	}
}

// Rust source: crates/noema-model-evals/src/manifest.rs::bundled_candidate_and_suite_manifests_are_valid
func TestRustModelEvals_BundledCandidateAndSuiteManifestsAreValid(t *testing.T) {
	root := filepath.Join("..", "..", "evals")
	candidates, err := loadLocalCandidates(filepath.Join(root, "local-models/candidates.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if len(candidates) != 21 {
		t.Fatalf("local candidate count = %d, want 21", len(candidates))
	}
	var localSuite suiteConfig
	if err := readTOML(filepath.Join(root, "local-models/suite.toml"), &localSuite); err != nil {
		t.Fatal(err)
	}
	if err := localSuite.validate(); err != nil {
		t.Fatal(err)
	}
	if localSuite.ContextWindowTokens != 8192 {
		t.Fatalf("local context window = %d, want 8192", localSuite.ContextWindowTokens)
	}
	_, _, _, err = loadMatrix(filepath.Dir(root), nil)
	if err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_manifest.rs::manifest_rejects_duplicate_roles_and_hides_private_base_url
func TestRustModelEvals_ManifestRejectsDuplicateRolesAndHidesPrivateBaseURL(t *testing.T) {
	value := rustModelEvalCandidate("candidate", 1)
	value.Roles = []string{"primary", "primary"}
	_, policies := rustModelEvalPolicies("candidate", 0)
	err := validateMatrix(rustModelEvalSuite(1), policies, []candidate{value}, false)
	if err == nil || !strings.Contains(err.Error(), "repeats an evaluation role") {
		t.Fatalf("duplicate role error = %v", err)
	}

	value.Roles = []string{"primary"}
	value.BaseURL = "https://user:secret@example.test/v1"
	value.Pricing = nil
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(encoded), "base_url") {
		t.Fatalf("private base URL was serialized: %s", encoded)
	}

	value.ID = "../escape"
	if err := validateMatrix(rustModelEvalSuite(1), policies, []candidate{value}, false); err == nil {
		t.Fatal("path escaping candidate ID was accepted")
	}
}

// Rust source: crates/noema-model-evals/src/matrix_manifest.rs::manifest_requires_exact_openrouter_mapping_and_rejects_local_targets
func TestRustModelEvals_ManifestRequiresExactOpenrouterMappingAndRejectsLocalTargets(t *testing.T) {
	value := rustModelEvalCandidate("candidate", 1)
	_, policies := rustModelEvalPolicies("candidate", 0)
	value.Targets[0].ModelProfile = "vendor/other"
	err := validateMatrix(rustModelEvalSuite(1), policies, []candidate{value}, false)
	if err == nil || !strings.Contains(err.Error(), "exact OpenRouter model and effort") {
		t.Fatalf("mismatched OpenRouter target error = %v", err)
	}

	value = rustModelEvalCandidate("candidate", 1)
	value.Targets = []recommendationTarget{{Provider: "local_models", ModelProfile: "local-model"}}
	err = validateMatrix(rustModelEvalSuite(1), policies, []candidate{value}, false)
	if err == nil || !strings.Contains(err.Error(), "cannot map local recommendation target") {
		t.Fatalf("local target error = %v", err)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_manifest.rs::bundled_luna_effort_candidates_are_primary_only
func TestRustModelEvals_BundledLunaEffortCandidatesArePrimaryOnly(t *testing.T) {
	var manifest struct {
		Candidates []candidate `toml:"candidates"`
	}
	path := filepath.Join("..", "..", "evals/model-matrix/candidates.toml")
	if err := readTOML(path, &manifest); err != nil {
		t.Fatal(err)
	}
	for _, expected := range []struct {
		id     string
		effort string
	}{
		{"openrouter-luna-medium-primary", "medium"},
		{"openrouter-luna-high-primary", "high"},
	} {
		index := -1
		for i := range manifest.Candidates {
			if manifest.Candidates[i].ID == expected.id {
				index = i
				break
			}
		}
		if index < 0 {
			t.Fatalf("missing %s", expected.id)
		}
		candidate := manifest.Candidates[index]
		if candidate.Model != "openai/gpt-5.6-luna" || candidate.ReasoningEffort != expected.effort || len(candidate.Roles) != 1 || candidate.Roles[0] != "primary" {
			t.Fatalf("unexpected %s candidate: %#v", expected.id, candidate)
		}
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::default_decision_suppresses_recommendations_until_finished
func TestRustModelEvals_DefaultDecisionSuppressesRecommendationsUntilFinished(t *testing.T) {
	cheap := rustModelEvalCandidate("cheap", 1)
	enabled := false
	cheap.Enabled = &enabled
	expensive := rustModelEvalCandidate("expensive", 2)
	report := newMatrixReport("test", rustModelEvalSuite(2), []candidate{expensive, cheap}, rustModelEvalPoliciesForReport("cheap", 0), "default_decision")
	for _, id := range []string{"expensive", "cheap"} {
		report.push(matrixEntry{CandidateID: id, Repetition: 1, Cases: rustModelEvalCases("primary")})
	}
	primary := report.Rankings[0]
	if primary.RecommendedCandidateID != "" {
		t.Fatal("incomplete report recommended a candidate")
	}
	for _, score := range primary.Candidates {
		if score.Qualified {
			t.Fatal("incomplete candidate qualified")
		}
	}
	report.finish()
	if report.Status != "incomplete" {
		t.Fatalf("status after first finish = %q", report.Status)
	}
	for _, id := range []string{"expensive", "cheap"} {
		report.push(matrixEntry{CandidateID: id, Repetition: 2, Cases: rustModelEvalCases("primary")})
	}
	if report.Rankings[0].RecommendedCandidateID != "" {
		t.Fatal("before final finish report recommended a candidate")
	}
	report.finish()
	if got := report.Rankings[0].RecommendedCandidateID; got != "cheap" {
		t.Fatalf("recommended candidate = %q, want cheap", got)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::exploration_never_emits_a_final_recommendation
func TestRustModelEvals_ExplorationNeverEmitsAFinalRecommendation(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("candidate", 1)}, rustModelEvalPoliciesForReport("candidate", 0), "exploration")
	report.push(matrixEntry{CandidateID: "candidate", Repetition: 1, Cases: rustModelEvalCases("primary")})
	report.finish()
	if report.Status != "incomplete" || report.Rankings[0].RecommendedCandidateID != "" {
		t.Fatalf("exploration report status/recommendation = %q/%q", report.Status, report.Rankings[0].RecommendedCandidateID)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::returned_model_identity_is_a_qualification_gate
func TestRustModelEvals_ReturnedModelIdentityIsAQualificationGate(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("requested/model", 1)}, rustModelEvalPoliciesForReport("requested/model", 0), "default_decision")
	cases := rustModelEvalCases("primary")
	cases[0].ResponseModel = "other/model"
	report.push(matrixEntry{CandidateID: "requested/model", Repetition: 1, Cases: cases})
	report.finish()
	score := report.Rankings[0].Candidates[0]
	if score.IdentityMatched || score.Qualified || report.Rankings[0].RecommendedCandidateID != "" {
		t.Fatalf("identity gate result = matched %t, qualified %t, recommendation %q", score.IdentityMatched, score.Qualified, report.Rankings[0].RecommendedCandidateID)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::one_failed_stateful_action_blocks_primary_qualification
func TestRustModelEvals_OneFailedStatefulActionBlocksPrimaryQualification(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("candidate", 1)}, rustModelEvalPoliciesForReport("candidate", 0), "default_decision")
	results := rustModelEvalCases("primary")
	failed := -1
	for i := range results {
		if results[i].CaseID == "primary_stateful_email_meeting_to_calendar" {
			failed = i
			break
		}
	}
	if failed < 0 {
		t.Fatal("stateful action case is unavailable")
	}
	results[failed].Passed = false
	results[failed].Failure = "did not complete the grounded action"
	report.push(matrixEntry{CandidateID: "candidate", Repetition: 1, Cases: results})
	report.finish()
	score := report.Rankings[0].Candidates[0]
	if score.DeterministicScore == nil || *score.DeterministicScore <= 0.90 {
		t.Fatalf("deterministic score = %v, want > 0.90", score.DeterministicScore)
	}
	if score.Qualified || report.Rankings[0].RecommendedCandidateID != "" {
		t.Fatal("failed critical stateful action still qualified")
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::markdown_exposes_each_stateful_primary_gap
func TestRustModelEvals_MarkdownExposesEachStatefulPrimaryGap(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("candidate", 1)}, rustModelEvalPoliciesForReport("candidate", 0), "default_decision")
	results := rustModelEvalCases("primary")
	failed := -1
	for i := range results {
		if results[i].CaseID == "primary_stateful_package_delivery" {
			failed = i
			break
		}
	}
	if failed < 0 {
		t.Fatal("package delivery case is unavailable")
	}
	results[failed].Passed = false
	results[failed].Failure = "answer omitted the delivery date"
	report.push(matrixEntry{CandidateID: "candidate", Repetition: 1, Cases: results})
	markdown := report.markdown()
	for _, expected := range []string{
		"### Stateful Primary diagnostics",
		"| Candidate | flight to calendar |",
		"| candidate | 1/1 | 1/1 | 1/1 | 1/1 | 0/1 | 1/1 | 1/1 |",
		"candidate · package_delivery · repetition 1",
		"answer omitted the delivery date",
	} {
		if !strings.Contains(markdown, expected) {
			t.Fatalf("markdown lacks %q:\n%s", expected, markdown)
		}
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::replacement_margin_retains_a_qualified_incumbent
func TestRustModelEvals_ReplacementMarginRetainsAQualifiedIncumbent(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("challenger", 0.5), rustModelEvalCandidate("incumbent", 1)}, rustModelEvalPoliciesForReport("incumbent", 0.1), "default_decision")
	for _, id := range []string{"challenger", "incumbent"} {
		report.push(matrixEntry{CandidateID: id, Repetition: 1, Cases: rustModelEvalCases("primary")})
	}
	report.finish()
	if report.Rankings[0].RecommendedCandidateID != "incumbent" || !strings.Contains(report.Rankings[0].SelectionReason, "below margin") {
		t.Fatalf("recommendation/reason = %q/%q", report.Rankings[0].RecommendedCandidateID, report.Rankings[0].SelectionReason)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::quality_and_tail_latency_thresholds_are_qualification_gates
func TestRustModelEvals_QualityAndTailLatencyThresholdsAreQualificationGates(t *testing.T) {
	policies := rustModelEvalPoliciesForReport("candidate", 0)
	primary := policyForRole(&policies, "primary")
	primary.MinimumQualityScore = 0.75
	primary.MaximumP95LatencyMS = 50
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("candidate", 1)}, policies, "default_decision")
	passing := rustModelEvalCase("primary")
	passing.CaseID = "fast"
	slow := rustModelEvalCase("primary")
	slow.CaseID = "slow"
	slow.LatencyMS = 51
	report.push(matrixEntry{CandidateID: "candidate", Repetition: 1, Cases: []runtime.EvaluationResult{passing, slow}})
	report.finish()
	score := report.Rankings[0].Candidates[0]
	if score.QualityScore == nil || *score.QualityScore != 1 {
		t.Fatalf("quality score = %v, want 1", score.QualityScore)
	}
	if score.P95LatencyMS == nil || *score.P95LatencyMS != 51 || score.Qualified {
		t.Fatalf("p95/qualification = %v/%t", score.P95LatencyMS, score.Qualified)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::required_comparison_keeps_decision_incomplete_until_recorded
func TestRustModelEvals_RequiredComparisonKeepsDecisionIncompleteUntilRecorded(t *testing.T) {
	policies := rustModelEvalPoliciesForReport("incumbent", 0)
	primary := policyForRole(&policies, "primary")
	primary.DeterministicWeight = 0.5
	primary.JudgeWeight = 0.5
	primary.JudgeCaseIDs = []string{"case"}
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("challenger", 1), rustModelEvalCandidate("incumbent", 1)}, policies, "default_decision")
	for _, id := range []string{"challenger", "incumbent"} {
		report.push(matrixEntry{CandidateID: id, Repetition: 1, Cases: rustModelEvalCases("primary")})
	}
	report.finish()
	if report.Status != "incomplete" {
		t.Fatalf("status without comparison = %q", report.Status)
	}
	report.pushComparison(roleComparison{Role: "primary", ChallengerCandidateID: "challenger", IncumbentCandidateID: "incumbent", JudgeModel: "judge/model", CandidateAID: "challenger", CandidateBID: "incumbent", WinnerCandidateID: "challenger", ChallengerScore: intPointer(90), IncumbentScore: intPointer(80), Rationale: "Challenger was more complete.", ResponseProvider: "openrouter", ResponseModel: "judge/model"})
	report.finish()
	if report.Status != "complete" {
		t.Fatalf("status with comparison = %q", report.Status)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_report/tests.rs::checkpoint_rejects_a_duplicate_case_without_overwriting
func TestRustModelEvals_CheckpointRejectsADuplicateCaseWithoutOverwriting(t *testing.T) {
	report := newMatrixReport("test", rustModelEvalSuite(1), []candidate{rustModelEvalCandidate("candidate", 1)}, rustModelEvalPoliciesForReport("candidate", 0), "default_decision")
	first := rustModelEvalCase("primary")
	first.CaseID = "case"
	if err := report.recordCase("candidate", 1, first); err != nil {
		t.Fatal(err)
	}
	duplicate := first
	if err := report.recordCase("candidate", 1, duplicate); err == nil || !strings.Contains(err.Error(), "already checkpointed") {
		t.Fatalf("duplicate checkpoint error = %v", err)
	}
	if len(report.Entries[0].Cases) != 1 {
		t.Fatalf("checkpoint case count = %d", len(report.Entries[0].Cases))
	}
	report.fail("provider unavailable")
	report.resume()
	if report.Status != "running" || report.Failure != "" || !report.hasCase("candidate", 1, "case") {
		t.Fatalf("resumed checkpoint state = status %q, failure %q, has case %t", report.Status, report.Failure, report.hasCase("candidate", 1, "case"))
	}
}

// Rust source: crates/noema-model-evals/src/matrix_runner.rs::default_selection_skips_disabled_candidates_but_explicit_selection_keeps_them
func TestRustModelEvals_DefaultSelectionSkipsDisabledCandidatesButExplicitSelectionKeepsThem(t *testing.T) {
	enabled := rustModelEvalCandidate("enabled", 1)
	trueValue := true
	enabled.Enabled = &trueValue
	disabled := rustModelEvalCandidate("disabled", 1)
	falseValue := false
	disabled.Enabled = &falseValue
	selected, err := selectEvaluationCandidates([]candidate{enabled, disabled}, nil)
	if err != nil || len(selected) != 1 {
		t.Fatalf("default selection = %#v, err = %v", selected, err)
	}
	selected, err = selectEvaluationCandidates([]candidate{enabled, disabled}, []string{"disabled"})
	if err != nil || len(selected) != 1 || selected[0].ID != "disabled" {
		t.Fatalf("explicit selection = %#v, err = %v", selected, err)
	}
}

// Rust source: crates/noema-model-evals/src/matrix_runner.rs::default_decision_requires_every_runtime_role
func TestRustModelEvals_DefaultDecisionRequiresEveryRuntimeRole(t *testing.T) {
	_, policies := rustModelEvalPolicies("primary-only", 0)
	err := validateDecisionCandidates([]candidate{rustModelEvalCandidate("primary-only", 1)}, policies)
	if err == nil || !strings.Contains(err.Error(), "task_simple") {
		t.Fatalf("missing-role error = %v", err)
	}
	custom := rustModelEvalCandidate("custom-endpoint", 1)
	custom.BaseURL = "https://example.test/v1"
	err = validateDecisionCandidates([]candidate{custom}, policies)
	if err == nil || !strings.Contains(err.Error(), "cannot override the OpenRouter base URL") {
		t.Fatalf("custom endpoint error = %v", err)
	}
}

// Rust source: crates/noema-model-evals/src/recommendation_proposal.rs::rendered_cell_line_matches_the_table_authority
func TestRustModelEvals_RenderedCellLineMatchesTheTableAuthority(t *testing.T) {
	recommendations := provider.ModelRecommendations("openrouter")
	var action provider.ModelRecommendation
	found := false
	for _, recommendation := range recommendations {
		if recommendation.UseCase == provider.ModelUseActionReviewer {
			action, found = recommendation, true
			break
		}
	}
	if !found {
		t.Fatal("action reviewer recommendation is unavailable")
	}
	block := recommendationCellLine(string(action.UseCase), action.ModelProfile, action.ReasoningEffort)
	if block != "\t\t\t{UseCase: ModelUseCase(\"action_reviewer\"), ModelProfile: \"openai/gpt-5.6-luna\", ReasoningEffort: \"low\"},\n" {
		t.Fatalf("rendered recommendation cell = %q", block)
	}
	source, err := os.ReadFile(filepath.Join("..", "provider", "recommendations.go"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(source), block) {
		t.Fatalf("recommendation source lacks the authoritative cell: %s", source)
	}
	if action.ModelProfile != "openai/gpt-5.6-luna" || action.ReasoningEffort != "low" {
		t.Fatalf("table authority action reviewer = %#v", action)
	}
}

// Rust source: crates/noema-model-evals/src/recommendation_proposal.rs::unmapped_winner_does_not_change_a_direct_provider
func TestRustModelEvals_UnmappedWinnerDoesNotChangeADirectProvider(t *testing.T) {
	report := rustModelEvalProposalReport(t)
	ranking := rustModelEvalRanking([]struct {
		id    string
		value float64
	}{
		{"openrouter-deepseek-v4-flash", 0.99},
		{"openrouter-luna-high-primary", 0.90},
	})
	current := recommendationFor(provider.ModelRecommendations("openai"), provider.ModelUsePrimary)
	winner, target, _ := selectForProvider(report, ranking, "openai", current)
	if winner != "openrouter-luna-high-primary" || target == nil || target.ModelProfile != "gpt-5.6-luna" {
		t.Fatalf("direct provider selection = %q/%#v", winner, target)
	}
}

// Rust source: crates/noema-model-evals/src/recommendation_proposal.rs::provider_selection_retains_incumbent_below_margin
func TestRustModelEvals_ProviderSelectionRetainsIncumbentBelowMargin(t *testing.T) {
	report := rustModelEvalProposalReport(t)
	ranking := rustModelEvalRanking([]struct {
		id    string
		value float64
	}{
		{"openrouter-luna-high-primary", 0.92},
		{"openrouter-terra-medium", 0.90},
	})
	current := recommendationFor(provider.ModelRecommendations("openai"), provider.ModelUsePrimary)
	winner, _, reason := selectForProvider(report, ranking, "openai", current)
	if winner != "openrouter-terra-medium" || !strings.Contains(reason, "below margin") {
		t.Fatalf("selection/reason = %q/%q", winner, reason)
	}
}

// Rust source: crates/noema-model-evals/src/recommendation_proposal.rs::patch_changes_only_the_exact_provider_role_cell
func TestRustModelEvals_PatchChangesOnlyTheExactProviderRoleCell(t *testing.T) {
	path := filepath.Join("..", "provider", "recommendations.go")
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	old := provider.ModelRecommendation{UseCase: provider.ModelUsePrimary, ModelProfile: "gpt-5.6-terra", ReasoningEffort: "medium"}
	replacement := provider.ModelRecommendation{UseCase: provider.ModelUsePrimary, ModelProfile: "gpt-5.6-luna", ReasoningEffort: "high"}
	patch, err := renderRecommendationPatch(path, source, []recommendationChange{{Provider: "openai", Role: string(provider.ModelUsePrimary), Old: old, New: replacement}})
	if err != nil {
		t.Fatal(err)
	}
	oldLine := recommendationCellLine(string(provider.ModelUsePrimary), old.ModelProfile, old.ReasoningEffort)
	newLine := recommendationCellLine(string(provider.ModelUsePrimary), replacement.ModelProfile, replacement.ReasoningEffort)
	if strings.Count(patch, "@@ ") != 1 || !strings.Contains(patch, "-"+oldLine) || !strings.Contains(patch, "+"+newLine) {
		t.Fatalf("recommendation patch = %q", patch)
	}
}

// Rust source: crates/noema-model-evals/src/role_policy.rs::bundled_role_policies_cover_every_role
func TestRustModelEvals_BundledRolePoliciesCoverEveryRole(t *testing.T) {
	_, policies, _, err := loadMatrix(filepath.Join("..", ".."), nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(policies.Policies) != len(roles()) {
		t.Fatalf("role policy count = %d, want %d", len(policies.Policies), len(roles()))
	}
}

func rustModelEvalSuite(repetitions int) suiteConfig {
	return suiteConfig{ContextWindowTokens: 8192, GenerationTimeoutSeconds: 120, StartupTimeoutSeconds: 180, WorkerTimeoutSeconds: 300, Repetitions: repetitions}
}

func rustModelEvalCandidate(id string, inputPrice float64) candidate {
	enabled := true
	return candidate{ID: id, Name: id, Model: id, Roles: []string{"primary"}, AcceptedResponseModels: []string{"case-model"}, Targets: []recommendationTarget{{Provider: "openrouter", ModelProfile: id}}, Pricing: &modelPricing{InputUSDPerMillion: inputPrice, OutputUSDPerMillion: 1}, Enabled: &enabled}
}

func rustModelEvalPolicies(incumbent string, margin float64) (suiteConfig, rolePolicies) {
	return rustModelEvalSuite(1), rustModelEvalPoliciesForReport(incumbent, margin)
}

func rustModelEvalPoliciesForReport(incumbent string, margin float64) rolePolicies {
	policies := rolePolicies{SchemaVersion: 2, Judge: judgePolicy{Model: "judge/model", MaximumOutputTokens: 256}}
	for _, role := range roles() {
		policies.Policies = append(policies.Policies, rolePolicy{Role: role, IncumbentCandidateID: incumbent, MinimumCases: 1, MinimumQualityScore: 1, MaximumErrorRate: 0, MaximumP95LatencyMS: 100, ReplacementQualityMargin: margin, DeterministicWeight: 1})
	}
	return policies
}

func policyForRole(policies *rolePolicies, role string) *rolePolicy {
	for i := range policies.Policies {
		if policies.Policies[i].Role == role {
			return &policies.Policies[i]
		}
	}
	return nil
}

func rustModelEvalCase(role string) runtime.EvaluationResult {
	return runtime.EvaluationResult{CaseID: "case", Role: role, Category: "category", Critical: true, Passed: true, ResponseProvider: "openrouter", ResponseModel: "case-model", LatencyMS: 10, FirstVisibleDeltaMS: int64Pointer(5), StreamedChars: 1, InputTokens: intPointer(1000), CachedInputTokens: intPointer(0), OutputTokens: intPointer(100), ToolCalls: []runtime.EvaluationToolCall{}}
}

func rustModelEvalCases(role string) []runtime.EvaluationResult {
	var results []runtime.EvaluationResult
	for _, descriptor := range runtime.EvaluationCases([]string{role}) {
		result := rustModelEvalCase(descriptor.Role)
		result.CaseID, result.Category, result.Critical, result.JudgeRubric = descriptor.ID, descriptor.Category, descriptor.Critical, descriptor.JudgeRubric
		results = append(results, result)
	}
	return results
}

func rustModelEvalRanking(values []struct {
	id    string
	value float64
}) roleRanking {
	result := roleRanking{Role: "primary"}
	for _, value := range values {
		quality := value.value
		result.Candidates = append(result.Candidates, candidateScore{CandidateID: value.id, CompletedRepetitions: 3, Qualified: true, IdentityMatched: true, QualityScore: &quality})
	}
	return result
}

func rustModelEvalProposalReport(t *testing.T) matrixReport {
	t.Helper()
	suite, policies, candidates, err := loadMatrix(filepath.Join("..", ".."), nil)
	if err != nil {
		t.Fatal(err)
	}
	return newMatrixReport("test", suite, candidates, policies, "default_decision")
}

func recommendationFor(recommendations []provider.ModelRecommendation, useCase provider.ModelUseCase) provider.ModelRecommendation {
	for _, recommendation := range recommendations {
		if recommendation.UseCase == useCase {
			return recommendation
		}
	}
	return provider.ModelRecommendation{}
}

func intPointer(value int) *int { return &value }

func int64Pointer(value int64) *int64 { return &value }
