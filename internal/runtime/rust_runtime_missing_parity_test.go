package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func TestRustRuntime_native_provider_can_call_web_fetch_and_continue(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/continuation_and_web.rs::native_provider_can_call_web_fetch_and_continue.
	chat, database, conversation := chatFixture(t)
	accounts, err := provider.NewAccountService(chat.home.Name(), database)
	if err != nil {
		t.Fatal(err)
	}
	if err := accounts.Initialize(t.Context(), time.Now()); err != nil {
		t.Fatal(err)
	}
	web, err := webtool.New(database, accounts, nil, nil, chat.home.Name(), "", 2, 1024)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(web.Close)
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.Header().Set("Content-Type", "application/json")
		_, _ = writer.Write([]byte(`{"success":true,"data":{"markdown":"Test page content","url":"https://example.com/page"}}`))
	}))
	t.Cleanup(server.Close)
	if err := web.SetEndpoint("firecrawl", server.URL); err != nil {
		t.Fatal(err)
	}
	if err := database.SaveWebProviderBinding(t.Context(), webtool.FetchName, "provider_account:firecrawl:public", time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := database.ObserveURLs(t.Context(), "search_result", "tool_call:test_search", []string{"https://example.com/page"}, time.Now()); err != nil {
		t.Fatal(err)
	}
	chat.web = web
	var requests []provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "call_fetch", ProviderName: webtool.FetchName,
				Name: webtool.FetchName, Payload: json.RawMessage(`{"url":"https://example.com/page","reason":"read the page"}`),
			}}}, nil
		}
		return provider.GenerationResult{Text: "I read the fetched page."}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Fetch the example page."}); err != nil {
		t.Fatal(err)
	}
	turnEvents := collectCompletedTurns(t, events, 1)
	if len(requests) < 2 || !rustRuntimeEventsContainAssistantText(turnEvents, "I read the fetched page.") {
		t.Fatalf("web fetch continuation = requests=%d events=%#v", len(requests), turnEvents)
	}
	var fetched bool
	for _, request := range requests {
		for _, message := range request.Messages {
			if message.ToolResult == nil || message.ToolResult.Name != webtool.FetchName || !message.ToolResult.Success {
				continue
			}
			var payload map[string]any
			if json.Unmarshal(message.ToolResult.Payload, &payload) == nil && payload["content"] == "Test page content" {
				fetched = true
			}
		}
	}
	if !fetched {
		t.Errorf("web.fetch result was not successful: %#v", requests)
	}
}

func TestRustRuntime_a2ui_action_resolution_emits_settled_projection_and_correlates_provider_resume(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/continuation_and_web.rs::a2ui_action_resolution_emits_settled_projection_and_correlates_provider_resume.
	chat, database, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	a2uiPayload, err := json.Marshal(map[string]string{"jsonl": validA2UIForm})
	if err != nil {
		t.Fatal(err)
	}
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "call_a2ui_form", ProviderName: presentA2UIName,
				Name: presentA2UIName, Payload: a2uiPayload,
			}}}, nil
		}
		return provider.GenerationResult{Text: "A2UI action continued"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "show the form"}); err != nil {
		t.Fatal(err)
	}
	initial := collectCompletedTurns(t, events, 1)
	var interactionID string
	for _, event := range initial {
		if event.Item == nil || event.Item.Kind != store.ConversationA2UICard {
			continue
		}
		if value, ok := event.Item.Payload["payload"].(map[string]any); ok {
			interactionID, _ = value["interaction_id"].(string)
		}
	}
	if interactionID == "" {
		t.Fatalf("pending A2UI interaction = %#v", initial)
	}
	model := map[string]any{"form": map[string]any{"name": "Ada"}}
	forged := map[string]any{"source": "forged", "model": model}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1, "main", "submit", "submit", forged, model, nil); err == nil || !strings.Contains(err.Error(), "context") {
		t.Errorf("forged A2UI context result = %v", err)
	}
	contextValue := map[string]any{"source": "surface", "model": model}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1, "main", "submit", "submit", contextValue, model, nil); err != nil {
		t.Fatal(err)
	}
	settled := collectCompletedTurns(t, events, 1)
	if !eventHasA2UILifecycle(settled, "answered") || !rustRuntimeEventsContainAssistantText(settled, "A2UI action continued") {
		t.Fatalf("settled A2UI events = %#v", settled)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	var settledProjection bool
	for _, item := range page.Items {
		if item.Kind != store.ConversationA2UICard {
			continue
		}
		payload, _ := item.Payload["payload"].(map[string]any)
		if payload["interaction_id"] == interactionID && payload["lifecycle"] == "answered" {
			settledProjection = true
			if !bytes.Contains(mustJSON(payload), []byte(`"name":"Ada"`)) {
				t.Errorf("settled data model = %#v", payload)
			}
		}
	}
	if !settledProjection {
		t.Errorf("answered A2UI projection was not persisted: %#v", page.Items)
	}
	if len(requests) < 2 {
		t.Errorf("A2UI action did not resume the provider: %d requests", len(requests))
	}
	if _, err := chat.SendA2UIAction(context.Background(), conversation.ID, interactionID, 1, "main", "submit", "submit", contextValue, model, nil); err == nil || !strings.Contains(err.Error(), "stale") {
		t.Errorf("stale A2UI action result = %v", err)
	}
}

