package modeleval

import (
	"errors"
	"fmt"
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
	var changes []recommendationChange
	var summary strings.Builder
	fmt.Fprintf(&summary, "# Model recommendation proposal\n\nDecision: %s.\n\nApply the patch after evidence review.\n\n", r.RunID)
	for _, kind := range []string{"codex", "openai", "openrouter"} {
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
			replacement := provider.ModelRecommendation{UseCase: provider.ModelUseCase(ranking.Role), ModelProfile: target.ModelProfile, ReasoningEffort: target.ReasoningEffort}
			if current[currentIndex] != replacement {
				changes = append(changes, recommendationChange{Provider: kind, Role: ranking.Role, Old: current[currentIndex], New: replacement})
			}
			fmt.Fprintf(&summary, "- %s / %s: %s (%s). %s\n", kind, ranking.Role, winner, target.ReasoningEffort, reason)
		}
	}
	if verify {
		fmt.Println("Shipped recommendations match complete qualified evidence.")
		return nil
	}
	path := filepath.Join(root, "internal/provider/recommendations.go")
	source, e := os.ReadFile(path)
	if e != nil {
		return e
	}
	diff, e := renderRecommendationPatch(path, source, changes)
	if e != nil {
		return e
	}
	if e = os.WriteFile(filepath.Join(dir, "recommendations.patch"), []byte(diff), 0600); e != nil {
		return e
	}
	return os.WriteFile(filepath.Join(dir, "proposal.md"), []byte(summary.String()), 0600)
}

type recommendationChange struct {
	Provider string
	Role     string
	Old      provider.ModelRecommendation
	New      provider.ModelRecommendation
}

func recommendationCellLine(role, model, effort string) string {
	return fmt.Sprintf("\t\t\t{UseCase: ModelUseCase(%q), ModelProfile: %q, ReasoningEffort: %q},\n", role, model, effort)
}

func renderRecommendationPatch(path string, source []byte, changes []recommendationChange) (string, error) {
	updated := string(source)
	for _, change := range changes {
		var err error
		updated, err = replaceRecommendationCell(updated, change)
		if err != nil {
			return "", err
		}
	}
	replacement, err := os.CreateTemp(filepath.Dir(path), ".recommendations-*.go")
	if err != nil {
		return "", err
	}
	replacementPath := replacement.Name()
	defer os.Remove(replacementPath)
	if _, err := replacement.WriteString(updated); err != nil {
		_ = replacement.Close()
		return "", err
	}
	if err := replacement.Close(); err != nil {
		return "", err
	}
	command := exec.Command("git", "diff", "--no-index", "--", path, replacementPath)
	diff, err := command.Output()
	var exit *exec.ExitError
	if err != nil && (!errors.As(err, &exit) || exit.ExitCode() != 1) {
		return "", err
	}
	lines := strings.Split(string(diff), "\n")
	for i, line := range lines {
		if strings.HasPrefix(line, "diff --git ") {
			lines[i] = "diff --git a/internal/provider/recommendations.go b/internal/provider/recommendations.go"
		} else if strings.HasPrefix(line, "--- ") {
			lines[i] = "--- a/internal/provider/recommendations.go"
		} else if strings.HasPrefix(line, "+++ ") {
			lines[i] = "+++ b/internal/provider/recommendations.go"
		} else if strings.HasPrefix(line, "@@ ") {
			if end := strings.LastIndex(line, " @@"); end >= 0 {
				lines[i] = line[:end+3]
			}
		}
	}
	return strings.Join(lines, "\n"), nil
}

func replaceRecommendationCell(source string, change recommendationChange) (string, error) {
	marker := fmt.Sprintf("case %q:", change.Provider)
	start := strings.Index(source, marker)
	if start < 0 {
		return "", fmt.Errorf("recommendation source has no provider %q", change.Provider)
	}
	segmentStart := start + len(marker)
	segment := source[segmentStart:]
	if next := strings.Index(segment, "\n\tcase "); next >= 0 {
		segment = segment[:next]
	} else if next := strings.Index(segment, "\ncase "); next >= 0 {
		segment = segment[:next]
	}
	oldLine := recommendationCellLine(change.Role, change.Old.ModelProfile, change.Old.ReasoningEffort)
	newLine := recommendationCellLine(change.Role, change.New.ModelProfile, change.New.ReasoningEffort)
	offset := strings.Index(segment, oldLine)
	if offset < 0 {
		return "", fmt.Errorf("recommendation source has no %s/%s cell", change.Provider, change.Role)
	}
	absolute := segmentStart + offset
	return source[:absolute] + newLine + source[absolute+len(oldLine):], nil
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
