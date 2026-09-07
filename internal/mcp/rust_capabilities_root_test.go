package mcp

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-capabilities/src/binding.rs::renamed_mcp_binding_omits_arguments_and_outputs.
func TestRustCapabilities_renamed_mcp_binding_omits_arguments_and_outputs(t *testing.T) {
	binding := Binding{Name: "read_docs", Description: "Test operation.", ServerID: "test", ToolID: "read_docs",
		SourceRevision: "source:read_docs", ConnectionRevision: "connection:read_docs", InputSchema: json.RawMessage(`{"type":"object"}`),
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true}}
	tools := GenerationTools([]Binding{binding})
	if binding.ServerID != "test" || binding.ToolID != "read_docs" || binding.SourceRevision != "source:read_docs" || binding.ConnectionRevision != "connection:read_docs" {
		t.Fatalf("binding authority changed = %#v", binding)
	}
	if len(tools) != 1 || tools[0].Name != "read_docs" || tools[0].Description != "Test operation." {
		t.Fatalf("generated binding = %#v", tools)
	}
	raw, err := json.Marshal(tools[0])
	if err != nil {
		t.Fatal(err)
	}
	var payload map[string]any
	if err := json.Unmarshal(raw, &payload); err != nil {
		t.Fatal(err)
	}
	if _, exists := payload["arguments"]; exists {
		t.Fatal("model tool exposed persisted arguments")
	}
	if _, exists := payload["output"]; exists {
		t.Fatal("model tool exposed persisted output")
	}
	if !reflect.DeepEqual(payload["input_schema"], map[string]any{"type": "object"}) {
		t.Fatalf("input schema = %#v", payload["input_schema"])
	}
}

