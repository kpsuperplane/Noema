package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

type rustCapabilityRuntimeMCPFixture struct {
	chat         *Chat
	database     *store.Store
	conversation store.Conversation
	service      *noemamcp.Service
	binding      noemamcp.Binding

	mu         sync.Mutex
	calls      int
	remoteName string
	arguments  map[string]any
}

func newRustCapabilityRuntimeMCPFixture(t *testing.T, readOnly, idempotent, destructive, openWorld, fail bool) *rustCapabilityRuntimeMCPFixture {
	t.Helper()
	chat, database, conversation := chatFixture(t)
	fixture := &rustCapabilityRuntimeMCPFixture{chat: chat, database: database, conversation: conversation}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "capabilities", Version: "1"}, nil)
	remote.AddTool(&mcpsdk.Tool{Name: "read", Description: "Read one value", InputSchema: map[string]any{
		"type": "object", "properties": map[string]any{
			"text":            map[string]any{"type": "string"},
			"forged_invoker":  map[string]any{"type": "string"},
			"operation_token": map[string]any{"type": "string"},
			"api_key":         map[string]any{"type": "string"},
		}, "additionalProperties": false,
	}, Annotations: &mcpsdk.ToolAnnotations{
		ReadOnlyHint: readOnly, IdempotentHint: idempotent, DestructiveHint: boolPointerForRustTest(destructive), OpenWorldHint: boolPointerForRustTest(openWorld),
	}}, func(_ context.Context, request *mcpsdk.CallToolRequest) (*mcpsdk.CallToolResult, error) {
		var arguments map[string]any
		if err := json.Unmarshal(request.Params.Arguments, &arguments); err != nil {
			return nil, err
		}
		fixture.mu.Lock()
		fixture.calls++
		fixture.remoteName = request.Params.Name
		fixture.arguments = arguments
		fixture.mu.Unlock()
		if fail {
			return &mcpsdk.CallToolResult{IsError: true, Content: []mcpsdk.Content{&mcpsdk.TextContent{Text: "declared failure"}}}, nil
		}
		return &mcpsdk.CallToolResult{
			StructuredContent: map[string]any{"text": arguments["text"], "ordinary": "ordinary"},
			Content:           []mcpsdk.Content{&mcpsdk.TextContent{Text: "ordinary"}},
		}, nil
	})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
	paths, err := home.FromRoot(chat.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	setup, err := service.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Capabilities", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS"})
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
	fixture.service, fixture.binding = service, bindings[0]
	chat.mcp = service
	return fixture
}

func boolPointerForRustTest(value bool) *bool { return &value }

