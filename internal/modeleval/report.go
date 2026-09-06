package modeleval

import (
	"cmp"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/kpsuperplane/noema/internal/runtime"
)

func rank(r matrixReport) []roleRanking {
	var rankings []roleRanking
	for _, p := range r.Policies.Policies {
		ranking := roleRanking{Role: p.Role, SelectionReason: "Evidence is incomplete."}
		for _, c := range r.Candidates {
			if !slices.Contains(c.Roles, p.Role) {
				continue
			}
			score := candidateScore{CandidateID: c.ID, IdentityMatched: true}
			cost := 0.0
			costKnown := c.Pricing != nil
			var latencies []int64
			errors := 0
			unique := map[string]bool{}
			for _, entry := range r.Entries {
				if entry.CandidateID != c.ID {
					continue
				}
				if entry.Error == "" && len(entry.Cases) == len(runtime.EvaluationCases(c.Roles)) {
					score.CompletedRepetitions++
				}
				for _, v := range entry.Cases {
					if v.Role != p.Role && v.Category != runtime.EvaluationProtocolCategory {
						continue
					}
					score.TotalCases++
					unique[v.CaseID] = true
					latencies = append(latencies, v.LatencyMS)
					if v.Passed {
						score.PassedCases++
					}
					if v.Critical {
						score.TotalCriticalCases++
						if v.Passed {
							score.PassedCriticalCases++
						}
					}
					if v.Failure != "" {
						errors++
					}
					if v.ResponseProvider != "openrouter" || v.ResponseModel != c.Model && !slices.Contains(c.AcceptedResponseModels, v.ResponseModel) {
						score.IdentityMatched = false
					}
					if !slices.Contains(score.ObservedModels, v.ResponseModel) {
						score.ObservedModels = append(score.ObservedModels, v.ResponseModel)
					}
					if c.Pricing == nil || v.InputTokens == nil || v.OutputTokens == nil || *v.InputTokens < 0 || *v.OutputTokens < 0 {
						costKnown = false
					} else {
						cached := 0
						if v.CachedInputTokens != nil {
							cached = max(0, min(*v.CachedInputTokens, *v.InputTokens))
						}
						cachedPrice := c.Pricing.InputUSDPerMillion
						if c.Pricing.CachedInputUSDPerMillion != nil {
							cachedPrice = *c.Pricing.CachedInputUSDPerMillion
						}
						cost += (float64(*v.InputTokens-cached)*c.Pricing.InputUSDPerMillion + float64(cached)*cachedPrice + float64(*v.OutputTokens)*c.Pricing.OutputUSDPerMillion) / 1e6
					}
				}
			}
			if costKnown {
				score.EstimatedCostUSD = &cost
			}
			if score.TotalCases > 0 {
				deterministic := float64(score.PassedCases) / float64(score.TotalCases)
				errorRate := float64(errors) / float64(score.TotalCases)
				score.DeterministicScore = &deterministic
				score.ErrorRate = &errorRate
				slices.Sort(latencies)
				median := latencies[len(latencies)/2]
				p95 := latencies[int(math.Ceil(float64(len(latencies))*.95))-1]
				score.MedianLatencyMS = &median
				score.P95LatencyMS = &p95
				judgeTotal, judgeCount := 0.0, 0
				for _, v := range r.Comparisons {
					if v.Role != p.Role || v.Error != "" {
						continue
					}
					if v.ChallengerCandidateID == c.ID && v.ChallengerScore != nil {
						judgeTotal += float64(*v.ChallengerScore) / 100
						judgeCount++
					}
					if v.IncumbentCandidateID == c.ID && v.IncumbentScore != nil {
						judgeTotal += float64(*v.IncumbentScore) / 100
						judgeCount++
					}
				}
				if judgeCount > 0 {
					value := judgeTotal / float64(judgeCount)
					score.JudgeScore = &value
				}
				if p.JudgeWeight == 0 || score.JudgeScore != nil {
					quality := p.DeterministicWeight * deterministic
					if score.JudgeScore != nil {
						quality += p.JudgeWeight * *score.JudgeScore
					}
					score.QualityScore = &quality
				}
				score.Qualified = score.CompletedRepetitions == r.Repetitions && len(unique) >= p.MinimumCases && score.IdentityMatched && score.PassedCriticalCases == score.TotalCriticalCases && score.QualityScore != nil && *score.QualityScore >= p.MinimumQualityScore && errorRate <= p.MaximumErrorRate && p95 <= p.MaximumP95LatencyMS
			}
			ranking.Candidates = append(ranking.Candidates, score)
		}
		slices.SortFunc(ranking.Candidates, func(a, b candidateScore) int {
			if a.Qualified != b.Qualified {
				if a.Qualified {
					return -1
				}
				return 1
			}
			for _, v := range []int{cmp.Compare(value(b.DeterministicScore, -1), value(a.DeterministicScore, -1)), cmp.Compare(value(b.QualityScore, -1), value(a.QualityScore, -1)), cmp.Compare(value(a.ErrorRate, 2), value(b.ErrorRate, 2)), cmp.Compare(value(a.EstimatedCostUSD, math.Inf(1)), value(b.EstimatedCostUSD, math.Inf(1))), cmp.Compare(value(a.P95LatencyMS, math.MaxInt64), value(b.P95LatencyMS, math.MaxInt64))} {
				if v != 0 {
					return v
				}
			}
			return strings.Compare(a.CandidateID, b.CandidateID)
		})
		if r.Mode == "default_decision" && r.Status == "complete" && complete(r) {
			ranking.RecommendedCandidateID = selectWinner(ranking.Candidates, p, p.IncumbentCandidateID)
			ranking.SelectionReason = "No candidate meets the role policy."
			if ranking.RecommendedCandidateID != "" {
				ranking.SelectionReason = "Qualified evidence and the incumbent replacement margin select this candidate."
			}
		}
		rankings = append(rankings, ranking)
	}
	return rankings
}
func value[T ~float64 | ~int64](p *T, fallback T) T {
	if p == nil {
		return fallback
	}
	return *p
}
func selectWinner(cs []candidateScore, p rolePolicy, incumbent string) string {
	index := slices.IndexFunc(cs, func(c candidateScore) bool { return c.Qualified })
	if index < 0 {
		return ""
	}
	best := cs[index]
	old := slices.IndexFunc(cs, func(c candidateScore) bool { return c.CandidateID == incumbent && c.Qualified })
	if old >= 0 && best.CandidateID != incumbent && value(best.QualityScore, 0)-value(cs[old].QualityScore, 0) < p.ReplacementQualityMargin {
		return incumbent
	}
	return best.CandidateID
}
func saveReport(dir string, r matrixReport) error {
	if e := writeJSON(filepath.Join(dir, "report.json"), r, false); e != nil {
		return e
	}
	var out strings.Builder
	fmt.Fprintf(&out, "# Model evaluation %s\n\nMode: %s. Status: %s.\n\n", r.RunID, r.Mode, r.Status)
	if r.Failure != "" {
		fmt.Fprintf(&out, "Failure: %s\n\n", r.Failure)
	}
	for _, ranking := range r.Rankings {
		fmt.Fprintf(&out, "## %s\n\n%s\n\nRecommended candidate: %s\n\n| Candidate | Qualified | Cases passed | Quality | P95 ms | Cost USD |\n| --- | --- | --- | --- | --- | --- |\n", ranking.Role, ranking.SelectionReason, ranking.RecommendedCandidateID)
		for _, s := range ranking.Candidates {
			fmt.Fprintf(&out, "| %s | %t | %d/%d | %.3f | %d | %.6f |\n", s.CandidateID, s.Qualified, s.PassedCases, s.TotalCases, value(s.QualityScore, -1), value(s.P95LatencyMS, -1), value(s.EstimatedCostUSD, -1))
		}
		out.WriteString("\n")
	}
	out.WriteString("## Case evidence\n\n| Candidate | Repetition | Case | Passed | Failure |\n| --- | --- | --- | --- | --- |\n")
	for _, entry := range r.Entries {
		for _, c := range entry.Cases {
			failure := strings.NewReplacer("|", "\\|", "\n", " ", "\r", " ").Replace(c.Failure)
			fmt.Fprintf(&out, "| %s | %d | %s | %t | %s |\n", entry.CandidateID, entry.Repetition, c.CaseID, c.Passed, failure)
		}
	}
	return os.WriteFile(filepath.Join(dir, "report.md"), []byte(out.String()), 0600)
}
