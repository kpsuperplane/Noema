package modeleval

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/kpsuperplane/noema/internal/runtime"
)

func TestBundledPlanAndBudget(t *testing.T) {
	s, p, cs, e := loadMatrix("../..", nil)
	if e != nil {
		t.Fatal(e)
	}
	cost, e := estimate(s, p, cs)
	if e != nil || cost <= 0 {
		t.Fatalf("cost %f: %v", cost, e)
	}
	plan := decisionPlan{SchemaVersion: 2, DecisionID: "decision-test", RuntimeSuiteVersion: runtime.EvaluationSuiteVersion, Suite: s, Policies: p, Candidates: cs, EstimatedMaxCostUSD: cost, SpendCeilingUSD: cost}
	plan.ContentFingerprint = plan.hash()
	if e = plan.validate(); e != nil {
		t.Fatal(e)
	}
	plan.Suite.Repetitions++
	if plan.validate() == nil {
		t.Fatal("modified plan accepted")
	}
	plan.Suite = s
	plan.SpendCeilingUSD = cost / 2
	plan.ContentFingerprint = plan.hash()
	if plan.validate() == nil {
		t.Fatal("insufficient budget accepted")
	}
	if _, _, _, e = loadMatrix("../..", []string{"absent"}); e == nil {
		t.Fatal("unknown candidate accepted")
	}
	p.Policies[0].JudgeCaseIDs = []string{"removed_case"}
	if validateMatrix(s, p, cs, true) == nil {
		t.Fatal("stale judge case accepted")
	}
}
func perfectReport(t *testing.T) matrixReport {
	t.Helper()
	s, p, cs, e := loadMatrix("../..", nil)
	if e != nil {
		t.Fatal(e)
	}
	r := matrixReport{SchemaVersion: 3, RuntimeSuiteVersion: runtime.EvaluationSuiteVersion, RunID: "test", Mode: "default_decision", Status: "complete", Repetitions: 1, Policies: p, Candidates: cs, DecisionFingerprint: reportFingerprint(s, p, cs)}
	tokens := 1
	for _, c := range cs {
		entry := matrixEntry{CandidateID: c.ID, Repetition: 1}
		for _, v := range runtime.EvaluationCases(c.Roles) {
			entry.Cases = append(entry.Cases, runtime.EvaluationResult{CaseID: v.ID, Role: v.Role, Category: v.Category, Critical: v.Critical, JudgeRubric: v.JudgeRubric, Passed: true, ResponseProvider: "openrouter", ResponseModel: c.Model, LatencyMS: 10, InputTokens: &tokens, OutputTokens: &tokens})
		}
		r.Entries = append(r.Entries, entry)
	}
	for _, policy := range p.Policies {
		if len(policy.JudgeCaseIDs) == 0 {
			continue
		}
		for _, c := range cs {
			if c.ID == policy.IncumbentCandidateID {
				continue
			}
			hasRole := false
			for _, role := range c.Roles {
				hasRole = hasRole || role == policy.Role
			}
			if !hasRole {
				continue
			}
			score := 100
			r.Comparisons = append(r.Comparisons, roleComparison{Role: policy.Role, ChallengerCandidateID: c.ID, IncumbentCandidateID: policy.IncumbentCandidateID, JudgeModel: p.Judge.Model, ResponseProvider: "openrouter", ResponseModel: p.Judge.Model, ChallengerScore: &score, IncumbentScore: &score})
		}
	}
	return r
}
func TestCheckpointCompletenessAndQualification(t *testing.T) {
	r := perfectReport(t)
	if !complete(r) {
		t.Fatal("complete evidence rejected")
	}
	rankings := rank(r)
	if rankings[0].RecommendedCandidateID != r.Policies.Policies[0].IncumbentCandidateID {
		t.Fatal("incumbent margin ignored")
	}
	r.Entries[0].Cases[0].ResponseModel = "wrong-model"
	for _, ranking := range rank(r) {
		for _, c := range ranking.Candidates {
			if c.CandidateID == r.Candidates[0].ID && c.Qualified {
				t.Fatal("wrong identity qualified")
			}
		}
	}
	r = perfectReport(t)
	r.Entries[0].Cases = append(r.Entries[0].Cases, r.Entries[0].Cases[0])
	if validateEvidence(r) == nil || complete(r) {
		t.Fatal("duplicate case accepted")
	}
	r = perfectReport(t)
	r.Entries[0].Cases = r.Entries[0].Cases[1:]
	if complete(r) {
		t.Fatal("missing case accepted")
	}
	r = perfectReport(t)
	r.Comparisons[0].Error = "judge failed"
	if complete(r) {
		t.Fatal("failed judge accepted")
	}
	r = perfectReport(t)
	r.Mode = "exploration"
	for _, ranking := range rank(r) {
		if ranking.RecommendedCandidateID != "" {
			t.Fatal("exploration changed defaults")
		}
	}
}
func TestStrictJudgeAndAtomicCheckpoint(t *testing.T) {
	good := `{"winner":"a","a_score":95,"b_score":90,"rationale":"A preserves the required source facts."}`
	if _, e := parseJudge(good); e != nil {
		t.Fatal(e)
	}
	for _, bad := range []string{`{"winner":"a","a_score":101,"b_score":90,"rationale":"x"}`, `{"winner":"tie","rationale":"x"}`, good + ` {}`, `{"winner":"c","a_score":95,"b_score":90,"rationale":"x"}`} {
		if _, e := parseJudge(bad); e == nil {
			t.Fatal("malformed judge accepted")
		}
	}
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	if e := writeJSON(path, map[string]int{"saved": 1}, true); e != nil {
		t.Fatal(e)
	}
	if writeJSON(path, map[string]int{"saved": 2}, true) == nil {
		t.Fatal("immutable plan overwritten")
	}
	if e := writeJSON(path, map[string]int{"saved": 3}, false); e != nil {
		t.Fatal(e)
	}
	var out map[string]int
	if e := readJSON(path, &out); e != nil || out["saved"] != 3 {
		t.Fatalf("checkpoint %v %v", out, e)
	}
}
func TestProposalRejectsIncompleteEvidence(t *testing.T) {
	dir := t.TempDir()
	s, p, cs, e := loadMatrix("../..", nil)
	if e != nil {
		t.Fatal(e)
	}
	cost, e := estimate(s, p, cs)
	if e != nil {
		t.Fatal(e)
	}
	plan := decisionPlan{SchemaVersion: 2, DecisionID: "test", RuntimeSuiteVersion: runtime.EvaluationSuiteVersion, Suite: s, Policies: p, Candidates: cs, EstimatedMaxCostUSD: cost, SpendCeilingUSD: cost}
	plan.ContentFingerprint = plan.hash()
	if e = writeJSON(filepath.Join(dir, "plan.json"), plan, true); e != nil {
		t.Fatal(e)
	}
	r := perfectReport(t)
	r.Entries = nil
	if e = writeJSON(filepath.Join(dir, "report.json"), r, true); e != nil {
		t.Fatal(e)
	}
	if proposal("../..", dir, false) == nil {
		t.Fatal("incomplete proposal accepted")
	}
	if _, e = os.Stat(filepath.Join(dir, "recommendations.patch")); !os.IsNotExist(e) {
		t.Fatal("unexpected patch")
	}
	b, _ := json.Marshal(r)
	if !json.Valid(b) {
		t.Fatal("report JSON")
	}
}
