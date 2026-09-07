package mcp

import (
	"context"
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
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true}, InvokerKey: "test", OperationToken: "read_docs",
		PersistencePolicy: BindingPersistenceOmitted}
	tools := GenerationTools([]Binding{binding})
	if binding.InvokerKey != "test" || binding.OperationToken != "read_docs" || binding.ServerID != "test" || binding.ToolID != "read_docs" || binding.SourceRevision != "source:read_docs" || binding.ConnectionRevision != "connection:read_docs" {
		t.Fatalf("binding authority changed = %#v", binding)
	}
	if len(tools) != 1 || tools[0].Name != "read_docs" || tools[0].Description != "Test operation." {
		t.Fatalf("generated binding = %#v", tools)
	}
	views := binding.PersistedViews(
		map[string]any{"private": "workspace query"},
		map[string]any{"private": "workspace result"},
	)
	if views.Arguments != nil || views.Output != nil {
		t.Fatalf("persisted binding views = %#v", views)
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
	builder := NewBindingCatalogBuilder()
	if err := builder.Add(rustCapabilityBinding("one")); err != nil {
		t.Fatal(err)
	}
	if err := builder.Add(rustCapabilityBinding("one")); err != ErrDuplicateBindingName {
		t.Fatalf("duplicate catalog error = %v", err)
	}
	if err := builder.Add(rustCapabilityBinding("two")); err != nil {
		t.Fatalf("builder was not reusable after rejection: %v", err)
	}
	bindings := builder.Build()
	if len(bindings) != 2 || bindings[0].Name != "one" || bindings[1].Name != "two" {
		t.Fatalf("reusable catalog = %#v", bindings)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::merges_snapshots_and_notices_in_configured_order.
func TestRustCapabilities_merges_snapshots_and_notices_in_configured_order(t *testing.T) {
	first := "first.hidden"
	composite := NewCompositeBindingSource(
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("first.one")}, AvailabilityNotices: []BindingAvailabilityNotice{{Capability: &first, Status: "unavailable"}}}),
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("second.one")}, AvailabilityNotices: []BindingAvailabilityNotice{{Capability: nil, Status: "authentication_required"}}}),
	)
	result, err := composite.Catalog(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if got := []string{result.Bindings[0].Name, result.Bindings[1].Name}; !reflect.DeepEqual(got, []string{"first.one", "second.one"}) {
		t.Fatalf("configured binding order = %#v", got)
	}
	wantNotices := []BindingAvailabilityNotice{{Capability: &first, Status: "unavailable"}, {Capability: nil, Status: "authentication_required"}}
	if !reflect.DeepEqual(result.AvailabilityNotices, wantNotices) {
		t.Fatalf("availability notices = %#v, want %#v", result.AvailabilityNotices, wantNotices)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::duplicate_canonical_name_fails_closed.
func TestRustCapabilities_duplicate_canonical_name_fails_closed(t *testing.T) {
	name := "same.name"
	composite := NewCompositeBindingSource(
		BindingSourceFunc(func(context.Context) (BindingCatalogResult, error) {
			return BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding(name)}}, nil
		}),
		BindingSourceFunc(func(context.Context) (BindingCatalogResult, error) {
			return BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding(name)}}, nil
		}),
	)
	if _, err := composite.Catalog(t.Context()); err != ErrInvalidBindingSource {
		t.Fatalf("duplicate canonical name error = %v", err)
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
	service, _, server, tool := rustCapabilityClassificationFixture(t)
	classified, err := service.ClassifyToolResponse(t.Context(), server.ID, server.ConnectionRevision, tool.ID,
		tool.SourceRevision, tool.PolicyRevision, `{"idempotent":true,"openWorld":false}`)
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
	pending := classified
	pending.Idempotent = store.MCPHint{}
	pending.OpenWorld = store.MCPHint{}
	defaulted := ApplyToolSafeDefaults(pending)
	if defaulted.Idempotent.Source != "safe_default" || defaulted.Idempotent.Value == nil || *defaulted.Idempotent.Value ||
		defaulted.OpenWorld.Source != "safe_default" || defaulted.OpenWorld.Value == nil || !*defaulted.OpenWorld.Value || defaulted.Status != "defaulted" {
		t.Fatalf("safe defaults = %#v", defaulted)
	}
}

func rustCapabilityClassificationFixture(t *testing.T) (*Service, *store.Store, store.MCPServer, store.MCPTool) {
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
	service, err := NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
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
	return service, database, server, persisted[0]
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

func rustCapabilityBinding(name string) Binding {
	return Binding{Name: name, Description: "Test operation.", InvokerKey: "test", OperationToken: name,
		InputSchema: json.RawMessage(`{"type":"object"}`), Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true}}
}

func asyncRustCapabilitySource(result BindingCatalogResult) BindingSource {
	return BindingSourceFunc(func(ctx context.Context) (BindingCatalogResult, error) {
		channel := make(chan BindingCatalogResult, 1)
		go func() { channel <- result }()
		select {
		case value := <-channel:
			return value, nil
		case <-ctx.Done():
			return BindingCatalogResult{}, ctx.Err()
		}
	})
}