func TestRustRuntime_native_provider_can_create_local_artifact_with_two_versions_and_continue(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/continuation_and_web.rs::native_provider_can_create_local_artifact_with_two_versions_and_continue.
	chat, database, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	artifactPayload, err := json.Marshal(map[string]any{
		"title": "Report", "artifact_kind": "document", "filename": "report.md", "media_type": "text/markdown",
		"versions": []any{map[string]any{"content": "# One\n"}, map[string]any{"title": "Final", "content": "# Two\n"}},
	})
	if err != nil {
		t.Fatal(err)
	}
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "artifact-call", ProviderName: artifactCreateLocalName,
				Name: artifactCreateLocalName, Payload: artifactPayload,
			}}}, nil
		}
		return provider.GenerationResult{Text: "I created the two-version artifact."}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Create a small artifact and revise it once."}); err != nil {
		t.Fatal(err)
	}
	turnEvents := collectCompletedTurns(t, events, 1)
	if !rustRuntimeEventsContainAssistantText(turnEvents, "I created the two-version artifact.") {
		t.Fatalf("artifact response = %#v", turnEvents)
	}
	artifacts, err := database.ArtifactsForOwner(context.Background(), store.ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(artifacts) != 1 {
		t.Errorf("artifact count = %d, want one: %#v", len(artifacts), artifacts)
	} else {
		if len(artifacts[0].Versions) != 2 || artifacts[0].CurrentVersion.Index != 2 {
			t.Errorf("artifact versions = %#v", artifacts[0])
		}
	}
	var created bool
	for _, request := range requests {
		for _, message := range request.Messages {
			if message.ToolResult == nil || message.ToolResult.Name != artifactCreateLocalName || !message.ToolResult.Success {
				continue
			}
			var payload map[string]any
			if json.Unmarshal(message.ToolResult.Payload, &payload) == nil && payload["current_version_index"] == float64(2) {
				created = true
			}
		}
	}
	if !created {
		t.Errorf("artifact tool result was not successful: %#v", requests)
	}
}

func TestRustRuntime_hard_ceiling_and_audit_failure_get_one_no_tools_finalization_attempt(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/continuation_and_web.rs::hard_ceiling_and_audit_failure_get_one_no_tools_finalization_attempt.
	for _, reason := range []string{"hard ceiling", "progress audit failed"} {
		instructions := toolFinalizationInstruction(reason)
		if !strings.Contains(instructions, "must stop now") {
			t.Fatalf("%s finalization instruction = %q", reason, instructions)
		}
		if strings.Contains(instructions, taskDelegateName) || strings.Contains(instructions, webtool.FetchName) {
			t.Fatalf("%s finalization exposed tools = %q", reason, instructions)
		}
	}
}

func TestRustRuntime_runtime_turn_streams_durable_assistant_item_and_idle_status(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::runtime_turn_streams_durable_assistant_item_and_idle_status.
	chat, database, conversation := chatFixture(t)
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, onEvent func(provider.StreamEvent)) (provider.GenerationResult, error) {
		onEvent(provider.StreamEvent{Kind: provider.TextDelta, Delta: "fake answer"})
		return provider.GenerationResult{Text: "fake answer"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "hello"}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	firstDelta, assistant := -1, -1
	for index, event := range all {
		if event.Kind == EventConversationItem && event.Item != nil && event.Item.Kind == store.ConversationAssistantText && event.Item.Status == "running" && firstDelta < 0 {
			firstDelta = index
		}
		if event.Kind == EventConversationItem && event.Item != nil && event.Item.Kind == store.ConversationAssistantText && event.Item.Status == "completed" && assistant < 0 {
			assistant = index
		}
	}
	if firstDelta < 0 || assistant < 0 || firstDelta >= assistant {
		t.Fatalf("delta and durable assistant order = %d, %d; events=%#v", firstDelta, assistant, all)
	}
	statuses := map[AgentStatus]bool{}
	for _, event := range all {
		if event.Kind == EventAgentStatus {
			statuses[event.Status] = true
		}
	}
	if !statuses[AgentStatusInputReceived] || !statuses[AgentStatusThinking] || !statuses[AgentStatusIdle] {
		t.Fatalf("status lifecycle = %#v", statuses)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	var durable bool
	for _, item := range page.Items {
		if item.Kind == store.ConversationAssistantText && item.ContentText == "fake answer" && item.Status == "completed" {
			durable = true
		}
	}
	if !durable {
		t.Fatalf("durable assistant replay = %#v", page.Items)
	}
}

func TestRustRuntime_runtime_turn_passes_conversation_id_to_provider_request(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::runtime_turn_passes_conversation_id_to_provider_request.
	chat, _, conversation := chatFixture(t)
	var request provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, current provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		request = current
		return provider.GenerationResult{Text: "answer"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "hello"}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if request.ConversationID != conversation.ID || request.ToolTransport != provider.ToolTransportNative {
		t.Fatalf("provider request identity = %#v", request)
	}
}

func TestRustRuntime_exact_reset_command_persists_notice_without_calling_provider(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::exact_reset_command_persists_notice_without_calling_provider.
	chat, database, conversation := chatFixture(t)
	var calls int
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		return provider.GenerationResult{Text: "provider called"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "/reset"}); err != nil {
		t.Fatal(err)
	}
	turnEvents := collectCompletedTurns(t, events, 1)
	if calls != 0 {
		t.Errorf("exact reset called provider %d times", calls)
	}
	var resetEvent bool
	for _, event := range turnEvents {
		if event.Kind != EventConversationItem || event.Item == nil || event.Item.Kind != store.ConversationActivity {
			continue
		}
		if event.Item.Status != string(store.ConversationItemCompleted) || event.Item.Payload["activity_kind"] != "context_reset" || event.Item.Payload["title"] != "Context reset" {
			t.Errorf("reset event = %#v", event.Item)
		}
		resetEvent = true
	}
	if !resetEvent {
		t.Fatalf("reset activity event = %#v", turnEvents)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 1 || page.Items[0].Payload["activity_kind"] != "context_reset" {
		t.Errorf("reset replay = %#v", page.Items)
	}
}

func TestRustRuntime_reset_with_arguments_remains_a_normal_provider_turn(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::reset_with_arguments_remains_a_normal_provider_turn.
	chat, _, conversation := chatFixture(t)
	var calls int
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		return provider.GenerationResult{Text: "normal"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "/reset now"}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if calls != 1 {
		t.Fatalf("argument reset provider calls = %d", calls)
	}
}

func TestRustRuntime_turn_persists_native_multiple_choice_tool_call(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::turn_persists_native_multiple_choice_tool_call.
	chat, database, conversation := chatFixture(t)
	var calls int
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if calls == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "choice-call", ProviderName: presentMultipleChoiceName,
				Name: presentMultipleChoiceName, Payload: json.RawMessage(`{"prompt":"Which?","selection_mode":"pick_one","options":[{"id":"ship","label":"Ship"}]}`),
			}}}, nil
		}
		return provider.GenerationResult{Text: "selected"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "choose"}); err != nil {
		t.Fatal(err)
	}
	initial := collectCompletedTurns(t, events, 1)
	var promptID string
	for _, event := range initial {
		if event.Item != nil && event.Item.Kind == store.ConversationMultipleChoicePrompt {
			promptID = event.Item.ID
		}
	}
	if promptID == "" {
		t.Fatalf("multiple-choice prompt missing: %#v", initial)
	}
	if _, err := chat.SendMultipleChoiceSelection(context.Background(), conversation.ID, promptID, []string{"ship"}, nil); err != nil {
		t.Fatal(err)
	}
	selection := collectCompletedTurns(t, events, 1)
	if !rustRuntimeEventsContainKind(selection, store.ConversationMultipleChoiceSelection) {
		t.Fatalf("selection event missing: %#v", selection)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	var call, result bool
	for _, item := range page.Items {
		call = call || item.Kind == store.ConversationToolCall && item.Payload["metadata"] != nil
		result = result || item.Kind == store.ConversationToolResult
	}
	if !call || !result {
		t.Errorf("multiple-choice durable call/result = %t/%t, items=%#v", call, result, page.Items)
	}
}

