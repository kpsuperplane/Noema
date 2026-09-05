package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestChatSerializesDetachedTurnsAndPublishesOrderedEvents(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	if ready := <-events; ready.Kind != EventSubscriptionReady {
		t.Fatalf("first event = %s", ready.Kind)
	}

	firstStarted := make(chan struct{})
	releaseFirst := make(chan struct{})
	requests := make(chan map[string]any, 2)
	var requestMu sync.Mutex
	requestCount := 0
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			return nil, err
		}
		requests <- body
		requestMu.Lock()
		requestCount++
		current := requestCount
		requestMu.Unlock()
		if current == 1 {
			close(firstStarted)
			select {
			case <-releaseFirst:
			case <-request.Context().Done():
				return nil, request.Context().Err()
			}
		}
		return openRouterStreamResponse("reply " + string(rune('0'+current))), nil
	}))

	clientOne, zone := "client-one", "America/Los_Angeles"
	requestContext, cancelRequest := context.WithCancel(context.Background())
	if _, err := chat.SendTurn(requestContext, SendTurnInput{
		ConversationID: conversation.ID, Input: "first", ClientMessageID: &clientOne,
		ClientTimeZone: &zone,
	}); err != nil {
		t.Fatal(err)
	}
	<-firstStarted
	cancelRequest()
	clientTwo := "client-two"
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "second", ClientMessageID: &clientTwo,
	}); err != nil {
		t.Fatal(err)
	}
	var firstRequest map[string]any
	select {
	case firstRequest = <-requests:
	default:
		t.Fatal("first provider request was not captured")
	}
	select {
	case unexpected := <-requests:
		t.Fatalf("second request started before first completed: %#v", unexpected)
	case <-time.After(25 * time.Millisecond):
	}
	close(releaseFirst)

	all := collectCompletedTurns(t, events, 2)
	first := eventsForClient(all, clientOne)
	wantKinds := []EventKind{
		EventConversationItem, EventConversationItem, EventTurnCompleted,
	}
	var clientKinds []EventKind
	for _, event := range first {
		clientKinds = append(clientKinds, event.Kind)
	}
	if !equalKinds(clientKinds, wantKinds) {
		t.Fatalf("client event kinds = %v, want %v", clientKinds, wantKinds)
	}
	var firstTurn []EventKind
	for _, event := range all {
		if event.TurnID == "" || event.TurnID == first[len(first)-1].TurnID {
			if event.Kind != EventAssistantDelta || event.TurnID == first[len(first)-1].TurnID {
				firstTurn = append(firstTurn, event.Kind)
			}
		}
		if event.Kind == EventTurnCompleted && event.ClientMessageID != nil && *event.ClientMessageID == clientOne {
			break
		}
	}
	wantOrder := []EventKind{
		EventAgentStatus, EventAgentStatus, EventConversationItem,
		EventAssistantDelta, EventConversationItem, EventAgentStatus, EventTurnCompleted,
	}
	if !equalKinds(firstTurn, wantOrder) {
		t.Fatalf("first turn order = %v, want %v", firstTurn, wantOrder)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 4 || page.Items[0].ContentText != "first" ||
		page.Items[1].ContentText != "reply 1" || page.Items[2].ContentText != "second" ||
		page.Items[3].ContentText != "reply 2" {
		t.Fatalf("stored transcript = %#v", page.Items)
	}
	messages := firstRequest["messages"].([]any)
	encodedMessages, _ := json.Marshal(messages)
	if !strings.Contains(string(encodedMessages), "America/Los_Angeles") ||
		messages[len(messages)-1].(map[string]any)["content"] != "first" ||
		firstRequest["max_completion_tokens"] != float64(8192) {
		t.Fatalf("provider messages = %#v", messages)
	}
}

