package mcp

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"reflect"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

// Rust source: crates/noema-capabilities/src/binding.rs::renamed_mcp_binding_omits_arguments_and_outputs.
func TestRustCapabilities_renamed_mcp_binding_omits_arguments_and_outputs(t *testing.T) {
	binding := Binding{Name: "mcp.mcp:docs.read", Description: "Read docs", ServerID: "mcp:docs", ToolID: "tool:read",
		SourceRevision: "source:1", ConnectionRevision: "connection:1", InputSchema: json.RawMessage(`{"type":"object"}`),
		Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true}}
	tools := GenerationTools([]Binding{binding})
	if len(tools) != 1 || tools[0].Name != "mcp.mcp:docs.read" || tools[0].Description != "Read docs" {
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
	_, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "read", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "read", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err == nil || !strings.Contains(err.Error(), "duplicate") {
		t.Fatalf("duplicate catalog error = %v", err)
	}
	tools, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "read", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "write", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err != nil || len(tools) != 2 || tools[0].Name != "read" || tools[1].Name != "write" {
		t.Fatalf("reusable catalog = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::merges_snapshots_and_notices_in_configured_order.
func TestRustCapabilities_merges_snapshots_and_notices_in_configured_order(t *testing.T) {
	bindings := []Binding{{Name: "first.one", Description: "First"}, {Name: "second.one", Description: "Second"}}
	tools := GenerationTools(bindings)
	if got := []string{tools[0].Name, tools[1].Name}; !reflect.DeepEqual(got, []string{"first.one", "second.one"}) {
		t.Fatalf("configured binding order = %#v", got)
	}
	if tools[0].Description != "First" || tools[1].Description != "Second" {
		t.Fatalf("binding descriptions = %#v", tools)
	}
}

// Rust source: crates/noema-capabilities/src/composite.rs::duplicate_canonical_name_fails_closed.
func TestRustCapabilities_duplicate_canonical_name_fails_closed(t *testing.T) {
	_, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "same.name", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "same.name", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err == nil {
		t.Fatal("duplicate canonical name was published")
	}
}

// Rust source: crates/noema-capabilities/src/integration.rs::resolver_preserves_original_safe_risky_and_review_matrix.
func TestRustCapabilities_resolver_preserves_original_safe_risky_and_review_matrix(t *testing.T) {
	server := store.MCPServer{UnsafeActionPolicy: "always_ask"}
	if route := reviewRoute(server, store.ActionBehavior{ReadOnly: true, RepeatSafe: true, OpenWorld: true}); route != "" {
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
		got := reviewRoute(server, store.ActionBehavior{Destructive: true, OpenWorld: true})
		if got != test.want {
			t.Errorf("policy %q route = %q, want %q", test.policy, got, test.want)
		}
	}
}

// Rust source: crates/noema-capabilities/src/integration.rs::classification_fills_only_missing_hints_and_defaults_fail_closed.
func TestRustCapabilities_classification_fills_only_missing_hints_and_defaults_fail_closed(t *testing.T) {
	readOnly, destructive := true, false
	tools, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "read", ReadOnly: &readOnly, Destructive: &destructive, InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err != nil || len(tools) != 1 {
		t.Fatalf("stored classified source = %#v, %v", tools, err)
	}
	tool := tools[0]
	if tool.ReadOnly.Source != "annotation" || tool.ReadOnly.Value == nil || !*tool.ReadOnly.Value ||
		tool.Destructive.Source != "annotation" || tool.Destructive.Value == nil || *tool.Destructive.Value {
		t.Fatalf("source hints changed = %#v", tool)
	}
	if tool.Idempotent.Source != "safe_default" || tool.Idempotent.Value == nil || *tool.Idempotent.Value ||
		tool.OpenWorld.Source != "safe_default" || tool.OpenWorld.Value == nil || !*tool.OpenWorld.Value || tool.Status != "defaulted" {
		t.Fatalf("missing hints were not fail-closed = %#v", tool)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::strict_resolution_rejects_unknown_and_forwards_exact_target.
func TestRustCapabilities_strict_resolution_rejects_unknown_and_forwards_exact_target(t *testing.T) {
	service, binding, calls := rustCapabilityMCPFixture(t, true, true, false, true, false)
	arguments := json.RawMessage(`{"text":"ordinary","forged_invoker":"ignored"}`)
	result, success, err := service.Call(t.Context(), binding, arguments)
	if err != nil || !success || len(result) == 0 || calls() != 1 {
		t.Fatalf("exact MCP call = %s, %t, %v, calls=%d", result, success, err, calls())
	}
	unknown := binding
	unknown.Name += ".unknown"
	if _, _, err := service.Call(t.Context(), unknown, arguments); err == nil {
		t.Fatal("unknown MCP operation was dispatched")
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::reviewed_decision_requires_explicit_reviewed_dispatch.
func TestRustCapabilities_reviewed_decision_requires_explicit_reviewed_dispatch(t *testing.T) {
	service, binding, _ := rustCapabilityMCPFixture(t, false, false, true, true, false)
	if binding.ReviewRoute != store.ActionHumanReview {
		t.Fatalf("risky MCP route = %q", binding.ReviewRoute)
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{"text":"reviewed"}`)); err != nil {
		t.Fatalf("reviewed MCP binding could not be resolved: %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::immediate_external_tool_reaches_the_invoker.
func TestRustCapabilities_immediate_external_tool_reaches_the_invoker(t *testing.T) {
	service, binding, calls := rustCapabilityMCPFixture(t, true, true, false, true, false)
	if _, success, err := service.Call(t.Context(), binding, json.RawMessage(`{"text":"immediate"}`)); err != nil || !success {
		t.Fatalf("immediate MCP dispatch = %t, %v", success, err)
	}
	if calls() != 1 {
		t.Fatalf("MCP invocations = %d", calls())
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::source_input_check_protects_immediate_and_reviewed_dispatch.
func TestRustCapabilities_source_input_check_protects_immediate_and_reviewed_dispatch(t *testing.T) {
	service, binding, calls := rustCapabilityMCPFixture(t, true, true, false, true, false)
	if err := ValidateArguments(binding.InputSchema, json.RawMessage(`{"text":"valid"}`)); err != nil {
		t.Fatalf("valid source arguments rejected: %v", err)
	}
	if err := ValidateArguments(binding.InputSchema, json.RawMessage(`{"value":7}`)); err == nil {
		t.Fatal("invalid source arguments accepted")
	}
	if _, _, err := service.Call(t.Context(), binding, json.RawMessage(`{"value":7}`)); err == nil {
		t.Fatal("invalid MCP dispatch reached the remote tool")
	}
	if calls() != 0 {
		t.Fatalf("invalid MCP invocations = %d", calls())
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::binding_policy_applies_to_every_control_plane_failure_view.
func TestRustCapabilities_binding_policy_applies_to_every_control_plane_failure_view(t *testing.T) {
	service, binding, _ := rustCapabilityMCPFixture(t, true, true, false, true, true)
	result, success, err := service.Call(t.Context(), binding, json.RawMessage(`{"text":"failure"}`))
	if err != nil || success {
		t.Fatalf("declared MCP failure = %s, %t, %v", result, success, err)
	}
	var payload map[string]any
	if json.Unmarshal(result, &payload) != nil || payload["isError"] != true {
		t.Fatalf("failure payload = %s", result)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::persisted_output_source_stays_out_of_model_payload.
func TestRustCapabilities_persisted_output_source_stays_out_of_model_payload(t *testing.T) {
	service, binding, _ := rustCapabilityMCPFixture(t, true, true, false, true, false)
	result, success, err := service.Call(t.Context(), binding, json.RawMessage(`{"text":"summary"}`))
	if err != nil || !success {
		t.Fatalf("MCP output = %s, %t, %v", result, success, err)
	}
	if !strings.Contains(string(result), "ordinary") {
		t.Fatalf("MCP output lost ordinary source = %s", result)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::tool_declared_failure_is_completed_dispatch_with_views.
func TestRustCapabilities_tool_declared_failure_is_completed_dispatch_with_views(t *testing.T) {
	service, binding, _ := rustCapabilityMCPFixture(t, true, true, false, true, true)
	result, success, err := service.Call(t.Context(), binding, json.RawMessage(`{"text":"declared"}`))
	if err != nil || success {
		t.Fatalf("tool-declared failure = %s, %t, %v", result, success, err)
	}
	if !strings.Contains(string(result), "declared failure") {
		t.Fatalf("failure reason was not retained = %s", result)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::unknown_invoker_and_stale_token_are_typed_and_sanitized.
func TestRustCapabilities_unknown_invoker_and_stale_token_are_typed_and_sanitized(t *testing.T) {
	service, binding, _ := rustCapabilityMCPFixture(t, true, true, false, true, false)
	stale := binding
	stale.SourceRevision = "stale-source"
	if _, _, err := service.Call(t.Context(), stale, json.RawMessage(`{"text":"stale"}`)); err == nil || !strings.Contains(err.Error(), "authority") {
		t.Fatalf("stale MCP authority error = %v", err)
	}
	unknown := binding
	unknown.Name = "mcp.unknown.read"
	if _, _, err := service.Call(t.Context(), unknown, json.RawMessage(`{"text":"unknown"}`)); err == nil {
		t.Fatal("unknown MCP authority was accepted")
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::duplicate_invoker_registration_is_rejected.
func TestRustCapabilities_duplicate_invoker_registration_is_rejected(t *testing.T) {
	_, err := storedTools("mcp:docs", []DiscoveredTool{{Name: "same", InputSchema: json.RawMessage(`{"type":"object"}`)}, {Name: "same", InputSchema: json.RawMessage(`{"type":"object"}`)}})
	if err == nil || !strings.Contains(err.Error(), "duplicate") {
		t.Fatalf("duplicate MCP registration error = %v", err)
	}
}

func rustCapabilityMCPFixture(t *testing.T, readOnly, idempotent, destructive, openWorld, fail bool) (*Service, Binding, func() int) {
	t.Helper()
	var calls int
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "capabilities", Version: "1"}, nil)
	remote.AddTool(&mcpsdk.Tool{Name: "read", Description: "Read one value", InputSchema: map[string]any{
		"type": "object", "properties": map[string]any{"text": map[string]any{"type": "string"}, "forged_invoker": map[string]any{"type": "string"}}, "additionalProperties": false,
	}, Annotations: &mcpsdk.ToolAnnotations{
		ReadOnlyHint: readOnly, IdempotentHint: idempotent, DestructiveHint: boolTestPointer(destructive), OpenWorldHint: boolTestPointer(openWorld),
	}}, func(_ context.Context, request *mcpsdk.CallToolRequest) (*mcpsdk.CallToolResult, error) {
		calls++
		if fail {
			return &mcpsdk.CallToolResult{IsError: true, Content: []mcpsdk.Content{&mcpsdk.TextContent{Text: "declared failure"}}}, nil
		}
		var arguments map[string]any
		_ = json.Unmarshal(request.Params.Arguments, &arguments)
		return &mcpsdk.CallToolResult{StructuredContent: map[string]any{"text": arguments["text"], "ordinary": "ordinary"}, Content: []mcpsdk.Content{&mcpsdk.TextContent{Text: "ordinary"}}}, nil
	})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
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
	setup, err := service.Create(t.Context(), SetupInput{DisplayName: "Capabilities", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("MCP setup = %#v, %v", setup, err)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings(t.Context())
	if err != nil || len(bindings) != 1 {
		t.Fatalf("MCP bindings = %#v, %v", bindings, err)
	}
	return service, bindings[0], func() int { return calls }
}
