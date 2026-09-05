package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	mcpsdk "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestTaskExecutionUsesGovernedMCPActionAndResumesExactRun(t *testing.T) {
	chat, database, _ := chatFixture(t)
	remoteCalls := 0
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "mail", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "send", Description: "Send one message",
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: false, IdempotentHint: false,
			DestructiveHint: runtimeBool(true), OpenWorldHint: runtimeBool(true)}},
		func(context.Context, *mcpsdk.CallToolRequest, struct {
			Text string `json:"text"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			remoteCalls++
			return nil, map[string]any{"sent": true}, nil
		})
	mcpHandler := mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil)
	var unauthorized atomic.Bool
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if unauthorized.Load() {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		mcpHandler.ServeHTTP(w, request)
	}))
	defer httpServer.Close()
	paths, _ := home.FromRoot(chat.home.Name())
	service, err := noemamcp.NewService(paths, database, false)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	setup, err := service.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Mail", TransportKind: "streamable_http",
		URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("setup = %#v, %v", setup, err)
	}
	if _, err = service.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision, 0,
		"allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, _ := service.Bindings(t.Context())
	unauthorized.Store(true)
	task := createQueuedRuntimeTask(t, database, chat.home, "Send the approved message.")
	roleCalls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		roleCalls[role]++
		switch role {
		case "planner":
			if roleCalls[role] == 1 {
				return taskToolResult("plan-write", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Task\n\nSend the approved message.\n"}), nil
			}
			return taskToolResult("plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			switch roleCalls[role] {
			case 1:
				if !taskRequestHasTool(request.Tools, bindings[0].Name) {
					t.Fatal("Task MCP tool was not advertised")
				}
				return taskToolResult("send", bindings[0].Name, map[string]any{"text": "approved"}), nil
			case 2:
				return taskToolResult("progress", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Task\n\nMessage sent.\n"}), nil
			case 3:
				return taskToolResult("result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "Message sent.\n"}), nil
			default:
				return taskToolResult("finish", taskFinishExecution, map[string]any{}), nil
			}
		default:
			return taskToolResult("review", taskFinishReview, map[string]any{"decision": "approve", "feedback": "Complete.", "notify_human": false}), nil
		}
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home, service)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	var action store.ActionRequest
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		actions, loadErr := database.PendingActionRequests(t.Context(), "human:local", nil, &task.ID, 10)
		if loadErr == nil && len(actions) == 1 {
			action = actions[0]
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if action.TaskID != task.ID || action.RunID == "" || remoteCalls != 0 {
		runs, _ := database.TaskRuns(t.Context(), task.ID, 10)
		current, _ := database.Task(t.Context(), task.ID)
		t.Fatalf("pending Task action = %#v; remote calls = %d; Task = %#v; runs = %#v", action, remoteCalls, current, runs)
	}
	if _, err = runtime.ResolveActionRequest(t.Context(), action.ID, action.Revision, "human:local", "approve"); err != nil {
		t.Fatal(err)
	}
	auth, err := database.PendingMCPAuthRequests(t.Context(), "human:local", nil, &task.ID, 10)
	if err != nil || len(auth) != 1 || auth[0].ActionID != action.ID {
		t.Fatalf("Task authentication = %#v, %v", auth, err)
	}
	attemptID := "mcp_oauth:" + strings.Repeat("a", 32)
	now := time.Now()
	if err = database.CreateMCPOAuthAttempt(t.Context(), store.MCPOAuthAttempt{ID: attemptID, OwnerHumanID: "human:local",
		ServerID: setup.Server.ID, ExpiresAt: now.Add(time.Hour), CreatedAt: now, UpdatedAt: now}); err != nil {
		t.Fatal(err)
	}
	if _, err = database.BeginMCPAuthentication(t.Context(), auth[0].ID, auth[0].Revision, "human:local", attemptID, now); err != nil {
		t.Fatal(err)
	}
	unauthorized.Store(false)
	if _, err = service.Continue(t.Context(), setup.Server.ID, noemamcp.SecretMaterial{}); err != nil {
		t.Fatal(err)
	}
	if handled, resumeErr := runtime.ResumeMCPAuthentication(t.Context(), attemptID); resumeErr != nil || !handled {
		t.Fatalf("resume Task authentication = %t, %v", handled, resumeErr)
	}
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	if current.State != store.TaskCompleted || remoteCalls != 1 {
		t.Fatalf("completed Task = %#v; remote calls = %d", current, remoteCalls)
	}
}

func TestTaskExecutionCompletesPlannerExecutorReviewerLineage(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Complete the exact work.")
	var mu sync.Mutex
	roleCalls := map[string]int{}
	requestProblem := ""
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		mu.Lock()
		roleCalls[role]++
		call := roleCalls[role]
		mu.Unlock()
		switch role {
		case "planner":
			if call == 1 {
				if !request.HostedWebSearch || !taskRequestHasTool(request.Tools, fileParseName) ||
					len(request.Messages) < 2 || request.Messages[0].Role != "system" ||
					strings.Contains(request.Messages[0].Content, "Complete the exact work.") ||
					request.Messages[1].Role != "user" || !strings.Contains(request.Messages[1].Content, "Complete the exact work.") {
					mu.Lock()
					requestProblem = "Task request did not preserve the tool, search, or data-role contract"
					mu.Unlock()
				}
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
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	if current.State != store.TaskCompleted || current.CompletedAt == nil {
		t.Fatalf("completed Task = %#v", current)
	}
	mu.Lock()
	problem := requestProblem
	mu.Unlock()
	if problem != "" {
		t.Fatal(problem)
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
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
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
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
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
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
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
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
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

func TestTaskExecutionDoesNotRepeatIncompleteToolCall(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Do not repeat an uncertain write.")
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(context.Background(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	assistant := store.TaskRunItemInput{Kind: "assistant_output", Status: "completed", Content: "I will write the file."}
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{assistant}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	arguments := json.RawMessage(`{"path":"uncertain.txt","content":"must not appear"}`)
	call := store.TaskRunItemInput{Kind: "tool_call", Status: "running", Payload: map[string]any{
		"name": taskFilesWrite, "arguments": arguments, "provider_call_id": "call:uncertain", "provider_name": taskFilesWrite,
	}}
	if err := database.AppendTaskRunItems(context.Background(), run.ID, run.Generation, []store.TaskRunItemInput{call}, store.TaskRunUsage{ToolCalls: 1}, time.Now()); err != nil {
		t.Fatal(err)
	}
	var replayedUncertain atomic.Bool
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		for _, message := range request.Messages {
			if message.ToolResult != nil && message.ToolResult.ProviderCallID == "call:uncertain" && !message.ToolResult.Success && strings.Contains(string(message.ToolResult.Payload), "uncertain_outcome") {
				replayedUncertain.Store(true)
			}
		}
		return taskToolResult("blocked-after-replay", taskReportBlocked, map[string]any{"gate_kind": "clarification", "question": "How should I recover?"}), nil
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	if !replayedUncertain.Load() {
		t.Fatal("incomplete tool result was not replayed as uncertain")
	}
	if _, err := home.ReadTaskFile(chat.home, task.ID, "uncertain.txt"); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("uncertain write was repeated: %v", err)
	}
}

func TestTaskExecutionTimeoutBecomesTerminalRecovery(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Bound provider time.")
	previousLimit := taskActiveLimit
	taskActiveLimit = 2 * time.Second
	generator := generatorFunc(func(ctx context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		<-ctx.Done()
		return provider.GenerationResult{}, ctx.Err()
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		runtime.Close()
		taskActiveLimit = previousLimit
	}()
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) != 1 || runs[0].ErrorCode == nil || *runs[0].ErrorCode != "active_time_limit" {
		t.Fatalf("timed out run = %#v, %v", runs, err)
	}
}

func TestTaskExecutionPreservesProviderReplayItems(t *testing.T) {
	reasoning := json.RawMessage(`{"type":"reasoning.encrypted","data":"ordinary-id"}`)
	result := provider.GenerationResult{
		ID: "response:ordinary", Model: "model:ordinary", Text: "Answer with a source.",
		Reasoning: []provider.GenerationReasoning{{ProviderDetails: []json.RawMessage{reasoning}}},
		Citations: []provider.Citation{{Title: "Ordinary source", URL: "https://example.test/path?value=ordinary"}},
		Searches:  []provider.HostedSearch{{ID: "search:ordinary", Name: "web_search", Status: "completed", Sources: []provider.WebSource{{Title: "Ordinary source", URL: "https://example.test/path?value=ordinary"}}}},
	}
	encoded, err := json.Marshal(taskAssistantPayload(result))
	if err != nil {
		t.Fatal(err)
	}
	var stored map[string]any
	if err := json.Unmarshal(encoded, &stored); err != nil {
		t.Fatal(err)
	}
	content := result.Text
	runtime := &TaskExecution{}
	messages, _, err := runtime.replayTaskItems(context.Background(), store.Task{}, store.TaskRun{}, []store.TaskRunItem{{Kind: "assistant_output", Content: &content, Payload: stored}})
	if err != nil || len(messages) != 2 || messages[0].HostedSearch == nil || len(messages[0].HostedSearch.Sources) != 1 ||
		messages[0].HostedSearch.Sources[0].URL != "https://example.test/path?value=ordinary" ||
		len(messages[1].ReasoningDetails) != 1 || !strings.Contains(string(messages[1].ReasoningDetails[0]), "ordinary-id") ||
		!strings.Contains(string(encoded), "https://example.test/path?value=ordinary") {
		t.Fatalf("provider replay = %#v, payload=%s, %v", messages, encoded, err)
	}
}

func TestTaskExecutionValidatesTaskDocumentsBeforePublication(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Keep Task documents consistent.")
	if err := home.WriteTaskFile(chat.home, task.ID, "REVIEW.md", "Previous review.\n"); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := database.ClaimTaskExecution(context.Background(), time.Now())
	if err != nil || !found {
		t.Fatalf("claim = %#v, %t, %v", run, found, err)
	}
	runtime := &TaskExecution{database: database, root: chat.home}
	reviewRun := run
	reviewRun.Kind = "reviewer"
	_, success, terminal, _ := runtime.executeTaskTool(context.Background(), task, reviewRun,
		taskFinishReview, json.RawMessage(`{"decision":"invalid","feedback":"New review.","notify_human":false}`), false)
	if success || terminal {
		t.Fatal("invalid review was accepted")
	}
	if review, _ := home.ReadTaskFile(chat.home, task.ID, "REVIEW.md"); review != "Previous review.\n" {
		t.Fatalf("invalid review changed REVIEW.md: %q", review)
	}
	validReview := json.RawMessage(`{"decision":"approve","feedback":"New review.","notify_human":false}`)
	if err := runtime.finishTaskTerminal(context.Background(), run, taskFinishReview, validReview); err == nil {
		t.Fatal("stale review transition succeeded")
	}
	if review, _ := home.ReadTaskFile(chat.home, task.ID, "REVIEW.md"); review != "Previous review.\n" {
		t.Fatalf("failed transition changed REVIEW.md: %q", review)
	}
	executorRun := run
	executorRun.Kind = "executor"
	_, success, terminal, _ = runtime.executeTaskTool(context.Background(), task, executorRun, taskContinueExecution, json.RawMessage(`{}`), true)
	if !success || !terminal {
		t.Fatal("continuation required RESULT.md")
	}
	parsed, success, _, _ := runtime.executeTaskTool(context.Background(), task, executorRun, fileParseName,
		json.RawMessage(`{"path":"TASK.md","max_chars":4000}`), true)
	if !success || !strings.Contains(string(parsed), "Keep Task documents consistent.") {
		t.Fatalf("Task file parse = %s, %t", parsed, success)
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

func taskRequestHasTool(tools []provider.GenerationTool, name string) bool {
	for _, tool := range tools {
		if tool.Name == name {
			return true
		}
	}
	return false
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