func TestChatExecutesDurableTaskInspectLoopWithBoundedReplay(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	taskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	document := strings.Repeat(`"`, 64<<10)
	if _, err := home.CreatePendingTaskDocument(chat.home, taskID, document); err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, taskID, "Large Task", "correlation:test:"+taskID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, taskID); err != nil {
		t.Fatal(err)
	}

	events, err := chat.Subscribe(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	requests := make(chan map[string]any, 3)
	var requestMu sync.Mutex
	requestCount := 0
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			return nil, err
		}
		requests <- body
		requestMu.Lock()
		requestCount++
		current := requestCount
		requestMu.Unlock()
		if current <= 2 {
			return openRouterToolResponse(
				taskID, "call_"+string(rune('0'+current)), "I will inspect it.",
			), nil
		}
		return openRouterStreamResponse("The Task is ready."), nil
	}))

	clientID := "inspect-client"
	if _, err := chat.SendTurn(ctx, SendTurnInput{
		ConversationID: conversation.ID, Input: "Inspect the Task", ClientMessageID: &clientID,
	}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	var visibleKinds []store.ConversationItemKind
	for _, event := range all {
		if event.Item != nil {
			visibleKinds = append(visibleKinds, event.Item.Kind)
		}
	}
	wantKinds := []store.ConversationItemKind{
		store.ConversationUserText, store.ConversationAssistantText,
		store.ConversationToolCall, store.ConversationToolResult, store.ConversationAssistantText,
		store.ConversationToolCall, store.ConversationToolResult, store.ConversationAssistantText,
	}
	if len(visibleKinds) != len(wantKinds) {
		t.Fatalf("visible item kinds = %v", visibleKinds)
	}
	for index := range wantKinds {
		if visibleKinds[index] != wantKinds[index] {
			t.Fatalf("visible item kinds = %v, want %v", visibleKinds, wantKinds)
		}
	}
	initial, firstContinuation, secondContinuation := <-requests, <-requests, <-requests
	tools, ok := initial["tools"].([]any)
	if !ok || len(tools) != 35 || initial["tool_choice"] != "auto" || initial["parallel_tool_calls"] != false {
		t.Fatalf("initial tool controls = %#v", initial)
	}
	toolNames := make(map[string]bool)
	for _, tool := range tools {
		wire := tool.(map[string]any)
		if wire["type"] == "openrouter:web_search" {
			toolNames["hosted_web_search"] = true
			continue
		}
		function := wire["function"].(map[string]any)
		toolNames[function["name"].(string)] = true
	}
	if !toolNames["run_lua"] || !toolNames["parse"] || !toolNames["inspect"] || !toolNames["download"] ||
		!toolNames["read_memory_page"] || !toolNames["search_memory"] || !toolNames["hosted_web_search"] {
		t.Fatalf("advertised tools = %#v", toolNames)
	}
	if len(firstContinuation["tools"].([]any)) != 35 || len(secondContinuation["tools"].([]any)) != 35 {
		t.Fatal("normal continuations did not retain the Chat tools")
	}
	messages := firstContinuation["messages"].([]any)
	var replayResult map[string]any
	for _, raw := range messages {
		message := raw.(map[string]any)
		if message["role"] == "tool" {
			replayResult = message
		}
	}
	if replayResult == nil || replayResult["tool_call_id"] != "call_1" {
		t.Fatalf("tool replay = %#v", replayResult)
	}
	var replayEnvelope map[string]any
	if err := json.Unmarshal([]byte(replayResult["content"].(string)), &replayEnvelope); err != nil {
		t.Fatal(err)
	}
	bounded, _ := replayEnvelope["payload"].(map[string]any)
	if bounded["truncated"] != true || len(replayResult["content"].(string)) >= modelToolResultLimit {
		t.Fatalf("bounded replay length = %d", len(replayResult["content"].(string)))
	}
	replayedCalls := map[string]bool{}
	for _, raw := range secondContinuation["messages"].([]any) {
		message := raw.(map[string]any)
		if message["role"] == "tool" {
			replayedCalls[message["tool_call_id"].(string)] = true
		}
	}
	if !replayedCalls["call_1"] || !replayedCalls["call_2"] {
		t.Fatalf("second continuation call replay = %#v", replayedCalls)
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	toolResults := 0
	var rounds []int
	for _, item := range page.Items {
		if item.Kind == store.ConversationToolCall {
			rounds = append(rounds, providerRound(item))
		}
		if item.Kind != store.ConversationToolResult {
			continue
		}
		toolResults++
		action, _ := nestedAction(item.Payload)
		payload, _ := action["payload"].(map[string]any)
		if payload["task_document"] != document || payload["task_id"] != taskID {
			t.Fatalf("stored inspect payload was not exact")
		}
	}
	if toolResults != 2 || len(rounds) != 2 || rounds[0] != 0 || rounds[1] != 1 {
		t.Fatalf("stored tool rounds = %v with %d results", rounds, toolResults)
	}
	final := page.Items[len(page.Items)-1]
	providerUsage := final.Metadata["provider_usage"].(map[string]any)
	if final.Metadata["stream_id"] != store.ConversationAssistantStreamID(final.TurnID, 2) ||
		providerUsage["total_tokens"] != float64(22) {
		t.Fatalf("final round metadata = %#v", final.Metadata)
	}
}

func TestChatPersistsHostedWebFactsWithoutOrdinaryHostedReplay(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	events, err := chat.Subscribe(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	requests := make(chan map[string]any, 2)
	requestCount := 0
	hostedStream := strings.Join([]string{
		`data: {"id":"chat-web","model":"openai/gpt-5.6-luna","choices":[{"index":0,"delta":{"content":"Current answer.","reasoning_details":[{"type":"reasoning.server_tool_call","id":"ws_ok","name":"web.search","status":"completed","arguments":{"query":"current trains"},"result":{"summary":"Found schedules"}},{"type":"reasoning.server_tool_call","id":"ws_fail","name":"web.search","status":"failed","arguments":{"query":"closed station"},"result":{"error":"provider-hosted web action failed"}}],"annotations":[{"type":"url_citation","url_citation":{"title":"Rail","url":"https://rail.example/times","start_index":0,"end_index":7}}]},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":4,"total_tokens":16,"server_tool_use":{"web_search_requests":2}}}` + "\n\n",
		"data: [DONE]\n\n",
	}, "")
	if _, err := provider.ParseChatStream(ctx, io.NopCloser(strings.NewReader(hostedStream)), nil); err != nil {
		t.Fatalf("hosted stream fixture: %v", err)
	}
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		var body map[string]any
		if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
			return nil, err
		}
		requests <- body
		requestCount++
		if requestCount > 1 {
			return openRouterStreamResponse("Updated."), nil
		}
		return &http.Response{
			StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/event-stream"}},
			Body: io.NopCloser(strings.NewReader(hostedStream)),
		}, nil
	}))
	for index, input := range []string{"Find current trains", "Check again"} {
		if _, err := chat.SendTurn(ctx, SendTurnInput{ConversationID: conversation.ID, Input: input}); err != nil {
			t.Fatal(err)
		}
		turnEvents := collectCompletedTurns(t, events, 1)
		for _, event := range turnEvents {
			if event.Item != nil && event.Item.Kind == store.ConversationErrorNotice {
				t.Fatalf("turn %d failed: %#v", index, event.Item)
			}
		}
	}
	firstRequest, secondRequest := <-requests, <-requests
	if len(firstRequest["tools"].([]any)) != 35 {
		t.Fatalf("hosted web tools = %#v", firstRequest["tools"])
	}
	var replayedSearches int
	for _, raw := range secondRequest["messages"].([]any) {
		message := raw.(map[string]any)
		for _, rawDetail := range anySlice(message["reasoning_details"]) {
			detail := rawDetail.(map[string]any)
			if detail["type"] == "reasoning.server_tool_call" {
				replayedSearches++
			}
		}
	}
	if replayedSearches != 0 {
		t.Fatalf("replayed hosted searches = %d in %#v", replayedSearches, secondRequest["messages"])
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	var calls, results int
	var failed bool
	var firstAnswer store.ConversationItem
	correlations := make(map[string]bool)
	for _, item := range page.Items {
		switch item.Kind {
		case store.ConversationToolCall:
			calls++
			display := item.Payload["metadata"].(map[string]any)["display"].(map[string]any)
			if display["marker"].(map[string]any)["kind"] != "web.search" {
				t.Fatalf("hosted web marker = %#v", display)
			}
			action := item.Payload["metadata"].(map[string]any)["action"].(map[string]any)
			correlations[action["id"].(string)] = false
		case store.ConversationToolResult:
			results++
			failed = failed || item.Status == "failed"
			action := item.Payload["metadata"].(map[string]any)["action"].(map[string]any)
			if _, exists := correlations[action["call_id"].(string)]; exists {
				correlations[action["call_id"].(string)] = true
			}
		case store.ConversationAssistantText:
			if item.ContentText == "Current answer." {
				firstAnswer = item
			}
		}
	}
	usage, _ := firstAnswer.Metadata["provider_usage"].(map[string]any)
	citations, _ := firstAnswer.Metadata["citations"].([]any)
	if calls != 2 || results != 2 || !failed || usage["web_search_requests"] != float64(2) ||
		len(citations) != 1 || citations[0].(map[string]any)["url"] != "https://rail.example/times" ||
		!correlations["ws_ok"] || !correlations["ws_fail"] {
		t.Fatalf("stored hosted web facts = calls %d, results %d, failed %v, answer %#v", calls, results, failed, firstAnswer)
	}
}

