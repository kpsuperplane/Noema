package runtime

import (
	"encoding/json"
	"strings"
	"testing"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
)

func TestRustRuntime_tool_result_transcript_keeps_output_without_repeating_arguments(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_continuation.rs::tool_result_transcript_keeps_output_without_repeating_arguments.
	result := provider.ReplayToolResult{Name: "adapter.propose_definition", Arguments: json.RawMessage(`{"manifest_json":"large input"}`), Success: true, Payload: json.RawMessage(`{"status":"review_required"}`)}
	messages := taskResultMessages(provider.GenerationResult{Text: "Task result", ToolCalls: []provider.GenerationToolCall{{Name: result.Name, Payload: result.Arguments}}})
	if len(messages) != 1 || messages[0].Content != "Task result" || strings.Contains(messages[0].Content, "large input") {
		t.Fatalf("task result transcript = %#v", messages)
	}
	if result.Success != true || !strings.Contains(string(result.Payload), "review_required") {
		t.Fatalf("tool result payload = %#v", result)
	}
}

func TestRustRuntime_hosted_web_search_items_preserve_ordered_sources(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_transcript.rs::hosted_web_search_items_preserve_ordered_sources.
	search := provider.HostedSearch{Index: 3, ID: "provider-search:1", Name: "web.search", Arguments: json.RawMessage(`{"query":"lowest fare weeks"}`), Result: json.RawMessage(`{"query":"lowest fare weeks"}`), Status: "completed", Sources: []provider.WebSource{{Title: "First", URL: "https://one.example"}, {Title: "Second", URL: "https://two.example"}}}
	messages := taskResultMessages(provider.GenerationResult{Searches: []provider.HostedSearch{search}, Text: "answer"})
	if len(messages) != 2 || messages[0].Role != "hosted_web_search" || messages[0].HostedSearch == nil {
		t.Fatalf("hosted search transcript = %#v", messages)
	}
	if len(messages[0].HostedSearch.Sources) != 2 || messages[0].HostedSearch.Sources[0].Title != "First" || messages[0].HostedSearch.Sources[1].URL != "https://two.example" {
		t.Fatalf("hosted search sources = %#v", messages[0].HostedSearch.Sources)
	}
}

func TestRustRuntime_task_transcript_redacts_secret_fields_recursively(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_transcript.rs::task_transcript_redacts_secret_fields_recursively.
	payload := persistedCapabilityArguments(map[string]noemamcp.Binding{
		"web.search": {Name: "web.search", PersistencePolicy: noemamcp.BindingPersistenceRedacted},
	}, "web.search", json.RawMessage(`{"query":"safe","headers":{"Authorization":"Bearer private"},"nested":[{"api_key":"private"}]}`))
	var value map[string]any
	if err := json.Unmarshal(payload, &value); err != nil {
		t.Fatal(err)
	}
	if value["query"] != "safe" || value["headers"].(map[string]any)["Authorization"] != "[REDACTED]" || value["nested"].([]any)[0].(map[string]any)["api_key"] != "[REDACTED]" {
		t.Fatalf("task transcript redaction = %s", payload)
	}
	if strings.Contains(string(payload), "Bearer private") || strings.Contains(string(payload), `"api_key":"private"`) {
		t.Fatalf("task transcript exposed secret fields: %s", payload)
	}
}

func TestRustRuntime_task_transcript_uses_binding_policy_after_mcp_rename(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_transcript.rs::task_transcript_uses_binding_policy_after_mcp_rename.
	payload := persistedCapabilityArguments(map[string]noemamcp.Binding{
		"workspace.lookup": {Name: "workspace.lookup", PersistencePolicy: noemamcp.BindingPersistenceOmitted},
	}, "workspace.lookup", json.RawMessage(`{"query":"private workspace query"}`))
	if string(payload) != `{"redacted":true,"reason":"capability_persistence_policy"}` {
		t.Fatalf("renamed MCP persistence policy = %s", payload)
	}
	if strings.Contains(string(payload), "private workspace query") {
		t.Fatal("renamed MCP capability payload bypassed persistence policy")
	}
}

func TestRustRuntime_task_transcript_preserves_artifact_file_contents(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_transcript.rs::task_transcript_preserves_artifact_file_contents.
	payload := persistedCapabilityArguments(map[string]noemamcp.Binding{
		"artifact.create_local_file": {Name: "artifact.create_local_file", PersistencePolicy: noemamcp.BindingPersistenceRedacted},
	}, "artifact.create_local_file", json.RawMessage(`{"arguments":{"filename":"private.md","title":"Safe title","api_key":"private secret","versions":[{"title":"Draft","content":"private artifact body"}]}}`))
	if !strings.Contains(string(payload), "private artifact body") || !strings.Contains(string(payload), `"filename":"private.md"`) {
		t.Fatalf("artifact contents were concealed: %s", payload)
	}
	if strings.Contains(string(payload), "private secret") {
		t.Fatal("artifact transcript exposed secret field")
	}
}

func TestRustRuntime_unknown_tool_arguments_are_omitted_without_inspection(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/task_transcript.rs::unknown_tool_arguments_are_omitted_without_inspection.
	payload := persistedCapabilityArguments(nil, "forged.tool", json.RawMessage(`{"private":"must not persist"}`))
	if string(payload) != `{"redacted":true,"reason":"capability_persistence_policy"}` {
		t.Fatalf("unknown tool persistence = %s", payload)
	}
	if strings.Contains(string(payload), "must not persist") {
		t.Fatal("unknown tool arguments were persisted")
	}
}
