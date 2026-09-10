package runtime

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestMemoryUpdatePublishesValidatedEvidenceAndCheckpoint(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	turn, human, err := database.BeginConversationTurn(ctx, conversation.ID, "Alice likes tea.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	assistant, err := database.CompleteConversationTurn(ctx, turn, "I will remember that.", "", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	called := make(chan provider.GenerateRequest, 1)
	release := make(chan struct{})
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		called <- request
		<-release
		payload, _ := json.Marshal(map[string]any{
			"upserts": []any{map[string]any{
				"id": root.ID, "expected_hash": root.Hash, "path": root.Path,
				"title": "Alice", "icon": "user", "body": "Alice likes tea.[^1]",
				"citations": []any{map[string]any{"sources": []string{human.ID}}},
			}},
			"metadata_updates": []any{}, "deletes": []any{},
		})
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: payload}}}, nil
	})
	events := chat.SubscribeMemory(ctx)
	accepted, err := chat.TriggerMemoryUpdate(ctx)
	if err != nil || !accepted {
		t.Fatalf("trigger Memory update = %t, %v", accepted, err)
	}
	if accepted, err := chat.TriggerMemoryUpdate(ctx); err != nil || accepted {
		t.Fatalf("repeat Memory update = %t, %v", accepted, err)
	}
	close(release)
	waitForMemoryUpdate(t, chat, events)
	request := <-called
	if request.Model != "openai/gpt-5.6-luna" || request.ToolChoice != provider.ToolChoiceRequired ||
		request.StoreResponse || len(request.Tools) != 1 || request.Tools[0].Name != memorySubmitTool {
		t.Fatalf("Memory generation request = %#v", request)
	}
	if !strings.Contains(request.Messages[1].Content, "human ["+human.ID+"] Alice likes tea.") ||
		!strings.Contains(request.Messages[1].Content, "assistant I will remember that.") {
		t.Fatalf("Memory source = %q", request.Messages[1].Content)
	}
	published, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if published.Title != "Alice" || len(published.Citations) != 1 || published.Citations[0].Sources[0] != human.ID {
		t.Fatalf("published Memory = %#v", published)
	}
	checkpoint, err := chat.memory.State()
	if err != nil {
		t.Fatal(err)
	}
	if checkpoint.ConversationID != conversation.ID || checkpoint.LastConsolidatedSequence != assistant.Sequence ||
		checkpoint.LastConsolidatedItem != assistant.ID {
		t.Fatalf("Memory checkpoint = %#v", checkpoint)
	}
}

func TestMemoryUpdateRetriesOnceWithoutPublishingInvalidSources(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Alice likes tea.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(ctx, turn, "Noted.", "", nil, time.Now()); err != nil {
		t.Fatal(err)
	}
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	requests := make([]provider.GenerateRequest, 0, 2)
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		mu.Lock()
		requests = append(requests, request)
		mu.Unlock()
		payload, _ := json.Marshal(map[string]any{
			"upserts": []any{map[string]any{
				"id": root.ID, "expected_hash": root.Hash, "path": root.Path,
				"title": root.Title, "icon": root.Icon, "body": "Unsupported claim.[^1]",
				"citations": []any{map[string]any{"sources": []string{"item:not-captured"}}},
			}},
			"metadata_updates": []any{}, "deletes": []any{},
		})
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: payload}}}, nil
	})
	events := chat.SubscribeMemory(ctx)
	if accepted, err := chat.TriggerMemoryUpdate(ctx); err != nil || !accepted {
		t.Fatalf("trigger Memory update = %t, %v", accepted, err)
	}
	waitForMemoryUpdate(t, chat, events)
	mu.Lock()
	defer mu.Unlock()
	if len(requests) != 2 || !strings.Contains(requests[1].Messages[0].Content, "previous native tool call was rejected") {
		t.Fatalf("corrective requests = %#v", requests)
	}
	checkpoint, err := chat.memory.State()
	if err != nil {
		t.Fatal(err)
	}
	status := chat.MemoryUpdateStatus()
	if checkpoint.LastConsolidatedSequence != 0 || status.Error == "" {
		t.Fatalf("failed Memory state = %#v, %#v", checkpoint, status)
	}
	unchanged, err := chat.memory.ReadRoot()
	if err != nil || unchanged.Hash != root.Hash {
		t.Fatalf("failed update changed Memory = %#v, %v", unchanged, err)
	}
}