func TestRustRuntime_primary_agent_preferences_route_model_and_reasoning_by_provider_kind(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/conversation_turns.rs::primary_agent_preferences_route_model_and_reasoning_by_provider_kind.
	chat, _, _ := chatFixture(t)
	assignment, err := chat.primaryAssignment(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if assignment.ProviderKind != "openrouter" || assignment.ModelProfile == "" {
		t.Fatalf("primary assignment = %#v", assignment)
	}
	for _, providerKind := range []string{"openrouter", "codex", "openai"} {
		if _, err := chat.generatorFor(providerKind); err != nil {
			t.Errorf("provider %s was not routable: %v", providerKind, err)
		}
	}
}

func rustRuntimeEventsContainAssistantText(events []Event, text string) bool {
	for _, event := range events {
		if event.Item != nil && event.Item.Kind == store.ConversationAssistantText && event.Item.ContentText == text {
			return true
		}
	}
	return false
}

func rustRuntimeEventsContainKind(events []Event, kind store.ConversationItemKind) bool {
	for _, event := range events {
		if event.Item != nil && event.Item.Kind == kind {
			return true
		}
	}
	return false
}

func TestRustRuntime_update_own_name_continuation_keeps_the_foreground_tool_catalog_stable(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/identity_and_memory.rs::update_own_name_continuation_keeps_the_foreground_tool_catalog_stable.
	chat, _, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Index: 0, ProviderCallID: "name", ProviderName: updateOwnNameToolName, Name: updateOwnNameToolName, Payload: json.RawMessage(`{"name":"Mira"}`)}}}, nil
		}
		return provider.GenerationResult{Text: "Mira it is."}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Your name is Mira."}); err != nil {
		t.Fatal(err)
	}
	turnEvents := collectCompletedTurns(t, events, 1)
	if !rustRuntimeEventsContainAssistantText(turnEvents, "Mira it is.") || len(requests) != 2 {
		t.Fatalf("name continuation = requests=%d events=%#v", len(requests), turnEvents)
	}
	names := func(request provider.GenerateRequest) map[string]bool {
		result := make(map[string]bool)
		for _, tool := range request.Tools {
			result[tool.Name] = true
		}
		return result
	}
	first, second := names(requests[0]), names(requests[1])
	if len(first) != len(second) {
		t.Errorf("foreground tool catalog changed across name continuation: %d != %d", len(first), len(second))
	}
	if !second[updateOwnNameToolName] {
		t.Errorf("name tool disappeared from continuation: %#v", second)
	}
}

func TestRustRuntime_update_own_name_tool_history_is_visible_before_later_turns(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/identity_and_memory.rs::update_own_name_tool_history_is_visible_before_later_turns.
	chat, database, conversation := chatFixture(t)
	var calls int
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if calls == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Index: 0, ProviderCallID: "name", ProviderName: updateOwnNameToolName, Name: updateOwnNameToolName, Payload: json.RawMessage(`{"name":"Momo"}`)}}}, nil
		}
		return provider.GenerationResult{Text: "yay acknowledged after saved name"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Let's rename you to Momo"}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Yay"}); err != nil {
		t.Fatal(err)
	}
	second := collectCompletedTurns(t, events, 1)
	agent, err := database.Agent(context.Background(), store.PrimaryAgentID)
	if err != nil {
		t.Fatal(err)
	}
	if agent.DisplayName == nil || *agent.DisplayName != "Momo" {
		t.Fatalf("saved agent name = %#v", agent.DisplayName)
	}
	if !rustRuntimeEventsContainAssistantText(second, "yay acknowledged after saved name") {
		t.Fatalf("later turn = %#v", second)
	}
}

func TestRustRuntime_runtime_prompt_includes_stored_agent_name_after_update(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/identity_and_memory.rs::runtime_prompt_includes_stored_agent_name_after_update.
	chat, _, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Index: 0, ProviderCallID: "name", ProviderName: updateOwnNameToolName, Name: updateOwnNameToolName, Payload: json.RawMessage(`{"name":"Mira"}`)}}}, nil
		}
		return provider.GenerationResult{Text: "saw stored identity"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Your name is Mira."}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "What is your name?"}); err != nil {
		t.Fatal(err)
	}
	second := collectCompletedTurns(t, events, 1)
	if !rustRuntimeEventsContainAssistantText(second, "saw stored identity") || len(requests) < 3 {
		t.Fatalf("identity turn = requests=%d events=%#v", len(requests), second)
	}
	encoded, _ := json.Marshal(requests[len(requests)-1].Messages)
	if !bytes.Contains(encoded, []byte("Mira")) {
		t.Errorf("stored name was absent from provider context: %s", encoded)
	}
}