// Rust source: crates/noema-capabilities/src/binding.rs::catalog_rejects_duplicate_authority_names.
func TestRustCapabilities_catalog_rejects_duplicate_authority_names(t *testing.T) {
	_, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "one", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "one", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err == nil || err.Error() != "MCP tool catalog contains duplicate names" {
		t.Fatalf("duplicate catalog error = %v", err)
	}
	tools, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "one", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "two", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err != nil || len(tools) != 2 || tools[0].Name != "one" || tools[1].Name != "two" {
		t.Fatalf("reusable catalog = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::merges_snapshots_and_notices_in_configured_order.
func TestRustCapabilities_merges_snapshots_and_notices_in_configured_order(t *testing.T) {
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	for _, value := range []struct {
		serverID, name, display, status string
	}{
		{"mcp_server:" + strings.Repeat("a", 32), "first.one", "First", "ready"},
		{"mcp_server:" + strings.Repeat("b", 32), "second.one", "Second", "ready"},
		{"mcp_server:" + strings.Repeat("c", 32), "unavailable.one", "Unavailable", "disabled"},
	} {
		server, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{
			Definition: store.MCPDefinition{ID: "mcp_definition:" + strings.Repeat(value.serverID[len(value.serverID)-1:], 32), Revision: "mcp_definition_revision:" + strings.Repeat(value.serverID[len(value.serverID)-1:], 32), DisplayName: value.display, TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)},
			ServerID:   value.serverID, ConnectionRevision: "mcp_connection_revision:" + strings.Repeat(value.serverID[len(value.serverID)-1:], 32), AuthStatus: "none",
			Tools: []store.MCPTool{{ID: "mcp_tool:" + strings.Repeat(value.serverID[len(value.serverID)-1:], 32), ServerID: value.serverID, Name: value.name, Description: value.display + " operation", InputSchema: json.RawMessage(`{"type":"object"}`), Annotations: json.RawMessage(`{}`), SourceRevision: strings.Repeat("d", 64),
				ReadOnly: store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"}, Idempotent: store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"}, Destructive: store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, OpenWorld: store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, Status: value.status, PolicyRevision: 1}},
		}, time.Now().UTC())
		if err != nil {
			t.Fatal(err)
		}
		if _, err := database.SaveMCPConnectionPolicy(t.Context(), server.ID, server.ConnectionRevision, 0, "allow_automatically", "always_ask", time.Now().UTC()); err != nil {
			t.Fatal(err)
		}
	}
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	bindings, err := service.Bindings(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	tools := GenerationTools(bindings)
	wantNames := []string{"mcp.mcp_server:" + strings.Repeat("a", 32) + ".first.one", "mcp.mcp_server:" + strings.Repeat("b", 32) + ".second.one"}
	if got := []string{tools[0].Name, tools[1].Name}; !reflect.DeepEqual(got, wantNames) {
		t.Fatalf("configured binding order = %#v", got)
	}
	if tools[0].Description != "First operation" || tools[1].Description != "Second operation" {
		t.Fatalf("binding descriptions = %#v", tools)
	}
	servers, err := service.Servers(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	foundUnavailable := false
	for _, server := range servers {
		if server.ID == "mcp_server:"+strings.Repeat("c", 32) {
			foundUnavailable = true
			if server.DisabledToolCount != 1 || server.AvailableToolCount != 0 {
				t.Fatalf("unavailable catalog notice = %#v", server)
			}
		}
	}
	if !foundUnavailable {
		t.Fatal("unavailable catalog notice was not published")
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::duplicate_canonical_name_fails_closed.
func TestRustCapabilities_duplicate_canonical_name_fails_closed(t *testing.T) {
	database := openRustCapabilityMCPStore(t)
	serverID := "mcp_server:" + strings.Repeat("e", 32)
	_, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{
		Definition: store.MCPDefinition{ID: "mcp_definition:" + strings.Repeat("e", 32), Revision: "mcp_definition_revision:" + strings.Repeat("f", 32), DisplayName: "Duplicate", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)},
		ServerID:   serverID, ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("a", 32), AuthStatus: "none", Tools: []store.MCPTool{
			rustCapabilityMCPTool(serverID, "same.name", "1", "ready"), rustCapabilityMCPTool(serverID, "same.name", "2", "ready"),
		}}, time.Now().UTC())
	if err == nil || !strings.Contains(err.Error(), "UNIQUE constraint failed: mcp_tools.mcp_server_id, mcp_tools.name") {
		t.Fatalf("duplicate canonical name error = %v", err)
	}
	if _, err := database.MCPServer(t.Context(), serverID); err == nil {
		t.Fatal("duplicate canonical name published a partial server")
	}
}

// Rust source: crates/noema-capabilities/src/integration.rs::resolver_preserves_original_safe_risky_and_review_matrix.
func TestRustCapabilities_resolver_preserves_original_safe_risky_and_review_matrix(t *testing.T) {
	server := store.MCPServer{DataSharingPolicy: "allow_automatically", UnsafeActionPolicy: "always_ask"}
	safe := store.ActionBehavior{ReadOnly: true, RepeatSafe: false, Destructive: false, OpenWorld: true}
	risky := store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: true, OpenWorld: false}
	if route := reviewRoute(server, safe); route != "" {
		t.Fatalf("safe route = %q", route)
	}
	for _, test := range []struct {
		policy string
		want   store.ActionReviewRoute
	}{
		{"always_ask", store.ActionHumanReview},
		{"reviewer_may_approve", store.ActionLLMReview},
		{"never_ask", ""},
	} {
		server.UnsafeActionPolicy = test.policy
		got := reviewRoute(server, risky)
		if got != test.want {
			t.Errorf("policy %q route = %q, want %q", test.policy, got, test.want)
		}
	}
}

// Rust source: crates/noema-capabilities/src/integration.rs::classification_fills_only_missing_hints_and_defaults_fail_closed.
func TestRustCapabilities_classification_fills_only_missing_hints_and_defaults_fail_closed(t *testing.T) {
	database, server, tool := rustCapabilityClassificationFixture(t)
	classified, err := database.ClassifyMCPTool(t.Context(), server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, tool.PolicyRevision, [4]bool{true, true, false, false}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	if classified.ReadOnly.Source != "annotation" || classified.ReadOnly.Value == nil || !*classified.ReadOnly.Value ||
		classified.Idempotent.Source != "model" || classified.Idempotent.Value == nil || !*classified.Idempotent.Value ||
		classified.Destructive.Source != "annotation" || classified.Destructive.Value == nil || *classified.Destructive.Value ||
		classified.OpenWorld.Source != "model" || classified.OpenWorld.Value == nil || *classified.OpenWorld.Value || classified.Status != "ready" {
		t.Fatalf("classified source hints = %#v", classified)
	}
	if classified.ReadOnly.Value == nil || classified.Idempotent.Value == nil || classified.Destructive.Value == nil || classified.OpenWorld.Value == nil {
		t.Fatalf("classified tool is not callable = %#v", classified)
	}
	idempotentDefault, openWorldDefault := false, true
	defaulted, err := database.ResetMCPToolPolicy(t.Context(), server.ID, server.ConnectionRevision, tool.ID,
		classified.SourceRevision, classified.PolicyRevision, [4]store.MCPHint{
			{Value: classified.ReadOnly.Value, Source: "annotation"},
			{Value: &idempotentDefault, Source: "safe_default"},
			{Value: classified.Destructive.Value, Source: "annotation"},
			{Value: &openWorldDefault, Source: "safe_default"},
		}, "defaulted", time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	if defaulted.Idempotent.Source != "safe_default" || defaulted.Idempotent.Value == nil || *defaulted.Idempotent.Value ||
		defaulted.OpenWorld.Source != "safe_default" || defaulted.OpenWorld.Value == nil || !*defaulted.OpenWorld.Value || defaulted.Status != "defaulted" {
		t.Fatalf("safe defaults = %#v", defaulted)
	}
}

func rustCapabilityClassificationFixture(t *testing.T) (*store.Store, store.MCPServer, store.MCPTool) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	serverID := "mcp_server:" + strings.Repeat("1", 32)
	definition := store.MCPDefinition{ID: "mcp_definition:" + strings.Repeat("2", 32), Revision: "mcp_definition_revision:" + strings.Repeat("3", 32),
		DisplayName: "Capabilities", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)}
	readOnly, destructive := true, false
	tools, err := storedTools(serverID, []DiscoveredTool{{Name: "read", SourceRevision: strings.Repeat("a", 64), ReadOnly: &readOnly, Destructive: &destructive, InputSchema: json.RawMessage(`{"type":"object"}`), Annotations: json.RawMessage(`{}`)}})
	if err != nil || len(tools) != 1 {
		t.Fatalf("stored classified source = %#v, %v", tools, err)
	}
	server, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{Definition: definition, ServerID: serverID,
		ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("4", 32), AuthStatus: "none", Tools: tools}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	persisted, err := database.MCPTools(t.Context(), server.ID)
	if err != nil || len(persisted) != 1 {
		t.Fatalf("persisted classified source = %#v, %v", persisted, err)
	}
	return database, server, persisted[0]
}

func openRustCapabilityMCPStore(t *testing.T) *store.Store {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	database, err := store.Open(t.Context(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	return database
}

func rustCapabilityMCPTool(serverID, name, idSuffix, status string) store.MCPTool {
	return store.MCPTool{ID: "mcp_tool:" + strings.Repeat(idSuffix, 32), ServerID: serverID, Name: name, InputSchema: json.RawMessage(`{"type":"object"}`), Annotations: json.RawMessage(`{}`), SourceRevision: strings.Repeat("d", 64),
		ReadOnly: store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"}, Idempotent: store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"}, Destructive: store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, OpenWorld: store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, Status: status, PolicyRevision: 1}
}

func rustCapabilityBool(value bool) *bool { return &value }
