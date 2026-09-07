package provider

import (
	"encoding/json"
	"reflect"
	"testing"
)

// Rust source: crates/noema-capabilities/src/tool.rs::serialized_spec_contains_no_execution_authority.
func TestRustCapabilities_serialized_spec_contains_no_execution_authority(t *testing.T) {
	tool := GenerationTool{Name: "web.search", Description: "Search the public web.", InputSchema: json.RawMessage(`{"type":"object"}`)}
	if err := tool.Validate(); err != nil {
		t.Fatalf("tool validation: %v", err)
	}
	raw, err := json.Marshal(tool)
	if err != nil {
		t.Fatalf("tool serialization: %v", err)
	}
	var got map[string]any
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatalf("tool JSON: %v", err)
	}
	want := map[string]any{"name": "web.search", "description": "Search the public web.", "input_schema": map[string]any{"type": "object"}}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("serialized tool = %#v, want %#v", got, want)
	}
	for _, forbidden := range []string{"invoker_key", "operation_token", "decision", "authority"} {
		if _, exists := got[forbidden]; exists {
			t.Errorf("execution authority %q entered serialized tool", forbidden)
		}
	}
}
