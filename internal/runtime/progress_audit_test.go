package runtime

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestProgressAuditDigestOmitsPrivateToolDataAndBoundsText(t *testing.T) {
	secret := "private-payload-value"
	progress := newToolProgress(strings.Repeat("goal", 100))
	call := provider.GenerationToolCall{Name: "memory.search", Payload: json.RawMessage(`{"query":"` + secret + `"}`)}
	progress.observe(call, json.RawMessage(`{"content":"`+secret+`"}`), true, false)
	digest, err := json.Marshal(progress.digest(1))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(digest), secret) || utf8.RuneCountInString(progress.digest(1).UserGoal) > progressTextLimit {
		t.Fatalf("unsafe or unbounded digest = %s", digest)
	}
	if _, err := parseProgressAudit(json.RawMessage(`{"decision":"continue","user_summary":"Useful progress.","next_goal":null}`)); err != nil {
		t.Fatal(err)
	}
	if _, err := parseProgressAudit(json.RawMessage(`{"decision":"continue","user_summary":"ok","next_goal":null,"extra":true}`)); err == nil {
		t.Fatal("extra audit field was accepted")
	}
	repeated := newToolProgress("check")
	first := provider.GenerationToolCall{Name: "read", Payload: json.RawMessage(`{"a":1,"b":2}`)}
	second := provider.GenerationToolCall{Name: "read", Payload: json.RawMessage(`{"b":2,"a":1}`)}
	repeated.observe(first, json.RawMessage(`{"same":true}`), true, false)
	repeated.observe(second, json.RawMessage(`{"same":true}`), false, false)
	repeated.observe(second, json.RawMessage(`{"same":true}`), false, false)
	if repeated.whole.RepeatedArgumentCount != 1 {
		t.Fatalf("semantic repeat count = %d", repeated.whole.RepeatedArgumentCount)
	}
}

func TestProgressAuditUsesIndependentRequiredToolRequest(t *testing.T) {
	_, database, _ := chatFixture(t)
	var got provider.GenerateRequest
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		got = request
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			Name:    progressAuditToolName,
			Payload: json.RawMessage(`{"decision":"finalize","user_summary":"Ready.","next_goal":null}`),
		}}}, nil
	})
	progress := newToolProgress("answer safely")
	outcome, err := runProgressAudit(context.Background(), database, func(kind string) (provider.Generator, error) {
		if kind != "openrouter" {
			t.Fatalf("provider kind = %q", kind)
		}
		return generator, nil
	}, progress.digest(8))
	if err != nil || outcome.Decision != "finalize" {
		t.Fatalf("audit outcome = %#v, %v", outcome, err)
	}
	if got.ConversationID != "" || got.PreviousResponseID != "" || got.StoreResponse ||
		got.ToolChoice != provider.ToolChoiceRequired || len(got.Tools) != 1 || got.Tools[0].Name != progressAuditToolName {
		t.Fatalf("audit request was not independent and required: %#v", got)
	}
}

func TestProgressAuditActivityIsReadableButExcludedFromProviderReplay(t *testing.T) {
	_, database, conversation := chatFixture(t)
	turn, _, err := database.BeginConversationTurn(context.Background(), conversation.ID, "Inspect it.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.AppendConversationActivity(context.Background(), turn, 8,
		"tool_progress_audit", "Still making progress", "Found useful evidence.", "completed",
		map[string]any{"decision": "continue"}, time.Now())
	if err != nil || item.Kind != "activity" {
		t.Fatalf("activity = %#v, %v", item, err)
	}
	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, candidate := range page.Items {
		found = found || candidate.ID == item.ID
	}
	if !found {
		t.Fatal("activity is absent from readable transcript")
	}
	replay, err := database.ConversationProviderItems(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	for _, candidate := range replay {
		if candidate.ID == item.ID {
			t.Fatal("activity leaked into provider replay")
		}
	}
}

func TestTaskProgressAuditStoresNoticeUnderCurrentRunFence(t *testing.T) {
	chat, database, _ := chatFixture(t)
	createQueuedRuntimeTask(t, database, chat.home, "Inspect the current work.")
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim run = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	generator := generatorFunc(func(_ context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{ToolCalls: []provider.GenerationToolCall{{
			Name:    progressAuditToolName,
			Payload: json.RawMessage(`{"decision":"continue","user_summary":"Evidence is growing.","next_goal":"Verify it."}`),
		}}}, nil
	})
	runtime := &TaskExecution{database: database, openRouter: generator, codex: generator, openAI: generator}
	progress := newToolProgress("Inspect the current work.")
	instruction, terminalOnly, err := runtime.afterTaskTool(context.Background(), run, &progress,
		provider.GenerationToolCall{Name: taskFilesRead, Payload: json.RawMessage(`{"path":"TASK.md"}`)},
		json.RawMessage(`{"content":"bounded"}`), true, false, taskProgressAuditInterval)
	if err != nil || instruction != "" || terminalOnly {
		t.Fatalf("Task audit result = %q, %t, %v", instruction, terminalOnly, err)
	}
	items, err := database.TaskRunReplayItems(context.Background(), run.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 || items[0].Kind != "progress_notice" ||
		items[0].Content == nil || *items[0].Content != "Evidence is growing." {
		t.Fatalf("Task audit items = %#v", items)
	}
}
