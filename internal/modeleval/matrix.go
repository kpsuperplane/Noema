package modeleval

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func hosted(ctx context.Context, cs []candidate, judge string) (provider.Generator, string, func(), error) {
	rawKey, present := os.LookupEnv("OPENROUTER_API_KEY")
	var keyValue *string
	if present {
		keyValue = &rawKey
	}
	key, e := openrouterAPIKey(keyValue)
	if e != nil {
		return nil, "", nil, e
	}
	key, e = (openRouterEnvCredentials{apiKey: key}).apiKeyFor("openrouter", openRouterDefaultAccountID)
	if e != nil {
		return nil, "", nil, e
	}
	root, e := os.MkdirTemp("", "noema-evaluation-*")
	if e != nil {
		return nil, "", nil, e
	}
	cleanup := func() { _ = os.RemoveAll(root) }
	db, e := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if e != nil {
		cleanup()
		return nil, "", nil, e
	}
	cleanup = func() { _ = db.Close(); _ = os.RemoveAll(root) }
	fail := func(e error) (provider.Generator, string, func(), error) { cleanup(); return nil, "", nil, e }
	accounts, e := provider.NewAccountService(root, db)
	if e != nil {
		return fail(e)
	}
	if e = accounts.Initialize(ctx, time.Now()); e != nil {
		return fail(e)
	}
	service, e := provider.NewOpenRouterService(accounts, "http://127.0.0.1:17831")
	if e != nil {
		return fail(e)
	}
	secret, e := provider.NewSecret(key)
	if e != nil {
		return fail(e)
	}
	account, e := service.CreateAPIKeyAccount(ctx, secret)
	if e != nil {
		return fail(e)
	}
	profiles, e := account.Metadata.ModelProfiles()
	if e != nil {
		return fail(e)
	}
	models := []string{judge}
	for _, c := range cs {
		models = append(models, c.Model)
	}
	for _, m := range models {
		if m != "" && slices.IndexFunc(profiles, func(p provider.ModelProfile) bool { return p.ID == m }) < 0 {
			return fail(fmt.Errorf("planned model %s is absent from the provider catalog", m))
		}
	}
	gen, e := provider.NewOpenRouterGenerator(accounts)
	if e != nil {
		return fail(e)
	}
	return gen, account.ID, cleanup, nil
}
func reportFingerprint(s suiteConfig, p rolePolicies, cs []candidate) string {
	return fingerprint(struct {
		Version    int
		Suite      suiteConfig
		Policies   rolePolicies
		Candidates []candidate
	}{runtime.EvaluationSuiteVersion, s, p, cs})
}
func runMatrix(ctx context.Context, root, dir, id, mode string, s suiteConfig, p rolePolicies, cs []candidate) error {
	if e := validateMatrix(s, p, cs, mode == "default_decision"); e != nil {
		return e
	}
	expected := newMatrixReport(id, s, cs, p, mode)
	r := expected
	path := filepath.Join(dir, "report.json")
	if _, e := os.Stat(path); e == nil {
		if e = readJSON(path, &r); e != nil {
			return e
		}
		if r.SchemaVersion != expected.SchemaVersion || r.RunID != id || r.Mode != mode || r.RuntimeSuiteVersion != expected.RuntimeSuiteVersion || r.DecisionFingerprint != expected.DecisionFingerprint || r.Repetitions != expected.Repetitions || fingerprint(r.Candidates) != fingerprint(cs) || fingerprint(r.Policies) != fingerprint(p) {
			return errors.New("checkpoint differs from the planned run")
		}
	} else if !errors.Is(e, os.ErrNotExist) {
		return e
	}
	if e := validateEvidence(r); e != nil {
		return e
	}
	if complete(r) {
		r.Status = "complete"
		r.Rankings = rank(r)
		return saveReport(dir, r)
	}
	gen, account, close, e := hosted(ctx, cs, p.Judge.Model)
	if e != nil {
		return e
	}
	defer close()
	checkpoint := func() error { r.Rankings = rank(r); return saveReport(dir, r) }
	r.Status = "running"
	r.Failure = ""
	if e = checkpoint(); e != nil {
		return e
	}
	for _, c := range cs {
		for repetition := 1; repetition <= r.Repetitions; repetition++ {
			index := slices.IndexFunc(r.Entries, func(v matrixEntry) bool { return v.CandidateID == c.ID && v.Repetition == repetition })
			if index < 0 {
				r.Entries = append(r.Entries, matrixEntry{CandidateID: c.ID, Repetition: repetition, Cases: []runtime.EvaluationResult{}})
				index = len(r.Entries) - 1
			}
			entry := &r.Entries[index]
			if entry.Error != "" {
				return errors.New("checkpoint contains an interrupted request; create a new plan to authorize another attempt")
			}
			for _, ec := range runtime.EvaluationCases(c.Roles) {
				if slices.ContainsFunc(entry.Cases, func(v runtime.EvaluationResult) bool { return v.CaseID == ec.ID }) {
					continue
				}
				fmt.Fprintf(os.Stderr, "%s repetition %d: %s\n", c.ID, repetition, ec.ID)
				entry.Error = "request in progress; its cost is uncertain after interruption"
				if e = checkpoint(); e != nil {
					return e
				}
				caseCtx, cancel := context.WithTimeout(ctx, time.Duration(s.GenerationTimeoutSeconds)*time.Second)
				result, err := runtime.RunEvaluationCase(caseCtx, gen, "openrouter", provider.GenerateRequest{AccountID: account, Model: c.Model, ReasoningEffort: c.ReasoningEffort}, ec.ID, uint32(s.ContextWindowTokens))
				cancel()
				if err != nil {
					entry.Error = err.Error()
					r.Status = "incomplete"
					r.Failure = err.Error()
					return errors.Join(err, checkpoint())
				}
				entry.Error = ""
				entry.Cases = append(entry.Cases, result)
				if e = checkpoint(); e != nil {
					return e
				}
			}
		}
	}
	if mode == "default_decision" {
		for _, policy := range p.Policies {
			if len(policy.JudgeCaseIDs) == 0 {
				continue
			}
			for _, c := range cs {
				if c.ID == policy.IncumbentCandidateID || !slices.Contains(c.Roles, policy.Role) || slices.ContainsFunc(r.Comparisons, func(v roleComparison) bool { return v.Role == policy.Role && v.ChallengerCandidateID == c.ID }) {
					continue
				}
				r.Comparisons = append(r.Comparisons, roleComparison{Role: policy.Role, ChallengerCandidateID: c.ID, IncumbentCandidateID: policy.IncumbentCandidateID, JudgeModel: p.Judge.Model, Error: "judge request interrupted; cost is uncertain"})
				if e = checkpoint(); e != nil {
					return e
				}
				cmp := judgePair(ctx, gen, account, r, s, policy, c.ID)
				r.Comparisons[len(r.Comparisons)-1] = cmp
				if e = checkpoint(); e != nil {
					return e
				}
			}
		}
	}
	r.Status = "incomplete"
	if complete(r) {
		r.Status = "complete"
	}
	if e = checkpoint(); e != nil {
		return e
	}
	if r.Status != "complete" {
		return errors.New("evaluation evidence is incomplete; inspect report.json")
	}
	return nil
}
func validateEvidence(r matrixReport) error {
	entries := map[string]bool{}
	for _, e := range r.Entries {
		ci := slices.IndexFunc(r.Candidates, func(c candidate) bool { return c.ID == e.CandidateID })
		key := fmt.Sprintf("%s/%d", e.CandidateID, e.Repetition)
		if ci < 0 || e.Repetition < 1 || e.Repetition > r.Repetitions || entries[key] {
			return errors.New("invalid checkpoint repetition")
		}
		entries[key] = true
		expected := runtime.EvaluationCases(r.Candidates[ci].Roles)
		seen := map[string]bool{}
		for _, v := range e.Cases {
			index := slices.IndexFunc(expected, func(c runtime.EvaluationCase) bool { return c.ID == v.CaseID })
			if index < 0 || seen[v.CaseID] {
				return errors.New("unexpected or duplicate checkpoint case")
			}
			seen[v.CaseID] = true
			c := expected[index]
			if c.Role != v.Role || c.Critical != v.Critical || c.Category != v.Category || c.JudgeRubric != v.JudgeRubric || v.LatencyMS < 0 || v.Passed && v.Failure != "" {
				return errors.New("checkpoint case contract changed")
			}
		}
	}
	comparisons := map[string]bool{}
	for _, c := range r.Comparisons {
		pi := slices.IndexFunc(r.Policies.Policies, func(p rolePolicy) bool { return p.Role == c.Role })
		ci := slices.IndexFunc(r.Candidates, func(v candidate) bool { return v.ID == c.ChallengerCandidateID && slices.Contains(v.Roles, c.Role) })
		key := c.Role + "/" + c.ChallengerCandidateID
		if pi < 0 || ci < 0 || comparisons[key] {
			return errors.New("unexpected or duplicate checkpoint comparison")
		}
		comparisons[key] = true
		p := r.Policies.Policies[pi]
		if len(p.JudgeCaseIDs) == 0 || c.IncumbentCandidateID != p.IncumbentCandidateID || c.ChallengerCandidateID == c.IncumbentCandidateID || c.JudgeModel != r.Policies.Judge.Model {
			return errors.New("checkpoint comparison contract changed")
		}
		if c.Error == "" && (c.ResponseProvider != "openrouter" || c.ResponseModel != c.JudgeModel || c.ChallengerScore == nil || c.IncumbentScore == nil || *c.ChallengerScore < 0 || *c.ChallengerScore > 100 || *c.IncumbentScore < 0 || *c.IncumbentScore > 100) {
			return errors.New("invalid judge evidence")
		}
	}
	return nil
}
func complete(r matrixReport) bool {
	if validateEvidence(r) != nil {
		return false
	}
	for _, c := range r.Candidates {
		for i := 1; i <= r.Repetitions; i++ {
			index := slices.IndexFunc(r.Entries, func(e matrixEntry) bool { return e.CandidateID == c.ID && e.Repetition == i })
			if index < 0 || r.Entries[index].Error != "" || len(r.Entries[index].Cases) != len(runtime.EvaluationCases(c.Roles)) {
				return false
			}
		}
	}
	if r.Mode == "default_decision" {
		for _, p := range r.Policies.Policies {
			if len(p.JudgeCaseIDs) == 0 {
				continue
			}
			for _, c := range r.Candidates {
				if c.ID == p.IncumbentCandidateID || !slices.Contains(c.Roles, p.Role) {
					continue
				}
				if !slices.ContainsFunc(r.Comparisons, func(v roleComparison) bool {
					return v.Role == p.Role && v.ChallengerCandidateID == c.ID && v.Error == ""
				}) {
					return false
				}
			}
		}
	}
	return true
}