func (f *rustCapabilityRuntimeMCPFixture) snapshotCall(t *testing.T, arguments json.RawMessage) (store.ConversationTurn, store.ConversationItem) {
	t.Helper()
	now := time.Now().UTC()
	turn, _, err := f.database.BeginConversationTurn(t.Context(), f.conversation.ID, "Use the configured capability.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := f.database.StartConversationToolRound(t.Context(), turn, store.ConversationToolRound{
		Provider: "openrouter",
		Call: store.ConversationToolCallInput{ProviderCallID: "call-capability", ProviderName: f.binding.Name,
			Name: f.binding.Name, Arguments: arguments},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	return turn, items[len(items)-1]
}

func (f *rustCapabilityRuntimeMCPFixture) prepare(t *testing.T, arguments json.RawMessage) (json.RawMessage, bool, *store.ConversationItem, store.ConversationTurn, store.ConversationItem) {
	t.Helper()
	turn, call := f.snapshotCall(t, arguments)
	payload, success, approval, err := f.chat.prepareMCPAction(f.conversation, turn, call,
		store.ModelAssignment{ProviderKind: "openrouter"}, 0, "response:capability", false, f.binding, arguments)
	if err != nil {
		t.Fatalf("prepare MCP action: %v", err)
	}
	return payload, success, approval, turn, call
}

func (f *rustCapabilityRuntimeMCPFixture) remoteSnapshot() (int, string, map[string]any) {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.calls, f.remoteName, f.arguments
}

// Rust source: crates/noema-capabilities/src/router.rs::strict_resolution_rejects_unknown_and_forwards_exact_target.
func TestRustCapabilities_strict_resolution_rejects_unknown_and_forwards_exact_target(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, false)
	arguments := json.RawMessage(`{"text":"ordinary","forged_invoker":"forged","operation_token":"forged"}`)
	payload, success, approval, _, _ := fixture.prepare(t, arguments)
	if approval != nil || !success || len(payload) == 0 {
		t.Fatalf("exact MCP call = %s, %t, approval=%v", payload, success, approval != nil)
	}
	calls, operation, forwarded := fixture.remoteSnapshot()
	if calls != 1 || operation != "read" || !reflect.DeepEqual(forwarded, map[string]any{"text": "ordinary", "forged_invoker": "forged", "operation_token": "forged"}) {
		t.Fatalf("forwarded MCP invocation = calls %d, operation %q, arguments %#v", calls, operation, forwarded)
	}
	if fixture.binding.Name != "mcp."+fixture.binding.ServerID+".read" || fixture.binding.InvokerKey != "mcp" || fixture.binding.OperationToken != operation || fixture.binding.ToolID == "" || fixture.binding.SourceRevision == "" || fixture.binding.ConnectionRevision == "" {
		t.Fatalf("forwarded MCP target authority = %#v", fixture.binding)
	}
	unknown := fixture.binding
	unknown.Name = "mcp.hidden.write"
	if payload, success, err := fixture.service.Call(t.Context(), unknown, arguments); err == nil || payload != nil || success || !errors.Is(err, noemamcp.ErrUnknownOperation) {
		t.Fatalf("unknown advertised name error = %v", err)
	}
	staleOperation := fixture.binding
	staleOperation.OperationToken = "stale-operation"
	if payload, success, err := fixture.service.Call(t.Context(), staleOperation, arguments); err == nil || payload != nil || success || !errors.Is(err, noemamcp.ErrUnknownOperation) {
		t.Fatalf("stale operation error = %v", err)
	}
	if pending, err := fixture.database.PendingActionRequests(t.Context(), "human:local", &fixture.conversation.ID, nil, 10); err != nil || len(pending) != 0 {
		t.Fatalf("unknown call persistence = %#v, %v", pending, err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::reviewed_decision_requires_explicit_reviewed_dispatch.
func TestRustCapabilities_reviewed_decision_requires_explicit_reviewed_dispatch(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, false, false, true, true, false)
	if fixture.binding.ReviewRoute != store.ActionHumanReview {
		t.Fatalf("risky MCP route = %q", fixture.binding.ReviewRoute)
	}
	if payload, success, err := fixture.service.Call(t.Context(), fixture.binding, json.RawMessage(`{"text":"reviewed"}`)); err == nil || payload != nil || success || !errors.Is(err, noemamcp.ErrDenied) {
		t.Fatalf("ordinary risky dispatch = payload %s, success %t, err=%v", payload, success, err)
	}
	_, success, approval, _, _ := fixture.prepare(t, json.RawMessage(`{"text":"reviewed"}`))
	if approval == nil || success {
		t.Fatalf("ordinary risky dispatch = success %t, approval %#v", success, approval)
	}
	if calls, _, _ := fixture.remoteSnapshot(); calls != 0 {
		t.Fatalf("remote calls before reviewed authorization = %d", calls)
	}
	pending, err := fixture.database.PendingActionRequests(t.Context(), "human:local", &fixture.conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 {
		t.Fatalf("pending reviewed action = %#v, %v", pending, err)
	}
	action, err := fixture.database.DecideActionRequest(t.Context(), pending[0].ID, pending[0].Revision, "human:local", "approve", time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	if action.OperationToken != fixture.binding.Name || action.CapabilityName != fixture.binding.Name ||
		action.Arguments["text"] != "reviewed" || action.AuthorizationContext["mcp_binding"] == nil {
		t.Fatalf("reviewed authorization lost exact target or arguments = %#v", action)
	}
	if action.ID == "" || action.Revision != 1 || action.ArgumentsSHA256 != rustCapabilityArgumentDigest(json.RawMessage(`{"text":"reviewed"}`)) {
		t.Fatalf("reviewed authorization identity = id %q revision %d digest %q", action.ID, action.Revision, action.ArgumentsSHA256)
	}
	var authorizedBinding noemamcp.Binding
	if err := json.Unmarshal(mustJSON(action.AuthorizationContext["mcp_binding"]), &authorizedBinding); err != nil {
		t.Fatal(err)
	}
	if authorizedBinding.Name != fixture.binding.Name || authorizedBinding.ServerID != fixture.binding.ServerID ||
		authorizedBinding.ToolID != fixture.binding.ToolID || authorizedBinding.SourceRevision != fixture.binding.SourceRevision ||
		authorizedBinding.ConnectionRevision != fixture.binding.ConnectionRevision || authorizedBinding.OperationToken != fixture.binding.OperationToken {
		t.Fatalf("reviewed binding authority = %#v", authorizedBinding)
	}
	payload, success, notice, err := fixture.chat.executeReviewedMCP(action)
	if err != nil || notice != nil || !success || len(payload) == 0 {
		t.Fatalf("reviewed MCP dispatch = %s, %t, notice=%v, err=%v", payload, success, notice != nil, err)
	}
	if calls, operation, forwarded := fixture.remoteSnapshot(); calls != 1 || operation != "read" || forwarded["text"] != "reviewed" {
		t.Fatalf("reviewed MCP invocation = calls %d, operation %q, arguments %#v", calls, operation, forwarded)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::immediate_external_tool_reaches_the_invoker.
func TestRustCapabilities_immediate_external_tool_reaches_the_invoker(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, false)
	payload, success, approval, _, _ := fixture.prepare(t, json.RawMessage(`{"text":"immediate"}`))
	if approval != nil || !success || len(payload) == 0 {
		t.Fatalf("immediate MCP dispatch = %s, %t, approval=%v", payload, success, approval != nil)
	}
	if calls, operation, forwarded := fixture.remoteSnapshot(); calls != 1 || operation != "read" || forwarded["text"] != "immediate" {
		t.Fatalf("MCP invocation = calls %d, operation %q, arguments %#v", calls, operation, forwarded)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::source_input_check_protects_immediate_and_reviewed_dispatch.
func TestRustCapabilities_source_input_check_protects_immediate_and_reviewed_dispatch(t *testing.T) {
	for _, reviewed := range []bool{false, true} {
		fixture := newRustCapabilityRuntimeMCPFixture(t, !reviewed, !reviewed, reviewed, reviewed, false)
		valid := json.RawMessage(`{"text":"valid"}`)
		invalid := json.RawMessage(`{"value":7}`)
		if err := noemamcp.ValidateArguments(fixture.binding.InputSchema, valid); err != nil {
			t.Fatalf("reviewed=%t valid source arguments rejected: %v", reviewed, err)
		}
		if err := noemamcp.ValidateArguments(fixture.binding.InputSchema, invalid); err == nil {
			t.Fatalf("reviewed=%t invalid source arguments accepted", reviewed)
		}
		if reviewed {
			if _, _, err := fixture.service.CallReviewed(t.Context(), fixture.binding, invalid, noemamcp.ReviewedAuthorization{
				ActionID: "action:test", Revision: 1, ArgumentsSHA256: rustCapabilityArgumentDigest(invalid),
			}); !errors.Is(err, noemamcp.ErrInvalidArguments) {
				t.Fatalf("reviewed invalid arguments error = %v", err)
			}
		} else if _, _, err := fixture.service.Call(t.Context(), fixture.binding, invalid); !errors.Is(err, noemamcp.ErrInvalidArguments) {
			t.Fatalf("immediate invalid arguments error = %v", err)
		}
		if _, _, approval, _, _ := fixture.prepare(t, valid); reviewed && approval == nil {
			t.Fatal("reviewed valid dispatch did not create an approval")
		}
		if reviewed {
			pending, err := fixture.database.PendingActionRequests(t.Context(), "human:local", &fixture.conversation.ID, nil, 10)
			if err != nil || len(pending) != 1 {
				t.Fatalf("reviewed valid action = %#v, %v", pending, err)
			}
			action, err := fixture.database.DecideActionRequest(t.Context(), pending[0].ID, pending[0].Revision, "human:local", "approve", time.Now().UTC())
			if err != nil {
				t.Fatal(err)
			}
			if _, success, notice, err := fixture.chat.executeReviewedMCP(action); err != nil || notice != nil || !success {
				t.Fatalf("reviewed valid dispatch = success %t, notice=%v, err=%v", success, notice != nil, err)
			}
			if calls, _, _ := fixture.remoteSnapshot(); calls != 1 {
				t.Fatalf("reviewed valid invocations = %d", calls)
			}
		} else {
			if calls, _, _ := fixture.remoteSnapshot(); calls != 1 {
				t.Fatalf("immediate valid invocations = %d", calls)
			}
		}

		invalidFixture := newRustCapabilityRuntimeMCPFixture(t, !reviewed, !reviewed, reviewed, reviewed, false)
		if reviewed {
			if _, _, approval, _, _ := invalidFixture.prepare(t, invalid); approval == nil {
				t.Fatal("reviewed invalid dispatch did not create an approval")
			}
			pending, err := invalidFixture.database.PendingActionRequests(t.Context(), "human:local", &invalidFixture.conversation.ID, nil, 10)
			if err != nil || len(pending) != 1 {
				t.Fatalf("reviewed invalid action = %#v, %v", pending, err)
			}
			action, err := invalidFixture.database.DecideActionRequest(t.Context(), pending[0].ID, pending[0].Revision, "human:local", "approve", time.Now().UTC())
			if err != nil {
				t.Fatal(err)
			}
			if _, success, notice, err := invalidFixture.chat.executeReviewedMCP(action); err != nil || notice != nil || success {
				t.Fatalf("reviewed invalid dispatch = success %t, notice=%v, err=%v", success, notice != nil, err)
			}
		} else if _, success, approval, _, _ := invalidFixture.prepare(t, invalid); approval != nil || success {
			t.Fatalf("immediate invalid dispatch = success %t, approval=%v", success, approval != nil)
		}
		if after, _, _ := invalidFixture.remoteSnapshot(); after != 0 {
			t.Fatalf("reviewed=%t invalid invocation count = %d", reviewed, after)
		}
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::binding_policy_applies_to_every_control_plane_failure_view.
func TestRustCapabilities_binding_policy_applies_to_every_control_plane_failure_view(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, true)
	payload, success, approval, turn, call := fixture.prepare(t, json.RawMessage(`{"text":"failure","api_key":"private"}`))
	if approval != nil || success || len(payload) == 0 {
		t.Fatalf("declared MCP failure = %s, %t, approval=%v", payload, success, approval != nil)
	}
	var result map[string]any
	if err := json.Unmarshal(payload, &result); err != nil || result["isError"] != true {
		t.Fatalf("failure payload = %s, %v", payload, err)
	}
	if strings.Contains(string(payload), "private") || strings.Contains(string(payload), "api_key") {
		t.Fatalf("control-plane failure exposed secret argument = %s", payload)
	}
	finished, err := fixture.database.FinishConversationToolCall(t.Context(), turn, store.ConversationToolResultInput{
		CallItemID: call.ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call-capability", ProviderName: fixture.binding.Name, Name: fixture.binding.Name,
		Success: success, Payload: payload,
	}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	persisted, err := fixture.database.VisibleConversationItem(t.Context(), finished.ID)
	if err != nil || persisted == nil || strings.Contains(string(mustJSON(persisted.Payload)), "private") || strings.Contains(string(mustJSON(persisted.Payload)), "api_key") {
		t.Fatalf("persisted control-plane failure exposed secret argument = %#v, %v", persisted, err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::persisted_output_source_stays_out_of_model_payload.
func TestRustCapabilities_persisted_output_source_stays_out_of_model_payload(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, false)
	payload, success, approval, turn, call := fixture.prepare(t, json.RawMessage(`{"text":"summary"}`))
	if approval != nil || !success || len(payload) == 0 {
		t.Fatalf("MCP output = %s, %t, approval=%v", payload, success, approval != nil)
	}
	if !strings.Contains(string(payload), "ordinary") || strings.Contains(string(payload), "forged_invoker") {
		t.Fatalf("MCP output changed ordinary model payload = %s", payload)
	}
	result, err := fixture.database.FinishConversationToolCall(t.Context(), turn, store.ConversationToolResultInput{
		CallItemID: call.ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call-capability", ProviderName: fixture.binding.Name, Name: fixture.binding.Name,
		Success: success, Payload: payload,
	}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	persisted, err := fixture.database.VisibleConversationItem(t.Context(), result.ID)
	if err != nil || persisted == nil || !strings.Contains(string(mustJSON(persisted.Payload)), "ordinary") || strings.Contains(string(mustJSON(persisted.Payload)), "forged_invoker") {
		t.Fatalf("persisted MCP output = %#v, %v", persisted, err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::tool_declared_failure_is_completed_dispatch_with_views.
func TestRustCapabilities_tool_declared_failure_is_completed_dispatch_with_views(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, true)
	payload, success, approval, turn, call := fixture.prepare(t, json.RawMessage(`{"text":"declared","api_key":"private"}`))
	if approval != nil || success || !strings.Contains(string(payload), "declared failure") {
		t.Fatalf("tool-declared failure = %s, %t, approval=%v", payload, success, approval != nil)
	}
	result, err := fixture.database.FinishConversationToolCall(t.Context(), turn, store.ConversationToolResultInput{
		CallItemID: call.ID, Provider: "openrouter", ProviderRound: 0, OutputIndex: 0,
		ProviderCallID: "call-capability", ProviderName: fixture.binding.Name, Name: fixture.binding.Name,
		Success: success, Payload: payload,
	}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	persisted, err := fixture.database.VisibleConversationItem(t.Context(), result.ID)
	if err != nil || persisted == nil || !strings.Contains(string(mustJSON(persisted.Payload)), "declared failure") || strings.Contains(string(mustJSON(persisted.Payload)), "private") || strings.Contains(string(mustJSON(persisted.Payload)), "api_key") {
		t.Fatalf("completed tool failure = %#v, %v", persisted, err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::unknown_invoker_and_stale_token_are_typed_and_sanitized.
func TestRustCapabilities_unknown_invoker_and_stale_token_are_typed_and_sanitized(t *testing.T) {
	fixture := newRustCapabilityRuntimeMCPFixture(t, true, true, false, true, false)
	stale := fixture.binding
	stale.SourceRevision = "stale-source"
	if _, _, err := fixture.service.Call(t.Context(), stale, json.RawMessage(`{"text":"stale","api_key":"private"}`)); err == nil || !errors.Is(err, noemamcp.ErrAuthorityChanged) || strings.Contains(err.Error(), "private") {
		t.Fatalf("stale MCP authority error = %v", err)
	}
	unknown := fixture.binding
	unknown.Name = "mcp.unknown.read"
	if _, _, err := fixture.service.Call(t.Context(), unknown, json.RawMessage(`{"text":"unknown","api_key":"private"}`)); err == nil || !errors.Is(err, noemamcp.ErrUnknownOperation) || strings.Contains(err.Error(), "private") {
		t.Fatalf("unknown MCP authority error = %v", err)
	}
	unknownInvoker := fixture.binding
	unknownInvoker.InvokerKey = "missing"
	if _, _, err := fixture.service.Call(t.Context(), unknownInvoker, json.RawMessage(`{"text":"unknown","api_key":"private"}`)); err == nil || !errors.Is(err, noemamcp.ErrUnknownInvoker) || strings.Contains(err.Error(), "private") {
		t.Fatalf("unknown MCP invoker error = %v", err)
	}
}

// Rust source: crates/noema-capabilities/src/router.rs::duplicate_invoker_registration_is_rejected.
func TestRustCapabilities_duplicate_invoker_registration_is_rejected(t *testing.T) {
	_, database, _ := chatFixture(t)
	definition := store.MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32),
		DisplayName: "Capabilities", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)}
	tool := func(id string) store.MCPTool {
		return store.MCPTool{ID: "mcp_tool:" + id, Name: "same", InputSchema: json.RawMessage(`{"type":"object"}`),
			Annotations: json.RawMessage(`{}`), SourceRevision: strings.Repeat("c", 64),
			ReadOnly:    store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"},
			Idempotent:  store.MCPHint{Value: rustCapabilityBool(true), Source: "annotation"},
			Destructive: store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"},
			OpenWorld:   store.MCPHint{Value: rustCapabilityBool(false), Source: "annotation"}, Status: "ready", PolicyRevision: 1}
	}
	_, err := database.CommitMCPConnection(t.Context(), store.NewMCPConnection{Definition: definition,
		ServerID: "mcp_server:" + strings.Repeat("d", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []store.MCPTool{tool(strings.Repeat("1", 32)), tool(strings.Repeat("2", 32))}}, time.Now().UTC())
	if err == nil || !strings.Contains(err.Error(), "UNIQUE constraint failed: mcp_tools.mcp_server_id, mcp_tools.name") {
		t.Fatalf("duplicate MCP registration error = %v", err)
	}
	if _, err := database.MCPServer(t.Context(), "mcp_server:"+strings.Repeat("d", 32)); err == nil {
		t.Fatal("duplicate MCP registration published a partial authority")
	}
}

func rustCapabilityBool(value bool) *bool { return &value }

func rustCapabilityArgumentDigest(arguments json.RawMessage) string {
	digest := sha256.Sum256(arguments)
	return hex.EncodeToString(digest[:])
}