func anySlice(value any) []any {
	values, _ := value.([]any)
	return values
}

func TestMemoryToolsEnforceInputRulesAndSelectPages(t *testing.T) {
	chat, _, _ := chatFixture(t)
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if err := chat.memory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: root.Path, Title: root.Title, Icon: root.Icon, Body: "People summary."},
		{Path: "people.md", Title: "People", Icon: "users", Body: "Alice likes tea."},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}
	for _, selector := range []string{"people.md", "memory:human:people.md"} {
		payload, success := chat.readMemoryPage(json.RawMessage(`{"page":` + strconv.Quote(selector) + `}`))
		if !success || !bytes.Contains(payload, []byte(`"path":"people.md"`)) {
			t.Fatalf("page selector %q = %s, %t", selector, payload, success)
		}
	}
	for _, test := range []struct {
		name, tool, arguments string
	}{
		{"read extra field", noemamemory.ReadPageToolName, `{"page":"people.md","extra":true}`},
		{"search extra field", noemamemory.SearchToolName, `{"query":"Alice","extra":true}`},
		{"search limit", noemamemory.SearchToolName, `{"query":"Alice","limit":17}`},
	} {
		payload, success := chat.executeChatTool(
			context.Background(), store.Conversation{}, test.tool, json.RawMessage(test.arguments),
			"", "",
		)
		if success || !bytes.Contains(payload, []byte(`"code":"invalid_input"`)) {
			t.Fatalf("%s = %s, %t", test.name, payload, success)
		}
	}
	payload, success := chat.searchMemory(json.RawMessage(`{"query":""}`))
	if !success || !bytes.Contains(payload, []byte(`"pages":[]`)) {
		t.Fatalf("empty search = %s, %t", payload, success)
	}
}

