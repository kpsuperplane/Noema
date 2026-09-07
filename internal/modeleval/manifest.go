// Package modeleval qualifies models through the production Go runtime.
package modeleval

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/pelletier/go-toml/v2"
)

func readTOML(path string, out any) error {
	f, e := os.Open(path)
	if e != nil {
		return e
	}
	defer f.Close()
	return toml.NewDecoder(f).DisallowUnknownFields().Decode(out)
}
func readJSON(path string, out any) error {
	b, e := os.ReadFile(path)
	if e != nil {
		return e
	}
	return decodeJSON(b, out)
}
func decodeJSON(b []byte, out any) error {
	d := json.NewDecoder(bytes.NewReader(b))
	d.DisallowUnknownFields()
	if e := d.Decode(out); e != nil {
		return e
	}
	var extra any
	if d.Decode(&extra) != io.EOF {
		return errors.New("unexpected trailing JSON")
	}
	return nil
}
func writeJSON(path string, value any, exclusive bool) error {
	b, e := json.MarshalIndent(value, "", "  ")
	if e != nil {
		return e
	}
	b = append(b, '\n')
	if e = os.MkdirAll(filepath.Dir(path), 0700); e != nil {
		return e
	}
	if exclusive {
		f, e := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
		if e != nil {
			return e
		}
		_, e = f.Write(b)
		closeErr := f.Close()
		return errors.Join(e, closeErr)
	}
	f, e := os.CreateTemp(filepath.Dir(path), ".checkpoint-*")
	if e != nil {
		return e
	}
	defer os.Remove(f.Name())
	_, e = f.Write(b)
	if e == nil {
		e = f.Sync()
	}
	e = errors.Join(e, f.Close())
	if e != nil {
		return e
	}
	return os.Rename(f.Name(), path)
}
func fingerprint(value any) string {
	b, _ := json.Marshal(value)
	h := sha256.Sum256(b)
	return hex.EncodeToString(h[:])
}
func gitState(root string) (string, bool, error) {
	c := exec.Command("git", "rev-parse", "HEAD")
	c.Dir = root
	b, e := c.Output()
	if e != nil {
		return "", false, e
	}
	s := exec.Command("git", "status", "--porcelain")
	s.Dir = root
	status, e := s.Output()
	return strings.TrimSpace(string(b)), len(status) > 0, e
}
func roles() []string {
	return []string{
		string(provider.ModelUsePrimary),
		string(provider.ModelUseTaskSimple),
		string(provider.ModelUseTaskMedium),
		string(provider.ModelUseTaskDifficult),
		string(provider.ModelUseTaskReviewer),
		string(provider.ModelUseWebFetchSummarizer),
		string(provider.ModelUseToolProgressAudit),
		string(provider.ModelUseActionReviewer),
		string(provider.ModelUseMemoryConsolidation),
	}
}

