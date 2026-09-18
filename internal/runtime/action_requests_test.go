package runtime

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

func runtimeDiscoveredMCPBinding(t *testing.T, bindings []noemamcp.Binding) noemamcp.Binding {
	t.Helper()
	for _, binding := range bindings {
		if binding.Name != "mcp.connect_service" {
			return binding
		}
	}
	t.Fatalf("MCP discovery binding missing: %#v", bindings)
	return noemamcp.Binding{}
}

func TestPrimaryChatCallsExactMCPBindingAndReplaysResult(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	remoteCalls := 0
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "calendar", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "lookup", Description: "Find an event",
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true, IdempotentHint: true,
			DestructiveHint: runtimeBool(false), OpenWorldHint: runtimeBool(true)}},
		func(_ context.Context, _ *mcpsdk.CallToolRequest, input struct {
			Query string `json:"query"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			remoteCalls++
			return nil, map[string]any{"event": input.Query}, nil
		})
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	defer httpServer.Close()
	paths, err := home.FromRoot(original.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	setup, err := mcpService.Create(context.Background(), noemamcp.SetupInput{DisplayName: "Calendar",
		TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("setup = %#v, %v", setup, err)
	}
	server, err := mcpService.SaveConnectionPolicy(context.Background(), setup.Server.ID,
		setup.Server.ConnectionRevision, 0, "allow_automatically", "reviewer_may_approve")
	if err != nil {
		t.Fatal(err)
	}
	bindings, err := mcpService.Bindings(context.Background())
	if err != nil || len(bindings) < 2 {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	modelName := modelMCPToolName("Calendar", binding.OperationToken)
	requests := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		if requests == 1 {
			found := false
			for _, tool := range request.Tools {
				if tool.Name == modelName {
					found = true
				}
			}
			if !found {
				t.Fatalf("MCP tool was not advertised: %#v", request.Tools)
			}
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "mcp-1",
				ProviderName: modelName, Name: modelName, Payload: json.RawMessage(`{"query":"standup"}`)}}}, nil
		}
		return provider.GenerationResult{Text: "The standup is listed."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, mcpService)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Find standup."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if remoteCalls != 1 || requests != 2 || !server.Enabled {
		t.Fatalf("calls = %d, requests = %d, server = %#v", remoteCalls, requests, server)
	}
}

func TestPrimaryChatDisclosesOnlySelectedPrivateFieldsAfterReview(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	source := "Reference: INVOICE-42\nClient: Café 日本語 Workshop\nAccount: SECRET-ACCOUNT\nInternal note: do not disclose\nTotal: EUR 416.50\n"
	if err := original.home.WriteFile("private-source.md", []byte(source), 0o600); err != nil {
		t.Fatal(err)
	}
	var remoteCalls int
	var received struct {
		Recipient string   `json:"recipient"`
		Selected  []string `json:"selected_fields"`
		Packet    string   `json:"packet"`
	}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "disclosure", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "disclose_selected_fields", Description: "Send selected fields to an approved recipient",
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: false, IdempotentHint: false, OpenWorldHint: runtimeBool(true)}},
		func(_ context.Context, _ *mcpsdk.CallToolRequest, input struct {
			Recipient string   `json:"recipient"`
			Selected  []string `json:"selected_fields"`
			Packet    string   `json:"packet"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			remoteCalls++
			received.Recipient, received.Selected, received.Packet = input.Recipient, input.Selected, input.Packet
			return nil, map[string]any{"receipt": "disclosure-receipt-42"}, nil
		})
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	defer httpServer.Close()
	paths, err := home.FromRoot(original.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)
	setup, err := mcpService.Create(context.Background(), noemamcp.SetupInput{DisplayName: "Disclosure",
		TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("setup = %#v, %v", setup, err)
	}
	if _, err := mcpService.SaveConnectionPolicy(context.Background(), setup.Server.ID,
		setup.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := mcpService.Bindings(context.Background())
	if err != nil || len(bindings) < 2 {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	modelName := modelMCPToolName("Disclosure", binding.OperationToken)
	requests := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		if requests == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "disclose-1",
				ProviderName: modelName, Name: modelName, Payload: json.RawMessage(`{"recipient":"audit-recipient@example.test","selected_fields":["reference","client","total"],"packet":"INVOICE-42; Café 日本語 Workshop; EUR 416.50"}`)}}}, nil
		}
		return provider.GenerationResult{Text: "The approved disclosure was sent."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, mcpService)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID,
		Input: "Disclose only the approved invoice reference, client, and total."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if remoteCalls != 0 {
		t.Fatalf("remote calls before review = %d", remoteCalls)
	}
	pending, err := database.PendingActionRequests(context.Background(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != store.ActionAwaitingApproval {
		t.Fatalf("pending disclosure = %#v, %v", pending, err)
	}
	action := pending[0]
	if action.CapabilityName != modelName || action.OperationToken != binding.Name {
		t.Fatalf("review authority = %q/%q, want model/canonical %q/%q", action.CapabilityName, action.OperationToken, modelName, binding.Name)
	}
	if action.Arguments["recipient"] != "audit-recipient@example.test" || action.Arguments["packet"] != "INVOICE-42; Café 日本語 Workshop; EUR 416.50" {
		t.Fatalf("review arguments = %#v", action.Arguments)
	}
	selected, ok := action.Arguments["selected_fields"].([]any)
	if !ok || len(selected) != 3 || selected[0] != "reference" || selected[1] != "client" || selected[2] != "total" {
		t.Fatalf("review fields = %#v", action.Arguments["selected_fields"])
	}
	encoded, _ := json.Marshal(action.Arguments)
	if strings.Contains(string(encoded), "SECRET-ACCOUNT") || strings.Contains(string(encoded), "Internal note") {
		t.Fatalf("unselected private fields entered review: %s", encoded)
	}
	resolved, err := chat.ResolveActionRequest(context.Background(), action.ID, action.Revision, "human:local", "approve")
	if err != nil || resolved.State != store.ActionSucceeded {
		t.Fatalf("resolved disclosure = %#v, %v", resolved, err)
	}
	collectCompletedTurns(t, events, 1)
	if remoteCalls != 1 || received.Recipient != "audit-recipient@example.test" ||
		strings.Join(received.Selected, ",") != "reference,client,total" || received.Packet != "INVOICE-42; Café 日本語 Workshop; EUR 416.50" {
		t.Fatalf("remote disclosure = calls %d, %#v", remoteCalls, received)
	}
	unchanged, err := original.home.ReadFile("private-source.md")
	if err != nil || string(unchanged) != source {
		t.Fatalf("source changed = %q, %v", unchanged, err)
	}
}

func runtimeBool(value bool) *bool { return &value }

func TestMCPAuthenticationInterruptionIsDurableAndSkippable(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "mail", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "read", Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: true}},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"ok": true}, nil
		})
	handler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var unauthorized atomic.Bool
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if unauthorized.Load() {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		handler.ServeHTTP(w, request)
	}))
	defer httpServer.Close()
	paths, _ := home.FromRoot(original.home.Name())
	service, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	setup, err := service.Create(context.Background(), noemamcp.SetupInput{DisplayName: "Mail", TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("setup = %#v, %v", setup, err)
	}
	if _, err := service.SaveConnectionPolicy(context.Background(), setup.Server.ID, setup.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, _ := service.Bindings(context.Background())
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	modelName := modelMCPToolName("Mail", binding.OperationToken)
	unauthorized.Store(true)
	requests := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		if requests == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "auth-1", ProviderName: modelName, Name: modelName, Payload: json.RawMessage(`{}`)}}}, nil
		}
		return provider.GenerationResult{Text: "Authentication was skipped."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, service)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, _ := chat.Subscribe(context.Background(), conversation.ID)
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Read mail."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	pending, err := database.PendingMCPAuthRequests(context.Background(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != "awaiting_user" {
		t.Fatalf("pending = %#v, %v", pending, err)
	}
	if _, err := chat.SkipMCPAuthentication(context.Background(), pending[0].ID, 1); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	request, err := database.MCPAuthRequest(context.Background(), pending[0].ID, 1)
	if err != nil || request.State != "cancelled" || requests != 2 {
		t.Fatalf("request = %#v, calls = %d, %v", request, requests, err)
	}
}

