package runtime

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestOnDemandDefinitionsFollowSavedSelectionAndCurrentAvailability(t *testing.T) {
	catalog := []provider.GenerationTool{
		{Name: "memory.read", Description: "Read memory.", InputSchema: json.RawMessage(`{"type":"object"}`)},
		{Name: "calendar.find", Description: strings.Repeat("Calendar details. ", 100), InputSchema: json.RawMessage(`{"type":"object","description":"` + strings.Repeat("field ", 1000) + `"}`), ServiceConnectionID: "calendar:one"},
	}
	fallback := onDemandTools(catalog, nil, "openrouter", "test")
	if len(fallback) != 2 || fallback[0].Name != catalog[0].Name || fallback[1].Name != loadToolsName || strings.Contains(fallback[1].Description, "Calendar details.") {
		t.Fatalf("fallback catalog did not replace the external definition: %#v", fallback)
	}
	if !strings.Contains(fallback[1].Description, "calendar.find") {
		t.Fatal("directory lost the exact tool name")
	}
	payload, success := loadToolDefinitions(catalog, json.RawMessage(`{"names":["calendar.find"]}`))
	if !success {
		t.Fatalf("load failed: %s", payload)
	}
	history := []provider.GenerationMessage{{Role: "tool", ToolResult: &provider.ReplayToolResult{Name: loadToolsName, Success: true, Payload: payload}}}
	selected := onDemandTools(catalog, history, "openrouter", "test")
	if len(selected) != 2 || selected[1].Name != catalog[1].Name || string(selected[1].InputSchema) != string(catalog[1].InputSchema) {
		t.Fatal("saved selection did not restore the exact definition")
	}
	removed := onDemandTools(catalog[:1], history, "openrouter", "test")
	if len(removed) != 1 {
		t.Fatal("saved selection restored a removed tool")
	}
	history[0].ToolResult.Success = false
	if onDemandTools(catalog, history, "openrouter", "test")[1].Name != loadToolsName {
		t.Fatal("failed loading exposed a definition")
	}

	native := onDemandTools(catalog, nil, "codex", "gpt-5.6-terra")
	if len(native) != 2 || native[0].Deferred || !native[1].Deferred {
		t.Fatal("native catalog deferred the wrong definitions")
	}
	eagerCount := CountModelContext(t.Context(), nil, nil, catalog, false)
	deferredCount := CountModelContext(t.Context(), nil, nil, native, false)
	if deferredCount >= eagerCount/2 || len(native[1].InputSchema) == 0 {
		t.Fatal("context accounting did not defer the schema without changing it")
	}
	history = []provider.GenerationMessage{{Role: "assistant", ToolCalls: []provider.ReplayToolCall{{Name: "calendar.find"}}}}
	if onDemandTools(catalog, history, "codex", "gpt-5.6-terra")[1].Deferred {
		t.Fatal("native replay must include the used definition eagerly")
	}
	if onDemandTools(catalog, nil, "codex", "unknown")[1].Name != loadToolsName {
		t.Fatal("unknown models must use local loading")
	}
}

func TestLoadDefinitionsRejectsUnavailableAndInvalidSelection(t *testing.T) {
	catalog := []provider.GenerationTool{{Name: "calendar.find", ServiceConnectionID: "calendar:one"}, {Name: "memory.read"}}
	for _, raw := range []string{
		`{}`, `{"names":[]}`, `{"names":["missing"]}`, `{"names":["memory.read"]}`,
		`{"names":["calendar.find","calendar.find"]}`, `{"names":["calendar.find"],"extra":true}`,
		`{"names":["a","b","c","d","e"]}`, `{"names":null}`,
	} {
		if _, success := loadToolDefinitions(catalog, json.RawMessage(raw)); success {
			t.Errorf("accepted %s", raw)
		}
	}
}
