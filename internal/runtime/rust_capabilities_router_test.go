package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"testing"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
)

type rustCapabilityRecordingInvoker struct {
	invocations []noemamcp.CapabilityInvocation
	output      noemamcp.CapabilityOutput
	err         error
}

func (invoker *rustCapabilityRecordingInvoker) Invoke(_ context.Context, invocation noemamcp.CapabilityInvocation) (noemamcp.CapabilityOutput, error) {
	invoker.invocations = append(invoker.invocations, invocation)
	if invoker.err != nil {
		return noemamcp.CapabilityOutput{}, invoker.err
	}
	return invoker.output, nil
}

func rustCapabilityRouterBinding(name, invoker, operation string, route store.ActionReviewRoute, policy noemamcp.BindingPersistencePolicy, schema string) noemamcp.Binding {
	return noemamcp.Binding{
		Name: name, Description: "Test operation.", ServerID: "mcp:docs", ToolID: name,
		SourceRevision: "source:1", ConnectionRevision: "connection:1", InvokerKey: invoker,
		OperationToken: operation, InputSchema: json.RawMessage(schema), ReviewRoute: route,
		Behavior: store.ActionBehavior{ReadOnly: route == "", RepeatSafe: true, Destructive: route != ""}, PersistencePolicy: policy,
	}
}

func rustCapabilityRouterSnapshot(t *testing.T, bindings ...noemamcp.Binding) noemamcp.BindingCatalogSnapshot {
	t.Helper()
	builder := noemamcp.NewBindingCatalogBuilder()
	for _, binding := range bindings {
		if err := builder.Add(binding); err != nil {
			t.Fatal(err)
		}
	}
	return builder.BuildSnapshot()
}

func rustCapabilityRouter(t *testing.T, registrations ...noemamcp.CapabilityInvokerRegistration) *noemamcp.CapabilityRegistryRouter {
	t.Helper()
	router, err := noemamcp.NewCapabilityRegistryRouter(registrations...)
	if err != nil {
		t.Fatal(err)
	}
	return router
}

func rustCapabilityArgumentDigest(arguments any) string {
	encoded, err := json.Marshal(arguments)
	if err != nil {
		panic(err)
	}
	digest := sha256.Sum256(encoded)
	return hex.EncodeToString(digest[:])
}

