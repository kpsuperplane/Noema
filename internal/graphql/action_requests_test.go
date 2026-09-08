package graphql

import (
	"context"
	"encoding/json"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

func TestPendingHumanInterventionsReturnsGovernedActions(t *testing.T) {
	resolver := openChatTestResolver(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 4, 0, 0, 0, time.UTC)
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "/workspace", now)
	if err != nil {
		t.Fatal(err)
	}
	setupTurn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Connect it.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	setupItems, err := resolver.Store.StartConversationToolRound(ctx, setupTurn, store.ConversationToolRound{
		Provider: "openrouter", Call: store.ConversationToolCallInput{ProviderCallID: "setup-1", ProviderName: "mcp.connect_service", Name: "mcp.connect_service", Arguments: json.RawMessage(`{"service_url":"https://example.test"}`)},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	setupPayload := json.RawMessage(`{"status":"needs_auth","service_url":"https://example.test/","display_name":"Example","endpoint_url":"https://mcp.example.test/","setup_result":{"setup_status":"needs_auth","discovered_tool_count":1}}`)
	if _, err = resolver.Store.FinishConversationToolCall(ctx, setupTurn, store.ConversationToolResultInput{
		CallItemID: setupItems[len(setupItems)-1].ID, Provider: "openrouter", ProviderCallID: "setup-1", ProviderName: "mcp.connect_service", Name: "mcp.connect_service", Success: true, Payload: setupPayload,
	}, now); err != nil {
		t.Fatal(err)
	}
	if _, err = resolver.Store.CompleteConversationTurn(ctx, setupTurn, "Authentication is required.", "Authentication is required.", nil, now); err != nil {
		t.Fatal(err)
	}
	turn, _, err := resolver.Store.BeginConversationTurn(ctx, conversation.ID, "Download it.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := resolver.Store.StartConversationToolRound(ctx, turn, store.ConversationToolRound{
		Provider: "openrouter", Call: store.ConversationToolCallInput{
			ProviderCallID: "official-read", ProviderName: "download", Name: "file.download",
			Arguments: json.RawMessage(`{"url":"https://example.net/report","path":"report"}`),
		},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	action, err := resolver.Store.CreateActionRequest(ctx, store.NewActionRequest{
		ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: items[len(items)-1].ID,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:primary",
		CapabilityName: "file.download", OperationToken: "file.download", ReviewRoute: store.ActionLLMReview,
		Behavior:    store.ActionBehavior{OpenWorld: true},
		Arguments:   json.RawMessage(`{"url":"https://example.net/report","path":"report"}`),
		InputSchema: json.RawMessage(`{"type":"object"}`), AuthorizationContext: map[string]any{"cwd": "/workspace"},
		SafeSummary: "Download a public file",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	action, _, err = resolver.Store.RecordActionAssessment(ctx, action.ID, 1, store.ActionAssessment{
		Status: "completed", Authorization: "absent", Risk: "high",
		ReviewerSelection: map[string]any{"model": "reviewer"},
		ReasonCodes:       []string{"authorization_absent"}, Explanation: "Approval is required.",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
	t.Cleanup(server.Close)
	result := postGraphQL(t, server.URL, `
query Pending($conversationId: String!) {
  pendingHumanInterventions(conversationId: $conversationId, first: 1) {
    __typename
    ... on GovernedAction { actionId capabilityName state }
  }
}`, map[string]any{"conversationId": conversation.ID})
	interventions := result.Data["pendingHumanInterventions"].([]any)
	if len(interventions) != 1 {
		t.Fatalf("pending interventions = %#v", interventions)
	}
	projected := interventions[0].(map[string]any)
	if projected["__typename"] != "GovernedAction" || projected["actionId"] != action.ID ||
		projected["capabilityName"] != "file.download" || projected["state"] != "AWAITING_APPROVAL" {
		t.Fatalf("pending intervention = %#v", projected)
	}
	browser := action
	browser.CapabilityName = "web.browse.interact"
	browser.Arguments = map[string]any{"snapshot_revision": float64(1), "ref": "e2", "action": "click"}
	browser.AuthorizationContext = map[string]any{"browser_review_context": map[string]any{
		"kind": "browser_interaction", "target": map[string]any{"name": "Submit"}}}
	browserModel, err := resolver.actionRequestModel(ctx, browser)
	if err != nil || browserModel.BrowserSessionAvailable == nil || *browserModel.BrowserSessionAvailable ||
		browserModel.Arguments["kind"] != "browser_interaction" {
		t.Fatalf("browser action projection = %#v, %v", browserModel, err)
	}
}
