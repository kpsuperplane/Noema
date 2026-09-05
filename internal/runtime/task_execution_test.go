package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskExecutionCompletesPlannerExecutorReviewerLineage(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Complete the exact work.")
	var mu sync.Mutex
	roleCalls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		mu.Lock()
		roleCalls[role]++
		call := roleCalls[role]
		mu.Unlock()
		switch role {
		case "planner":
			if call == 1 {
				return taskToolResult("plan-write", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Task\n\n- [ ] Implement the exact work.\n"}), nil
			}
			return taskToolResult("plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			switch call {
			case 1:
				return taskToolResult("execute-progress", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Task\n\n- [x] Implement the exact work.\n"}), nil
			case 2:
				return taskToolResult("execute-result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "The exact work is complete.\n"}), nil
			default:
				return taskToolResult("execute-finish", taskFinishExecution, map[string]any{}), nil
			}
		default:
			return taskToolResult("review-finish", taskFinishReview, map[string]any{"decision": "approve", "feedback": "The result meets the exact requirement.", "notify_human": true}), nil
		}
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	if current.State != store.TaskCompleted || current.CompletedAt == nil {
		t.Fatalf("completed Task = %#v", current)
	}
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) != 3 {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
	if runs[0].Kind != "reviewer" || runs[0].ReviewRound != 1 || runs[0].ParentRunID != runs[1].ID ||
		runs[1].Kind != "executor" || runs[1].ParentRunID != runs[2].ID || runs[2].Kind != "planner" {
		t.Fatalf("run lineage = %#v", runs)
	}
	for _, run := range runs {
		if run.Status != "completed" || run.ProviderCallCount == 0 || run.EndedAt == nil {
			t.Fatalf("run metrics = %#v", run)
		}
	}
	if result, err := home.ReadTaskFile(chat.home, task.ID, "RESULT.md"); err != nil || result == "" {
		t.Fatalf("result = %q, %v", result, err)
	}
	if review, err := home.ReadTaskFile(chat.home, task.ID, "REVIEW.md"); err != nil || review == "" {
		t.Fatalf("review = %q, %v", review, err)
	}
}

func TestTaskExecutionOpensExplicitHumanGate(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Ask before choosing.")
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return taskToolResult("blocked", taskReportBlocked, map[string]any{
			"gate_kind": "clarification", "question": "Which target should I use?",
			"context_markdown": "Two valid targets remain.", "suggested_answers": []string{"First", "Second"},
		}), nil
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	gate, err := database.TaskGate(context.Background(), current.ActiveGateID)
	if err != nil || gate.Kind != "clarification" || gate.OriginatingRunID == nil || len(gate.SuggestedAnswers) != 2 {
		t.Fatalf("gate = %#v, %v", gate, err)
	}
	runs, _ := database.TaskRuns(context.Background(), task.ID, 10)
	if len(runs) != 1 || runs[0].Status != "waiting_for_approval" {
		t.Fatalf("waiting run = %#v", runs)
	}
}

func TestTaskExecutionRetriesThenOpensRecovery(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Retry bounded failures.")
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		return provider.GenerationResult{}, errors.New("provider unavailable")
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	gate, err := database.TaskGate(context.Background(), current.ActiveGateID)
	if err != nil || gate.Kind != "recovery" || gate.RecoveryReason == nil || *gate.RecoveryReason != "infrastructure_retries_exhausted" {
		t.Fatalf("recovery gate = %#v, %v", gate, err)
	}
	runs, _ := database.TaskRuns(context.Background(), task.ID, 10)
	if len(runs) != 4 {
		t.Fatalf("retry runs = %#v", runs)
	}
	for index, run := range runs {
		wantAttempt := int64(3 - index)
		if run.AttemptIndex != wantAttempt || run.Status != "failed" {
			t.Fatalf("retry %d = %#v", index, run)
		}
	}
}

func TestTaskCancellationFencesActiveProviderResult(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Cancel active work.")
	started := make(chan struct{})
	var once sync.Once
	generator := generatorFunc(func(ctx context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		once.Do(func() { close(started) })
		<-ctx.Done()
		return provider.GenerationResult{}, ctx.Err()
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	select {
	case <-started:
	case <-time.After(5 * time.Second):
		t.Fatal("provider did not start")
	}
	current, err := database.Task(context.Background(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	_, err = database.CancelTask(context.Background(), task.ID, current.Revision, current.Generation, "stop",
		runtimeTaskCommand("cancel_task", "cancel"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	current = waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "cancelled" })
	time.Sleep(25 * time.Millisecond)
	runs, _ := database.TaskRuns(context.Background(), task.ID, 10)
	if current.Generation != 2 || len(runs) != 1 || runs[0].Status != "cancelled" {
		t.Fatalf("cancelled Task = %#v, runs = %#v", current, runs)
	}
}

func TestTaskExecutionReplaysSavedTerminalBeforeProviderRestart(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Resume saved completion.")
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	arguments := json.RawMessage(`{"complexity":"simple"}`)
	call := store.TaskRunItemInput{Kind: "tool_call", Status: "running", Payload: map[string]any{
		"name": taskFinishPlanning, "arguments": arguments, "provider_call_id": "call:saved",
	}}
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{call}, store.TaskRunUsage{ToolCalls: 1}, time.Now()); err != nil {
		t.Fatal(err)
	}
	items, _ := database.TaskRunReplayItems(context.Background(), run.ID)
	result := store.TaskRunItemInput{Kind: "tool_result", Status: "completed", ParentID: items[0].ID, Payload: map[string]any{
		"name": taskFinishPlanning, "arguments": arguments, "provider_call_id": "call:saved", "result": json.RawMessage(`{"finished":true}`), "success": true,
	}}
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{result}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	providerCalls := 0
	generator := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
		providerCalls++
		return taskToolResult("executor-blocked", taskReportBlocked, map[string]any{"gate_kind": "clarification", "question": "Continue?"}), nil
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	runs, _ := database.TaskRuns(context.Background(), task.ID, 10)
	if providerCalls != 1 || len(runs) != 2 || runs[1].ID != run.ID || runs[1].Status != "completed" || runs[0].Kind != "executor" {
		t.Fatalf("restart calls=%d, runs=%#v", providerCalls, runs)
	}
}

func createQueuedRuntimeTask(t *testing.T, database *store.Store, root *os.Root, content string) store.Task {
	t.Helper()
	id, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(context.Background(), id, "Runtime Task", "correlation:runtime:create", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := home.CreatePendingTaskDocument(root, id, content); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(root, id); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(context.Background(), id, task.Revision, task.Generation,
		runtimeTaskCommand("queue_task", id), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	return queued.Task
}

func runtimeTaskCommand(name, key string) store.TaskCommand {
	sum := sha256.Sum256([]byte(name + "\x00" + key))
	return store.TaskCommand{Name: name, ClientMutationID: key, RequestDigest: hex.EncodeToString(sum[:]), CorrelationID: "correlation:runtime:" + key}
}

func taskToolResult(id, name string, arguments map[string]any) provider.GenerationResult {
	payload, _ := json.Marshal(arguments)
	return provider.GenerationResult{ID: "response:" + id, Model: "model-a", FinishReason: "tool_calls",
		Usage: provider.Usage{InputTokens: 4, OutputTokens: 2}, ToolCalls: []provider.GenerationToolCall{{
			ProviderItemID: "item:" + id, ProviderCallID: "call:" + id, ProviderName: name, Name: name, Payload: payload,
		}}}
}

func taskRequestRole(tools []provider.GenerationTool) string {
	for _, tool := range tools {
		switch tool.Name {
		case taskFinishPlanning:
			return "planner"
		case taskFinishExecution:
			return "executor"
		case taskFinishReview:
			return "reviewer"
		}
	}
	return "unknown"
}

func waitRuntimeTask(t *testing.T, database *store.Store, id string, ready func(store.Task) bool) store.Task {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		value, err := database.Task(context.Background(), id)
		if err == nil && ready(value) {
			return value
		}
		time.Sleep(10 * time.Millisecond)
	}
	value, err := database.Task(context.Background(), id)
	runs, _ := database.TaskRuns(context.Background(), id, 10)
	var items []store.TaskRunItem
	if len(runs) > 0 {
		items, _ = database.TaskRunReplayItems(context.Background(), runs[0].ID)
	}
	t.Fatalf("Task did not reach expected state: %#v, %v; runs=%#v items=%#v", value, err, runs, items)
	return store.Task{}
}
