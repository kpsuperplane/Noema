package provider

import (
	"encoding/json"
	"reflect"
	"testing"
)

// Rust source: crates/noema-capabilities/src/tool.rs::canonical_tool_spec_requires_object_input_schema.
func TestRustCapabilities_canonical_tool_spec_requires_object_input_schema(t *testing.T) {
	if err := (GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"string"}`),
	}).Validate(); err == nil {
		t.Error("non-object input schema was accepted")
	}
	if err := (GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"object","properties":{}}`),
	}).Validate(); err != nil {
		t.Fatalf("object input schema: %v", err)
	}
	if err := (GenerationTool{
		Name: "web.search", Description: "  ", InputSchema: json.RawMessage(`{"type":"object"}`),
	}).Validate(); err == nil {
		t.Error("empty description was accepted")
	}
}

// Rust source: crates/noema-capabilities/src/tool.rs::tool_name_accepts_canonical_noema_names.
func TestRustCapabilities_tool_name_accepts_canonical_noema_names(t *testing.T) {
	for _, name := range []string{"search_memory", "web.search", "mcp.mcp:docs.read"} {
		if err := validateGenerationToolName(name); err != nil {
			t.Errorf("canonical name %q rejected: %v", name, err)
		}
	}
	for _, name := range []string{"", " web.search", "web/search", "web..search", "web search"} {
		if err := validateGenerationToolName(name); err == nil {
			t.Errorf("invalid name %q was accepted", name)
		}
	}
	if err := validateGenerationToolName("web/search"); err == nil {
		t.Error("slash name was accepted")
	}
	if err := validateGenerationToolName("web.search"); err != nil {
		t.Errorf("dotted name was rejected: %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/tool.rs::tool_schema_serializes_as_raw_json_schema.
func TestRustCapabilities_tool_schema_serializes_as_raw_json_schema(t *testing.T) {
	tool := GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"object","required":[]}`),
	}
	if err := tool.Validate(); err != nil {
		t.Fatalf("object schema: %v", err)
	}
	raw, err := json.Marshal(tool)
	if err != nil {
		t.Fatalf("schema serialization: %v", err)
	}
	var got map[string]any
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatalf("schema JSON: %v", err)
	}
	if !reflect.DeepEqual(got, map[string]any{
		"name": "web.search", "description": "Search.", "input_schema": map[string]any{"type": "object", "required": []any{}},
	}) {
		t.Errorf("schema = %#v", got)
	}
	if err := (GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"array"}`),
	}).Validate(); err == nil {
		t.Error("array schema was accepted")
	}
}
