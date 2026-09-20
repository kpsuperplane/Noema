package provider

import "strings"

// Codex support is bounded to the model verified against its live endpoint.
// OpenAI documents tool search for GPT-5.4 and later families listed here.
func SupportsDeferredTools(kind, model string) bool {
	if kind == "codex" {
		return model == "gpt-5.6-terra"
	}
	if kind != "openai" {
		return false
	}
	for _, family := range []string{"gpt-5.4", "gpt-5.5", "gpt-5.6", "gpt-6"} {
		if model == family || strings.HasPrefix(model, family+"-") {
			return true
		}
	}
	return false
}
