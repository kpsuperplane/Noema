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
	"github.com/kpsuperplane/noema/internal/home"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestToolActivityRetriesKeepSeparateDisplayIDs(t *testing.T) {
	seen := map[string]bool{}
	for round := 0; round < 2; round++ {
		callID := store.ConversationOutputID("turn:test", "tool_call", round, 0)
		for _, kind := range []string{"tool_call", "tool_result"} {
			action := map[string]any{"name": "mcp.docs.update", "id": "old-call", "call_id": "old-call"}
			display := map[string]any{"description": "I will retry the update."}
			status := "running"
			if kind == "tool_result" {
				status = "completed"
				if round == 0 {
					status = "failed"
				}
			}
			item := store.ConversationItem{
				ID: store.ConversationOutputID("turn:test", kind, round, 0), ParentItemID: callID,
				TurnID: "turn:test", Kind: store.ConversationItemKind(kind), Status: status,
				Metadata: map[string]any{"provider_round": float64(round), "output_index": float64(0)},
				Payload: map[string]any{"id": kind + ":old", "activity_kind": kind, "title": "Update",
					"metadata": map[string]any{"action": action, "display": display}},
			}
			value, err := transcriptItemModel(item)
			if err != nil {
				t.Fatal(err)
			}
			activity := value.(model.Activity)
			shown := activity.Metadata["display"].(map[string]any)
			if shown["description"] != nil || shown["name"] == "" || display["description"] != "I will retry the update." {
				t.Fatal("tool label repeats commentary or changed source history")
			}
			want := item.ID
			if activity.ID != want || seen[activity.ID] {
				t.Fatalf("attempt IDs collided: %q, want %q", activity.ID, want)
			}
			seen[activity.ID] = true
			key := "id"
			if kind == "tool_result" {
				key = "call_id"
			}
			if activity.Metadata["action"].(map[string]any)[key] != callID || action[key] != "old-call" {
				t.Fatal("call pairing or original history changed")
			}
			wantStatus := strings.ToUpper(status)
			if kind == "tool_call" {
				wantStatus = "STARTED"
			}
			if string(activity.Status) != wantStatus {
				t.Fatal("retry result status changed")
			}
			if kind == "tool_call" {
				item.Kind = store.ConversationActivity
				item.Payload["id"] = "transient-call"
				streamed, err := transcriptItemModel(item)
				if err != nil || streamed.(model.Activity).ID != activity.ID {
					t.Fatal("streamed call does not match saved call")
				}
			}
		}
	}
}

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
	server := httptest.NewServer(rustAPIAuthenticatedHandler(resolver))
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
				`data: {"id":"graphql-chat","model":"openai/gpt-5.6-luna","choices":[{"index":0,"delta":{"reasoning_details":[{"type":"reasoning.summary","summary":"Hidden planning summary","index":0}]}}]}` + "\n\n" +
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
		"status:INPUT_RECEIVED", "status:THINKING", "item:user",
		"item:assistant", "status:IDLE", "completed",
	}
	gotTypes := make([]string, 0, len(wantTypes))
	deadline := time.After(5 * time.Second)
	for len(gotTypes) == 0 || gotTypes[len(gotTypes)-1] != "completed" {
		select {
		case event := <-events:
			switch value := event.(type) {
			case model.AgentStatusEvent:
				gotTypes = append(gotTypes, "status:"+value.Status.String())
			case model.ConversationItemEvent:
				if value.Item == nil {
					t.Fatal("live stream emitted an empty item that violates the GraphQL contract")
				}
				switch value.Item.(type) {
				case model.UserText:
					gotTypes = append(gotTypes, "item:user")
				case model.AssistantText:
					if gotTypes[len(gotTypes)-1] != "item:assistant" {
						gotTypes = append(gotTypes, "item:assistant")
					}
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
	saved, err := resolver.Store.ConversationItemPage(ctx, conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	foundSummary := false
	for _, item := range saved.Items {
		if item.ContentText == "Hidden planning summary" {
			foundSummary = true
		}
	}
	if !foundSummary {
		t.Fatal("test provider did not produce the hidden summary")
	}
	firstID, secondID := page.Items[0].ItemID, page.Items[1].ItemID
	if err := resolver.Chat.Close(); err != nil {
		t.Fatal(err)
	}
	generator, err := provider.NewOpenRouterGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	codexGenerator, err := provider.NewCodexGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	resolver.Chat, err = noemaruntime.NewChat(
		resolver.Store, generator, codexGenerator, codexGenerator, resolver.home, resolver.Memory,
	)
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

func TestConversationActivitiesPreserveCancellationAndAuthentication(t *testing.T) {
	if status, err := activityStatusModel("cancelled"); err != nil || status != model.TurnActivityStatusFailed {
		t.Fatalf("cancelled activity status = %q, %v", status, err)
	}
	legacy, err := transcriptItemModel(store.ConversationItem{
		Kind: store.ConversationApprovalRequest, Status: "completed",
		Payload:  map[string]any{"id": "mcp_auth:one", "activity_kind": "authentication_request", "title": "Authentication required"},
		Metadata: map[string]any{"source": "mcp_server_authentication"},
	})
	if err != nil {
		t.Fatal(err)
	}
	if activity, ok := legacy.(model.Activity); !ok || activity.Metadata["source"] != "mcp_server_authentication" {
		t.Fatalf("legacy activity metadata = %#v", legacy)
	}
}

func TestConversationMultipleChoiceItemsPreserveOrderedContract(t *testing.T) {
	options := []map[string]any{{"id": "b", "label": "Beta"}, {"id": "a", "label": "Alpha"}}
	promptValue, err := transcriptItemModel(store.ConversationItem{
		Kind:    store.ConversationMultipleChoicePrompt,
		Payload: map[string]any{"prompt": "Which?", "selection_mode": "pick_many", "options": options},
	})
	if err != nil {
		t.Fatal(err)
	}
	prompt, ok := promptValue.(model.MultipleChoicePrompt)
	if !ok || prompt.SelectionMode != model.MultipleChoiceSelectionModePickMany ||
		len(prompt.Options) != 2 || prompt.Options[0].ID != "b" {
		t.Fatalf("multiple-choice prompt = %#v", promptValue)
	}
	selectionValue, err := transcriptItemModel(store.ConversationItem{
		Kind: store.ConversationMultipleChoiceSelection,
		Payload: map[string]any{
			"prompt_item_id": "item:prompt", "selection_mode": "pick_many", "selected_options": options,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	selection, ok := selectionValue.(model.MultipleChoiceSelection)
	if !ok || selection.PromptItemID != "item:prompt" || len(selection.SelectedOptions) != 2 ||
		selection.SelectedOptions[1].ID != "a" {
		t.Fatalf("multiple-choice selection = %#v", selectionValue)
	}
}

func TestConversationNotificationReferencesPreserveClientContract(t *testing.T) {
	resolver := openChatTestResolver(t)
	taskID := "task:0123456789abcdef0123456789abcdef"
	if _, err := home.CreatePendingTaskDocument(resolver.home, taskID, "Task notes"); err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.CreateTask(context.Background(), taskID, "Notify Chat", "correlation:test", time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(resolver.home, taskID); err != nil {
		t.Fatal(err)
	}
	taskValue, err := resolver.conversationItemModel(context.Background(), store.ConversationItem{Kind: store.ConversationTaskReference,
		Payload: map[string]any{"task_id": taskID}})
	if err != nil {
		t.Fatal(err)
	}
	task, ok := taskValue.Item.(model.TaskReference)
	if !ok || task.TaskID != taskID || task.Task == nil || task.Task.Title != "Notify Chat" {
		t.Fatalf("Task reference = %#v", taskValue)
	}
	artifactValue, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationArtifactReference,
		Payload: map[string]any{"artifact_id": "artifact:one", "artifact_version_id": "artifact_version:one",
			"title": "Result", "artifact_kind": "report", "storage_kind": "local_file",
			"download_url": "/artifacts/versions/one/download", "media_type": "text/plain"}})
	if err != nil {
		t.Fatal(err)
	}
	artifact, ok := artifactValue.(model.ArtifactReference)
	if !ok || artifact.ArtifactVersionID == nil || *artifact.ArtifactVersionID != "artifact_version:one" ||
		artifact.DownloadURL == nil || *artifact.DownloadURL != "/artifacts/versions/one/download" {
		t.Fatalf("Artifact reference = %#v", artifactValue)
	}
}

func TestConversationA2UISurfacePreservesClientContract(t *testing.T) {
	interactionID := "interaction:test"
	value, err := transcriptItemModel(store.ConversationItem{Kind: store.ConversationA2UICard, Payload: map[string]any{
		"id": "a2ui:test", "schema": "a2ui.v0.9.1", "interaction_state": "pending",
		"payload": map[string]any{"interaction_id": interactionID, "interaction_revision": float64(1), "lifecycle": "pending",
			"catalog": map[string]any{"catalog_id": "com.noema.a2ui/catalog/v0.9.1"}, "surfaces": map[string]any{
				"main": map[string]any{"surface_id": "main", "version": "v0.9.1", "revision": float64(3),
					"components": map[string]any{"root": map[string]any{"component": "TextField", "variant": "obscured"}},
					"data_model": map[string]any{"private": "kept"}, "actions": []any{map[string]any{"name": "submit"}},
				},
			},
		},
	}})
	if err != nil {
		t.Fatal(err)
	}
	surface, ok := value.(model.A2UISurface)
	if !ok || surface.InteractionID == nil || *surface.InteractionID != interactionID ||
		surface.InteractionRevision == nil || *surface.InteractionRevision != 1 || surface.Revision != 3 ||
		!surface.HasActions || surface.Snapshot["data_model"].(map[string]any)["private"] != "kept" {
		t.Fatalf("A2UI GraphQL surface = %#v", value)
	}
}

func TestA2UIActionInputPreservesArrayDataModel(t *testing.T) {
	input, err := new(executionContext).unmarshalInputProviderInteractionActionInput(context.Background(), map[string]any{
		"conversationId": "conversation:test", "interactionId": "interaction:test", "expectedRevision": 1,
		"surfaceId": "main", "sourceComponentId": "submit", "actionName": "submit",
		"dataModel": []any{"first", map[string]any{"private": "kept"}},
	})
	modelValue, ok := input.DataModel.Value.([]any)
	if err != nil || !ok || len(modelValue) != 2 || modelValue[1].(map[string]any)["private"] != "kept" {
		t.Fatalf("A2UI action data model = %#v, %v", input.DataModel.Value, err)
	}
}

func openChatTestResolver(t *testing.T) *Resolver {
	t.Helper()
	resolver := openProviderTestResolver(t)
	generator, err := provider.NewOpenRouterGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	codexGenerator, err := provider.NewCodexGenerator(resolver.ProviderAccounts)
	if err != nil {
		t.Fatal(err)
	}
	nativeMemory, err := noemamemory.New(resolver.home)
	if err != nil {
		t.Fatal(err)
	}
	resolver.Memory = nativeMemory
	t.Cleanup(func() { _ = nativeMemory.Close() })
	chat, err := noemaruntime.NewChat(
		resolver.Store, generator, codexGenerator, codexGenerator, resolver.home, nativeMemory,
	)
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
