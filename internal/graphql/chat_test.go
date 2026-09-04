package graphql

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestPrimaryConversationServesEmptyReadyChat(t *testing.T) {
	resolver := openChatTestResolver(t)
	ctx := context.Background()
	if conversation, err := resolver.primaryConversation(ctx); err != nil || conversation != nil {
		t.Fatalf("fresh primary conversation = %#v, %v", conversation, err)
	}
	stored, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := resolver.primaryConversation(ctx)
	if err != nil || conversation == nil || conversation.ConversationID != stored.ID {
		t.Fatalf("primary conversation = %#v, %v", conversation, err)
	}
	if conversation.LatestTranscriptPage == nil || len(conversation.LatestTranscriptPage.Items) != 0 {
		t.Fatalf("latest transcript = %#v", conversation.LatestTranscriptPage)
	}
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	result := postGraphQL(t, server.URL, `
query ReadyChat($input: ConversationTranscriptPageInput!) {
  primaryConversation {
    conversationId
    provider
    latestTranscriptPage { items { itemId } pageInfo { beforeCursor hasMoreBefore } }
  }
  conversationTranscriptPage(input: $input) {
    items { itemId }
    pageInfo { beforeCursor hasMoreBefore }
  }
}`, map[string]any{"input": map[string]any{"conversationId": stored.ID}})
	primary := result.Data["primaryConversation"].(map[string]any)
	if primary["conversationId"] != stored.ID || primary["provider"] != "openrouter" {
		t.Fatalf("GraphQL primary conversation = %#v", primary)
	}
	page, err := resolver.conversationTranscriptPage(ctx, model.ConversationTranscriptPageInput{
		ConversationID: stored.ID,
	})
	if err != nil || len(page.Items) != 0 || page.PageInfo.HasMoreBefore {
		t.Fatalf("empty transcript page = %#v, %v", page, err)
	}

	streamCtx, cancel := context.WithCancel(ctx)
	events, err := resolver.conversationEvents(streamCtx, stored.ID)
	if err != nil {
		t.Fatal(err)
	}
	event := <-events
	ready, ok := event.(model.SubscriptionReadyEvent)
	if !ok || ready.ConversationID != stored.ID {
		t.Fatalf("first conversation event = %#v", event)
	}
	cancel()
	if _, open := <-events; open {
		t.Fatal("conversation event stream stayed open after disconnect")
	}
}