func TestMemoryUpdateCorrectsMisplacedExpectedHash(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	calls := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		payload := map[string]any{
			"upserts": []any{}, "metadata_updates": []any{}, "deletes": []any{},
		}
		if calls == 1 {
			payload["metadata_updates"] = []any{map[string]any{
				"path": root.Path, "icon": root.Icon, "expected_hash": root.Hash,
			}}
		} else {
			correction := request.Messages[0].Content
			if !strings.Contains(correction, "metadata_updates") || !strings.Contains(correction, "expected_hash") {
				t.Errorf("correction does not locate the rejected field: %s", correction)
			}
			current, readErr := chat.memory.ReadRoot()
			state, stateErr := chat.memory.State()
			if readErr != nil || stateErr != nil || current.Hash != root.Hash || state.LastConsolidatedSequence != 0 {
				t.Errorf("invalid proposal changed Memory: %v, %v", readErr, stateErr)
			}
			payload["upserts"] = []any{map[string]any{
				"id": root.ID, "expected_hash": root.Hash, "path": root.Path,
				"title": root.Title, "icon": root.Icon, "body": root.Body, "citations": []any{},
			}}
		}
		encoded, _ := json.Marshal(payload)
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{Name: memorySubmitTool, Payload: encoded}}}, nil
	})
	err = chat.generateAndPublishMemory(context.Background(), generator, store.ModelAssignment{}, "", "",
		[]noemamemory.Page{root}, map[string]bool{root.Path: true}, nil,
		noemamemory.State{ConversationID: conversation.ID, LastConsolidatedSequence: 1})
	if err != nil {
		t.Fatal(err)
	}
	state, err := chat.memory.State()
	if err != nil || calls != 2 || state.LastConsolidatedSequence != 1 {
		t.Fatalf("corrected publication: calls=%d state=%+v error=%v", calls, state, err)
	}
}

func TestMemorySourceOmitsBrowserScreenshotAndPreservesOrdinaryPayload(t *testing.T) {
	payload := map[string]any{
		"snapshot": map[string]any{"url": "https://example.test", "node_id": "node:opaque"},
		"result": map[string]any{
			"screenshot": map[string]any{"data": "encoded-image"},
			"status":     "complete",
		},
	}
	item := store.ConversationItem{
		ID: "item:browser", Kind: store.ConversationToolResult,
		Payload: map[string]any{"metadata": map[string]any{"action": map[string]any{
			"name": "web.browse.interact", "payload": payload,
		}}},
	}
	evidence := memoryEvidencePayload(item)
	if strings.Contains(evidence, "screenshot") || strings.Contains(evidence, "encoded-image") {
		t.Fatalf("browser evidence retained screenshot: %s", evidence)
	}
	if !strings.Contains(evidence, `"url":"https://example.test"`) ||
		!strings.Contains(evidence, `"node_id":"node:opaque"`) ||
		!strings.Contains(evidence, `"status":"complete"`) {
		t.Fatalf("browser evidence removed ordinary payload: %s", evidence)
	}
	if payload["result"].(map[string]any)["screenshot"].(map[string]any)["data"] != "encoded-image" {
		t.Fatalf("stored browser payload changed: %#v", payload)
	}
	if rendered := renderMemorySourceItem(item); strings.Contains(rendered, "encoded-image") {
		t.Fatalf("rendered browser source retained screenshot: %s", rendered)
	}
}

func TestMemorySourceOmitsTaskHistory(t *testing.T) {
	item := store.ConversationItem{
		ID: "item:tasks", Kind: store.ConversationToolResult,
		Payload: map[string]any{"metadata": map[string]any{"action": map[string]any{
			"name": "task.list", "payload": map[string]any{
				"tasks": []any{map[string]any{"task_id": "task:opaque"}},
			},
		}}},
	}
	if evidence := memoryEvidencePayload(item); evidence != "" {
		t.Fatalf("task history was retained as memory evidence: %s", evidence)
	}
	if rendered := renderMemorySourceItem(item); rendered != "" {
		t.Fatalf("task history was rendered for memory: %s", rendered)
	}
}