func TestChatMemoryContextAndToolResultsReplayWithoutConcealment(t *testing.T) {
	original, database, conversation := chatFixture(t)
	root, _ := original.memory.ReadRoot()
	ordinary := "authorization_state=opaque-123"
	if err := original.memory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: root.Path, Title: root.Title, Icon: root.Icon, Body: ordinary},
		{Path: "people.md", Title: "People", Icon: "users", Body: "Alice likes tea and trains."},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}
	if err := original.Close(); err != nil {
		t.Fatal(err)
	}
	requests := make([]provider.GenerateRequest, 0, 3)
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		switch len(requests) {
		case 1:
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "memory_search_1", ProviderName: noemamemory.SearchToolName,
				Name: noemamemory.SearchToolName, Payload: json.RawMessage(`{"query":"Alice","limit":1}`),
			}}}, nil
		case 2:
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Index: 0, ProviderCallID: "memory_read_1", ProviderName: noemamemory.ReadPageToolName,
				Name: noemamemory.ReadPageToolName, Payload: json.RawMessage(`{"page":"memory:human:people.md"}`),
			}}}, nil
		default:
			return provider.GenerationResult{Text: "Alice likes tea."}, nil
		}
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
		ConversationID: conversation.ID, Input: "What does Alice like?",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if len(requests) != 3 || len(requests[0].Tools) != 34 ||
		requests[0].Tools[2].Name != "read_memory_page" || requests[0].Tools[3].Name != "search_memory" ||
		requests[0].Tools[4].Name != fileDownloadName {
		t.Fatalf("provider Memory tools = %#v", requests[0].Tools)
	}
	initialContext := requests[0].Messages[0].Content
	if !strings.Contains(initialContext, ordinary) ||
		!strings.Contains(initialContext, "People (people.md, memory:human:people.md)") {
		t.Fatalf("root Memory context = %q", initialContext)
	}
	replayed := make(map[string]json.RawMessage)
	for _, request := range requests[1:] {
		for _, message := range request.Messages {
			if message.ToolResult != nil {
				replayed[message.ToolResult.Name] = message.ToolResult.Payload
			}
		}
	}
	if !bytes.Contains(replayed[noemamemory.SearchToolName], []byte(`"snippet":"Alice likes tea and trains"`)) ||
		!bytes.Contains(replayed[noemamemory.ReadPageToolName], []byte(`"body":"Alice likes tea and trains."`)) {
		t.Fatalf("Memory replay = %#v", replayed)
	}
}

