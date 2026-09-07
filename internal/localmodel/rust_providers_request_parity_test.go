package localmodel

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func localParityTool(name string) provider.GenerationTool {
	return provider.GenerationTool{
		Name: name, Description: "Search governed Noema memory.",
		InputSchema: []byte(`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}`),
	}
}

func localRequestBody(t *testing.T, request provider.GenerateRequest) map[string]any {
	t.Helper()
	body, _, err := localGenerationBody(request.Model, request)
	if err != nil {
		t.Fatal(err)
	}
	var value map[string]any
	if err := json.Unmarshal(body, &value); err != nil {
		t.Fatal(err)
	}
	return value
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::chat_request_preserves_replay_items_and_generation_controls (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChatRequestPreservesReplayItemsAndGenerationControls(t *testing.T) {
	maxTokens := uint32(321)
	body := localRequestBody(t, provider.GenerateRequest{
		Model: "local-model", Messages: []provider.GenerationMessage{
			{Role: "system", Content: "system rules"},
			{Role: "user", Content: "question"},
			{Role: "assistant", Content: "answer"},
			{Role: "assistant", ToolCalls: []provider.ReplayToolCall{{ProviderCallID: "call-1", Name: "read_file", Arguments: json.RawMessage(`{"path":"notes.txt"}`)}}},
			{Role: "tool", ToolResult: &provider.ReplayToolResult{ProviderCallID: "call-1", Name: "read_file", Success: true, Payload: json.RawMessage(`{"text":"contents"}`)}},
		},
		Tools:         []provider.GenerationTool{{Name: "read_file", Description: "Read a file.", InputSchema: json.RawMessage(`{"type":"object"}`)}},
		ToolTransport: provider.ToolTransportNative, ToolChoice: provider.ToolChoiceAuto,
		MaxOutputTokens: &maxTokens,
	})
	messages := body["messages"].([]any)
	if len(messages) != 5 || messages[0].(map[string]any)["role"] != "system" || messages[1].(map[string]any)["role"] != "user" || messages[2].(map[string]any)["role"] != "assistant" || messages[3].(map[string]any)["role"] != "assistant" || messages[3].(map[string]any)["content"] != nil || len(messages[3].(map[string]any)["tool_calls"].([]any)) != 1 || messages[4].(map[string]any)["role"] != "tool" || messages[4].(map[string]any)["tool_call_id"] != "call-1" || body["max_tokens"] != float64(321) {
		t.Fatalf("local replay request = %#v", body)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::chat_request_coalesces_developer_context_into_the_leading_system_message (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_ChatRequestCoalescesDeveloperContextIntoTheLeadingSystemMessage(t *testing.T) {
	body := localRequestBody(t, provider.GenerateRequest{
		Model: "local-model", Messages: []provider.GenerationMessage{
			{Role: "system", Content: "system rules"},
			{Role: "developer", Content: "identity update"},
			{Role: "developer", Content: "memory tool catalog"},
			{Role: "user", Content: "question"},
		},
	})
	messages := body["messages"].([]any)
	if len(messages) != 2 || messages[0].(map[string]any)["role"] != "system" || messages[0].(map[string]any)["content"] != "system rules\n\nidentity update\n\nmemory tool catalog" || messages[1].(map[string]any)["role"] != "user" || messages[1].(map[string]any)["content"] != "question" {
		t.Fatalf("developer context lowering = %#v", body)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::native_request_uses_openai_tool_fields_without_response_format (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeRequestUsesOpenaiToolFieldsWithoutResponseFormat(t *testing.T) {
	body := localRequestBody(t, provider.GenerateRequest{
		Model: "local-model", Messages: []provider.GenerationMessage{{Role: "user", Content: "search"}},
		Tools: []provider.GenerationTool{localParityTool("search_memory")}, ToolTransport: provider.ToolTransportNative, ToolChoice: provider.ToolChoiceAuto,
	})
	tools := body["tools"].([]any)
	if len(tools) != 1 || tools[0].(map[string]any)["type"] != "function" || tools[0].(map[string]any)["function"].(map[string]any)["name"] != "search_memory" || body["tool_choice"] != "auto" {
		t.Fatalf("native local request = %#v", body)
	}
	if _, exists := body["response_format"]; exists {
		t.Fatal("local request included response_format")
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::native_tool_qualification_requests_one_required_empty_object_function (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_NativeToolQualificationRequestsOneRequiredEmptyObjectFunction(t *testing.T) {
	tools := []provider.GenerationTool{{Name: "noema_local_qualification", Description: "Qualification-only function.", InputSchema: json.RawMessage(`{"type":"object","properties":{},"required":[],"additionalProperties":false}`)}}
	maxTokens := uint32(64)
	body := localRequestBody(t, provider.GenerateRequest{Model: "local-model", Messages: []provider.GenerationMessage{{Role: "user", Content: "qualify"}}, Tools: tools, ToolTransport: provider.ToolTransportNative, ToolChoice: provider.ToolChoiceRequired, MaxOutputTokens: &maxTokens})
	if len(tools) != 1 || tools[0].Name != "noema_local_qualification" || tools[0].InputSchema == nil || body["tool_choice"] != "required" || body["max_tokens"] != float64(64) {
		t.Fatalf("qualification request = %#v", body)
	}
	parameters := body["tools"].([]any)[0].(map[string]any)["function"].(map[string]any)["parameters"].(map[string]any)
	if parameters["type"] != "object" {
		t.Fatalf("qualification schema = %#v", parameters)
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::required_tool_choice_is_sent_as_native_policy (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RequiredToolChoiceIsSentAsNativePolicy(t *testing.T) {
	body := localRequestBody(t, provider.GenerateRequest{Model: "local-model", Messages: []provider.GenerationMessage{{Role: "user", Content: "finish"}}, Tools: []provider.GenerationTool{{Name: "task.finish_execution", Description: "Finish.", InputSchema: json.RawMessage(`{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}`)}}, ToolTransport: provider.ToolTransportNative, ToolChoice: provider.ToolChoiceRequired})
	if body["tool_choice"] != "required" {
		t.Fatalf("required choice = %#v", body["tool_choice"])
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::required_tool_choice_rejects_an_empty_catalog (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_RequiredToolChoiceRejectsAnEmptyCatalog(t *testing.T) {
	_, _, err := localGenerationBody("local-model", provider.GenerateRequest{Model: "local-model", Messages: []provider.GenerationMessage{{Role: "user", Content: "finish"}}, ToolChoice: provider.ToolChoiceRequired})
	if err == nil {
		t.Fatal("required empty catalog was accepted")
	}
}

// Rust source: crates/noema-providers/src/local_models/provider.rs::local_tool_schema_drops_unsupported_string_grammar (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_LocalToolSchemaDropsUnsupportedStringGrammar(t *testing.T) {
	raw := json.RawMessage(`{"type":"object","properties":{"title":{"type":"string","pattern":".*\\S.*","minLength":1,"maxLength":4000}},"required":["title"]}`)
	var lowered map[string]any
	if err := json.Unmarshal(raw, &lowered); err != nil {
		t.Fatal(err)
	}
	normalizeLocalSchema(lowered)
	title := lowered["properties"].(map[string]any)["title"].(map[string]any)
	if title["type"] != "string" {
		t.Fatalf("lowered title = %#v", title)
	}
	for _, key := range []string{"pattern", "minLength", "maxLength"} {
		if _, exists := title[key]; exists {
			t.Fatalf("unsupported schema key %q retained", key)
		}
	}
	var source map[string]any
	if err := json.Unmarshal(raw, &source); err != nil || source["properties"].(map[string]any)["title"].(map[string]any)["pattern"] != ".*\\S.*" {
		t.Fatal("source schema was mutated")
	}
}

// Rust source: crates/noema-providers/src/local_models/runtime_assets.rs::every_pinned_runtime_asset_resolves_for_its_injected_target_platform (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_EveryPinnedRuntimeAssetResolvesForItsInjectedTargetPlatform(t *testing.T) {
	var manifest struct {
		Assets []struct {
			Target  string `json:"target_triple"`
			Backend string `json:"backend"`
		} `json:"assets"`
	}
	if err := json.Unmarshal(runtimeManifestJSON, &manifest); err != nil {
		t.Fatal(err)
	}
	if len(manifest.Assets) == 0 {
		t.Fatal("runtime asset manifest is empty")
	}
	for _, asset := range manifest.Assets {
		if asset.Target == "" || asset.Backend == "" {
			t.Fatalf("incomplete runtime asset = %#v", asset)
		}
		if !strings.Contains(asset.Target, "-") {
			t.Fatalf("invalid target triple = %q", asset.Target)
		}
	}
}
