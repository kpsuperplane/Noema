package provider

import (
	"encoding/json"
	"errors"
	"reflect"
	"testing"
)

// Rust source: crates/noema-capabilities/src/tool.rs::canonical_tool_spec_requires_object_input_schema.
func TestRustCapabilities_canonical_tool_spec_requires_object_input_schema(t *testing.T) {
	if err := (GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"string"}`),
	}).Validate(); err == nil || !errors.Is(err, ErrGenerationToolSchemaInvalid) {
		t.Error("non-object input schema was accepted")
	}
	tool := GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"object","properties":{}}`),
	}
	if err := tool.Validate(); err != nil {
		t.Fatalf("object input schema: %v", err)
	}
	schema, err := NewInputToolSchema(tool.Name, tool.InputSchema)
	if err != nil {
		t.Fatalf("object input schema returned an error: %v", err)
	}
	raw, err := json.Marshal(schema)
	if err != nil {
		t.Fatalf("object input schema serialization: %v", err)
	}
	var returned map[string]any
	if err := json.Unmarshal(raw, &returned); err != nil {
		t.Fatalf("object input schema JSON: %v", err)
	}
	if returned["type"] != "object" {
		t.Fatalf("object input schema type = %#v", returned["type"])
	}
	if err := (GenerationTool{
		Name: "web.search", Description: "  ", InputSchema: json.RawMessage(`{"type":"object"}`),
	}).Validate(); err == nil || !errors.Is(err, ErrGenerationToolDescriptionInvalid) {
		t.Error("empty description was accepted")
	}
}

// Rust source: crates/noema-capabilities/src/tool.rs::tool_name_accepts_canonical_noema_names.
func TestRustCapabilities_tool_name_accepts_canonical_noema_names(t *testing.T) {
	for _, name := range []string{"search_memory", "web.search", "mcp.mcp:docs.read"} {
		toolName, err := NewToolName(name)
		if err != nil || toolName.String() != name {
			t.Errorf("canonical name %q rejected: %v", name, err)
		}
	}
	for _, name := range []string{"", " web.search", "web/search", "web..search", "web search"} {
		if _, err := NewToolName(name); err == nil {
			t.Errorf("invalid name %q was accepted", name)
		}
	}
	var decoded ToolName
	if err := json.Unmarshal([]byte(`"web/search"`), &decoded); !errors.Is(err, ErrGenerationToolNameInvalid) {
		t.Fatalf("slash name JSON error = %v", err)
	}
	if err := json.Unmarshal([]byte(`"web.search"`), &decoded); err != nil || decoded.String() != "web.search" {
		t.Fatalf("valid name JSON = %q, %v", decoded.String(), err)
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
	schema, err := NewInputToolSchema("web.search", tool.InputSchema)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := json.Marshal(schema)
	if err != nil {
		t.Fatalf("schema serialization: %v", err)
	}
	var got map[string]any
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatalf("schema JSON: %v", err)
	}
	if !reflect.DeepEqual(got, map[string]any{"type": "object", "required": []any{}}) {
		t.Errorf("schema = %#v", got)
	}
	toolRaw, err := json.Marshal(tool)
	if err != nil {
		t.Fatal(err)
	}
	var toolJSON map[string]any
	if err := json.Unmarshal(toolRaw, &toolJSON); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(toolJSON, map[string]any{
		"name": "web.search", "description": "Search.", "input_schema": map[string]any{"type": "object", "required": []any{}},
	}) {
		t.Errorf("tool schema = %#v", toolJSON)
	}
	var decoded ToolSchema
	if err := json.Unmarshal(raw, &decoded); err != nil {
		t.Fatal(err)
	}
	var invalid ToolSchema
	if err := json.Unmarshal([]byte(`{"type":"array"}`), &invalid); !errors.Is(err, ErrGenerationToolSchemaInvalid) {
		t.Fatalf("array schema unmarshal error = %v", err)
	}
	if err := json.Unmarshal([]byte(`{"type":"object"}`), &decoded); err != nil {
		t.Fatalf("object schema unmarshal: %v", err)
	}
	if err := (GenerationTool{
		Name: "web.search", Description: "Search.", InputSchema: json.RawMessage(`{"type":"array"}`),
	}).Validate(); err == nil || !errors.Is(err, ErrGenerationToolSchemaInvalid) {
		t.Error("array schema was accepted")
	}
}