func TestChatRoutesCodexAssignmentThroughToolContinuation(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	homeRoot, err := os.OpenRoot(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = homeRoot.Close() })
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	const accountID = "provider_account:codex:default"
	now := time.Now()
	if _, err := database.CreateProviderAccount(ctx, provider.Account{
		ID: accountID, ProviderKind: "codex", AccountKey: "default", DisplayName: "Codex",
		AuthMethod: provider.AuthOAuthDeviceCode, IsActive: true, IsDefault: true,
		Status: provider.StatusAuthenticated, Metadata: provider.AccountMetadata{},
		CreatedAt: now, UpdatedAt: now,
	}); err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{
			Role: role, ProviderKind: "codex", ProviderAccountID: accountID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
			FastMode:      role == store.HostedModelNoema,
		})
	}
	if created, err := database.ConfirmHostedModelAssignments(ctx, accountID, assignments); err != nil || !created {
		t.Fatalf("confirm Codex assignments = %t, %v", created, err)
	}
	conversation, err := database.EnsurePrimaryConversation(ctx, "codex", "/workspace", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	openRouterCalls := 0
	openRouter := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		openRouterCalls++
		return provider.GenerationResult{}, errors.New("unexpected OpenRouter request")
	})
	var requests []provider.GenerateRequest
	codex := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{
				ID: "resp_1", Model: "gpt-5.6-terra", Text: "I will inspect it.", Usage: provider.Usage{TotalTokens: 3},
				Searches: []provider.HostedSearch{{ID: "search_1", Name: "web.search", Status: "completed",
					Arguments: json.RawMessage(`{"query":"current"}`), Result: json.RawMessage(`{"status":"completed"}`)}},
				Reasoning: []provider.GenerationReasoning{{ProviderDetails: []json.RawMessage{
					json.RawMessage(`{"type":"reasoning","id":"rs_1","encrypted_content":"opaque"}`),
				}}},
				ToolCalls: []provider.GenerationToolCall{{
					ProviderItemID: "fc_1", ProviderCallID: "call_1",
					ProviderName: taskInspectName, Name: taskInspectName,
					Payload: json.RawMessage(`{"task_id":"task:missing"}`),
				}},
			}, nil
		}
		return provider.GenerationResult{
			ID: "resp_2", Model: "gpt-5.6-terra", Text: "Codex complete.", Usage: provider.Usage{TotalTokens: 4},
		}, nil
	})
	chat, err := NewChat(database, openRouter, codex, codex, homeRoot, openChatMemory(t, homeRoot))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	events, err := chat.Subscribe(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err := chat.SendTurn(ctx, SendTurnInput{ConversationID: conversation.ID, Input: "Inspect it"}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if openRouterCalls != 0 || len(requests) != 2 {
		t.Fatalf("provider calls = OpenRouter %d, Codex %d", openRouterCalls, len(requests))
	}
	for index, request := range requests {
		if request.AccountID != accountID || request.Model != "gpt-5.6-terra" ||
			request.ReasoningEffort != "medium" || !request.FastMode {
			t.Fatalf("Codex request %d = %#v", index, request)
		}
	}
	if requests[0].PreviousResponseID != "" || !requests[0].StoreResponse ||
		requests[1].PreviousResponseID != "resp_1" || !requests[1].StoreResponse {
		t.Fatalf("Codex response continuation = %#v", requests)
	}
	replayedCall, incrementalResult := false, false
	for _, message := range requests[1].Messages {
		for _, call := range message.ToolCalls {
			replayedCall = replayedCall || call.ProviderItemID == "fc_1"
		}
		incrementalResult = incrementalResult ||
			message.ToolResult != nil && message.ToolResult.ProviderCallID == "call_1"
	}
	if replayedCall || !incrementalResult || len(requests[1].Messages) != 2 ||
		requests[1].Messages[1].Role != "developer" ||
		!strings.Contains(requests[1].Messages[1].Content, "Active Project catalog:") {
		t.Fatalf("Codex incremental continuation = call %t, result %t, messages %#v",
			replayedCall, incrementalResult, requests[1].Messages)
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	finalText := ""
	for _, item := range page.Items {
		if item.Kind == store.ConversationUserText {
			continue
		}
		if item.Metadata["provider"] != "codex" {
			t.Fatalf("item %s provider metadata = %#v", item.ID, item.Metadata)
		}
		if item.Kind == store.ConversationToolCall || item.Kind == store.ConversationToolResult {
			activity := item.Payload["metadata"].(map[string]any)
			if activity["provider"] != "codex" {
				t.Fatalf("item %s activity provider = %#v", item.ID, activity)
			}
		}
		if item.Kind == store.ConversationAssistantText && item.Metadata["phase"] == "final_answer" {
			finalText = item.ContentText
			if item.Metadata["provider_usage"].(map[string]any)["provider"] != "codex" {
				t.Fatalf("final provider usage = %#v", item.Metadata["provider_usage"])
			}
		}
	}
	if finalText != "Codex complete." {
		t.Fatalf("durable final text = %q", finalText)
	}
	assignment, err := chat.primaryAssignment(ctx)
	if err != nil {
		t.Fatal(err)
	}
	openAIRequest := provider.GenerateRequest{}
	incremental := provider.GenerationMessage{Role: "tool", ToolResult: &provider.ReplayToolResult{
		ProviderCallID: "call_openai", ProviderName: taskInspectName, Name: taskInspectName,
		Success: true, Payload: json.RawMessage(`{"title":"One"}`),
	}}
	assignment.ProviderKind = "openai"
	assignment.ProviderAccountID = "provider_account:openai:default"
	_, _, err = chat.generateChatToolContinuation(
		queuedTurn{conversation: conversation, location: time.UTC},
		store.ConversationTurn{ID: page.Items[0].TurnID, ConversationID: conversation.ID},
		assignment,
		generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
			openAIRequest = request
			return provider.GenerationResult{}, nil
		}),
		2, "", "", "resp_openai", true, incremental, nil,
	)
	if err != nil || openAIRequest.PreviousResponseID != "resp_openai" || !openAIRequest.StoreResponse ||
		len(openAIRequest.Messages) != 2 || openAIRequest.Messages[0].ToolResult == nil ||
		openAIRequest.Messages[0].ToolResult.ProviderCallID != "call_openai" ||
		openAIRequest.Messages[1].Role != "developer" ||
		!strings.Contains(openAIRequest.Messages[1].Content, "Active Project catalog:") {
		t.Fatalf("OpenAI incremental continuation = %#v, %v", openAIRequest, err)
	}
	for _, providerKind := range []string{"codex", "openai"} {
		assignment.ProviderKind = providerKind
		called := false
		_, _, err = chat.generateChatToolContinuation(
			queuedTurn{conversation: conversation, location: time.UTC},
			store.ConversationTurn{ID: page.Items[0].TurnID, ConversationID: conversation.ID},
			assignment,
			generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
				called = true
				return provider.GenerationResult{}, nil
			}),
			2, "", "", "", true, provider.GenerationMessage{}, nil,
		)
		if err == nil || called || !strings.Contains(err.Error(), "provider-hosted web state") {
			t.Fatalf("missing %s hosted state = called %t, error %v", providerKind, called, err)
		}
	}
}

func TestChatSelectsOpenAIWithHostedSearch(t *testing.T) {
	selected := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{}, nil
	})
	chat := &Chat{openAI: selected}
	generator, err := chat.generatorFor("openai")
	if err != nil || generator == nil || !hostedWebSearchEnabled("openai", provider.ToolTransportNative) {
		t.Fatalf("OpenAI Chat route = %v, %v", generator, err)
	}
}

