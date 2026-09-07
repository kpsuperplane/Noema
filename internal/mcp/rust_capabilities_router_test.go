package mcp

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

type rustCapabilityRecordingInvoker struct {
	invocations []CapabilityInvocation
	output      CapabilityOutput
	err         error
}

func (invoker *rustCapabilityRecordingInvoker) Invoke(_ context.Context, invocation CapabilityInvocation) (CapabilityOutput, error) {
	invoker.invocations = append(invoker.invocations, invocation)
	if invoker.err != nil {
		return CapabilityOutput{}, invoker.err
	}
	return invoker.output, nil
}

type rustCapabilityTokenCheckingInvoker struct {
	currentToken string
	invocations  []CapabilityInvocation
}

func (invoker *rustCapabilityTokenCheckingInvoker) Invoke(_ context.Context, invocation CapabilityInvocation) (CapabilityOutput, error) {
	invoker.invocations = append(invoker.invocations, invocation)
	if invocation.OperationToken != invoker.currentToken {
		return CapabilityOutput{}, ErrCapabilityUnknownOperation
	}
	return CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}, nil
}

func rustCapabilityRawDigest(value any) string {
	raw, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	digest := sha256.Sum256(raw)
	return hex.EncodeToString(digest[:])
}

func rustCapabilityRouterServiceFixture(t *testing.T, schema string, unsafePolicy string, invoker CapabilityInvoker) (*Service, *store.Store, Binding) {
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
		DisplayName: "Capabilities", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"http://127.0.0.1:1"}`)}
	toolID := "mcp_tool:" + strings.Repeat("4", 32)
	tool := store.MCPTool{ID: toolID, ServerID: serverID, Name: "read", Description: "Test operation.",
		InputSchema: json.RawMessage(schema), Annotations: json.RawMessage(`{}`), SourceRevision: strings.Repeat("a", 64),
		ReadOnly:    store.MCPHint{Value: rustCapabilityBool(unsafePolicy == "never_ask"), Source: "annotation"},
		Idempotent:  store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"},
		Destructive: store.MCPHint{Value: rustCapabilityBool(unsafePolicy != "" && unsafePolicy != "never_ask"), Source: "annotation"},
		OpenWorld:   store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, Status: "ready", PolicyRevision: 1}
	server, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{Definition: definition, ServerID: serverID,
		ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("4", 32), AuthStatus: "none", Tools: []store.MCPTool{tool}}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	server, err = service.SaveConnectionPolicy(t.Context(), server.ID, server.ConnectionRevision, 0, "allow_automatically", unsafePolicy)
	if err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings(t.Context())
	if err != nil || len(bindings) != 1 {
		t.Fatalf("live MCP bindings = %#v, %v", bindings, err)
	}
	router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	if err != nil {
		t.Fatal(err)
	}
	service.router = router
	return service, database, bindings[0]
}

// Rust source: crates/noema-capabilities/src/router.rs::strict_resolution_rejects_unknown_and_forwards_exact_target.
func TestRustCapabilities_strict_resolution_rejects_unknown_and_forwards_exact_target(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	if err != nil {
		t.Fatal(err)
	}
	snapshot := rustCapabilityRouterSnapshot(t, Binding{
		Name: "mcp.docs.read", Description: "Read docs.", InvokerKey: "mcp", OperationToken: "reviewed:1",
		InputSchema: json.RawMessage(`{"type":"object"}`), Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true},
		PersistencePolicy: BindingPersistenceRedacted,
	})
	arguments := json.RawMessage(`{"query":"rust","invoker_key":"forged","operation_token":"forged"}`)
	dispatch, failure := router.Dispatch(t.Context(), snapshot, "mcp.docs.read", arguments)
	if failure.Error != nil || !dispatch.Output.Success || !reflect.DeepEqual(dispatch.Output.Payload, map[string]any{"ok": true}) {
		t.Fatalf("exact dispatch = %#v, %#v", dispatch, failure)
	}
	if len(invoker.invocations) != 1 || invoker.invocations[0].Operation != "mcp.docs.read" || invoker.invocations[0].OperationToken != "reviewed:1" || string(invoker.invocations[0].Arguments.(json.RawMessage)) != string(arguments) {
		t.Fatalf("forwarded exact target = %#v", invoker.invocations)
	}
	unknownDispatch, unknownFailure := router.Dispatch(t.Context(), snapshot, "mcp.hidden.write", json.RawMessage(`{"private":"never persist"}`))
	if unknownDispatch.Output.Payload != nil || !errors.Is(unknownFailure.Error, ErrCapabilityUnknownOperation) || unknownFailure.Persisted.Arguments != nil || unknownFailure.Persisted.Output != nil {
		t.Fatalf("unknown advertised name = %#v, %#v", unknownDispatch, unknownFailure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::reviewed_decision_requires_explicit_reviewed_dispatch.
func TestRustCapabilities_reviewed_decision_requires_explicit_reviewed_dispatch(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	if err != nil {
		t.Fatal(err)
	}
	destination, err := store.NewCapabilityDestination("mcp", "mcp:docs", nil, "1")
	if err != nil {
		t.Fatal(err)
	}
	binding := Binding{
		Name: "mcp.docs.write", Description: "Write docs.", ServerID: "mcp:docs", ToolID: "mcp_tool:write",
		SourceRevision: "source:write", ConnectionRevision: "connection:docs", ServerPolicyRevision: 1, ToolPolicyRevision: 1,
		InvokerKey: "mcp", OperationToken: "reviewed:write", InputSchema: json.RawMessage(`{"type":"object"}`),
		Behavior:    store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: true, OpenWorld: true},
		ReviewRoute: store.ActionLLMReview, Destination: &destination, PersistencePolicy: BindingPersistenceOmitted,
	}
	snapshot := rustCapabilityRouterSnapshot(t, binding)
	arguments := json.RawMessage(`{"body":"exact"}`)
	_, denied := router.Dispatch(t.Context(), snapshot, binding.Name, arguments)
	if !errors.Is(denied.Error, ErrCapabilityDenied) || len(invoker.invocations) != 0 {
		t.Fatalf("ordinary reviewed dispatch = %v, invocations=%d", denied.Error, len(invoker.invocations))
	}
	wrong := ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityRawDigest(json.RawMessage(`{"body":"forged"}`))}
	_, forged := router.DispatchReviewed(t.Context(), snapshot, binding.Name, arguments, wrong)
	if !errors.Is(forged.Error, ErrCapabilityDenied) || len(invoker.invocations) != 0 {
		t.Fatalf("forged reviewed authorization = %v, invocations=%d", forged.Error, len(invoker.invocations))
	}
	missingDestination := binding
	missingDestination.Destination = nil
	missingSnapshot := rustCapabilityRouterSnapshot(t, missingDestination)
	_, missing := router.DispatchReviewed(t.Context(), missingSnapshot, binding.Name, arguments, ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityRawDigest(arguments)})
	if !errors.Is(missing.Error, ErrCapabilityDenied) || len(invoker.invocations) != 0 {
		t.Fatalf("reviewed dispatch without destination = %v, invocations=%d", missing.Error, len(invoker.invocations))
	}
	authorization := ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityRawDigest(arguments)}
	dispatch, failure := router.DispatchReviewed(t.Context(), snapshot, binding.Name, arguments, authorization)
	if failure.Error != nil || !dispatch.Output.Success || len(invoker.invocations) != 1 {
		t.Fatalf("reviewed dispatch = %#v, %#v", dispatch, failure)
	}
	received := invoker.invocations[0]
	if received.ReviewedAuthorization == nil || received.ReviewedAuthorization.ActionID != "action:test" || received.ReviewedAuthorization.Revision != 1 || received.ReviewedAuthorization.ArgumentsSHA256 != authorization.ArgumentsSHA256 {
		t.Fatalf("reviewed authorization lost at invoker boundary = %#v", received.ReviewedAuthorization)
	}
	if binding.ServerID != "mcp:docs" || binding.ToolID != "mcp_tool:write" || binding.SourceRevision != "source:write" || binding.ConnectionRevision != "connection:docs" || binding.ServerPolicyRevision != 1 || binding.ToolPolicyRevision != 1 || binding.Destination == nil || binding.Destination.ServiceID != "mcp" || binding.Destination.ConnectionID != "mcp:docs" || binding.Destination.Revision != "1" {
		t.Fatalf("reviewed destination authority = %#v", binding)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::immediate_external_tool_reaches_the_invoker.
func TestRustCapabilities_immediate_external_tool_reaches_the_invoker(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	if err != nil {
		t.Fatal(err)
	}
	destination, err := store.NewCapabilityDestination("mcp", "mcp:docs", nil, "1")
	if err != nil {
		t.Fatal(err)
	}
	binding := Binding{
		Name: "mcp.docs.write", Description: "Write docs.", ServerID: "mcp:docs", ToolID: "mcp_tool:write",
		SourceRevision: "source:write", ConnectionRevision: "connection:docs", ServerPolicyRevision: 1, ToolPolicyRevision: 1,
		InvokerKey: "mcp", OperationToken: "reviewed:write", InputSchema: json.RawMessage(`{"type":"object"}`),
		Behavior:    store.ActionBehavior{ReadOnly: false, RepeatSafe: false, Destructive: false, OpenWorld: false},
		Destination: &destination, PersistencePolicy: BindingPersistenceOmitted,
	}
	dispatch, failure := router.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, json.RawMessage(`{"body":"exact"}`))
	if failure.Error != nil || !dispatch.Output.Success || !reflect.DeepEqual(dispatch.Output.Payload, map[string]any{"ok": true}) || len(invoker.invocations) != 1 {
		t.Fatalf("safe external tool dispatch = %#v, %#v", dispatch, failure)
	}
	if binding.ServerID != "mcp:docs" || binding.ToolID != "mcp_tool:write" || binding.SourceRevision != "source:write" || binding.ConnectionRevision != "connection:docs" || binding.ServerPolicyRevision != 1 || binding.ToolPolicyRevision != 1 || binding.Destination == nil || binding.Destination.ServiceID != "mcp" || binding.Destination.ConnectionID != "mcp:docs" || binding.Destination.Revision != "1" {
		t.Fatalf("immediate destination authority = %#v", binding)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::source_input_check_protects_immediate_and_reviewed_dispatch.
func TestRustCapabilities_source_input_check_protects_immediate_and_reviewed_dispatch(t *testing.T) {
	for _, test := range []struct {
		name         string
		unsafePolicy string
		reviewed     bool
	}{
		{"immediate", "never_ask", false},
		{"reviewed", "reviewer_may_approve", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
			router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "checked", Invoker: invoker})
			if err != nil {
				t.Fatal(err)
			}
			binding := Binding{
				Name: "checked.call", Description: "Checked call.", InvokerKey: "checked", OperationToken: "checked",
				InputSchema: json.RawMessage(`{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}`),
				Behavior:    store.ActionBehavior{ReadOnly: true, RepeatSafe: true}, PersistencePolicy: BindingPersistenceRedacted,
			}
			if test.reviewed {
				destination, destinationErr := store.NewCapabilityDestination("test", "checked", nil, "1")
				if destinationErr != nil {
					t.Fatal(destinationErr)
				}
				binding.ReviewRoute = store.ActionLLMReview
				binding.Destination = &destination
			}
			snapshot := rustCapabilityRouterSnapshot(t, binding)
			valid := map[string]any{"value": "ok"}
			invalid := map[string]any{"value": 7}
			var validDispatch, invalidDispatch CapabilityDispatch
			var validFailure, invalidFailure CapabilityDispatchFailure
			if test.reviewed {
				validDispatch, validFailure = router.DispatchReviewed(t.Context(), snapshot, binding.Name, valid, ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityRawDigest(valid)})
				invalidDispatch, invalidFailure = router.DispatchReviewed(t.Context(), snapshot, binding.Name, invalid, ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityRawDigest(invalid)})
			} else {
				validDispatch, validFailure = router.Dispatch(t.Context(), snapshot, binding.Name, valid)
				invalidDispatch, invalidFailure = router.Dispatch(t.Context(), snapshot, binding.Name, invalid)
			}
			if validFailure.Error != nil || !validDispatch.Output.Success || len(invoker.invocations) != 1 {
				t.Fatalf("valid source dispatch = %#v, %#v", validDispatch, validFailure)
			}
			if invalidDispatch.Output.Payload != nil || invalidFailure.Error == nil || !errors.Is(invalidFailure.Error, ErrCapabilityInvalidArguments) || len(invoker.invocations) != 1 {
				t.Fatalf("invalid source dispatch = %#v, %#v, invocations=%d", invalidDispatch, invalidFailure, len(invoker.invocations))
			}
		})
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::binding_policy_applies_to_every_control_plane_failure_view.
func TestRustCapabilities_binding_policy_applies_to_every_control_plane_failure_view(t *testing.T) {
	redactedInvoker := &rustCapabilityRecordingInvoker{err: ErrCapabilityUnavailable}
	service, _, binding := rustCapabilityRouterServiceFixture(t, `{"type":"object"}`, "never_ask", redactedInvoker)
	_, redactedFailure := service.dispatch(t.Context(), binding, json.RawMessage(`{"api_key":"private","query":"safe"}`), nil)
	if !errors.Is(redactedFailure.Error, ErrCapabilityUnavailable) || !reflect.DeepEqual(redactedFailure.Persisted.Arguments, map[string]any{"api_key": "[REDACTED]", "query": "safe"}) || !reflect.DeepEqual(redactedFailure.Persisted.Output, map[string]any{"error": "unavailable", "message": "capability is unavailable", "recovery": "retry_later"}) {
		t.Fatalf("redacted control-plane failure = %#v", redactedFailure)
	}

	omittedBinding := binding
	omittedBinding.PersistencePolicy = BindingPersistenceOmitted
	omittedInvoker := &rustCapabilityRecordingInvoker{err: ErrCapabilityUnavailable}
	omittedRouter, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: omittedInvoker})
	if err != nil {
		t.Fatal(err)
	}
	omittedSnapshot := rustCapabilityRouterSnapshot(t, omittedBinding)
	_, omittedFailure := omittedRouter.Dispatch(t.Context(), omittedSnapshot, omittedBinding.Name, map[string]any{"private": "workspace"})
	if !errors.Is(omittedFailure.Error, ErrCapabilityUnavailable) || omittedFailure.Persisted.Arguments != nil || omittedFailure.Persisted.Output != nil {
		t.Fatalf("omitted control-plane failure = %#v", omittedFailure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::persisted_output_source_stays_out_of_model_payload.
func TestRustCapabilities_persisted_output_source_stays_out_of_model_payload(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: true, Payload: map[string]any{"page": "summary"}}.WithPersistedOutputSource(map[string]any{"screenshot": "png"})}
	service, _, binding := rustCapabilityRouterServiceFixture(t, `{"type":"object"}`, "never_ask", invoker)
	dispatch, failure := service.dispatch(t.Context(), binding, json.RawMessage(`{"query":"safe"}`), nil)
	if failure.Error != nil || !dispatch.Output.Success || !reflect.DeepEqual(dispatch.Output.Payload, map[string]any{"page": "summary"}) {
		t.Fatalf("dispatch output = %#v, %#v", dispatch, failure)
	}
	if strings.Contains(string(rustCapabilityJSON(dispatch.Output.Payload)), "screenshot") || !reflect.DeepEqual(dispatch.Persisted.Arguments, map[string]any{"query": "safe"}) || !reflect.DeepEqual(dispatch.Persisted.Output, map[string]any{"screenshot": "png"}) {
		t.Fatalf("persisted output source entered model or changed views = %#v", dispatch)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::tool_declared_failure_is_completed_dispatch_with_views.
func TestRustCapabilities_tool_declared_failure_is_completed_dispatch_with_views(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: CapabilityOutput{Success: false, Payload: map[string]any{"error": "tool_declared", "password": "private"}, Failure: &CapabilityFailure{Kind: "invalid_request", Recovery: "correct_arguments"}}}
	service, _, binding := rustCapabilityRouterServiceFixture(t, `{"type":"object"}`, "never_ask", invoker)
	dispatch, failure := service.dispatch(t.Context(), binding, json.RawMessage(`{"query":"safe"}`), nil)
	if failure.Error != nil || dispatch.Output.Success || !reflect.DeepEqual(dispatch.Persisted.Output, map[string]any{"error": "tool_declared", "failure_kind": "invalid_request", "password": "[REDACTED]", "recovery": "correct_arguments"}) {
		t.Fatalf("completed tool failure = %#v, %#v", dispatch, failure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::unknown_invoker_and_stale_token_are_typed_and_sanitized.
func TestRustCapabilities_unknown_invoker_and_stale_token_are_typed_and_sanitized(t *testing.T) {
	snapshot := rustCapabilityRouterSnapshot(t, Binding{
		Name: "mcp.docs.read", Description: "Read docs.", InvokerKey: "mcp", OperationToken: "reviewed:1",
		InputSchema: json.RawMessage(`{"type":"object"}`), Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true},
		PersistencePolicy: BindingPersistenceRedacted,
	})
	emptyRouter, err := NewCapabilityRegistryRouter()
	if err != nil {
		t.Fatal(err)
	}
	missingDispatch, missingFailure := emptyRouter.Dispatch(t.Context(), snapshot, "mcp.docs.read", map[string]any{"query": "safe"})
	if missingDispatch.Output.Payload != nil || !errors.Is(missingFailure.Error, ErrCapabilityUnknownInvoker) || !reflect.DeepEqual(missingFailure.Persisted.Arguments, map[string]any{"query": "safe"}) {
		t.Fatalf("unknown invoker = %#v, %#v", missingDispatch, missingFailure)
	}

	staleInvoker := &rustCapabilityTokenCheckingInvoker{currentToken: "current-token"}
	staleRouter, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{Key: "mcp", Invoker: staleInvoker})
	if err != nil {
		t.Fatal(err)
	}
	staleSnapshot := rustCapabilityRouterSnapshot(t, Binding{
		Name: "mcp.docs.read", Description: "Read docs.", InvokerKey: "mcp", OperationToken: "stale-token",
		InputSchema: json.RawMessage(`{"type":"object"}`), Behavior: store.ActionBehavior{ReadOnly: true, RepeatSafe: true},
		PersistencePolicy: BindingPersistenceRedacted,
	})
	_, staleFailure := staleRouter.Dispatch(t.Context(), staleSnapshot, "mcp.docs.read", map[string]any{})
	if !errors.Is(staleFailure.Error, ErrCapabilityUnknownOperation) {
		t.Fatalf("stale operation = %#v", staleFailure)
	}
	if len(staleInvoker.invocations) != 1 || staleInvoker.invocations[0].OperationToken != "stale-token" {
		t.Fatalf("stale token was not checked at invoker boundary = %#v", staleInvoker.invocations)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::duplicate_invoker_registration_is_rejected.
func TestRustCapabilities_duplicate_invoker_registration_is_rejected(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{}
	_, err := NewCapabilityRegistryRouter(
		CapabilityInvokerRegistration{Key: "runtime", Invoker: invoker},
		CapabilityInvokerRegistration{Key: "runtime", Invoker: invoker},
	)
	if !errors.Is(err, ErrDuplicateCapabilityInvoker) {
		t.Fatalf("duplicate invoker registration error = %v", err)
	}
}

func rustCapabilityRouterSnapshot(t *testing.T, bindings ...Binding) BindingCatalogSnapshot {
	t.Helper()
	builder := NewBindingCatalogBuilder()
	for _, binding := range bindings {
		if err := builder.Add(binding); err != nil {
			t.Fatal(err)
		}
	}
	return builder.BuildSnapshot()
}

func rustCapabilityJSON(value any) string {
	raw, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(raw)
}
