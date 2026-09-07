package modeleval

import (
	"errors"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/token"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"
)

func proposal(root, dir string, verify bool) error {
	var p decisionPlan
	var r matrixReport
	if e := readJSON(filepath.Join(dir, "plan.json"), &p); e != nil {
		return e
	}
	if e := p.validate(); e != nil {
		return e
	}
	if e := readJSON(filepath.Join(dir, "report.json"), &r); e != nil {
		return e
	}
	if r.SchemaVersion != 3 || r.RunID != p.DecisionID || r.Mode != "default_decision" || r.Status != "complete" || r.DecisionFingerprint != reportFingerprint(p.Suite, p.Policies, p.Candidates) || fingerprint(r.Candidates) != fingerprint(p.Candidates) || fingerprint(r.Policies) != fingerprint(p.Policies) || r.Repetitions != p.Suite.repetitions(true) || !complete(r) {
		return errors.New("proposal requires complete evidence from the exact decision plan")
	}
	rankings := rank(r)
	var body strings.Builder
	body.WriteString("func ModelRecommendations(providerKind string) []ModelRecommendation {\nswitch providerKind {\n")
	var summary strings.Builder
	fmt.Fprintf(&summary, "# Model recommendation proposal\n\nDecision: %s.\n\nApply the patch after evidence review.\n\n", r.RunID)
	for _, kind := range []string{"codex", "openai", "openrouter"} {
		fmt.Fprintf(&body, "case %q: return []ModelRecommendation{\n", kind)
		current := provider.ModelRecommendations(kind)
		for _, ranking := range rankings {
			currentIndex := slices.IndexFunc(current, func(value provider.ModelRecommendation) bool { return string(value.UseCase) == ranking.Role })
			if currentIndex < 0 {
				return fmt.Errorf("shipped recommendation is unavailable for %s/%s", kind, ranking.Role)
			}
			winner, target, reason := selectForProvider(r, ranking, kind, current[currentIndex])
			if winner == "" || target == nil {
				return fmt.Errorf("no qualified mapped candidate for %s/%s", kind, ranking.Role)
			}
			if verify && !slices.ContainsFunc(current, func(v provider.ModelRecommendation) bool {
				return string(v.UseCase) == ranking.Role && v.ModelProfile == target.ModelProfile && v.ReasoningEffort == target.ReasoningEffort
			}) {
				return fmt.Errorf("shipped recommendation differs for %s/%s", kind, ranking.Role)
			}
			body.WriteString(recommendationCellLine(ranking.Role, target.ModelProfile, target.ReasoningEffort))
			fmt.Fprintf(&summary, "- %s / %s: %s (%s). %s\n", kind, ranking.Role, winner, target.ReasoningEffort, reason)
		}
		body.WriteString("}\n")
	}
	body.WriteString("}\nreturn nil\n}\n")
	if verify {
		fmt.Println("Shipped recommendations match complete qualified evidence.")
		return nil
	}
	path := filepath.Join(root, "internal/provider/recommendations.go")
	source, e := os.ReadFile(path)
	if e != nil {
		return e
	}
	fs := token.NewFileSet()
	file, e := parser.ParseFile(fs, path, source, 0)
	if e != nil {
		return e
	}
	found := false
	for _, decl := range file.Decls {
		fn, ok := decl.(*ast.FuncDecl)
		if ok && fn.Name.Name == "ModelRecommendations" {
			start, end := fs.Position(fn.Pos()).Offset, fs.Position(fn.End()).Offset
			source = append(append(append([]byte{}, source[:start]...), body.String()...), source[end:]...)
			found = true
			break
		}
	}
	if !found {
		return errors.New("recommendation function is unavailable")
	}
	source, e = format.Source(source)
	if e != nil {
		return e
	}
	replacement := filepath.Join(dir, "recommendations.go")
	if e = os.WriteFile(replacement, source, 0600); e != nil {
		return e
	}
	command := exec.Command("git", "diff", "--no-index", "--", path, replacement)
	diff, e := command.Output()
	var exit *exec.ExitError
	if e != nil && (!errors.As(e, &exit) || exit.ExitCode() != 1) {
		return e
	}
	lines := strings.Split(string(diff), "\n")
	for i, line := range lines {
		if strings.HasPrefix(line, "diff --git ") {
			lines[i] = "diff --git a/internal/provider/recommendations.go b/internal/provider/recommendations.go"
		} else if strings.HasPrefix(line, "--- ") {
			lines[i] = "--- a/internal/provider/recommendations.go"
		} else if strings.HasPrefix(line, "+++ ") {
			lines[i] = "+++ b/internal/provider/recommendations.go"
		}
	}
	if e = os.WriteFile(filepath.Join(dir, "recommendations.patch"), []byte(strings.Join(lines, "\n")), 0600); e != nil {
		return e
	}
	return os.WriteFile(filepath.Join(dir, "proposal.md"), []byte(summary.String()), 0600)
}

func recommendationCellLine(role, model, effort string) string {
	return fmt.Sprintf("{UseCase: ModelUseCase(%q), ModelProfile: %q, ReasoningEffort: %q},\n", role, model, effort)
}

func selectForProvider(report matrixReport, ranking roleRanking, providerKind string, current provider.ModelRecommendation) (string, *recommendationTarget, string) {
	policyIndex := slices.IndexFunc(report.Policies.Policies, func(policy rolePolicy) bool { return policy.Role == ranking.Role })
	if policyIndex < 0 {
		return "", nil, "missing role policy"
	}
	policy := report.Policies.Policies[policyIndex]
	var eligible []candidateScore
	targets := map[string]recommendationTarget{}
	oldID := ""
	for _, score := range ranking.Candidates {
		candidateIndex := slices.IndexFunc(report.Candidates, func(candidate candidate) bool { return candidate.ID == score.CandidateID })
		if candidateIndex < 0 {
			continue
		}
		candidate := report.Candidates[candidateIndex]
		targetIndex := slices.IndexFunc(candidate.Targets, func(target recommendationTarget) bool { return target.Provider == providerKind })
		if targetIndex < 0 {
			continue
		}
		target := candidate.Targets[targetIndex]
		targets[candidate.ID] = target
		eligible = append(eligible, score)
		if string(current.UseCase) == ranking.Role && current.ModelProfile == target.ModelProfile && current.ReasoningEffort == target.ReasoningEffort {
			oldID = candidate.ID
		}
	}
	winner := selectWinner(eligible, policy, oldID)
	if winner == "" {
		return "", nil, "no qualified candidate has an explicit mapping"
	}
	target, ok := targets[winner]
	if !ok {
		return "", nil, "no qualified candidate has an explicit mapping"
	}
	reason := "highest-ranked qualified mapped candidate cleared policy"
	if oldID != "" && winner == oldID && len(eligible) > 0 && eligible[0].CandidateID != oldID {
		oldIndex := slices.IndexFunc(eligible, func(score candidateScore) bool { return score.CandidateID == oldID })
		if oldIndex >= 0 {
			improvement := value(eligible[0].QualityScore, 0) - value(eligible[oldIndex].QualityScore, 0)
			if improvement < policy.ReplacementQualityMargin {
				reason = fmt.Sprintf("retained mapped incumbent; improvement %.3f was below margin %.3f", improvement, policy.ReplacementQualityMargin)
			}
		}
	}
	if target.ModelProfile == current.ModelProfile && target.ReasoningEffort == current.ReasoningEffort {
		if reason == "highest-ranked qualified mapped candidate cleared policy" {
			reason = "shipped recommendation remains selected"
		}
	}
	return winner, &target, reason
}