func TestChatPersistsProviderFailureAndRejectsInvalidTimezone(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	replaceDefaultTransport(t, roundTripFunc(func(*http.Request) (*http.Response, error) {
		return &http.Response{
			StatusCode: http.StatusServiceUnavailable, Header: make(http.Header),
			Body: io.NopCloser(strings.NewReader("unavailable")),
		}, nil
	}))
	invalidZone := "Mars/Olympus_Mons"
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "invalid", ClientTimeZone: &invalidZone,
	}); err == nil || !strings.Contains(err.Error(), "valid IANA timezone") {
		t.Fatalf("invalid timezone error = %v", err)
	}
	clientID := "failed-client"
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "fail", ClientMessageID: &clientID,
	}); err != nil {
		t.Fatal(err)
	}
	all := collectCompletedTurns(t, events, 1)
	var sawErrorStatus, sawNotice bool
	for _, event := range all {
		sawErrorStatus = sawErrorStatus || event.Status == AgentStatusError
		sawNotice = sawNotice || event.Item != nil && event.Item.Kind == store.ConversationErrorNotice
	}
	if !sawErrorStatus || !sawNotice {
		t.Fatalf("failure events = %#v", all)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 2 || page.Items[1].Kind != store.ConversationErrorNotice ||
		page.Items[1].ContentText != "The provider request failed." {
		t.Fatalf("stored failure = %#v", page.Items)
	}
}

func TestChatClosesTurnWhenProviderReturnsWhitespace(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	responses := 0
	replaceDefaultTransport(t, roundTripFunc(func(*http.Request) (*http.Response, error) {
		responses++
		if responses == 1 {
			return openRouterStreamResponse("   "), nil
		}
		return openRouterStreamResponse("Recovered"), nil
	}))
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "First",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Second",
	}); err != nil {
		t.Fatalf("turn after whitespace response: %v", err)
	}
	collectCompletedTurns(t, events, 1)
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 4 || page.Items[1].Kind != store.ConversationErrorNotice ||
		page.Items[3].ContentText != "Recovered" {
		t.Fatalf("transcript after whitespace response = %#v", page.Items)
	}
}

func TestChatFinalizesDeterministicTaskInspectStops(t *testing.T) {
	cases := []struct {
		name        string
		toolCalls   int
		createTask  bool
		rejectFinal bool
		reason      string
	}{
		{"repeated result", 5, true, true, "repeated tool arguments and results"},
		{"failure streak", 6, false, false, "consecutive tool failures"},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			chat, database, conversation := chatFixture(t)
			ctx := context.Background()
			taskID := "task:" + strings.Repeat("0", 31) + "1"
			if test.createTask {
				if _, err := home.CreatePendingTaskDocument(chat.home, taskID, "Stable"); err != nil {
					t.Fatal(err)
				}
				if _, err := database.CreateTask(ctx, taskID, "Stable", "correlation:test:"+taskID, time.Now()); err != nil {
					t.Fatal(err)
				}
				if err := home.CommitTaskDocument(chat.home, taskID); err != nil {
					t.Fatal(err)
				}
			}
			events, err := chat.Subscribe(ctx, conversation.ID)
			if err != nil {
				t.Fatal(err)
			}
			<-events
			requestCount := 0
			var finalRequest map[string]any
			replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
				var body map[string]any
				if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
					return nil, err
				}
				requestCount++
				if requestCount <= test.toolCalls {
					currentTaskID := taskID
					if !test.createTask {
						currentTaskID = "task:" + strings.Repeat("0", 31) + string(rune('0'+requestCount))
					}
					return openRouterToolResponse(
						currentTaskID, "stop_call_"+string(rune('0'+requestCount)), "Checking.",
					), nil
				}
				if test.rejectFinal && requestCount == test.toolCalls+1 {
					return &http.Response{
						StatusCode: http.StatusBadRequest, Header: make(http.Header),
						Body: io.NopCloser(strings.NewReader("context exceeded")),
					}, nil
				}
				finalRequest = body
				return openRouterStreamResponse("Stopped safely."), nil
			}))
			if _, err := chat.SendTurn(ctx, SendTurnInput{
				ConversationID: conversation.ID, Input: "Keep checking",
			}); err != nil {
				t.Fatal(err)
			}
			collectCompletedTurns(t, events, 1)
			wantRequests := test.toolCalls + 1
			if test.rejectFinal {
				wantRequests++
			}
			if requestCount != wantRequests || finalRequest == nil {
				t.Fatalf("provider request count = %d", requestCount)
			}
			if _, exists := finalRequest["tools"]; exists {
				t.Fatalf("finalization request advertised tools: %#v", finalRequest["tools"])
			}
			encoded, _ := json.Marshal(finalRequest["messages"])
			if !strings.Contains(string(encoded), test.reason) {
				t.Fatalf("finalization reason is missing from %s", encoded)
			}
			toolResults := 0
			for _, raw := range finalRequest["messages"].([]any) {
				message := raw.(map[string]any)
				if message["role"] == "tool" {
					toolResults++
				}
			}
			if toolResults != test.toolCalls {
				t.Fatalf("finalization replay has %d tool results, want %d", toolResults, test.toolCalls)
			}
			page, err := database.ConversationItemPage(ctx, conversation.ID, "", 40)
			if err != nil {
				t.Fatal(err)
			}
			calls, results := 0, 0
			for _, item := range page.Items {
				if item.Kind == store.ConversationToolCall {
					calls++
				}
				if item.Kind == store.ConversationToolResult {
					results++
				}
			}
			if calls != test.toolCalls || results != test.toolCalls ||
				page.Items[len(page.Items)-1].ContentText != "Stopped safely." {
				t.Fatalf("stored stop transcript has %d calls and %d results", calls, results)
			}
		})
	}
}

