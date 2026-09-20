package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func contextTestStore(t *testing.T, window uint32) *store.Store {
	t.Helper()
	database, err := store.Open(t.Context(), filepath.Join(t.TempDir(), "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	profiles, _ := json.Marshal([]provider.ModelProfile{{
		ID: "test", Label: "Test", ContextWindowTokens: &window,
	}})
	_, err = database.CreateProviderAccount(t.Context(), provider.Account{
		ID: "provider_account:openrouter:context-test", ProviderKind: "openrouter",
		AccountKey: "context-test", DisplayName: "Context test", AuthMethod: provider.AuthSecretInput,
		IsActive: true, Status: provider.StatusAuthenticated,
		Metadata: provider.AccountMetadata{"profiles": profiles}, CreatedAt: time.Now(), UpdatedAt: time.Now(),
	})
	if err != nil {
		t.Fatal(err)
	}
	return database
}

func TestContextCountsTextWithoutJSONEscaping(t *testing.T) {
	content := strings.Repeat("\"<page>\"\n世界", 100)
	want := uint32((len([]rune("developer: "+content+"\n")) + 2) / 3)
	if got := CountModelContext(t.Context(), nil, []provider.GenerationMessage{{Role: "developer", Content: content}}, nil, false); got != want {
		t.Fatalf("text tokens = %d, want %d", got, want)
	}
}

func TestLargeNativeToolCatalogFitsAfterCompactionAt128K(t *testing.T) {
	database := contextTestStore(t, 128_000)
	tools := localChatTools()
	for i := 0; i < 45; i++ {
		schema, _ := json.Marshal(map[string]any{"type": "object", "description": strings.Repeat("schema detail ", 260)})
		tools = append(tools, provider.GenerationTool{Name: fmt.Sprintf("docs.read_%d", i), Description: strings.Repeat("tool detail ", 125), InputSchema: schema})
	}
	visibility := toolVisibilityMessage(tools, provider.ToolTransportNative, false)
	active := []provider.GenerationMessage{{Role: "user", Content: "fetch a random page"}, visibility}
	base := developerMessages(nil, "", "", false)
	messages, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
		database: database, accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		base: base, active: active, restoredContext: []provider.GenerationMessage{visibility}, tools: tools, outputReserve: 8192,
		completed: []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("completed history ", 20000)}},
		generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			return provider.GenerationResult{Text: "Earlier discussion."}, nil
		}),
	})
	if err != nil || !compacted {
		t.Fatalf("large catalog compaction: %t, %v", compacted, err)
	}
	if tokens := CountModelContext(t.Context(), nil, messages, tools, false); tokens > 128_000-8192-contextSafetyTokens {
		t.Fatalf("compacted input exceeds budget: %d", tokens)
	}
	for _, tool := range tools {
		if strings.Contains(visibility.Content, tool.Description) || tool.Description == "" || len(tool.InputSchema) == 0 {
			t.Fatal("tool definitions were duplicated or lost")
		}
	}
}

