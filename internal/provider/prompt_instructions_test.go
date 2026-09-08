package provider

import (
	"encoding/json"
	"testing"
)

// Rust baseline 4d29f6ba: adapters/responses/request.rs serializes instructions
// separately from input, including on requests with previous_response_id.
func TestResponsesPreservesExactSystemInstructions(t *testing.T) {
	const instructions = "  Noema instructions.\n\nKeep `literal` text.\n"
	for _, previous := range []string{"", "resp_previous"} {
		t.Run(previous, func(t *testing.T) {
			request := GenerateRequest{AccountID: codexGenerationAccountID, Model: "gpt-5.6", PreviousResponseID: previous, StoreResponse: true,
				Messages: []GenerationMessage{{Role: "system", Instructions: true, Content: instructions}, {Role: "system", Content: "Runtime system context"}, {Role: "developer", Content: "Current environment"}, {Role: "user", Content: "Hello"}}}
			body, _, err := prepareResponsesGeneration(request, responsesGenerationProfile{accountID: codexGenerationAccountID, providerName: "Codex"})
			if err != nil {
				t.Fatal(err)
			}
			var wire struct {
				Instructions string `json:"instructions"`
				Input        []struct {
					Role    string `json:"role"`
					Content string `json:"content"`
				} `json:"input"`
			}
			if err := json.Unmarshal(body, &wire); err != nil {
				t.Fatal(err)
			}
			if wire.Instructions != instructions {
				t.Fatalf("instructions changed: %q", wire.Instructions)
			}
			if len(wire.Input) != 3 || wire.Input[0].Role != "system" || wire.Input[0].Content != "Runtime system context" || wire.Input[1].Role != "developer" || wire.Input[1].Content != "Current environment" || wire.Input[2].Role != "user" {
				t.Fatalf("instructions leaked into input or context changed: %+v", wire.Input)
			}
		})
	}
}