func TestProviderUsageAggregationRejectsOverflow(t *testing.T) {
	maximum := int(^uint(0) >> 1)
	total := provider.Usage{TotalTokens: maximum}
	if err := addProviderUsage(&total, provider.Usage{TotalTokens: 1}); err == nil ||
		total.TotalTokens != maximum {
		t.Fatalf("overflow result = %#v, %v", total, err)
	}
}

func TestTaskInspectFinalizationCompactsReplay(t *testing.T) {
	messages := []provider.GenerationMessage{
		{Role: "user", Content: "old"},
		{Role: "assistant", Content: "old response"},
		{Role: "user", Content: "current"},
	}
	for index := range 10 {
		callID := fmt.Sprintf("call_%d", index)
		messages = append(messages,
			provider.GenerationMessage{
				Role: "assistant", Content: strings.Repeat("c", 9<<10),
				ReasoningDetails: []json.RawMessage{json.RawMessage(`{"type":"reasoning","data":"opaque"}`)},
				ToolCalls: []provider.ReplayToolCall{{
					ProviderCallID: callID, Name: "task.inspect", Arguments: json.RawMessage(`{"task_id":"task:1"}`),
				}},
			},
			provider.GenerationMessage{
				Role: "tool", ToolResult: &provider.ReplayToolResult{
					ProviderCallID: callID, Name: "task.inspect", Success: true,
					Payload: json.RawMessage(fmt.Sprintf(`{"round":%d}`, index)),
				},
			},
		)
	}
	compacted := compactToolFinalizationMessages(messages, modelToolPayloadLimit)
	if len(compacted) != 21 || compacted[0].Role != "user" || compacted[0].Content != "current" {
		t.Fatalf("compacted replay shape = %#v", compacted)
	}
	for index := 0; index < 10; index++ {
		assistant := compacted[1+index*2]
		result := compacted[2+index*2]
		wantID := fmt.Sprintf("call_%d", index)
		if len(assistant.ToolCalls) != 1 || assistant.ToolCalls[0].ProviderCallID != wantID ||
			result.ToolResult == nil || result.ToolResult.ProviderCallID != wantID {
			t.Fatalf("compacted pair %d = %#v, %#v", index, assistant, result)
		}
		if len(assistant.Content) != 2<<10 || len(assistant.ReasoningDetails) != 0 {
			t.Fatalf("compacted assistant %d retained large private context", index)
		}
	}
}

func TestChatFinalizesRejectedTaskInspectReplay(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	ctx := context.Background()
	taskID, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CreatePendingTaskDocument(chat.home, taskID, strings.Repeat("x", 64<<10)); err != nil {
		t.Fatal(err)
	}
	if _, err := chat.database.CreateTask(ctx, taskID, "Large Task", "correlation:test:"+taskID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, taskID); err != nil {
		t.Fatal(err)
	}
	events, err := chat.Subscribe(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	requestCount := 0
	var finalRequest map[string]any
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		requestCount++
		switch requestCount {
		case 1:
			return openRouterToolResponse(taskID, "context_call", "Checking."), nil
		case 2:
			return &http.Response{
				StatusCode: http.StatusBadRequest, Header: make(http.Header),
				Body: io.NopCloser(strings.NewReader("context exceeded")),
			}, nil
		default:
			if err := json.NewDecoder(request.Body).Decode(&finalRequest); err != nil {
				return nil, err
			}
			return openRouterStreamResponse("I could not retain more Task context."), nil
		}
	}))
	if _, err := chat.SendTurn(ctx, SendTurnInput{
		ConversationID: conversation.ID, Input: "Inspect the large Task",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if requestCount != 3 || finalRequest == nil || finalRequest["max_completion_tokens"] != float64(1024) {
		t.Fatalf("finalization requests = %d, final = %#v", requestCount, finalRequest)
	}
	if _, exists := finalRequest["tools"]; exists {
		t.Fatalf("rejected replay finalization advertised tools: %#v", finalRequest)
	}
	encoded, _ := json.Marshal(finalRequest["messages"])
	if len(encoded) >= 4<<10 || !strings.Contains(string(encoded), "provider replay limit reached") {
		t.Fatalf("rejected replay was not compact: %d bytes", len(encoded))
	}
}

func TestChatCloseCancelsActiveProviderAndRejectsNewTurns(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	started := make(chan struct{})
	replaceDefaultTransport(t, roundTripFunc(func(request *http.Request) (*http.Response, error) {
		close(started)
		<-request.Context().Done()
		return nil, request.Context().Err()
	}))
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "running",
	}); err != nil {
		t.Fatal(err)
	}
	<-started
	if err := chat.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "late",
	}); !errors.Is(err, ErrChatClosed) {
		t.Fatalf("send after close error = %v", err)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Items) != 1 || page.Items[0].Kind != store.ConversationUserText {
		t.Fatalf("shutdown transcript = %#v", page.Items)
	}
	stale, _, err := database.BeginConversationTurn(
		context.Background(), conversation.ID, "stale", nil, time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	restarted, err := NewChat(database, chat.openRouter, chat.codex, chat.openAI, chat.home, chat.memory)
	if err != nil {
		t.Fatal(err)
	}
	if err := restarted.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(
		context.Background(), stale, "late result", "", nil, time.Now(),
	); err == nil || !strings.Contains(err.Error(), "already final") {
		t.Fatalf("complete recovered turn error = %v", err)
	}
}

