package runtime

import (
	"context"
	"encoding/json"
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
		len(request.Tools) != 1 || request.Tools[0].Name != memorySubmitTool {
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

func TestMemoryAutomaticThresholdUsesPendingSourceSize(t *testing.T) {
	thresholdCharacters := (memoryContextTokens - memoryOutputTokens) * memoryThresholdNumerator /
		memoryThresholdDenominator * memoryCharsPerToken
	base := store.ConversationItem{ID: "item:human", Kind: store.ConversationUserText}
	base.ContentText = strings.Repeat("a", thresholdCharacters-100)
	if memorySourceReachedThreshold([]store.ConversationItem{base}) {
		t.Fatal("source below the automatic threshold was accepted")
	}
	base.ContentText += strings.Repeat("b", 200)
	if !memorySourceReachedThreshold([]store.ConversationItem{base}) {
		t.Fatal("source above the automatic threshold was rejected")
	}
}

func TestCompletedPrimaryTurnSchedulesMemoryAtThreshold(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	root, err := chat.memory.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if len(request.Tools) == 1 && request.Tools[0].Name == memorySubmitTool {
			return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
				Name:    memorySubmitTool,
				Payload: json.RawMessage(`{"upserts":[],"metadata_updates":[],"deletes":[]}`),
			}}}, nil
		}
		return provider.GenerationResult{Text: "Done.", Model: "openai/gpt-5.6-luna"}, nil
	})
	ctx := context.Background()
	turnEvents, err := chat.Subscribe(ctx, conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-turnEvents
	memoryEvents := chat.SubscribeMemory(ctx)
	input := strings.Repeat("a", (memoryContextTokens-memoryOutputTokens)*memoryThresholdNumerator/
		memoryThresholdDenominator*memoryCharsPerToken)
	if _, err := chat.SendTurn(ctx, SendTurnInput{ConversationID: conversation.ID, Input: input}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, turnEvents, 1)
	deadline := time.After(5 * time.Second)
	for {
		select {
		case <-memoryEvents:
			checkpoint, err := chat.memory.State()
			if err != nil {
				t.Fatal(err)
			}
			if checkpoint.LastConsolidatedSequence > 0 {
				if checkpoint.ConversationID != conversation.ID {
					t.Fatalf("automatic Memory checkpoint = %#v", checkpoint)
				}
				published, err := chat.memory.ReadRoot()
				if err != nil || published.Hash != root.Hash {
					t.Fatalf("empty automatic update changed root = %#v, %v", published, err)
				}
				return
			}
		case <-deadline:
			t.Fatal("automatic Memory update did not publish its checkpoint")
		}
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
	eventCount := 0
	for {
		select {
		case event, open := <-events:
			if !open {
				t.Fatal("Memory event stream closed")
			}
			if event.Kind != EventMemoryChanged {
				t.Fatalf("Memory event kind = %s", event.Kind)
			}
			eventCount++
			if eventCount == 2 {
				return
			}
		case <-deadline:
			t.Fatalf("Memory update timed out with status %#v", chat.MemoryUpdateStatus())
		}
	}
}