func createDecisionPlan(root string, suite suiteConfig, policies rolePolicies, candidates []candidate) (decisionPlan, error) {
	cost, err := estimate(suite, policies, candidates)
	if err != nil {
		return decisionPlan{}, err
	}
	commit, dirty, err := gitState(root)
	if err != nil {
		return decisionPlan{}, err
	}
	now := time.Now()
	shortCommit := commit
	if len(shortCommit) > 12 {
		shortCommit = shortCommit[:12]
	}
	plan := decisionPlan{
		SchemaVersion:        2,
		DecisionID:           fmt.Sprintf("decision-%d-%s", now.UnixNano(), shortCommit),
		CreatedAtUnixSeconds: now.Unix(),
		GitCommit:            commit,
		GitDirty:             dirty,
		RuntimeSuiteVersion:  runtime.EvaluationSuiteVersion,
		Suite:                suite,
		Policies:             policies,
		Candidates:           candidates,
		EstimatedMaxCostUSD:  cost,
		SpendCeilingUSD:      cost,
	}
	plan.ContentFingerprint = plan.hash()
	return plan, nil
}
func validID(id string) bool {
	if id == "" || id == "." || id == ".." {
		return false
	}
	for _, c := range id {
		if !(c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '-' || c == '_' || c == '.') {
			return false
		}
	}
	return true
}
func finiteNonnegative(v float64) bool { return !math.IsNaN(v) && !math.IsInf(v, 0) && v >= 0 }
func (s suiteConfig) repetitions(decision bool) int {
	if decision {
		if s.DecisionRepetitions != nil {
			return *s.DecisionRepetitions
		}
		return s.Repetitions
	}
	if s.ExplorationRepetitions != nil {
		return *s.ExplorationRepetitions
	}
	return 1
}
func (s suiteConfig) validate() error {
	if s.ContextWindowTokens <= 0 || s.ContextWindowTokens > 1_000_000 || s.GenerationTimeoutSeconds <= 0 || s.GenerationTimeoutSeconds > 86400 || s.StartupTimeoutSeconds <= 0 || s.StartupTimeoutSeconds > 86400 || s.WorkerTimeoutSeconds <= 0 || s.WorkerTimeoutSeconds > 86400 || s.Repetitions <= 0 || s.repetitions(true) <= 0 || s.repetitions(false) <= 0 {
		return errors.New("suite settings must have bounded positive values")
	}
	return nil
}
func loadMatrix(root string, ids []string) (suiteConfig, rolePolicies, []candidate, error) {
	var s suiteConfig
	var p rolePolicies
	var m struct {
		Candidates []candidate `toml:"candidates"`
	}
	dir := filepath.Join(root, "evals/model-matrix")
	for _, v := range []struct {
		name string
		out  any
	}{{"suite.toml", &s}, {"role-policies.toml", &p}, {"candidates.toml", &m}} {
		if e := readTOML(filepath.Join(dir, v.name), v.out); e != nil {
			return s, p, nil, e
		}
	}
	if e := validateMatrix(s, p, m.Candidates, false); e != nil {
		return s, p, nil, e
	}
	selected, e := selectEvaluationCandidates(m.Candidates, ids)
	if e != nil {
		return s, p, nil, e
	}
	return s, p, selected, nil
}
func validateMatrix(s suiteConfig, p rolePolicies, cs []candidate, decision bool) error {
	if e := s.validate(); e != nil {
		return e
	}
	if p.SchemaVersion != 2 || p.Judge.Model == "" || p.Judge.MaximumOutputTokens <= 0 {
		return errors.New("invalid candidates or policy version")
	}
	if e := validateCandidates(cs); e != nil {
		return e
	}
	allRoles := roles()
	seen := map[string]bool{}
	for _, r := range p.Policies {
		if !slices.Contains(allRoles, r.Role) || seen[r.Role] || r.MinimumCases < 5 || r.MaximumP95LatencyMS <= 0 {
			return fmt.Errorf("invalid policy %s", r.Role)
		}
		seen[r.Role] = true
		for _, v := range []float64{r.MinimumQualityScore, r.MaximumErrorRate, r.ReplacementQualityMargin, r.DeterministicWeight, r.JudgeWeight} {
			if !finiteNonnegative(v) || v > 1 {
				return errors.New("invalid policy threshold")
			}
		}
		if math.Abs(r.DeterministicWeight+r.JudgeWeight-1) > 1e-9 || (r.JudgeWeight > 0) != (len(r.JudgeCaseIDs) > 0) {
			return errors.New("invalid policy weights")
		}
		cases := runtime.EvaluationCases([]string{r.Role})
		js := map[string]bool{}
		for _, id := range r.JudgeCaseIDs {
			if js[id] || slices.IndexFunc(cases, func(c runtime.EvaluationCase) bool { return c.ID == id && c.JudgeRubric != "" }) < 0 {
				return fmt.Errorf("policy %s references unavailable judge case %s", r.Role, id)
			}
			js[id] = true
		}
		if decision && slices.IndexFunc(cs, func(c candidate) bool { return c.ID == r.IncumbentCandidateID && slices.Contains(c.Roles, r.Role) }) < 0 {
			return fmt.Errorf("missing incumbent for %s", r.Role)
		}
	}
	if len(seen) != len(allRoles) {
		return errors.New("policies must cover every production role")
	}
	if decision {
		if e := validateDecisionCandidates(cs, p); e != nil {
			return e
		}
	}
	return nil
}

func validateCandidates(cs []candidate) error {
	if len(cs) == 0 {
		return errors.New("invalid candidates or policy version")
	}
	seen := map[string]bool{}
	allRoles := roles()
	for _, c := range cs {
		if !validID(c.ID) || seen[c.ID] || c.Model == "" || len(c.Roles) == 0 {
			return fmt.Errorf("invalid or duplicate candidate %s", c.ID)
		}
		seen[c.ID] = true
		rs := map[string]bool{}
		for _, r := range c.Roles {
			if !slices.Contains(allRoles, r) || rs[r] {
				if rs[r] {
					return fmt.Errorf("candidate %s repeats an evaluation role", c.ID)
				}
				return fmt.Errorf("invalid role for %s", c.ID)
			}
			rs[r] = true
		}
		ts := map[string]bool{}
		for _, t := range c.Targets {
			if !slices.Contains([]string{"openrouter", "codex", "openai", "foundation_local", "local_models"}, t.Provider) || ts[t.Provider] || t.ModelProfile == "" {
				return fmt.Errorf("invalid target for %s", c.ID)
			}
			ts[t.Provider] = true
			if slices.Contains([]string{"foundation_local", "local_models"}, t.Provider) {
				return fmt.Errorf("candidate %s cannot map local recommendation target %s", c.ID, t.Provider)
			}
		}
		target := slices.IndexFunc(c.Targets, func(t recommendationTarget) bool { return t.Provider == "openrouter" })
		if target >= 0 {
			if c.Targets[target].ModelProfile != c.Model || c.Targets[target].ReasoningEffort != c.ReasoningEffort {
				return fmt.Errorf("candidate %s must map its exact OpenRouter model and effort", c.ID)
			}
		} else {
			return fmt.Errorf("candidate %s must map its exact OpenRouter model and effort", c.ID)
		}
		if c.Pricing != nil {
			q := c.Pricing
			if !finiteNonnegative(q.InputUSDPerMillion) || !finiteNonnegative(q.OutputUSDPerMillion) || q.CachedInputUSDPerMillion != nil && !finiteNonnegative(*q.CachedInputUSDPerMillion) {
				return errors.New("invalid pricing")
			}
		}
	}
	return nil
}