// Rust source: crates/noema-capabilities/src/router.rs::strict_resolution_rejects_unknown_and_forwards_exact_target.
func TestRustCapabilities_strict_resolution_rejects_unknown_and_forwards_exact_target(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	binding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "reviewed:1", "", noemamcp.BindingPersistenceRedacted, `{"type":"object"}`)
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	snapshot := rustCapabilityRouterSnapshot(t, binding)
	arguments := map[string]any{"query": "rust", "invoker_key": "forged", "operation_token": "forged"}
	dispatch, failure := router.Dispatch(t.Context(), snapshot, "mcp.docs.read", arguments)
	if failure.Error != nil || !dispatch.Output.Success || !reflect.DeepEqual(dispatch.Output.Payload, map[string]any{"ok": true}) {
		t.Fatalf("exact dispatch = %#v, %#v", dispatch, failure)
	}
	if len(invoker.invocations) != 1 || invoker.invocations[0].OperationToken != "reviewed:1" || !reflect.DeepEqual(invoker.invocations[0].Arguments, arguments) {
		t.Fatalf("forwarded exact target = %#v", invoker.invocations)
	}
	unknownDispatch, unknownFailure := router.Dispatch(t.Context(), snapshot, "mcp.hidden.write", map[string]any{"private": "never persist"})
	if unknownDispatch.Output.Payload != nil || !errors.Is(unknownFailure.Error, noemamcp.ErrCapabilityUnknownOperation) || unknownFailure.Persisted.Arguments != nil || unknownFailure.Persisted.Output != nil {
		t.Fatalf("unknown advertised name = %#v, %#v", unknownDispatch, unknownFailure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::reviewed_decision_requires_explicit_reviewed_dispatch.
func TestRustCapabilities_reviewed_decision_requires_explicit_reviewed_dispatch(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	binding := rustCapabilityRouterBinding("mcp.docs.write", "mcp", "reviewed:write", store.ActionLLMReview, noemamcp.BindingPersistenceOmitted, `{"type":"object"}`)
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	snapshot := rustCapabilityRouterSnapshot(t, binding)
	arguments := map[string]any{"body": "exact"}
	if dispatch, failure := router.Dispatch(t.Context(), snapshot, binding.Name, arguments); dispatch.Output.Payload != nil || !errors.Is(failure.Error, noemamcp.ErrCapabilityDenied) || len(invoker.invocations) != 0 {
		t.Fatalf("ordinary reviewed dispatch = %#v, %#v, invocations=%d", dispatch, failure, len(invoker.invocations))
	}
	wrong := noemamcp.ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityArgumentDigest(map[string]any{"body": "forged"})}
	if dispatch, failure := router.DispatchReviewed(t.Context(), snapshot, binding.Name, arguments, wrong); dispatch.Output.Payload != nil || !errors.Is(failure.Error, noemamcp.ErrCapabilityDenied) || len(invoker.invocations) != 0 {
		t.Fatalf("forged reviewed authorization = %#v, %#v, invocations=%d", dispatch, failure, len(invoker.invocations))
	}
	authorization := noemamcp.ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityArgumentDigest(arguments)}
	dispatch, failure := router.DispatchReviewed(t.Context(), snapshot, binding.Name, arguments, authorization)
	if failure.Error != nil || !dispatch.Output.Success || len(invoker.invocations) != 1 {
		t.Fatalf("reviewed dispatch = %#v, %#v", dispatch, failure)
	}
	received := invoker.invocations[0]
	if received.ReviewedAuthorization == nil || received.ReviewedAuthorization.ActionID != "action:test" || received.ReviewedAuthorization.Revision != 1 || received.ReviewedAuthorization.ArgumentsSHA256 != authorization.ArgumentsSHA256 {
		t.Fatalf("reviewed authorization lost at invoker boundary = %#v", received.ReviewedAuthorization)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::immediate_external_tool_reaches_the_invoker.
func TestRustCapabilities_immediate_external_tool_reaches_the_invoker(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
	binding := rustCapabilityRouterBinding("mcp.docs.write", "mcp", "reviewed:write", "", noemamcp.BindingPersistenceOmitted, `{"type":"object"}`)
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	dispatch, failure := router.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, map[string]any{"body": "exact"})
	if failure.Error != nil || !dispatch.Output.Success || len(invoker.invocations) != 1 {
		t.Fatalf("safe external tool dispatch = %#v, %#v", dispatch, failure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::source_input_check_protects_immediate_and_reviewed_dispatch.
func TestRustCapabilities_source_input_check_protects_immediate_and_reviewed_dispatch(t *testing.T) {
	for _, reviewed := range []bool{false, true} {
		var route store.ActionReviewRoute
		if reviewed {
			route = store.ActionLLMReview
		}
		invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: true, Payload: map[string]any{"ok": true}}}
		binding := rustCapabilityRouterBinding("checked.call", "checked", "checked", route, noemamcp.BindingPersistenceRedacted, `{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}`)
		router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "checked", Invoker: invoker})
		snapshot := rustCapabilityRouterSnapshot(t, binding)
		valid := map[string]any{"value": "ok"}
		invalid := map[string]any{"value": float64(7)}
		var validDispatch noemamcp.CapabilityDispatch
		var validFailure noemamcp.CapabilityDispatchFailure
		if reviewed {
			validDispatch, validFailure = router.DispatchReviewed(t.Context(), snapshot, binding.Name, valid, noemamcp.ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityArgumentDigest(valid)})
		} else {
			validDispatch, validFailure = router.Dispatch(t.Context(), snapshot, binding.Name, valid)
		}
		if validFailure.Error != nil || !validDispatch.Output.Success {
			t.Fatalf("reviewed=%t valid source dispatch = %#v, %#v", reviewed, validDispatch, validFailure)
		}
		var invalidDispatch noemamcp.CapabilityDispatch
		var invalidFailure noemamcp.CapabilityDispatchFailure
		if reviewed {
			invalidDispatch, invalidFailure = router.DispatchReviewed(t.Context(), snapshot, binding.Name, invalid, noemamcp.ReviewedAuthorization{ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityArgumentDigest(invalid)})
		} else {
			invalidDispatch, invalidFailure = router.Dispatch(t.Context(), snapshot, binding.Name, invalid)
		}
		if invalidDispatch.Output.Payload != nil || !errors.Is(invalidFailure.Error, noemamcp.ErrCapabilityInvalidArguments) || len(invoker.invocations) != 1 {
			t.Fatalf("reviewed=%t invalid source dispatch = %#v, %#v, invocations=%d", reviewed, invalidDispatch, invalidFailure, len(invoker.invocations))
		}
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::binding_policy_applies_to_every_control_plane_failure_view.
func TestRustCapabilities_binding_policy_applies_to_every_control_plane_failure_view(t *testing.T) {
	omittedInvoker := &rustCapabilityRecordingInvoker{err: noemamcp.ErrCapabilityUnavailable}
	omittedBinding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "reviewed:1", "", noemamcp.BindingPersistenceOmitted, `{"type":"object"}`)
	omittedRouter := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: omittedInvoker})
	_, omittedFailure := omittedRouter.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, omittedBinding), omittedBinding.Name, map[string]any{"private": "workspace"})
	if !errors.Is(omittedFailure.Error, noemamcp.ErrCapabilityUnavailable) || omittedFailure.Persisted.Arguments != nil || omittedFailure.Persisted.Output != nil {
		t.Fatalf("omitted control-plane failure = %#v", omittedFailure)
	}

	redactedInvoker := &rustCapabilityRecordingInvoker{err: noemamcp.ErrCapabilityUnavailable}
	redactedBinding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "reviewed:1", "", noemamcp.BindingPersistenceRedacted, `{"type":"object"}`)
	redactedRouter := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: redactedInvoker})
	_, redactedFailure := redactedRouter.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, redactedBinding), redactedBinding.Name, map[string]any{"api_key": "private", "query": "safe"})
	if !errors.Is(redactedFailure.Error, noemamcp.ErrCapabilityUnavailable) || !reflect.DeepEqual(redactedFailure.Persisted.Arguments, map[string]any{"api_key": "[REDACTED]", "query": "safe"}) || !reflect.DeepEqual(redactedFailure.Persisted.Output, map[string]any{"error": "unavailable", "message": "capability is unavailable", "recovery": "retry_later"}) {
		t.Fatalf("redacted control-plane failure = %#v", redactedFailure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::persisted_output_source_stays_out_of_model_payload.
func TestRustCapabilities_persisted_output_source_stays_out_of_model_payload(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: true, Payload: map[string]any{"page": "summary"}}.WithPersistedOutputSource(map[string]any{"screenshot": "png"})}
	binding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "reviewed:1", "", noemamcp.BindingPersistenceRedacted, `{"type":"object"}`)
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	dispatch, failure := router.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, map[string]any{"query": "safe"})
	if failure.Error != nil || !dispatch.Output.Success || !reflect.DeepEqual(dispatch.Output.Payload, map[string]any{"page": "summary"}) {
		t.Fatalf("dispatch output = %#v, %#v", dispatch, failure)
	}
	if strings.Contains(string(rustCapabilityJSON(dispatch.Output.Payload)), "screenshot") || !reflect.DeepEqual(dispatch.Persisted.Arguments, map[string]any{"query": "safe"}) || !reflect.DeepEqual(dispatch.Persisted.Output, map[string]any{"screenshot": "png"}) {
		t.Fatalf("persisted output source entered model or changed views = %#v", dispatch)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::tool_declared_failure_is_completed_dispatch_with_views.
func TestRustCapabilities_tool_declared_failure_is_completed_dispatch_with_views(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{output: noemamcp.CapabilityOutput{Success: false, Payload: map[string]any{"error": "tool_declared", "failure_kind": "invalid_request", "password": "private", "recovery": "correct_arguments"}}}
	binding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "reviewed:1", "", noemamcp.BindingPersistenceRedacted, `{"type":"object"}`)
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	dispatch, failure := router.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, map[string]any{"query": "safe"})
	if failure.Error != nil || dispatch.Output.Success || !reflect.DeepEqual(dispatch.Persisted.Output, map[string]any{"error": "tool_declared", "failure_kind": "invalid_request", "password": "[REDACTED]", "recovery": "correct_arguments"}) {
		t.Fatalf("completed tool failure = %#v, %#v", dispatch, failure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::unknown_invoker_and_stale_token_are_typed_and_sanitized.
func TestRustCapabilities_unknown_invoker_and_stale_token_are_typed_and_sanitized(t *testing.T) {
	binding := rustCapabilityRouterBinding("mcp.docs.read", "mcp", "stale-token", "", noemamcp.BindingPersistenceRedacted, `{"type":"object"}`)
	missingRouter := rustCapabilityRouter(t)
	_, missingFailure := missingRouter.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, map[string]any{"query": "safe", "api_key": "private"})
	if !errors.Is(missingFailure.Error, noemamcp.ErrCapabilityUnknownInvoker) || !reflect.DeepEqual(missingFailure.Persisted.Arguments, map[string]any{"query": "safe", "api_key": "[REDACTED]"}) || strings.Contains(missingFailure.Error.Error(), "private") {
		t.Fatalf("unknown invoker = %#v", missingFailure)
	}
	invoker := &rustCapabilityRecordingInvoker{err: noemamcp.ErrCapabilityUnknownOperation}
	router := rustCapabilityRouter(t, noemamcp.CapabilityInvokerRegistration{Key: "mcp", Invoker: invoker})
	_, staleFailure := router.Dispatch(t.Context(), rustCapabilityRouterSnapshot(t, binding), binding.Name, map[string]any{"query": "safe", "api_key": "private"})
	if !errors.Is(staleFailure.Error, noemamcp.ErrCapabilityUnknownOperation) || !reflect.DeepEqual(staleFailure.Persisted.Arguments, map[string]any{"query": "safe", "api_key": "[REDACTED]"}) || strings.Contains(staleFailure.Error.Error(), "private") {
		t.Fatalf("stale operation = %#v", staleFailure)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::duplicate_invoker_registration_is_rejected.
func TestRustCapabilities_duplicate_invoker_registration_is_rejected(t *testing.T) {
	invoker := &rustCapabilityRecordingInvoker{}
	_, err := noemamcp.NewCapabilityRegistryRouter(
		noemamcp.CapabilityInvokerRegistration{Key: "runtime", Invoker: invoker},
		noemamcp.CapabilityInvokerRegistration{Key: "runtime", Invoker: invoker},
	)
	if !errors.Is(err, noemamcp.ErrDuplicateCapabilityInvoker) {
		t.Fatalf("duplicate invoker registration error = %v", err)
	}
}

func rustCapabilityJSON(value any) string {
	raw, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(raw)
}
