package modeleval

import (
	"fmt"
	goruntime "runtime"
	"strings"

	"github.com/kpsuperplane/noema/internal/runtime"
)

func newMatrixReport(runID string, suite suiteConfig, candidates []candidate, policies rolePolicies, mode string) matrixReport {
	report := matrixReport{
		SchemaVersion:       3,
		RunID:               runID,
		Mode:                mode,
		Status:              "running",
		RuntimeSuiteVersion: runtime.EvaluationSuiteVersion,
		DecisionFingerprint: reportFingerprint(suite, policies, candidates),
		Repetitions:         suite.repetitions(mode == "default_decision"),
		Policies:            policies,
		Candidates:          candidates,
		Environment:         evaluationEnvironment{EvaluatorVersion: "go", TargetOS: goruntime.GOOS, TargetArch: goruntime.GOARCH},
	}
	report.Rankings = rank(report)
	return report
}

func (r *matrixReport) push(entry matrixEntry) {
	r.Entries = append(r.Entries, entry)
	r.Rankings = rank(*r)
}

func (r *matrixReport) recordCase(candidateID string, repetition int, result runtime.EvaluationResult) error {
	index := -1
	for i := range r.Entries {
		if r.Entries[i].CandidateID == candidateID && r.Entries[i].Repetition == repetition {
			index = i
			break
		}
	}
	if index < 0 {
		r.Entries = append(r.Entries, matrixEntry{CandidateID: candidateID, Repetition: repetition})
		index = len(r.Entries) - 1
	}
	for _, existing := range r.Entries[index].Cases {
		if existing.CaseID == result.CaseID {
			return fmt.Errorf("case %s already checkpointed for %s repetition %d", result.CaseID, candidateID, repetition)
		}
	}
	r.Entries[index].Cases = append(r.Entries[index].Cases, result)
	r.Rankings = rank(*r)
	return nil
}

func (r matrixReport) hasCase(candidateID string, repetition int, caseID string) bool {
	for _, entry := range r.Entries {
		if entry.CandidateID != candidateID || entry.Repetition != repetition {
			continue
		}
		for _, result := range entry.Cases {
			if result.CaseID == caseID {
				return true
			}
		}
	}
	return false
}

func (r *matrixReport) pushComparison(comparison roleComparison) {
	r.Comparisons = append(r.Comparisons, comparison)
	r.Rankings = rank(*r)
}

func (r *matrixReport) finish() {
	if r.Mode == "exploration" {
		r.Status = "incomplete"
	} else if complete(*r) {
		r.Status = "complete"
	} else {
		r.Status = "incomplete"
	}
	r.Rankings = rank(*r)
}

func (r *matrixReport) fail(message string) {
	r.Status = "failed"
	r.Failure = message
	r.Rankings = rank(*r)
}

func (r *matrixReport) resume() {
	r.Status = "running"
	r.Failure = ""
	r.Rankings = rank(*r)
}

