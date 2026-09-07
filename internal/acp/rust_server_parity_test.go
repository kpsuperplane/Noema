package acp

import (
	"encoding/json"
	"testing"
)

// Rust source: crates/noema-server/src/bin/noema-acp-task-mcp.rs::blocked_tool_schema_restricts_gate_kinds.
func TestRustServer_blocked_tool_schema_restricts_gate_kinds(t *testing.T) {
	tools := taskTerminalTools()
	if len(tools) != 3 {
		t.Fatalf("terminal tool count = %d", len(tools))
	}
	blocked, ok := tools[2].(map[string]any)
	if !ok {
		t.Fatalf("blocked tool type = %T", tools[2])
	}
	schema, ok := blocked["inputSchema"].(map[string]any)
	if !ok {
		t.Fatalf("blocked schema type = %T", blocked["inputSchema"])
	}
	properties, ok := schema["properties"].(map[string]any)
	if !ok {
		t.Fatalf("blocked properties type = %T", schema["properties"])
	}
	gateKind, ok := properties["gate_kind"].(map[string]any)
	if !ok {
		t.Fatalf("gate_kind schema type = %T", properties["gate_kind"])
	}
	values, ok := gateKind["enum"].([]string)
	if !ok || len(values) != 2 || values[0] != "clarification" || values[1] != "approval" {
		encoded, _ := json.Marshal(gateKind["enum"])
		t.Fatalf("gate_kind enum = %s", encoded)
	}
}
