package provider

import (
	"encoding/json"
	"reflect"
	"testing"
)

// Rust source: crates/noema-capabilities/src/tool.rs::canonical_tool_spec_requires_object_input_schema.
func TestRustCapabilities_canonical_tool_spec_requires_object_input_schema(t *testing.T) {
	if _, _, err := prepareOpenRouterTools([]GenerationTool{{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"string"}`),
	}}); err == nil {
		t.Error("non-object input schema was accepted")
	}
	_, wire, err := prepareOpenRouterTools([]GenerationTool{{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"object","properties":{}}`),
	}})
	if err != nil {
		t.Fatalf("object input schema: %v", err)
	}
	if len(wire) != 1 || wire[0].Function == nil {
		t.Fatalf("object input schema wire payload = %#v", wire)
	}
	parameters, ok := wire[0].Function.Parameters.(map[string]any)
	if !ok || parameters["type"] != "object" {
		t.Errorf("object input schema parameters = %#v", wire[0].Function.Parameters)
	}
	if _, _, err := prepareOpenRouterTools([]GenerationTool{{
		Name: "web.search", Description: "  ", InputSchema: json.RawMessage(`{"type":"object"}`),
	}}); err == nil {
		t.Error("empty description was accepted")
	}
}

// Rust source: crates/noema-capabilities/src/tool.rs::tool_name_accepts_canonical_noema_names.
func TestRustCapabilities_tool_name_accepts_canonical_noema_names(t *testing.T) {
	for _, name := range []string{"search_memory", "web.search", "mcp.mcp:docs.read"} {
		if err := validateOpenRouterToolName(name); err != nil {
			t.Errorf("canonical name %q rejected: %v", name, err)
		}
	}
	for _, name := range []string{"", " web.search", "web/search", "web..search", "web search"} {
		if err := validateOpenRouterToolName(name); err == nil {
			t.Errorf("invalid name %q was accepted", name)
		}
	}
	if err := validateOpenRouterToolName("web/search"); err == nil {
		t.Error("slash name was accepted")
	}
	if err := validateOpenRouterToolName("web.search"); err != nil {
		t.Errorf("dotted name was rejected: %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/tool.rs::tool_schema_serializes_as_raw_json_schema.
func TestRustCapabilities_tool_schema_serializes_as_raw_json_schema(t *testing.T) {
	_, wire, err := prepareOpenRouterTools([]GenerationTool{{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"object","required":[]}`),
	}})
	if err != nil {
		t.Fatalf("object schema: %v", err)
	}
	if len(wire) != 1 || wire[0].Function == nil {
		t.Fatalf("schema wire payload = %#v", wire)
	}
	parameters, ok := wire[0].Function.Parameters.(map[string]any)
	if !ok {
		t.Fatalf("schema parameters = %#v", wire[0].Function.Parameters)
	}
	if !reflect.DeepEqual(parameters, map[string]any{"type": "object", "required": []any{}}) {
		t.Errorf("schema parameters = %#v", parameters)
	}
	if _, _, err := prepareOpenRouterTools([]GenerationTool{{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"array"}`),
	}}); err == nil {
		t.Error("array schema was accepted")
	}
}