func TestRustRuntime_compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text.
	database := contextTestStore(t, 4_000)
	completed := make([]provider.GenerationMessage, 8)
	for index := range completed {
		completed[index] = provider.GenerationMessage{Role: "assistant", Content: strings.Repeat(string(rune('a'+index)), 1_000)}
	}
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 0 || request.ToolTransport != provider.ToolTransportNone || request.ToolChoice != provider.ToolChoiceNone {
			t.Errorf("compaction controls = %#v", request)
		}
		return provider.GenerationResult{Text: "Summary: compacted checkpoint facts."}, nil
	})
	messages, compacted, err := prepareModelContext(context.Background(), modelContextRequest{
		database: database, generator: generator, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		base: []provider.GenerationMessage{{Role: "developer", Content: "runtime instructions"}}, completed: completed,
		active: []provider.GenerationMessage{{Role: "user", Content: "post checkpoint user"}}, outputReserve: 512,
	})
	if err != nil || !compacted {
		t.Fatalf("context compaction = %t, %v", compacted, err)
	}
	if messages[1].Role != "assistant" || !strings.Contains(messages[1].Content, "Summary: compacted checkpoint facts") || strings.Contains(messages[0].Content, "Summary: compacted checkpoint facts") {
		t.Fatalf("summary was not an input checkpoint: %#v", messages)
	}
	if messages[len(messages)-1].Content != "post checkpoint user" {
		t.Fatalf("post-checkpoint input was lost: %#v", messages)
	}
}

func TestRustRuntime_context_reset_excludes_prior_transcript_and_compacted_summary(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::context_reset_excludes_prior_transcript_and_compacted_summary.
	chat, _, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		return provider.GenerationResult{Text: "answer"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	for _, input := range []string{"old private question", "/reset", "new question"} {
		if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: input}); err != nil {
			t.Fatal(err)
		}
		collectCompletedTurns(t, events, 1)
	}
	if len(requests) == 0 {
		t.Fatal("no provider request captured")
	}
	encoded, _ := json.Marshal(requests[len(requests)-1].Messages)
	if bytes.Contains(encoded, []byte("old private question")) {
		t.Errorf("context reset replayed old transcript: %s", encoded)
	}
}

func TestRustRuntime_runtime_omits_reasoning_and_tool_replay_after_hosted_search(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::runtime_omits_reasoning_and_tool_replay_after_hosted_search.
	call := rustRuntimeToolItem(store.ConversationToolCall, "call:search", 2, "search", webtool.SearchName, map[string]any{"query": "current trains"}, false)
	call.TurnID = "turn:old"
	result := rustRuntimeToolItem(store.ConversationToolResult, "result:search", 3, "search", webtool.SearchName, map[string]any{"success": true, "payload": map[string]any{"summary": "Found schedules"}}, false)
	result.ParentItemID = call.ID
	result.TurnID = "turn:old"
	call.Metadata["provider_round"] = 0
	result.Metadata["provider_round"] = 0
	callMetadata := call.Payload["metadata"].(map[string]any)
	callAction := callMetadata["action"].(map[string]any)
	callAction["hosted_web_search"] = true
	reasoning := rustRuntimeConversationItem(store.ConversationReasoning, "reasoning:old", 1, "")
	reasoning.TurnID = "turn:old"
	reasoning.Metadata["provider"] = "openrouter"
	reasoning.Payload["provider_details"] = []any{map[string]any{
		"type": "reasoning.server_tool_call", "name": webtool.SearchName, "result": map[string]any{"summary": "Found schedules"},
	}}
	messages, err := providerMessagesFromItems([]store.ConversationItem{reasoning, call, result}, "turn:new", "openrouter")
	if err != nil {
		t.Fatal(err)
	}
	encoded, _ := json.Marshal(messages)
	if bytes.Contains(encoded, []byte("opaque reasoning")) || bytes.Contains(encoded, []byte("Found schedules")) {
		t.Errorf("old hosted search or reasoning was replayed: %s", encoded)
	}
}

func TestRustRuntime_prompt_context_keeps_all_post_checkpoint_items_for_budgeting(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::prompt_context_keeps_all_post_checkpoint_items_for_budgeting.
	completed := make([]provider.GenerationMessage, 45)
	for index := range completed {
		completed[index] = provider.GenerationMessage{Role: "user", Content: "post checkpoint item " + itoa(index+1)}
	}
	database := contextTestStore(t, 20_000)
	messages, compacted, err := prepareModelContext(context.Background(), modelContextRequest{
		database: database, generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			return provider.GenerationResult{Text: "answer"}, nil
		}),
		accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", completed: completed,
		active: []provider.GenerationMessage{{Role: "user", Content: "current turn"}}, outputReserve: 512,
	})
	if err != nil || compacted {
		t.Fatalf("post-checkpoint budgeting = compacted %t, %v", compacted, err)
	}
	encoded, _ := json.Marshal(messages)
	if !bytes.Contains(encoded, []byte("post checkpoint item 1")) || !bytes.Contains(encoded, []byte("post checkpoint item 45")) {
		t.Fatalf("post-checkpoint items were dropped: %s", encoded)
	}
}

func TestRustRuntime_prompt_context_falls_back_to_estimates_when_token_count_fails(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::prompt_context_falls_back_to_estimates_when_token_count_fails.
	messages := []provider.GenerationMessage{{Role: "user", Content: "hello"}}
	if CountModelContext(context.Background(), nil, messages, nil, false) == 0 {
		t.Fatal("token estimate was zero when tokenizer was unavailable")
	}
	database := contextTestStore(t, 20_000)
	if _, _, err := prepareModelContext(context.Background(), modelContextRequest{database: database, generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{Text: "answer"}, nil
	}), accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test", active: messages, outputReserve: 512}); err != nil {
		t.Fatalf("estimate fallback failed: %v", err)
	}
}