func TestConversationTurnStreamsPersistsAndReplays(t *testing.T) {
	resolver := openChatTestResolver(t)
	ctx := context.Background()
	secret, err := provider.NewSecret("graphql-chat-key")
	if err != nil {
		t.Fatal(err)
	}
	profiles := []provider.ModelProfile{{
		ID: "openai/gpt-5.6-luna", Label: "GPT-5.6 Luna",
		ReasoningEfforts: []string{"high"}, DefaultReasoningEffort: "high",
	}, {
		ID: "openai/gpt-5.6-sol", Label: "GPT-5.6 Sol",
		ReasoningEfforts: []string{"medium"}, DefaultReasoningEffort: "medium",
	}}
	account, err := resolver.ProviderAccounts.PublishVerifiedSecret(
		ctx, "provider_account:openrouter:default", 0, provider.AuthSecretInput,
		secret, profiles, time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{
			Role: role, ProviderKind: "openrouter", ProviderAccountID: account.ID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
		})
	}
	if created, err := resolver.Store.ConfirmHostedModelAssignments(ctx, account.ID, assignments); err != nil || !created {
		t.Fatalf("confirm model assignments = %t, %v", created, err)
	}
	conversation, err := resolver.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}

	requestBody := make(chan map[string]any, 1)
	replaceChatTransport(t, chatRoundTripFunc(func(request *http.Request) (*http.Response, error) {
		if request.Header.Get("Authorization") != "Bearer graphql-chat-key" {
			t.Fatalf("provider authorization = %q", request.Header.Get("Authorization"))
		}
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			return nil, err
		}
		requestBody <- body
		return &http.Response{
			StatusCode: http.StatusOK,
			Header:     http.Header{"Content-Type": {"text/event-stream"}},
			Body: io.NopCloser(strings.NewReader(
				"data: {\"id\":\"graphql-chat\",\"model\":\"openai/gpt-5.6-luna\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello \"}}]}\n\n" +
					"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"back\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\ndata: [DONE]\n\n",
			)),
			Request: request,
		}, nil
	}))

	eventContext, cancelEvents := context.WithCancel(ctx)
	defer cancelEvents()
	events, err := resolver.conversationEvents(eventContext, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := (<-events).(model.SubscriptionReadyEvent); !ok {
		t.Fatal("first Chat event was not readiness")
	}
	clientID, zone := "graphql-client-message", "America/New_York"
	accepted, err := resolver.sendConversationTurn(ctx, model.SendConversationTurnInput{
		ConversationID: conversation.ID, Input: "Hello", ClientMessageID: &clientID,
		ClientTimeZone: &zone,
	})
	if err != nil || accepted.ClientMessageID == nil || *accepted.ClientMessageID != clientID {
		t.Fatalf("accepted turn = %#v, %v", accepted, err)
	}

	wantTypes := []string{
		"status:INPUT_RECEIVED", "status:THINKING", "item:user", "delta", "delta",
		"item:assistant", "status:IDLE", "completed",
	}
	gotTypes := make([]string, 0, len(wantTypes))
	deadline := time.After(5 * time.Second)
	for len(gotTypes) < len(wantTypes) {
		select {
		case event := <-events:
			switch value := event.(type) {
			case model.AgentStatusEvent:
				gotTypes = append(gotTypes, "status:"+value.Status.String())
			case model.ConversationItemEvent:
				switch value.Item.(type) {
				case model.UserText:
					gotTypes = append(gotTypes, "item:user")
				case model.AssistantText:
					gotTypes = append(gotTypes, "item:assistant")
				}
			case model.AssistantTextDeltaEvent:
				gotTypes = append(gotTypes, "delta")
			case model.TurnCompletedEvent:
				gotTypes = append(gotTypes, "completed")
			}
		case <-deadline:
			t.Fatalf("Chat events timed out after %v", gotTypes)
		}
	}
	if strings.Join(gotTypes, ",") != strings.Join(wantTypes, ",") {
		t.Fatalf("Chat event order = %v, want %v", gotTypes, wantTypes)
	}
	body := <-requestBody
	if body["model"] != "openai/gpt-5.6-luna" || body["prompt_cache_key"] != conversation.ID ||
		body["max_completion_tokens"] != float64(8192) {
		t.Fatalf("provider request = %#v", body)
	}

	page, err := resolver.conversationTranscriptPage(ctx, model.ConversationTranscriptPageInput{
		ConversationID: conversation.ID,
	})
	if err != nil || len(page.Items) != 2 {
		t.Fatalf("stored transcript = %#v, %v", page, err)
	}
	firstID, secondID := page.Items[0].ItemID, page.Items[1].ItemID
	if err := resolver.Chat.Close(); err != nil {
		t.Fatal(err)
	}
	generator, err := provider.NewOpenRouterGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	resolver.Chat, err = noemaruntime.NewChat(resolver.Store, generator, resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	replayed, err := resolver.conversationTranscriptPage(ctx, model.ConversationTranscriptPageInput{
		ConversationID: conversation.ID,
	})
	if err != nil || len(replayed.Items) != 2 || replayed.Items[0].ItemID != firstID || replayed.Items[1].ItemID != secondID {
		t.Fatalf("replayed transcript = %#v, %v", replayed, err)
	}
}

func TestConversationTranscriptValidatesOwnershipAndLimit(t *testing.T) {
	resolver := openTestResolver(t)
	limit := 201
	if _, err := resolver.conversationTranscriptPage(context.Background(), model.ConversationTranscriptPageInput{
		ConversationID: "conversation:00000000000000000000000000000000", Limit: &limit,
	}); err == nil {
		t.Fatal("oversized transcript page limit succeeded")
	}
	limit = 1
	if _, err := resolver.conversationTranscriptPage(context.Background(), model.ConversationTranscriptPageInput{
		ConversationID: "conversation:00000000000000000000000000000000", Limit: &limit,
	}); err == nil {
		t.Fatal("unknown conversation read succeeded")
	}
}

func TestConversationToolActivitiesPreserveStoredIdentityAndStatus(t *testing.T) {
	payload := map[string]any{
		"id": "tool_call:one", "activity_kind": "tool_call",
		"title": "Tool call: task.inspect", "summary": "task.inspect",
		"metadata": map[string]any{"provider": "openrouter"},
	}
	started, err := transcriptItemModel(store.ConversationItem{
		Kind: store.ConversationToolCall, Status: "running", Payload: payload,
	})
	if err != nil {
		t.Fatal(err)
	}
	activity, ok := started.(model.Activity)
	if !ok || activity.ID != "tool_call:one" || activity.ActivityKind != "tool_call" ||
		activity.Status != model.TurnActivityStatusStarted || activity.Summary == nil ||
		*activity.Summary != "task.inspect" {
		t.Fatalf("started activity = %#v", started)
	}
	payload["id"] = "tool_result:one"
	payload["activity_kind"] = "tool_result"
	payload["title"] = "Tool result: task.inspect"
	completed, err := transcriptItemModel(store.ConversationItem{
		Kind: store.ConversationToolResult, Status: "completed", Payload: payload,
	})
	if err != nil {
		t.Fatal(err)
	}
	activity, ok = completed.(model.Activity)
	if !ok || activity.ID != "tool_result:one" || activity.Status != model.TurnActivityStatusCompleted {
		t.Fatalf("completed activity = %#v", completed)
	}
	if status, err := activityStatusModel("cancelled"); err != nil || status != model.TurnActivityStatusFailed {
		t.Fatalf("cancelled activity status = %q, %v", status, err)
	}
}

func openChatTestResolver(t *testing.T) *Resolver {
	t.Helper()
	resolver := openProviderTestResolver(t)
	generator, err := provider.NewOpenRouterGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	chat, err := noemaruntime.NewChat(resolver.Store, generator, resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	resolver.Chat = chat
	t.Cleanup(func() { _ = resolver.Chat.Close() })
	return resolver
}

type chatRoundTripFunc func(*http.Request) (*http.Response, error)

func (function chatRoundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return function(request)
}

func replaceChatTransport(t *testing.T, transport http.RoundTripper) {
	t.Helper()
	previous := http.DefaultTransport
	http.DefaultTransport = transport
	t.Cleanup(func() { http.DefaultTransport = previous })
}