func TestMemoryUpdateUsesSelectedModelContextWindow(t *testing.T) {
	for _, window := range []uint32{128_000, 8_000, 1_024} {
		t.Run(fmt.Sprint(window), func(t *testing.T) {
			chat, database, conversation := chatFixture(t)
			profiles, _ := json.Marshal([]provider.ModelProfile{{ID: "memory-test", Label: "Memory test", ContextWindowTokens: &window}})
			account, err := database.CreateProviderAccount(t.Context(), provider.Account{
				ID: "provider_account:openrouter:memory-test", ProviderKind: "openrouter",
				AccountKey: "memory-test", DisplayName: "Memory test", AuthMethod: provider.AuthSecretInput,
				IsActive: true, Status: provider.StatusAuthenticated,
				Metadata: provider.AccountMetadata{"profiles": profiles}, CreatedAt: time.Now(), UpdatedAt: time.Now(),
			})
			if err != nil {
				t.Fatal(err)
			}
			template := strings.Repeat("connector setup code", 2_000)
			item := store.ConversationItem{ID: "item:template", Sequence: 1, Kind: "activity", Payload: map[string]any{
				"activity_kind": "tool_result",
				"metadata": map[string]any{"action": map[string]any{
					"name": "adapter.definition_template", "payload": map[string]any{"revision_base": template},
				}},
			}}
			called := false
			generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
				called = true
				if request.Messages[1].Content != renderMemorySourceItem(item) {
					t.Fatal("memory input lost source content")
				}
				return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
					Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[],"deletes":[]}`),
				}}}, nil
			})
			err = chat.consolidateMemoryRange(t.Context(), generator, store.ModelAssignment{
				ProviderAccountID: account.ID, ProviderKind: account.ProviderKind, ModelProfile: "memory-test",
			}, store.MemorySourceRange{ConversationID: conversation.ID, CapturedHead: 1, Items: []store.ConversationItem{item}})
			state, stateErr := chat.memory.State()
			if stateErr != nil {
				t.Fatal(stateErr)
			}
			if window == 128_000 {
				if err != nil || !called || state.LastConsolidatedSequence != 1 {
					t.Fatalf("large model update failed: %#v, %v, called %t", state, err, called)
				}
			} else if err == nil || !strings.Contains(err.Error(), "exceeds the model context budget") || called || state.LastConsolidatedSequence != 0 {
				t.Fatalf("small model limit was not enforced: %#v, %v, called %t", state, err, called)
			}
		})
	}
}

func TestMemoryRelevantPagesUseAssistantFallbackContext(t *testing.T) {
	chat, _, _ := chatFixture(t)
	if err := chat.memory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{Path: "interests.md", Title: "Interests", Icon: "file-text", Body: "Stargazing plans."},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}
	pages, err := chat.memory.ListPages()
	if err != nil {
		t.Fatal(err)
	}
	selected := map[string]bool{}
	includeRelevantMemoryPages(chat.memory, pages, []store.ConversationItem{{
		Kind: store.ConversationAssistantText, ContentText: "Discuss stargazing next.",
	}}, selected, 100_000)
	if !selected["interests.md"] {
		t.Fatalf("assistant fallback selected pages = %#v", selected)
	}
}

func TestMemoryInvalidationsCoalesceForSlowSubscriber(t *testing.T) {
	chat, _, _ := chatFixture(t)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	events := chat.SubscribeMemory(ctx)
	for range subscriberQueueLimit * 2 {
		chat.publishMemoryChanged()
	}
	event, open := <-events
	if !open || event.Kind != EventMemoryChanged {
		t.Fatalf("coalesced Memory event = %#v, open %t", event, open)
	}
	select {
	case extra, open := <-events:
		t.Fatalf("Memory invalidations did not coalesce: %#v, open %t", extra, open)
	default:
	}
	chat.publishMemoryChanged()
	select {
	case event, open = <-events:
		if !open || event.Kind != EventMemoryChanged {
			t.Fatalf("later Memory event = %#v, open %t", event, open)
		}
	case <-time.After(time.Second):
		t.Fatal("later Memory invalidation was not delivered")
	}
}

func TestPrimaryMemoryUpdateFollowsContextCompaction(t *testing.T) {
	for _, test := range []struct {
		name        string
		historySize int
		wantUpdate  bool
	}{
		{"pending source exceeds old threshold", 13_000, false},
		{"completed context requires compaction", 300_000, true},
	} {
		t.Run(test.name, func(t *testing.T) {
			chat, database, conversation := chatFixture(t)
			ctx := context.Background()
			window := uint32(128_000)
			profiles, _ := json.Marshal([]provider.ModelProfile{{ID: "openai/gpt-5.6-luna", ContextWindowTokens: &window}})
			account, err := database.CreateProviderAccount(ctx, provider.Account{
				ID: "provider_account:openrouter:memory-context", ProviderKind: "openrouter", AccountKey: "memory-context",
				DisplayName: "Memory context", AuthMethod: provider.AuthSecretInput, IsActive: true, IsDefault: true, Status: provider.StatusAuthenticated,
				Metadata: provider.AccountMetadata{"profiles": profiles}, CreatedAt: time.Now(), UpdatedAt: time.Now(),
			})
			if err != nil {
				t.Fatal(err)
			}
			assignments, err := database.HostedModelAssignments(ctx)
			if err != nil {
				t.Fatal(err)
			}
			for _, assignment := range assignments {
				assignment.ProviderAccountID = account.ID
				assignment.SelectionMode = store.ModelSelectionExplicitProfile
				assignment.ModelProfile = "openai/gpt-5.6-luna"
				if _, err := database.SaveHostedModelAssignment(ctx, assignment); err != nil {
					t.Fatal(err)
				}
			}
			for offset := 0; offset < test.historySize; offset += 5_000 {
				turn, _, err := database.BeginConversationTurn(ctx, conversation.ID,
					strings.Repeat("a", min(5_000, test.historySize-offset)), nil, time.Now())
				if err != nil {
					t.Fatal(err)
				}
				if _, err := database.CompleteConversationTurn(ctx, turn, "Noted.", "", nil, time.Now()); err != nil {
					t.Fatal(err)
				}
			}
			chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
				if len(request.Tools) == 1 && request.Tools[0].Name == memorySubmitTool {
					return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
						Name: memorySubmitTool, Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[],"deletes":[]}`),
					}}}, nil
				}
				return provider.GenerationResult{Text: "Earlier context summarized.", Model: "openai/gpt-5.6-luna"}, nil
			})
			events, err := chat.Subscribe(ctx, conversation.ID)
			if err != nil {
				t.Fatal(err)
			}
			<-events
			memoryEvents := chat.SubscribeMemory(ctx)
			if _, err := chat.SendTurn(ctx, SendTurnInput{ConversationID: conversation.ID, Input: "Continue."}); err != nil {
				t.Fatal(err)
			}
			collectCompletedTurns(t, events, 1)
			if test.wantUpdate {
				deadline := time.After(5 * time.Second)
				for {
					state, err := chat.memory.State()
					if err != nil {
						t.Fatal(err)
					}
					if state.LastConsolidatedSequence > 0 {
						break
					}
					select {
					case <-memoryEvents:
					case <-deadline:
						t.Fatal("compaction did not publish a Memory checkpoint")
					}
				}
			}
			if err := chat.Close(); err != nil {
				t.Fatal(err)
			}
			state, err := chat.memory.State()
			if err != nil {
				t.Fatal(err)
			}
			if got := state.LastConsolidatedSequence > 0; got != test.wantUpdate {
				t.Fatalf("Memory updated = %t, want %t", got, test.wantUpdate)
			}
		})
	}
}

