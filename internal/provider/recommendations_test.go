package provider

import "testing"

func TestModelRecommendationsMatchShippedMatrix(t *testing.T) {
	t.Parallel()

	for _, kind := range []string{"codex", "openai", "openrouter", "foundation_local"} {
		recommendations := ModelRecommendations(kind)
		if len(recommendations) != 9 {
			t.Fatalf("%s recommendation count = %d", kind, len(recommendations))
		}
		if recommendations[0].UseCase != ModelUsePrimary || recommendations[7].UseCase != ModelUseActionReviewer {
			t.Fatalf("%s recommendation order changed: %#v", kind, recommendations)
		}
	}

	openRouter := ModelRecommendations("openrouter")
	if openRouter[0].ModelProfile != "openai/gpt-5.6-luna" || openRouter[0].ReasoningEffort != "high" {
		t.Fatalf("unexpected OpenRouter primary recommendation: %#v", openRouter[0])
	}
	if openRouter[3].ModelProfile != "openai/gpt-5.6-sol" || openRouter[3].ReasoningEffort != "medium" {
		t.Fatalf("unexpected OpenRouter difficult recommendation: %#v", openRouter[3])
	}
	foundation := ModelRecommendations("foundation_local")
	if foundation[0].ModelProfile != "default" || foundation[0].ReasoningEffort != "" ||
		foundation[3].ModelProfile != "default" {
		t.Fatalf("unexpected Foundation Models recommendations: %#v", foundation)
	}
	if got := ModelRecommendations("local_models"); got != nil {
		t.Fatalf("local recommendations = %#v", got)
	}
}
