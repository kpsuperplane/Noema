package adapter

import (
	"encoding/json"
	"strings"
	"testing"
)

// Rust source: crates/noema-capabilities/src/binding.rs::url_policy_composes_url_and_exact_standard_credential_cleanup.
func TestRustCapabilities_url_policy_composes_url_and_exact_standard_credential_cleanup(t *testing.T) {
	sanitize := func(value any) any {
		return sanitizeSensitiveOutputWithQueryNames(value, nil, nil, nil)
	}
	arguments := sanitize(map[string]any{
		"url": "https://user:secret@example.com/path?view=full#section",
		"headers": map[string]any{
			"Authorization": "Bearer private",
		},
		"content": "authorized private source",
	}).(map[string]any)
	if arguments["url"] != "https://example.com/path?view=full#section" {
		t.Fatalf("sanitized URL = %#v", arguments["url"])
	}
	headers, ok := arguments["headers"].(map[string]any)
	if !ok || headers["Authorization"] != "[REDACTED]" {
		t.Fatalf("sanitized URL headers = %#v", arguments["headers"])
	}
	if arguments["content"] != "authorized private source" {
		t.Fatalf("ordinary content changed = %#v", arguments["content"])
	}
	if strings.Contains(mustRustJSON(arguments), "user:secret") {
		t.Fatal("URL credentials entered the persisted argument view")
	}

	output := sanitize(map[string]any{
		"access_token": "private",
		"snapshot":     map[string]any{"text": "authorized private page"},
	}).(map[string]any)
	if output["access_token"] != "[REDACTED]" || output["snapshot"].(map[string]any)["text"] != "authorized private page" {
		t.Fatalf("sanitized URL output = %#v", output)
	}
}

// Rust source: crates/noema-capabilities/src/binding.rs::artifact_policy_preserves_content_and_redacts_secrets_on_both_paths.
func TestRustCapabilities_artifact_policy_preserves_content_and_redacts_secrets_on_both_paths(t *testing.T) {
	sanitize := func(value any) any {
		return sanitizeSensitiveOutputWithQueryNames(value, nil, nil, nil)
	}
	arguments := sanitize(map[string]any{
		"filename": "draft.md",
		"api_key":  "private-argument",
		"versions": []any{map[string]any{"title": "Draft", "content": "argument body"}},
	}).(map[string]any)
	if arguments["filename"] != "draft.md" || arguments["versions"].([]any)[0].(map[string]any)["title"] != "Draft" ||
		arguments["versions"].([]any)[0].(map[string]any)["content"] != "argument body" || arguments["api_key"] != "[REDACTED]" {
		t.Fatalf("artifact arguments = %#v", arguments)
	}

	output := sanitize(map[string]any{
		"artifact_id":  "artifact:1",
		"access_token": "private-output",
		"versions":     []any{map[string]any{"title": "Saved", "content": "output body"}},
	}).(map[string]any)
	if output["artifact_id"] != "artifact:1" || output["versions"].([]any)[0].(map[string]any)["title"] != "Saved" ||
		output["versions"].([]any)[0].(map[string]any)["content"] != "output body" || output["access_token"] != "[REDACTED]" {
		t.Fatalf("artifact output = %#v", output)
	}

	nestedArguments := sanitize(map[string]any{
		"arguments": map[string]any{
			"filename": "nested.md",
			"password": "private-nested-argument",
			"versions": []any{map[string]any{"title": "Nested draft", "content": "nested argument body"}},
		},
	}).(map[string]any)
	nestedArgument := nestedArguments["arguments"].(map[string]any)
	if nestedArgument["filename"] != "nested.md" || nestedArgument["versions"].([]any)[0].(map[string]any)["content"] != "nested argument body" || nestedArgument["password"] != "[REDACTED]" {
		t.Fatalf("nested artifact arguments = %#v", nestedArgument)
	}

	nestedOutput := sanitize(map[string]any{
		"result": map[string]any{
			"artifact_id": "artifact:2",
			"cookie":      "private-nested-output",
			"versions":    []any{map[string]any{"title": "Nested saved", "content": "nested output body"}},
		},
	}).(map[string]any)
	nestedResult := nestedOutput["result"].(map[string]any)
	if nestedResult["artifact_id"] != "artifact:2" || nestedResult["versions"].([]any)[0].(map[string]any)["content"] != "nested output body" || nestedResult["cookie"] != "[REDACTED]" {
		t.Fatalf("nested artifact output = %#v", nestedResult)
	}
}

func mustRustJSON(value any) string {
	raw, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(raw)
}
