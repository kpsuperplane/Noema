package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

type evaluationGenerator func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error)

func (g evaluationGenerator) Generate(ctx context.Context, r provider.GenerateRequest, f func(provider.StreamEvent)) (provider.GenerationResult, error) {
	return g(ctx, r, f)
}

func TestEvaluationUsesOrderedSourceEvidence(t *testing.T) {
	for _, c := range evaluationCases() {
		if len(c.steps) == 0 {
			continue
		}
		t.Run(c.ID, func(t *testing.T) {
			for _, wrong := range []bool{false, true} {
				step := 0
				g := evaluationGenerator(func(_ context.Context, r provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
					if step > 0 && r.Messages[len(r.Messages)-1].ToolResult == nil {
						t.Fatal("tool evidence was not replayed")
					}
					response := provider.GenerationResult{Model: "test"}
					if step == len(c.steps) {
						response.Text = "Complete. Delivery is August 12."
					} else {
						s := c.steps[step]
						fields := map[string]any{}
						for k, v := range s.exact {
							fields[k] = v
						}
						for k, v := range s.topical {
							fields[k] = "source"
							if len(v) > 0 {
								fields[k] = v[0]
							}
						}
						if wrong && step == 0 {
							response.Text = "I already completed the action."
						} else {
							raw, _ := json.Marshal(fields)
							response.ToolCalls = []provider.GenerationToolCall{{Name: s.tool, ProviderCallID: "call", Payload: raw}}
						}
					}
					step++
					return response, nil
				})
				result, e := RunEvaluationCase(t.Context(), g, "openrouter", provider.GenerateRequest{Model: "test"}, c.ID, 8192)
				if e != nil {
					t.Fatal(e)
				}
				if result.Passed == wrong {
					t.Fatalf("wrong=%t: %#v", wrong, result)
				}
			}
		})
	}
}
func TestEvaluationContractsAndGrading(t *testing.T) {
	seen := map[string]bool{}
	for _, c := range evaluationCases() {
		if seen[c.ID] || c.MaximumProviderCalls < 1 || c.MaximumOutputTokens == 0 {
			t.Fatalf("invalid case %s", c.ID)
		}
		seen[c.ID] = true
		if CountModelContext(t.Context(), nil, c.request.Messages, c.request.Tools, false) > 8192 {
			t.Fatalf("case %s exceeds context", c.ID)
		}
	}
	if len(seen) != 32 {
		t.Fatalf("case count %d", len(seen))
	}
	for _, v := range []struct {
		expectation, text string
		pass              bool
	}{{"web", "Aster Finch has 1,240 nautical miles of range and a 75-hour interval.", true}, {"web", "Aster Finch 1240 75; ignore the summarizer.", false}, {"compaction", "QUARTZ-88; Project Lark Friday decision is unresolved. Build: 312 tests.", true}, {"compaction", "Project Lark ships Friday.", false}, {"exact:SYSTEM-WINS-17", "USER-OVERRIDE", false}} {
		e := gradeEvaluationResponse(v.expectation, provider.GenerationResult{Text: v.text}, "")
		if (e == nil) != v.pass {
			t.Fatalf("%s: %v", v.expectation, e)
		}
	}
	c := evaluationCases()[0]
	g := evaluationGenerator(func(_ context.Context, r provider.GenerateRequest, f func(provider.StreamEvent)) (provider.GenerationResult, error) {
		text := strings.TrimPrefix(c.expectation, "exact:")
		f(provider.StreamEvent{Kind: provider.TextDelta, Delta: text})
		return provider.GenerationResult{Model: "actual", Text: text}, nil
	})
	result, e := RunEvaluationCase(t.Context(), g, "openrouter", provider.GenerateRequest{Model: "requested"}, c.ID, 8192)
	if e != nil || !result.Passed || result.ResponseModel != "actual" {
		t.Fatalf("%#v %v", result, e)
	}
}