func TestMemoryChangesRejectOpenOrIncompleteOutput(t *testing.T) {
	for _, payload := range []string{
		`{"upserts":[],"metadata_updates":[],"deletes":[],"extra":true}`,
		`{"upserts":[{"path":"root.md","title":"Human","icon":"user","citations":[]}],"metadata_updates":[],"deletes":[]}`,
	} {
		result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			Name: memorySubmitTool, Payload: json.RawMessage(payload),
		}}}
		if _, err := parseMemoryChanges(result, map[string]bool{}, nil, map[string]bool{}); err == nil {
			t.Fatalf("open or incomplete Memory output was accepted: %s", payload)
		}
	}
}

func TestMemoryMetadataCanUpdateCatalogOnlyPageWithoutContentAccess(t *testing.T) {
	chat, _, _ := chatFixture(t)
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if err := chat.memory.Publish(noemamemory.ChangeSet{Upserts: []noemamemory.PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: root.Path, Title: root.Title, Icon: root.Icon, Body: "People."},
		{Path: "people.md", Title: "People", Icon: "file-text", Body: "Alice."},
	}}, noemamemory.State{}); err != nil {
		t.Fatal(err)
	}
	pages, err := chat.memory.ListPages()
	if err != nil {
		t.Fatal(err)
	}
	result := provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
		Name:    memorySubmitTool,
		Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[{"path":"people.md","icon":"users"}],"deletes":[]}`),
	}}}
	changes, err := parseMemoryChanges(
		result, map[string]bool{}, pages, map[string]bool{noemamemory.RootPagePath: true},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(changes.Upserts) != 1 || changes.Upserts[0].Path != "people.md" ||
		changes.Upserts[0].Icon != "users" || changes.Upserts[0].Body != "Alice." {
		t.Fatalf("metadata change = %#v", changes)
	}
	result.ToolCalls[0].Payload = json.RawMessage(`{"upserts":[{"id":"memory:human:people.md","expected_hash":null,"path":"people.md","title":"People","icon":"users","body":"Changed.","citations":[]}],"metadata_updates":[],"deletes":[]}`)
	if _, err := parseMemoryChanges(
		result, map[string]bool{}, pages, map[string]bool{noemamemory.RootPagePath: true},
	); err == nil || !strings.Contains(err.Error(), "catalog-only") {
		t.Fatalf("catalog-only content error = %v", err)
	}
}

func waitForMemoryUpdate(t *testing.T, chat *Chat, events <-chan Event) {
	t.Helper()
	deadline := time.After(5 * time.Second)
	for {
		select {
		case event, open := <-events:
			if !open {
				t.Fatal("Memory event stream closed")
			}
			if event.Kind != EventMemoryChanged {
				t.Fatalf("Memory event kind = %s", event.Kind)
			}
			if !chat.MemoryUpdateStatus().Active {
				return
			}
		case <-deadline:
			t.Fatalf("Memory update timed out with status %#v", chat.MemoryUpdateStatus())
		}
	}
}