func TestPrepareModelContextCompactsOnceAndDisablesTools(t *testing.T) {
	database := contextTestStore(t, 4_000)
	var summaryRequests, checkpoints int
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		summaryRequests++
		if len(request.Messages) != 2 || request.Messages[0].Role != "system" || request.Messages[0].Content != compactionInstructions(*request.MaxOutputTokens) {
			t.Fatal("compaction omitted exact Rust system instructions")
		}
		if len(request.Tools) != 0 || request.HostedWebSearch || request.StoreResponse ||
			request.ToolTransport != provider.ToolTransportNone || request.ToolChoice != provider.ToolChoiceNone {
			t.Fatalf("summary controls = %#v", request)
		}
		return provider.GenerationResult{Text: "The earlier rounds established the durable decision."}, nil
	})
	completed := make([]provider.GenerationMessage, 6)
	for index := range completed {
		completed[index] = provider.GenerationMessage{Role: "assistant", Content: strings.Repeat(string(rune('a'+index)), 1_100)}
	}
	messages, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
		database: database, generator: generator,
		accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		base:      []provider.GenerationMessage{{Role: "developer", Content: "Current instructions"}},
		completed: completed, active: []provider.GenerationMessage{{Role: "user", Content: "Continue"}},
		tools: []provider.GenerationTool{{Name: "read"}}, hostedWeb: true, outputReserve: 512,
		persist: func(summary string, recent []provider.GenerationMessage) error {
			checkpoints++
			if summary == "" || len(recent) == 0 {
				t.Fatal("checkpoint did not preserve recent context")
			}
			return nil
		},
	})
	if err != nil || !compacted || summaryRequests != 1 || checkpoints != 1 {
		t.Fatalf("compaction = %t, summaries = %d, checkpoints = %d, error = %v", compacted, summaryRequests, checkpoints, err)
	}
	if messages[1].Role != "assistant" || !strings.Contains(messages[1].Content, "durable decision") || messages[len(messages)-1].Content != "Continue" {
		t.Fatalf("compacted messages = %#v", messages)
	}
	call := provider.ReplayToolCall{Name: "read", ProviderCallID: "call"}
	result := provider.ReplayToolResult{Name: "read", ProviderCallID: "call", Success: true}
	_, recent := compactionPrefix([]provider.GenerationMessage{
		{Role: "user", Content: strings.Repeat("old", 1_000)},
		{Role: "hosted_web_search", HostedSearch: &provider.HostedSearch{ID: "search"}},
		{Role: "hosted_web_search", HostedSearch: &provider.HostedSearch{ID: "search-2"}},
		{Role: "assistant", ToolCalls: []provider.ReplayToolCall{call}},
		{Role: "tool", ToolResult: &result},
	}, 1_000, false)
	if len(recent) != 4 || recent[0].Role != "hosted_web_search" || recent[2].Role != "assistant" || recent[3].Role != "tool" {
		t.Fatalf("recent provider round = %#v", recent)
	}
	completedRound, activeRound := splitActiveHistory(recent, recent[3:])
	if len(completedRound) != 0 || len(activeRound) != 4 || activeRound[0].Role != "hosted_web_search" {
		t.Fatalf("active provider round = completed %#v, active %#v", completedRound, activeRound)
	}
}

func TestPrepareModelContextRejectsHardOverflowWithoutHistory(t *testing.T) {
	database := contextTestStore(t, 1_000)
	called := false
	_, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
		database: database,
		generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			called = true
			return provider.GenerationResult{}, nil
		}),
		accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		active: []provider.GenerationMessage{{Role: "user", Content: strings.Repeat("x", 4_000)}}, outputReserve: 128,
	})
	if !errors.Is(err, errContextWindowExceeded) || compacted || called || !strings.Contains(err.Error(), "no completed history") {
		t.Fatalf("overflow = compacted %t, called %t, error %v", compacted, called, err)
	}
	softDatabase := contextTestStore(t, 4_000)
	completed := make([]provider.GenerationMessage, 6)
	for index := range completed {
		completed[index] = provider.GenerationMessage{Role: "assistant", Content: strings.Repeat("a", 1_000)}
	}
	original, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
		database: softDatabase,
		generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			return provider.GenerationResult{Text: strings.Repeat("verbose", 2_000)}, nil
		}),
		accountID: "provider_account:openrouter:context-test", providerKind: "openrouter", model: "test",
		completed: completed, active: []provider.GenerationMessage{{Role: "user", Content: "next"}}, outputReserve: 512,
	})
	if err != nil || compacted || len(original) != len(completed)+1 {
		t.Fatalf("soft fallback = %d messages, compacted %t, error %v", len(original), compacted, err)
	}
}