func TestRustRuntime_prompt_context_sends_prior_transcript_as_provider_messages(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/prompt_context.rs::prompt_context_sends_prior_transcript_as_provider_messages.
	items := []store.ConversationItem{
		rustRuntimeConversationItem(store.ConversationUserText, "user:1", 1, "first durable question"),
		rustRuntimeConversationItem(store.ConversationAssistantText, "assistant:1", 2, "first durable answer"),
		rustRuntimeConversationItem(store.ConversationUserText, "user:2", 3, "second durable question"),
	}
	messages, err := providerMessagesFromItems(items, "", "openrouter")
	if err != nil {
		t.Fatal(err)
	}
	if len(messages) != 3 || messages[0].Role != "user" || messages[0].Content != "first durable question" || messages[1].Role != "assistant" || messages[1].Content != "first durable answer" || messages[2].Content != "second durable question" {
		t.Fatalf("provider transcript = %#v", messages)
	}
}

func TestRustRuntime_start_primary_conversation_generates_initial_name_onboarding_message(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/replay_and_tools.rs::start_primary_conversation_generates_initial_name_onboarding_message.
	chat, database, conversation := chatFixture(t)
	session := &sessionTestGenerator{closed: make(chan struct{}, 1)}
	session.generate = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 0 || request.ToolTransport != provider.ToolTransportNone || !strings.Contains(request.Messages[0].Content, "newly started primary conversation") {
			t.Fatalf("initial onboarding request = %#v", request)
		}
		return provider.GenerationResult{Model: "openai/gpt-5.6-luna", Text: "hey, i’m glad to be here with you 👋\n---\ni can help you think, plan, make, untangle, and keep life moving with a little more ease\n---\nwhat would you like to name me?"}, nil
	})
	chat.openRouter = session
	if err := chat.StartPrimaryConversation(context.Background(), conversation.ID); err != nil {
		t.Fatal(err)
	}
	items, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 30)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range items.Items {
		if item.Kind == store.ConversationUserText {
			t.Errorf("initial onboarding created a fake user item: %#v", item)
		}
	}
	if session.direct != 0 || session.opens != 1 || session.closes != 1 {
		t.Fatal("initial welcome bypassed the provider session and its request policy")
	}
	got := make([]string, 0, 3)
	for _, item := range items.Items {
		if item.Kind == store.ConversationAssistantText {
			got = append(got, item.ContentText)
		}
	}
	want := []string{"hey, i’m glad to be here with you 👋", "i can help you think, plan, make, untangle, and keep life moving with a little more ease", "what would you like to name me?"}
	if len(got) != len(want) {
		t.Fatalf("initial onboarding assistant messages = %#v", got)
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("initial onboarding assistant messages = %#v, want %#v", got, want)
		}
	}
	if _, err := chat.modelEnvironment(context.Background(), conversation, time.UTC, time.Now()); err != nil {
		t.Fatal(err)
	}
}

func TestRustRuntime_failed_initial_name_onboarding_logs_runtime_invariant(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/replay_and_tools.rs::failed_initial_name_onboarding_logs_runtime_invariant.
	chat, database, conversation := chatFixture(t)
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{Model: "openai/gpt-5.6-luna"}, nil
	})
	firstError := chat.StartPrimaryConversation(context.Background(), conversation.ID)
	secondError := chat.StartPrimaryConversation(context.Background(), conversation.ID)
	if firstError == nil || secondError == nil || firstError.Error() != secondError.Error() || !strings.Contains(firstError.Error(), "initial onboarding response did not include assistant text") {
		t.Fatalf("failed onboarding errors = %v, %v", firstError, secondError)
	}
	environment, err := chat.modelEnvironment(context.Background(), conversation, time.UTC, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(environment[0].Content, "update_own_name") || !strings.Contains(environment[0].Content, "do not have a name") {
		t.Errorf("unnamed onboarding authority = %+v", environment)
	}
	items, err := database.ConversationProviderItems(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 0 {
		t.Errorf("failed onboarding left provider items = %#v", items)
	}
}

func TestRustRuntime_runtime_rejects_mixed_delegation_batch_without_executing_any_call(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/replay_and_tools.rs::runtime_rejects_mixed_delegation_batch_without_executing_any_call.
	chat, database, conversation := chatFixture(t)
	calls := 0
	chat.openRouter = generatorFunc(func(_ context.Context, req provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if calls > 1 {
			results := 0
			for _, message := range req.Messages {
				if message.ToolResult != nil {
					results++
				}
			}
			if results != 2 {
				t.Errorf("continuation received %d tool results", results)
			}
			return provider.GenerationResult{Text: "I could not combine delegation with another tool."}, nil
		}
		return provider.GenerationResult{Text: "I started the task and renamed myself.", ToolCalls: []provider.GenerationToolCall{
			{Index: 0, ProviderCallID: "call_task_mixed", ProviderName: taskDelegateName, Name: taskDelegateName, Payload: json.RawMessage(`{"title":"Mixed task","task_document":"Complete Mixed task and report the result.","project":{"kind":"none"},"execution_intent":{"request_markdown":"Complete Mixed task and report the result.","complexity":"simple"}}`)},
			{Index: 1, ProviderCallID: "call_name_mixed", ProviderName: updateOwnNameToolName, Name: updateOwnNameToolName, Payload: json.RawMessage(`{"name":"Mira"}`)},
		}}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Delegate the task and rename yourself."}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	tasks, err := database.ListTasks(context.Background(), store.TaskListFilter{Scope: "all"}, 100, nil)
	if err != nil || len(tasks.Tasks) != 0 {
		t.Fatalf("mixed batch created tasks: %#v, %v", tasks, err)
	}
	agent, err := database.Agent(context.Background(), "agent:primary")
	if err != nil || agent.DisplayName != nil {
		t.Errorf("mixed batch changed agent: %#v, %v", agent, err)
	}
	for _, event := range all {
		if event.Item != nil && strings.HasPrefix(event.Item.ID, "transient:tool_call:") {
			t.Error("mixed batch published a tool start")
		}
	}
	var texts []string
	failed := 0
	for _, item := range page.Items {
		if item.Kind == store.ConversationAssistantText {
			texts = append(texts, item.ContentText)
		}
		if item.Kind == store.ConversationToolResult && item.Status == "failed" {
			failed++
			action, _ := nestedAction(item.Payload)
			payload, _ := action["payload"].(map[string]any)
			if payload["error"] != "task_delegate_mixed_tool_batch" {
				t.Errorf("wrong rejection: %#v", item.Payload)
			}
		}
	}
	if failed != 2 || len(texts) != 2 || texts[0] != "I started the task and renamed myself." || texts[1] != "I could not combine delegation with another tool." {
		t.Errorf("mixed batch results: %d failures, texts=%q", failed, texts)
	}

}

func TestRustRuntime_runtime_actor_persists_provider_tool_items_before_turn_failure(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/replay_and_tools.rs::runtime_actor_persists_provider_tool_items_before_turn_failure.
	chat, database, conversation := chatFixture(t)
	var calls int
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if calls == 1 {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Index: 0, ProviderCallID: "search_call", ProviderName: noemamemory.SearchToolName, Name: noemamemory.SearchToolName, Payload: json.RawMessage(`{"query":"ordinary"}`)}}}, nil
		}
		return provider.GenerationResult{}, errors.New("provider failure")
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Use your tool"}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 40)
	if err != nil {
		t.Fatal(err)
	}
	var persisted bool
	for _, item := range page.Items {
		persisted = persisted || item.Kind == store.ConversationToolCall && strings.Contains(string(mustJSON(item.Payload)), noemamemory.SearchToolName)
	}
	if !persisted {
		t.Errorf("provider tool call was lost before failure: events=%#v items=%#v", all, page.Items)
	}
	status := false
	for _, event := range all {
		status = status || event.Kind == EventAgentStatus && event.Status == AgentStatusError
	}
	if !status {
		t.Errorf("provider failure did not publish error status: %#v", all)
	}
}