func TestReviewedMCPAuthenticationSurvivesRestartAndCompletesAction(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "calendar", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "change", Annotations: &mcpsdk.ToolAnnotations{}},
		func(context.Context, *mcpsdk.CallToolRequest, struct{}) (*mcpsdk.CallToolResult, map[string]any, error) {
			return nil, map[string]any{"changed": true}, nil
		})
	handler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var unauthorized atomic.Bool
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if unauthorized.Load() {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		handler.ServeHTTP(w, request)
	}))
	defer httpServer.Close()
	paths, _ := home.FromRoot(original.home.Name())
	service, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	setup, err := service.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Calendar",
		TransportKind: "streamable_http", URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("setup = %#v, %v", setup, err)
	}
	if _, err := service.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision,
		0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, _ := service.Bindings(t.Context())
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	modelName := modelMCPToolName("Calendar", binding.OperationToken)
	unauthorized.Store(true)
	requests := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests++
		if requests == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "reviewed-auth",
				ProviderName: modelName, Name: modelName, Payload: json.RawMessage(`{}`)}}}, nil
		}
		return provider.GenerationResult{Text: "The calendar change was skipped."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, service)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := chat.Subscribe(t.Context(), conversation.ID)
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{ConversationID: conversation.ID, Input: "Change it."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	actions, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(actions) != 1 {
		t.Fatalf("pending actions = %#v, %v", actions, err)
	}
	if action, err := chat.ResolveActionRequest(t.Context(), actions[0].ID, 1, "human:local", "approve"); err != nil || action.State != store.ActionExecuting {
		t.Fatalf("suspended action = %#v, %v", action, err)
	}
	profile, err := database.RuntimeDebugProfile(t.Context(), store.RuntimeDebugScope{Kind: "conversation_turn", ID: actions[0].TurnID})
	if err != nil {
		t.Fatal(err)
	}
	preparation, execution := 0, 0
	for _, span := range profile.Spans {
		if span.Metadata.CorrelationID == "reviewed-auth" {
			if span.Metadata.Phase == "review_preparation" {
				preparation++
			}
			if span.Metadata.Phase == "execution" {
				execution++
			}
		}
	}
	if preparation != 1 || execution != 1 {
		t.Fatalf("reviewed tool phase counts = %d, %d", preparation, execution)
	}
	collectCompletedTurns(t, events, 1)
	pending, err := database.PendingMCPAuthRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].ActionID != actions[0].ID {
		t.Fatalf("pending authentication = %#v, %v", pending, err)
	}
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}
	restarted, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, service)
	if err != nil {
		t.Fatal(err)
	}
	defer restarted.Close()
	action, err := database.ActionRequest(t.Context(), actions[0].ID, 1)
	if err != nil || action.State != store.ActionExecuting {
		t.Fatalf("recovered action = %#v, %v", action, err)
	}
	recoveryEvents, _ := restarted.Subscribe(t.Context(), conversation.ID)
	<-recoveryEvents
	if _, err := restarted.SkipMCPAuthentication(t.Context(), pending[0].ID, 1); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, recoveryEvents, 1)
	action, err = database.ActionRequest(t.Context(), actions[0].ID, 1)
	if err != nil || action.State != store.ActionFailed || requests != 2 {
		t.Fatalf("finished action = %#v, requests = %d, %v", action, requests, err)
	}
}