func TestTaskReplayRestoresCheckpointAndPriorTaskWrite(t *testing.T) {
	arguments := json.RawMessage(`{"path":"TASK.md","content":"updated"}`)
	content := "later output"
	parentID := "call"
	items := []store.TaskRunItem{
		{ID: "call", Kind: "tool_call", Payload: map[string]any{"name": taskFilesWrite, "arguments": arguments}},
		{Kind: "tool_result", ParentID: &parentID, Payload: map[string]any{"success": true}},
		{Kind: "context_checkpoint", Payload: map[string]any{
			"summary":         "Earlier work is complete.",
			"recent_messages": []provider.GenerationMessage{{Role: "tool", ToolResult: &provider.ReplayToolResult{Name: "read", Success: true}}},
		}},
		{Kind: "assistant_output", Content: &content, Payload: map[string]any{}},
	}
	replay, wroteTask, err := (&TaskExecution{}).replayTaskItems(t.Context(), store.Task{}, store.TaskRun{}, items)
	if err != nil || !wroteTask || len(replay) != 3 || replay[0].Role != "assistant" ||
		!strings.Contains(replay[0].Content, "Earlier work") || replay[1].ToolResult == nil || replay[2].Content != content {
		t.Fatalf("replay = %#v, wrote Task = %t, error = %v", replay, wroteTask, err)
	}
}

func TestOptionalCompactionFailurePreservesFittingHistory(t *testing.T) {
	for _, size := range []int{8000, 16000} {
		t.Run(fmt.Sprint(size), func(t *testing.T) {
			database := contextTestStore(t, 4000)
			calls := 0
			generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
				calls++
				return provider.GenerationResult{}, errors.New("summary unavailable")
			})
			completed := []provider.GenerationMessage{{Role: "assistant", Content: strings.Repeat("x", size)}}
			active := []provider.GenerationMessage{{Role: "user", Content: "Continue"}}
			messages, compacted, err := prepareModelContext(t.Context(), modelContextRequest{
				database: database, generator: generator, accountID: "provider_account:openrouter:context-test",
				providerKind: "openrouter", model: "test", completed: completed, active: active, outputReserve: 512,
				persist: func(string, []provider.GenerationMessage) error {
					t.Fatal("failed summary must not replace history")
					return nil
				},
			})
			if calls != 1 || compacted {
				t.Fatalf("calls=%d compacted=%v", calls, compacted)
			}
			if size == 8000 {
				if err != nil || len(messages) != 2 || messages[0].Content != completed[0].Content || messages[1].Content != active[0].Content {
					t.Fatalf("fitting history was not retained: %v", err)
				}
			} else if err == nil || messages != nil {
				t.Fatal("oversized history must still fail admission")
			}
		})
	}
}

func TestFixedCatalogDoesNotCauseRepeatedSoftCompaction(t *testing.T) {
	database := contextTestStore(t, 4000)
	tools := []provider.GenerationTool{{Name: "docs.read", Description: strings.Repeat("x", 8400)}}
	summaries := 0
	request := modelContextRequest{database: database, accountID: "provider_account:openrouter:context-test",
		providerKind: "openrouter", model: "test", tools: tools, outputReserve: 512,
		generator: generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
			summaries++
			return provider.GenerationResult{Text: "Earlier discussion."}, nil
		})}
	request.completed = []provider.GenerationMessage{{Role: "assistant", Content: "Earlier discussion."}}
	for _, input := range []string{"Yem", "Yum"} {
		request.active = []provider.GenerationMessage{{Role: "user", Content: input}}
		messages, compacted, err := prepareModelContext(t.Context(), request)
		if err != nil || compacted || summaries != 0 || len(messages) != len(request.completed)+1 {
			t.Fatalf("short turn: compacted=%v summaries=%d error=%v", compacted, summaries, err)
		}
		request.completed = messages
	}
	request.completed = append(request.completed, provider.GenerationMessage{Role: "assistant", Content: strings.Repeat("old ", 1500)})
	messages, compacted, err := prepareModelContext(t.Context(), request)
	if err != nil || !compacted || summaries == 0 || CountModelContext(t.Context(), nil, messages, tools, false) > 4000-512-contextSafetyTokens {
		t.Fatalf("hard limit: compacted=%v summaries=%d error=%v", compacted, summaries, err)
	}
}