func chatFixture(t *testing.T) (*Chat, *store.Store, store.Conversation) {
	return chatFixtureAt(t, "/workspace")
}

func chatFixtureAt(t *testing.T, cwd string) (*Chat, *store.Store, store.Conversation) {
	t.Helper()
	ctx := context.Background()
	root := t.TempDir()
	homeRoot, err := os.OpenRoot(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = homeRoot.Close() })
	database, err := store.Open(ctx, filepath.Join(root, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	accounts, err := provider.NewAccountService(root, database)
	if err != nil {
		t.Fatal(err)
	}
	secret, err := provider.NewSecret("runtime-test-key")
	if err != nil {
		t.Fatal(err)
	}
	account, err := accounts.CreateSecretAccount(ctx, "openrouter", "", secret, time.Now())
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
	if created, err := database.ConfirmHostedModelAssignments(ctx, account.ID, assignments); err != nil || !created {
		t.Fatalf("confirm assignments = %t, %v", created, err)
	}
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", cwd, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	generator, err := provider.NewOpenRouterGenerator(accounts)
	if err != nil {
		t.Fatal(err)
	}
	codexGenerator, err := provider.NewCodexGenerator(accounts)
	if err != nil {
		t.Fatal(err)
	}
	chat, err := NewChat(
		database, generator, codexGenerator, codexGenerator, homeRoot, openChatMemory(t, homeRoot),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = chat.Close() })
	return chat, database, conversation
}

func openChatMemory(t *testing.T, root *os.Root) *noemamemory.Store {
	t.Helper()
	store, err := noemamemory.New(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	return store
}

func collectCompletedTurns(t *testing.T, events <-chan Event, count int) []Event {
	t.Helper()
	deadline := time.After(5 * time.Second)
	collected := make([]Event, 0)
	completed := 0
	for completed < count {
		select {
		case event, ok := <-events:
			if !ok {
				t.Fatal("event stream closed before completion")
			}
			collected = append(collected, event)
			if event.Kind == EventTurnCompleted {
				completed++
			}
		case <-deadline:
			t.Fatalf("timed out after events %#v", collected)
		}
	}
	return collected
}

func eventsForClient(events []Event, clientID string) []Event {
	var result []Event
	for _, event := range events {
		if event.ClientMessageID != nil && *event.ClientMessageID == clientID {
			result = append(result, event)
		}
	}
	return result
}

func equalKinds(left []EventKind, right []EventKind) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func openRouterStreamResponse(text string) *http.Response {
	body := "data: {\"id\":\"chat-runtime\",\"model\":\"openai/gpt-5.6-luna\",\"choices\":[{\"index\":0,\"delta\":{\"content\":" + strconvQuote(text) + "},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\ndata: [DONE]\n\n"
	return &http.Response{
		StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/event-stream"}},
		Body: io.NopCloser(strings.NewReader(body)),
	}
}

func openRouterToolResponse(taskID string, callID string, commentary string) *http.Response {
	arguments := strconvQuote(`{"task_id":"` + taskID + `"}`)
	body := "data: {\"id\":\"inspect-tool\",\"model\":\"openai/gpt-5.6-luna\",\"choices\":[{\"index\":0,\"delta\":{\"content\":" + strconvQuote(commentary) + ",\"tool_calls\":[{\"index\":0,\"id\":" + strconvQuote(callID) + ",\"type\":\"function\",\"function\":{\"name\":\"inspect\",\"arguments\":" + arguments + "}}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3,\"total_tokens\":8}}\n\ndata: [DONE]\n\n"
	return &http.Response{
		StatusCode: http.StatusOK, Header: http.Header{"Content-Type": {"text/event-stream"}},
		Body: io.NopCloser(strings.NewReader(body)),
	}
}

func strconvQuote(value string) string {
	encoded, _ := json.Marshal(value)
	return string(encoded)
}

type roundTripFunc func(*http.Request) (*http.Response, error)

type generatorFunc func(
	context.Context, provider.GenerateRequest, func(provider.StreamEvent),
) (provider.GenerationResult, error)

func (function generatorFunc) Generate(
	ctx context.Context,
	request provider.GenerateRequest,
	onEvent func(provider.StreamEvent),
) (provider.GenerationResult, error) {
	return function(ctx, request, onEvent)
}

func (function roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return function(request)
}

func replaceDefaultTransport(t *testing.T, transport http.RoundTripper) {
	t.Helper()
	previous := http.DefaultTransport
	http.DefaultTransport = transport
	t.Cleanup(func() { http.DefaultTransport = previous })
}