func TestGovernedDownloadRequiresApprovalAndResumesExactChatCall(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	primary := make([]provider.GenerateRequest, 0, 2)
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) == 1 && request.Tools[0].Name == actionReviewToolName {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				ProviderCallID: "review-1", ProviderName: actionReviewToolName, Name: actionReviewToolName,
				Payload: json.RawMessage(`{"authorization":"absent","risk":"high","reason_codes":["authorization_absent"],"explanation":"The destination needs human approval."}`),
			}}}, nil
		}
		primary = append(primary, request)
		if len(primary) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "download-1", ProviderName: "download", Name: fileDownloadName,
				Payload: json.RawMessage(`{"url":"https://example.net/report.pdf","path":"report.pdf"}`),
			}}}, nil
		}
		return provider.GenerationResult{Text: "The download was declined."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Save the report if it is appropriate.",
	}); err != nil {
		t.Fatal(err)
	}
	proposalEvents := collectCompletedTurns(t, events, 1)
	if !containsEventKind(proposalEvents, EventHumanInterventionsChanged) {
		t.Fatalf("proposal events = %#v", proposalEvents)
	}
	pending, err := database.PendingActionRequests(context.Background(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 || pending[0].State != store.ActionAwaitingApproval {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
	resolved, err := chat.ResolveActionRequest(context.Background(), pending[0].ID, 1, "human:local", "decline")
	if err != nil || resolved.State != store.ActionDeclined {
		t.Fatalf("resolved action = %#v, %v", resolved, err)
	}
	resolutionEvents := collectCompletedTurns(t, events, 1)
	if !containsEventKind(resolutionEvents, EventHumanInterventionsChanged) {
		t.Fatalf("resolution events = %#v", resolutionEvents)
	}
	if len(primary) != 2 {
		t.Fatalf("primary requests = %d", len(primary))
	}
	var replay *provider.ReplayToolResult
	for _, message := range primary[1].Messages {
		if message.ToolResult != nil && message.ToolResult.ProviderCallID == "download-1" {
			replay = message.ToolResult
		}
	}
	if replay == nil || replay.Name != fileDownloadName || replay.Success {
		t.Fatalf("resumed tool result = %#v", replay)
	}
}

func TestCredentialDownloadIsRejectedBeforeDurableToolCall(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	calls := 0
	generator := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			ProviderCallID: "download-secret", ProviderName: "download", Name: fileDownloadName,
			Payload: json.RawMessage(`{"url":"https://alice:password@example.net/report","path":"report"}`),
		}}}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Download the report.",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	encoded, _ := json.Marshal(page.Items)
	if calls != 1 || strings.Contains(string(encoded), "password") {
		t.Fatalf("provider calls = %d, transcript = %s", calls, encoded)
	}
	for _, item := range page.Items {
		if item.Kind == store.ConversationToolCall || item.Kind == store.ConversationToolResult {
			t.Fatalf("credential call entered transcript: %#v", item)
		}
	}
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 0 {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
}