func (r matrixReport) markdown() string {
	var out strings.Builder
	fmt.Fprintf(&out, "# Noema OpenRouter model evaluation %s\n\nMode: `%s`. Status: `%s`. Repetitions: %d. Suite version: %d. Decision fingerprint: `%s`.\n", r.RunID, r.Mode, r.Status, r.Repetitions, r.RuntimeSuiteVersion, r.DecisionFingerprint)
	if r.Failure != "" {
		fmt.Fprintf(&out, "\nRun failure: %s\n", strings.NewReplacer("|", "\\|", "\n", " ", "\r", " ").Replace(r.Failure))
	}
	for _, ranking := range r.Rankings {
		recommendation := ranking.RecommendedCandidateID
		if recommendation == "" {
			if r.Mode == "default_decision" && r.Status == "complete" {
				recommendation = "none; no candidate qualified"
			} else {
				recommendation = "none until a default decision completes"
			}
		}
		fmt.Fprintf(&out, "\n## %s\n\nFinal recommendation: **%s**. %s.\n\n| Candidate | Qualified | Quality | Errors | Identity | Critical | All cases | Estimated cost | Median / p95 latency | Runs |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n", ranking.Role, recommendation, ranking.SelectionReason)
		for _, score := range ranking.Candidates {
			quality := "-"
			if score.QualityScore != nil {
				quality = fmt.Sprintf("%.3f", *score.QualityScore)
			}
			errors := "-"
			if score.ErrorRate != nil {
				errors = fmt.Sprintf("%.3f", *score.ErrorRate)
			}
			cost := "-"
			if score.EstimatedCostUSD != nil {
				cost = fmt.Sprintf("$%.6f", *score.EstimatedCostUSD)
			}
			latency := "-"
			if score.MedianLatencyMS != nil && score.P95LatencyMS != nil {
				latency = fmt.Sprintf("%d / %d ms", *score.MedianLatencyMS, *score.P95LatencyMS)
			}
			critical := fmt.Sprintf("%d/%d", score.PassedCriticalCases, score.TotalCriticalCases)
			allCases := fmt.Sprintf("%d/%d", score.PassedCases, score.TotalCases)
			fmt.Fprintf(&out, "| %s | %t | %s | %s | %s | %s | %s | %s | %s | %d/%d |\n", score.CandidateID, score.Qualified, quality, errors, yesNo(score.IdentityMatched), critical, allCases, cost, latency, score.CompletedRepetitions, r.Repetitions)
		}
		if ranking.Role == "primary" {
			writeStatefulDiagnostics(&out, r, ranking)
		}
	}
	out.WriteString("\nPrices are dated manifest snapshots and costs are estimates from OpenRouter-reported usage, not billing records. Final recommendations appear only for a completed default-decision run.\n")
	return out.String()
}

func yesNo(value bool) string {
	if value {
		return "yes"
	}
	return "no"
}

func writeStatefulDiagnostics(out *strings.Builder, report matrixReport, ranking roleRanking) {
	var caseIDs []string
	for _, c := range runtime.EvaluationCases([]string{"primary"}) {
		if c.Category == "stateful_action" {
			caseIDs = append(caseIDs, c.ID)
		}
	}
	if len(caseIDs) == 0 {
		return
	}
	out.WriteString("\n### Stateful Primary diagnostics\n\nEach cell is passed/observed repetitions. These critical scenarios expose which behavior disqualified a candidate.\n\n| Candidate |")
	for _, id := range caseIDs {
		label := strings.ReplaceAll(strings.TrimPrefix(id, "primary_stateful_"), "_", " ")
		fmt.Fprintf(out, " %s |", label)
	}
	out.WriteString("\n|---|")
	for range caseIDs {
		out.WriteString("---:|")
	}
	out.WriteByte('\n')
	for _, score := range ranking.Candidates {
		fmt.Fprintf(out, "| %s |", score.CandidateID)
		for _, id := range caseIDs {
			observed := 0
			passed := 0
			for _, entry := range report.Entries {
				if entry.CandidateID != score.CandidateID {
					continue
				}
				for _, result := range entry.Cases {
					if result.CaseID == id {
						observed++
						if result.Passed {
							passed++
						}
					}
				}
			}
			if observed == 0 {
				out.WriteString(" - |")
			} else {
				fmt.Fprintf(out, " %d/%d |", passed, observed)
			}
		}
		out.WriteByte('\n')
	}
	var failures []string
	for _, entry := range report.Entries {
		for _, result := range entry.Cases {
			if result.Category == "stateful_action" && !result.Passed {
				label := strings.TrimPrefix(result.CaseID, "primary_stateful_")
				failure := result.Failure
				if failure == "" {
					failure = "failed without a reason"
				}
				failures = append(failures, fmt.Sprintf("%s · %s · repetition %d: %s", entry.CandidateID, label, entry.Repetition, failure))
			}
		}
	}
	if len(failures) > 0 {
		out.WriteString("\nObserved stateful failures:\n")
		for _, failure := range failures {
			fmt.Fprintf(out, "- %s\n", strings.ReplaceAll(failure, "|", "\\|"))
		}
	}
}