func TestRustRuntime_runtime_executes_every_homogeneous_delegation_and_uses_provider_handoff_narration(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/replay_and_tools.rs::runtime_executes_every_homogeneous_delegation_and_uses_provider_handoff_narration.
	chat, database, conversation := chatFixture(t)
	var calls int
	delegate := func(index int, id, title string) provider.GenerationToolCall {
		return provider.GenerationToolCall{Index: index, ProviderCallID: id, ProviderName: taskDelegateName, Name: taskDelegateName, Payload: mustToolJSON(t, map[string]any{
			"title": title, "task_document": "Complete " + title + " and report the result.", "project": map[string]any{"kind": "none"},
			"execution_intent": map[string]any{"request_markdown": "Complete " + title + " and report the result.", "complexity": "simple"},
		})}
	}
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, emit func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		emit(provider.StreamEvent{Kind: provider.TextDelta, Delta: "I started all three background tasks. They are underway."})
		// Go combines Rust's two text items. Separate reply items remain an open port gap.
		return provider.GenerationResult{Text: "I started all three background tasks. They are underway.", ToolCalls: []provider.GenerationToolCall{
			delegate(0, "call_task_canada", "Research Canada"), delegate(1, "call_task_usa", "Research USA"),
			{Index: 2, ProviderCallID: "call_task_invalid", ProviderName: taskDelegateName, Name: taskDelegateName, Payload: json.RawMessage(`{"title":"Invalid task"}`)},
		}}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "Start the Canada and USA research tasks."}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 60)
	if err != nil {
		t.Fatal(err)
	}
	if !rustRuntimeEventsContainAssistantText(all, "I started all three background tasks. They are underway.") {
		t.Errorf("provider handoff narration = %#v", all)
	}
	list, err := database.ListTasks(context.Background(), store.TaskListFilter{Scope: "all"}, 100, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(list.Tasks) != 2 {
		t.Errorf("homogeneous delegation created %d tasks, page=%#v", len(list.Tasks), page.Items)
	}
	if calls != 1 {
		t.Errorf("delegation requested %d provider responses", calls)
	}
	created := map[string]bool{}
	for _, task := range list.Tasks {
		created[task.SourceToolCallID] = true
	}
	if !created["call_task_canada"] || !created["call_task_usa"] || created["call_task_invalid"] {
		t.Errorf("creation calls: %#v", created)
	}
	for _, item := range page.Items {
		if item.Kind == "task_reference" || item.Metadata["source"] == "task_delegation_receipt" {
			t.Errorf("unexpected receipt: %#v", item)
		}
	}

}

func TestRustRuntime_runtime_actor_allocates_distinct_conversation_ids(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::runtime_actor_allocates_distinct_conversation_ids.
	firstChat, _, first := chatFixture(t)
	secondChat, _, second := chatFixture(t)
	_ = firstChat
	_ = secondChat
	if first.ID == second.ID || !strings.HasPrefix(first.ID, "conversation:") || !strings.HasPrefix(second.ID, "conversation:") {
		t.Fatalf("conversation allocation = %q, %q", first.ID, second.ID)
	}
}

func TestRustRuntime_runtime_handle_generate_once_does_not_block_subsequent_commands(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::runtime_handle_generate_once_does_not_block_subsequent_commands.
	chat, _, conversation := chatFixture(t)
	started := make(chan struct{})
	chat.openRouter = generatorFunc(func(ctx context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		close(started)
		<-ctx.Done()
		return provider.GenerationResult{}, ctx.Err()
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "slow"}); err != nil {
		t.Fatal(err)
	}
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("provider did not start")
	}
	start := time.Now()
	if _, err := chat.Subscribe(context.Background(), conversation.ID); err != nil {
		t.Fatal(err)
	}
	if time.Since(start) > 100*time.Millisecond {
		t.Errorf("subsequent command waited for provider: %s", time.Since(start))
	}
}

