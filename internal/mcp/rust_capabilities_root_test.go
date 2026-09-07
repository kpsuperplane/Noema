package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-capabilities/src/binding.rs::renamed_mcp_binding_omits_arguments_and_outputs.
func TestRustCapabilities_renamed_mcp_binding_omits_arguments_and_outputs(t *testing.T) {
	binding := Binding{Name: "read_docs", Description: "Test operation.", ServerID: "test", ToolID: "read_docs",
		SourceRevision: "source:read_docs", ConnectionRevision: "connection:read_docs", InputSchema: json.RawMessage(`{"type":"object"}`),
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true}, InvokerKey: "test", OperationToken: "read_docs",
		PersistencePolicy: BindingPersistenceOmitted}
	builder := NewBindingCatalogBuilder()
	if err := builder.Add(binding); err != nil {
		t.Fatal(err)
	}
	resolved, ok := builder.BuildSnapshot().Resolve("read_docs")
	if !ok {
		t.Fatal("binding was not resolved from the built catalog")
	}
	debug := fmt.Sprintf("%#v", resolved)
	if !strings.Contains(debug, `InvokerKey("test")`) || !strings.Contains(debug, `OperationToken("read_docs")`) {
		t.Fatalf("binding debug authority = %s", debug)
	}
	if resolved.InvokerKey != "test" || resolved.OperationToken != "read_docs" || resolved.ServerID != "test" || resolved.ToolID != "read_docs" || resolved.SourceRevision != "source:read_docs" || resolved.ConnectionRevision != "connection:read_docs" {
		t.Fatalf("binding authority changed = %#v", resolved)
	}
	tools := GenerationTools([]Binding{resolved})
	if len(tools) != 1 || tools[0].Name != "read_docs" || tools[0].Description != "Test operation." {
		t.Fatalf("generated binding = %#v", tools)
	}
	views := resolved.PersistedViews(
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
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("first.one")}, AvailabilityNotices: []BindingAvailabilityNotice{{Capability: &first, Status: BindingUnavailable}}}),
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("second.one")}, AvailabilityNotices: []BindingAvailabilityNotice{{Capability: nil, Status: BindingAuthenticationRequired}}}),
	)
	result, err := composite.Catalog(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if got := []string{result.Bindings[0].Name, result.Bindings[1].Name}; !reflect.DeepEqual(got, []string{"first.one", "second.one"}) {
		t.Fatalf("configured binding order = %#v", got)
	}
	wantNotices := []BindingAvailabilityNotice{{Capability: &first, Status: BindingUnavailable}, {Capability: nil, Status: BindingAuthenticationRequired}}
	if !reflect.DeepEqual(result.AvailabilityNotices, wantNotices) {
		t.Fatalf("availability notices = %#v, want %#v", result.AvailabilityNotices, wantNotices)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::duplicate_canonical_name_fails_closed.
func TestRustCapabilities_duplicate_canonical_name_fails_closed(t *testing.T) {
	composite := NewCompositeBindingSource(
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("same.name")}}),
		asyncRustCapabilitySource(BindingCatalogResult{Bindings: []Binding{rustCapabilityBinding("same.name")}}),
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
	for _, test := range []struct {
		policy string
		want   store.ActionReviewRoute
	}{
		{"always_ask", store.ActionHumanReview},
		{"reviewer_may_approve", store.ActionLLMReview},
		{"never_ask", ""},
	} {
		server.UnsafeActionPolicy = test.policy
		if route := reviewRoute(server, safe); route != "" {
			t.Fatalf("policy %q safe route = %q", test.policy, route)
		}
		got := reviewRoute(server, risky)
		if got != test.want {
			t.Errorf("policy %q route = %q, want %q", test.policy, got, test.want)
		}
	}
}

func rustCapabilityBinding(name string) Binding {
	return Binding{Name: name, Description: "Test operation.", InvokerKey: "test", OperationToken: name,
		InputSchema: json.RawMessage(`{"type":"object"}`), Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true},
		PersistencePolicy: BindingPersistenceRedacted}
}

func asyncRustCapabilitySource(result BindingCatalogResult) BindingSource {
	return BindingSourceFunc(func(context.Context) (BindingCatalogResult, error) {
		return result, nil
	})
}
