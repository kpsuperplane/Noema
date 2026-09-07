package adapter

import "testing"

// Rust source: crates/noema-capabilities/src/binding.rs::url_policy_composes_url_and_exact_standard_credential_cleanup.
func TestRustCapabilities_url_policy_composes_url_and_exact_standard_credential_cleanup(t *testing.T) {
	payload := sanitizeSensitiveOutputWithQueryNames(map[string]any{
		"arguments": map[string]any{
			"url":           "https://user:secret@example.com/path?view=full&access_token=private#section",
			"authorization": "Bearer private",
			"content":       "ordinary-content",
		},
		"output": map[string]any{"access_token": "private", "snapshot": "ordinary-snapshot"},
	}, nil, nil, nil).(map[string]any)
	arguments := payload["arguments"].(map[string]any)
	if arguments["url"] != "https://example.com/path?view=full#section" ||
		arguments["authorization"] != "[REDACTED]" || arguments["content"] != "ordinary-content" {
		t.Fatalf("sanitized URL arguments = %#v", arguments)
	}
	output := payload["output"].(map[string]any)
	if output["access_token"] != "[REDACTED]" || output["snapshot"] != "ordinary-snapshot" {
		t.Fatalf("sanitized URL output = %#v", output)
	}
}

// Rust source: crates/noema-capabilities/src/binding.rs::artifact_policy_preserves_content_and_redacts_secrets_on_both_paths.
func TestRustCapabilities_artifact_policy_preserves_content_and_redacts_secrets_on_both_paths(t *testing.T) {
	payload := sanitizeOutput(map[string]any{
		"arguments": map[string]any{"content": "ordinary-content", "api_key": "private", "nested": map[string]any{"value": "ordinary"}},
		"result":    map[string]any{"content": "ordinary-result", "access_token": "private", "nested": map[string]any{"value": "ordinary-result-nested"}},
	}).(map[string]any)
	arguments := payload["arguments"].(map[string]any)
	if arguments["content"] != "ordinary-content" || arguments["api_key"] != "[REDACTED]" || arguments["nested"].(map[string]any)["value"] != "ordinary" {
		t.Fatalf("artifact arguments = %#v", arguments)
	}
	result := payload["result"].(map[string]any)
	if result["content"] != "ordinary-result" || result["access_token"] != "[REDACTED]" || result["nested"].(map[string]any)["value"] != "ordinary-result-nested" {
		t.Fatalf("artifact result = %#v", result)
	}
}