func TestRustRuntime_capability_setup_completions_narrate_once_per_connection_revision(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::capability_setup_completions_narrate_once_per_connection_revision.
	chat, database, conversation := chatFixture(t)
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) != 0 || request.ToolTransport != provider.ToolTransportNone || request.HostedWebSearch {
			t.Errorf("capability narration provider controls = %#v", request)
		}
		return provider.GenerationResult{Text: "fake answer"}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	now := time.Now()
	if err := database.RecordCapabilityReady(context.Background(), "api", "Gmail", "connection:gmail", "revision:one", nil, 2, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RecordCapabilityReady(context.Background(), "api", "Gmail", "connection:gmail", "revision:one", nil, 2, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RecordCapabilityReady(context.Background(), "mcp", "Notion", "connection:notion", "revision:one", nil, 8, now); err != nil {
		t.Fatal(err)
	}
	collected := collectCompletedTurns(t, events, 2)
	if len(collected) == 0 {
		t.Fatal("capability setup did not narrate")
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 30)
	if err != nil {
		t.Fatal(err)
	}
	count := 0
	for _, item := range page.Items {
		if item.Kind == store.ConversationAssistantText && item.Metadata["source"] == "capability_setup" {
			count++
		}
	}
	if count != 2 {
		t.Errorf("capability setup narration count = %d, items=%#v", count, page.Items)
	}
}

func TestRustRuntime_capability_setup_narration_projects_provider_citation_markers(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::capability_setup_narration_projects_provider_citation_markers.
	chat, _, conversation := chatFixture(t)
	marker := "\ue200cite\ue202source\ue201"
	chat.openRouter = generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{Text: "Connected " + marker, Citations: []provider.Citation{{Title: "Source", URL: "https://example.com/source"}}}, nil
	})
	write, prompt, mapped, err := chat.primaryNotification(conversation, store.WorkEvent{Kind: "capability.ready", Payload: map[string]any{"integration_kind": "api", "integration_name": "Example", "connection_id": "example-connection", "connection_revision": "1", "enabled_tool_count": 1}})
	if err != nil || !mapped || write.Source != "capability_setup" || !strings.Contains(prompt, "Example") {
		t.Fatalf("capability notification mapping = %#v, %q, %t, %v", write, prompt, mapped, err)
	}
	text, _, err := chat.narratePrimaryNotification(conversation, prompt)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(text, "Connected ") {
		t.Fatalf("capability narration = %q", text)
	}
}

func TestRustRuntime_notification_delivery_waits_for_foreground_turn_and_publishes_exact_item(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::notification_delivery_waits_for_foreground_turn_and_publishes_exact_item.
	chat, database, conversation := chatFixture(t)
	// This test owns notification delivery; stop the automatic event consumer.
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}
	chat.ctx = context.Background()
	taskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	created, err := database.CreateTaskWithOptions(context.Background(), taskID, "Waiting notification", runtimeTaskCommand("notification-task", taskID), store.TaskCreateOptions{InitialRunKind: "executor", ExecutionComplexity: "simple"}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	task := created.Task
	if _, err := home.CreatePendingTaskDocument(chat.home, task.ID, "Wait for the human."); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, task.ID); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("notification run claim = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := database.BlockTaskExecution(context.Background(), run.ID, run.Generation, "clarification", "Which value?", "Choose the required value.", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	task, err = database.Task(context.Background(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "foreground", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := database.CancelConversationTurn(context.Background(), turn, time.Now()); err != nil {
		t.Fatal(err)
	}
	cursor, err := database.PrimaryTaskNotificationCursor(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	event := store.WorkEvent{ID: cursor + 1, EventID: "event:waiting", WorkspaceID: "workspace:personal", Kind: "gate.opened", TaskID: task.ID, RunID: run.ID, Payload: map[string]any{"gate_id": task.ActiveGateID, "gate_kind": "clarification"}}
	write, prompt, mapped, err := chat.primaryNotification(conversation, event)
	if err != nil || !mapped || write.Source != "work_notification" || prompt == "" {
		t.Fatalf("notification mapping = %#v, %q, %t, %v", write, prompt, mapped, err)
	}
	write.Text = "The Task is waiting."
	write.Metadata = map[string]any{"notification_kind": "task_waiting", "notification_id": "notification:one", "work_notification": map[string]any{"task_id": task.ID}}
	items, err := database.CommitPrimaryNotification(context.Background(), write, time.Now())
	if err != nil || len(items) != 2 || items[0].Kind != store.ConversationAssistantText || items[0].ContentText != write.Text || items[1].Kind != store.ConversationTaskReference {
		t.Fatalf("notification assistant item = %#v, %v", items, err)
	}
	repeated, err := database.CommitPrimaryNotification(context.Background(), write, time.Now())
	if err != nil || len(repeated) != 0 {
		t.Fatalf("duplicate notification = %#v, %v", repeated, err)
	}
}

func TestRustRuntime_task_supervisor_starts_distinct_tasks_concurrently(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::task_supervisor_starts_distinct_tasks_concurrently.
	chat, database, conversation := chatFixture(t)
	raw := json.RawMessage(`{"title":"Concurrent task","task_document":"Do the work","project":{"kind":"none"}}`)
	var wait sync.WaitGroup
	results := make(chan bool, 2)
	for index := 0; index < 2; index++ {
		index := index
		wait.Add(1)
		go func() {
			defer wait.Done()
			_, ok := chat.executeChatTool(context.Background(), conversation, taskDelegateName, raw, "concurrent-"+itoa(index), "turn:concurrent")
			results <- ok
		}()
	}
	wait.Wait()
	close(results)
	for ok := range results {
		if !ok {
			t.Errorf("concurrent task delegation failed")
		}
	}
	list, err := database.ListTasks(context.Background(), store.TaskListFilter{Scope: "all"}, 20, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(list.Tasks) != 2 {
		t.Errorf("concurrent task count = %d", len(list.Tasks))
	}
}

func TestRustRuntime_background_task_pins_local_provider_generation_across_replacement(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::background_task_pins_local_provider_generation_across_replacement.
	chat, _, _ := chatFixture(t)
	old := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{Text: "old generation"}, nil
	})
	newGenerator := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{Text: "replacement generation"}, nil
	})
	chat.openRouter = old
	pinned := chat.openRouter
	chat.openRouter = newGenerator
	first, err := pinned.Generate(context.Background(), provider.GenerateRequest{Model: "old-model"}, func(provider.StreamEvent) {})
	if err != nil || first.Text != "old generation" {
		t.Fatalf("pinned provider response = %#v, %v", first, err)
	}
	current, err := chat.openRouter.Generate(context.Background(), provider.GenerateRequest{Model: "new-model"}, func(provider.StreamEvent) {})
	if err != nil || current.Text != "replacement generation" {
		t.Fatalf("replacement provider response = %#v, %v", current, err)
	}
}