func TestRecoveredUncertainDownloadDoesNotResume(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	reviewer := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) == 1 && request.Tools[0].Name == actionReviewToolName {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				ProviderCallID: "review-recovery", ProviderName: actionReviewToolName, Name: actionReviewToolName,
				Payload: json.RawMessage(`{"authorization":"absent","risk":"high","reason_codes":["authorization_absent"],"explanation":"Approval is required."}`),
			}}}, nil
		}
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			ProviderCallID: "download-recovery", ProviderName: "download", Name: fileDownloadName,
			Payload: json.RawMessage(`{"url":"https://example.net/report","path":"report"}`),
		}}}, nil
	})
	chat, err := NewChat(database, reviewer, original.codex, original.openAI, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{ConversationID: conversation.ID, Input: "Download it."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	pending, err := database.PendingActionRequests(t.Context(), "human:local", &conversation.ID, nil, 10)
	if err != nil || len(pending) != 1 {
		t.Fatalf("pending actions = %#v, %v", pending, err)
	}
	action, err := database.DecideActionRequest(t.Context(), pending[0].ID, 1, "human:local", "approve", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err = database.ClaimActionRequest(t.Context(), action.ID, 1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}

	continuations := 0
	continuation := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		continuations++
		return provider.GenerationResult{Text: "unexpected"}, nil
	})
	restarted, err := NewChat(database, continuation, original.codex, original.openAI, original.home, original.memory)
	if err != nil {
		t.Fatal(err)
	}
	if err := restarted.Close(); err != nil {
		t.Fatal(err)
	}
	if continuations != 0 {
		t.Fatalf("recovery continuations = %d", continuations)
	}

	page, err := database.ConversationItemPage(t.Context(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	results := 0
	for _, item := range page.Items {
		if item.Kind == store.ConversationToolResult {
			results++
		}
	}
	if results != 1 {
		t.Fatalf("tool results = %d in %#v", results, page.Items)
	}
}

func containsEventKind(events []Event, kind EventKind) bool {
	for _, event := range events {
		if event.Kind == kind {
			return true
		}
	}
	return false
}

func TestMCPSetupPublishesInterventionAfterSavedResult(t *testing.T) {
	original, database, conversation := chatFixture(t)
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	var origin string
	remote := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/.well-known/mcp.json" {
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(map[string]any{"name": "Notes", "endpoint": origin + "/mcp"})
			return
		}
		w.WriteHeader(http.StatusUnauthorized)
	}))
	defer remote.Close()
	origin = remote.URL
	paths, err := home.FromRoot(original.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	service, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer service.Close()
	calls := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if calls == 1 {
			arguments, _ := json.Marshal(map[string]string{"service_url": origin})
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{ProviderCallID: "setup-notes", ProviderName: noemamcp.ConnectServiceToolName, Name: noemamcp.ConnectServiceToolName, Payload: arguments}}}, nil
		}
		return provider.GenerationResult{Text: "Connect your Notes account."}, nil
	})
	chat, err := NewChat(database, generator, original.codex, original.openAI, original.home, original.memory, service)
	if err != nil {
		t.Fatal(err)
	}
	defer chat.Close()
	events, err := chat.Subscribe(t.Context(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(t.Context(), SendTurnInput{ConversationID: conversation.ID, Input: "Connect Notes."}); err != nil {
		t.Fatal(err)
	}
	completed := collectCompletedTurns(t, events, 1)
	setups, err := database.PendingMCPSetupItems(t.Context(), conversation.ID, 10)
	if err != nil || len(setups) != 1 {
		t.Fatalf("pending setup count=%d: %v", len(setups), err)
	}
	saved := false
	for _, event := range completed {
		if event.Kind == EventConversationItem && event.Item != nil && event.Item.ID == setups[0].ID {
			saved = true
		}
		if event.Kind == EventHumanInterventionsChanged && event.ConversationID == conversation.ID && saved {
			return
		}
	}
	t.Fatal("saved MCP setup did not trigger the intervention refresh event")
}
