package provider

import (
	"encoding/json"
	"testing"
)

func TestResponsesDeferredToolsWireAndReplay(t *testing.T) {
	request := basicCodexGenerationRequest()
	request.ToolTransport = ToolTransportNative
	request.Tools = []GenerationTool{{Name: "shipping.eta", Description: "Read shipping ETA.", Deferred: true, InputSchema: json.RawMessage(`{"type":"object","properties":{"order_id":{"type":"string"}},"required":["order_id"],"additionalProperties":false}`)}}
	wire, names, err := prepareCodexGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	var body struct {
		Tools []map[string]json.RawMessage `json:"tools"`
	}
	if err := json.Unmarshal(wire, &body); err != nil {
		t.Fatal(err)
	}
	if len(body.Tools) != 2 || string(body.Tools[0]["defer_loading"]) != "true" || string(body.Tools[1]["type"]) != `"tool_search"` || len(body.Tools[0]["parameters"]) == 0 {
		t.Fatalf("invalid deferred wire: %s", wire)
	}
	providerName := names.canonicalToName["shipping.eta"]
	call, _ := json.Marshal(map[string]any{"type": "function_call", "call_id": "call_eta", "name": providerName, "arguments": `{"order_id":"order_42"}`})
	parsed := codexStreamResult{Output: []codexOutputItem{
		{Index: 0, Raw: json.RawMessage(`{"type":"tool_search_call","call_id":"search_one","arguments":{"query":"shipping ETA"}}`)},
		{Index: 1, Raw: json.RawMessage(`{"type":"tool_search_output","call_id":"search_one","tools":[]}`)},
		{Index: 2, Raw: call},
	}}
	result, err := normalizeCodexGeneration(request, parsed, names)
	if err != nil || len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != "shipping.eta" {
		t.Fatalf("hosted discovery was not normalized: %#v, %v", result, err)
	}
	request.Messages = append(request.Messages, result.ReplayMessages()...)
	request.Messages = append(request.Messages, GenerationMessage{Role: "tool", ToolResult: &ReplayToolResult{Name: "shipping.eta", ProviderName: providerName, ProviderCallID: "call_eta", Success: true, Payload: json.RawMessage(`{"eta":"tomorrow"}`)}})
	request.Tools[0].Deferred = false
	wire, _, err = prepareCodexGeneration(request)
	if err != nil {
		t.Fatal(err)
	}
	body.Tools = nil
	if err = json.Unmarshal(wire, &body); err != nil {
		t.Fatal(err)
	}
	if len(body.Tools) != 1 || body.Tools[0]["defer_loading"] != nil {
		t.Fatalf("used definition must be eager for full replay: %s", wire)
	}
}

func TestDeferredToolsCapabilityUsesFallbackForUnknownProvidersAndModels(t *testing.T) {
	for _, test := range []struct {
		kind, model string
		want        bool
	}{
		{"codex", "gpt-5.6-terra", true}, {"codex", "gpt-5.6-codex", false},
		{"openai", "gpt-5.4", true}, {"openai", "gpt-4.1", false},
		{"openrouter", "gpt-5.6-terra", false}, {"local", "gpt-5.4", false},
	} {
		if got := SupportsDeferredTools(test.kind, test.model); got != test.want {
			t.Errorf("%s/%s = %v", test.kind, test.model, got)
		}
	}
}