func validateDecisionCandidates(cs []candidate, p rolePolicies) error {
	if index := slices.IndexFunc(cs, func(c candidate) bool { return c.BaseURL != "" }); index >= 0 {
		return fmt.Errorf("default decision candidate %s cannot override the OpenRouter base URL", cs[index].ID)
	}
	for _, role := range roles() {
		if !slices.ContainsFunc(cs, func(c candidate) bool { return slices.Contains(c.Roles, role) }) {
			return fmt.Errorf("default decision has no candidate for role %s", role)
		}
		policyIndex := slices.IndexFunc(p.Policies, func(v rolePolicy) bool { return v.Role == role })
		if policyIndex < 0 || !slices.ContainsFunc(cs, func(c candidate) bool {
			return c.ID == p.Policies[policyIndex].IncumbentCandidateID && slices.Contains(c.Roles, role)
		}) {
			incumbent := ""
			if policyIndex >= 0 {
				incumbent = p.Policies[policyIndex].IncumbentCandidateID
			}
			return fmt.Errorf("default decision is missing incumbent %s for role %s", incumbent, role)
		}
	}
	return nil
}

func selectEvaluationCandidates(candidates []candidate, ids []string) ([]candidate, error) {
	selected := make([]candidate, 0, len(candidates))
	if len(ids) == 0 {
		for _, candidate := range candidates {
			if candidate.Enabled == nil || *candidate.Enabled {
				selected = append(selected, candidate)
			}
		}
		return selected, nil
	}
	for _, id := range ids {
		index := slices.IndexFunc(candidates, func(candidate candidate) bool { return candidate.ID == id })
		if index < 0 || slices.ContainsFunc(selected, func(candidate candidate) bool { return candidate.ID == id }) {
			return nil, fmt.Errorf("unknown or duplicate candidate %s", id)
		}
		selected = append(selected, candidates[index])
	}
	return selected, nil
}

func estimate(s suiteConfig, p rolePolicies, cs []candidate) (float64, error) {
	if e := validateMatrix(s, p, cs, true); e != nil {
		return 0, e
	}
	total := 0.0
	for _, c := range cs {
		if c.Pricing == nil {
			return 0, fmt.Errorf("candidate %s has no pricing", c.ID)
		}
		for _, v := range runtime.EvaluationCases(c.Roles) {
			total += float64(s.repetitions(true)*v.MaximumProviderCalls) * (float64(s.ContextWindowTokens)*c.Pricing.InputUSDPerMillion + float64(v.MaximumOutputTokens)*c.Pricing.OutputUSDPerMillion) / 1e6
		}
	}
	var judge *modelPricing
	for _, c := range cs {
		if c.Model == p.Judge.Model && c.ReasoningEffort == p.Judge.ReasoningEffort {
			judge = c.Pricing
		}
	}
	if judge == nil {
		return 0, errors.New("pinned judge needs a matching priced candidate")
	}
	for _, r := range p.Policies {
		if len(r.JudgeCaseIDs) == 0 {
			continue
		}
		for _, c := range cs {
			if c.ID != r.IncumbentCandidateID && slices.Contains(c.Roles, r.Role) {
				total += (float64(s.ContextWindowTokens)*judge.InputUSDPerMillion + float64(p.Judge.MaximumOutputTokens)*judge.OutputUSDPerMillion) / 1e6
			}
		}
	}
	if !finiteNonnegative(total) {
		return 0, errors.New("invalid cost estimate")
	}
	return total, nil
}
func (p decisionPlan) hash() string { p.ContentFingerprint = ""; return fingerprint(p) }
func (p decisionPlan) validate() error {
	if p.SchemaVersion != 2 || p.RuntimeSuiteVersion != runtime.EvaluationSuiteVersion || !validID(p.DecisionID) {
		return errors.New("unsupported decision plan")
	}
	cost, e := estimate(p.Suite, p.Policies, p.Candidates)
	if e != nil {
		return e
	}
	if math.Abs(cost-p.EstimatedMaxCostUSD) > 1e-6 || !finiteNonnegative(p.SpendCeilingUSD) || p.SpendCeilingUSD < cost || p.ContentFingerprint != p.hash() {
		return errors.New("decision content, estimate, or budget changed")
	}
	return nil
}

func (p decisionPlan) validateExecutionGitState(root string) error {
	commit, dirty, e := gitState(root)
	if e != nil {
		return e
	}
	if p.GitDirty || dirty || commit != p.GitCommit {
		return errors.New("defaults run requires the exact clean Git commit recorded by a clean plan")
	}
	return nil
}