func TestRustRuntime_runtime_shutdown_cancels_generate_once_and_inline_turn(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/tests/runtime_lifecycle.rs::runtime_shutdown_cancels_generate_once_and_inline_turn.
	chat, _, conversation := chatFixture(t)
	started := make(chan struct{})
	cancelled := make(chan struct{})
	chat.openRouter = generatorFunc(func(ctx context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		close(started)
		<-ctx.Done()
		close(cancelled)
		return provider.GenerationResult{}, ctx.Err()
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{ConversationID: conversation.ID, Input: "slow"}); err != nil {
		t.Fatal(err)
	}
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("blocked provider did not start")
	}
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}
	select {
	case <-cancelled:
	case <-time.After(time.Second):
		t.Fatal("shutdown did not cancel provider")
	}
}

func TestRustRuntime_a2ui_validation_and_reduction_cases(t *testing.T) {
	// Rust source: crates/noema-runtime/tests/a2ui_validation.rs::a2ui_validation_and_reduction_cases.
	valid := `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1","sendDataModel":true}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"submit","component":"Button","child":"label","action":{"event":{"name":"submit"}}},{"id":"label","component":"Text","text":"Save"},{"id":"root","component":"Column","children":["label","submit"]}]}}
{"version":"v0.9.1","updateDataModel":{"surfaceId":"main","path":"/form/name","value":"Ada"}}`
	batch, repair := reduceA2UI("conversation/one", valid)
	if repair != nil || batch == nil || len(batch.Surfaces) != 1 {
		t.Fatalf("valid A2UI reduction = %#v, %#v", batch, repair)
	}
	var surface *a2uiSurface
	for _, value := range batch.Surfaces {
		surface = value
	}
	if surface == nil {
		t.Fatalf("valid A2UI surface = %#v", surface)
	}
	dataModel, _ := surface.DataModel.(map[string]any)
	form, _ := dataModel["form"].(map[string]any)
	if surface.SurfaceID != "main" || !strings.Contains(surface.NamespacedSurfaceID, "conversation%2Fone") || form["name"] != "Ada" || len(surface.Actions) != 1 || surface.Actions[0].Name != "submit" {
		t.Fatalf("valid A2UI surface = %#v", surface)
	}
	cases := []struct {
		name  string
		jsonl string
		code  string
	}{
		{"unsupported_version", `{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}`, "unsupported_version"},
		{"ordering", `{"version":"v0.9.1","updateDataModel":{"surfaceId":"main","path":"/","value":{}}}`, "invalid_ordering"},
		{"single_surface_protocol_subset", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","createSurface":{"surfaceId":"secondary","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}`, "bounds_exceeded"},
		{"reference_and_root", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Card","child":"missing"}]}}`, "invalid_reference"},
		{"unreachable_action", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Text","text":"Visible"},{"id":"hidden","component":"Button","child":"hidden_label","action":{"event":{"name":"hidden"}}},{"id":"hidden_label","component":"Text","text":"Hidden"}]}}`, "invalid_reference"},
		{"interactive_input_requires_binding", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1","sendDataModel":true}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"field","component":"TextField","label":"Name","value":"Ada"},{"id":"submit","component":"Button","child":"label","action":{"event":{"name":"submit"}}},{"id":"label","component":"Text","text":"Save"},{"id":"root","component":"Column","children":["field","submit"]}]}}`, "invalid_data_path"},
		{"interactive_input_requires_synchronized_model", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"field","component":"TextField","label":"Name","value":{"path":"/form/name"}},{"id":"submit","component":"Button","child":"label","action":{"event":{"name":"submit"}}},{"id":"label","component":"Text","text":"Save"},{"id":"root","component":"Column","children":["field","submit"]}]}}`, "invalid_protocol"},
		{"unsafe_html", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Text","text":"<script>alert(1)</script>"}]}}`, "unsafe_content"},
		{"client_function", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Button","child":"label","action":{"functionCall":{"call":"openUrl"}}},{"id":"label","component":"Text","text":"Go"}]}}`, "unsafe_content"},
		{"unsupported_media", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Image","url":"https://example.test/a.png"}]}}`, "unsupported_catalog"},
		{"invalid_path", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Text","text":"x"}]}}
{"version":"v0.9.1","updateDataModel":{"surfaceId":"main","path":"/bad~2path","value":true}}`, "invalid_data_path"},
		{"catalog_required_field", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Text"}]}}`, "invalid_component"},
		{"catalog_dynamic_type", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"CheckBox","label":"Ready","value":[]}]}}`, "invalid_component"},
		{"catalog_unknown_field", `{"version":"v0.9.1","createSurface":{"surfaceId":"main","catalogId":"com.noema.a2ui/catalog/v0.9.1"}}
{"version":"v0.9.1","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Divider","extra":true}]}}`, "invalid_protocol"},
	}
	for _, test := range cases {
		batch, repair := reduceA2UI("conversation/one", test.jsonl)
		if repair == nil || batch != nil || repair.Status != "VALIDATION_FAILED" || repair.Code != test.code {
			t.Errorf("%s: reduction = %#v, %#v; want %s", test.name, batch, repair, test.code)
		}
	}
}
