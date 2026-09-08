package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/home"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/webtool"
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
	service, err := noemamcp.NewService(paths, database, false, nil)
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
	binding := runtimeDiscoveredMCPBinding(t, bindings)
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
				if !taskRequestHasTool(request.Tools, binding.Name) {
					t.Fatal("Task MCP tool was not advertised")
				}
				return taskToolResult("send", binding.Name, map[string]any{"text": "approved"}), nil
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
	profile, err := database.RuntimeDebugProfile(t.Context(), store.RuntimeDebugScope{Kind: "task_run", ID: action.RunID})
	if err != nil {
		t.Fatal(err)
	}
	preparation, execution := 0, 0
	for _, span := range profile.Spans {
		if span.Metadata.CorrelationID == "call:send" {
			if span.Metadata.Phase == "review_preparation" {
				preparation++
			}
			if span.Metadata.Phase == "execution" {
				execution++
			}
		}
	}
	if preparation != 1 || execution != 1 {
		t.Fatalf("reviewed Task tool phase counts = %d, %d", preparation, execution)
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

func TestTaskExecutionCreatesReviewedPrivatePacketAndUploadsOnce(t *testing.T) {
	chat, database, _ := chatFixture(t)
	packet := "# Private Invoice Packet\n\n- Reference: INVOICE-AUDIT-42\n- Client: Café 日本語 Workshop\n- Quantity: 3\n- Unit price: EUR 125.50\n- Handling: EUR 40.00\n- Calculated amount: EUR 416.50\n- Tax: Not supplied; not included.\n"
	remoteCalls := 0
	var received struct {
		Destination       string `json:"destination"`
		ArtifactID        string `json:"artifact_id"`
		ArtifactVersionID string `json:"artifact_version_id"`
		Packet            string `json:"packet_markdown"`
	}
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "packet-sink", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "audit_upload_packet", Description: "Upload one approved packet",
		Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: false, IdempotentHint: false, OpenWorldHint: runtimeBool(true)}},
		func(_ context.Context, _ *mcpsdk.CallToolRequest, input struct {
			Destination       string `json:"destination"`
			ArtifactID        string `json:"artifact_id"`
			ArtifactVersionID string `json:"artifact_version_id"`
			Packet            string `json:"packet_markdown"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			remoteCalls++
			received.Destination, received.ArtifactID, received.ArtifactVersionID, received.Packet = input.Destination, input.ArtifactID, input.ArtifactVersionID, input.Packet
			return nil, map[string]any{"receipt": "packet-receipt-42"}, nil
		})
	httpServer := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(httpServer.Close)
	paths, err := home.FromRoot(chat.home.Name())
	if err != nil {
		t.Fatal(err)
	}
	mcpService, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mcpService.Close)
	setup, err := mcpService.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Packet sink", TransportKind: "streamable_http",
		URL: httpServer.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("MCP setup = %#v, %v", setup, err)
	}
	if _, err = mcpService.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision, 0,
		"allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := mcpService.Bindings(t.Context())
	if err != nil || len(bindings) < 2 {
		t.Fatalf("MCP bindings = %#v, %v", bindings, err)
	}
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	task := createQueuedRuntimeTask(t, database, chat.home, "Prepare the private invoice packet and upload it once.\n\n"+packet)
	roleCalls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		roleCalls[role]++
		switch role {
		case "planner":
			if roleCalls[role] == 1 {
				return taskToolResult("packet-plan", taskFilesWrite, map[string]any{"path": "TASK.md", "content": packet}), nil
			}
			return taskToolResult("packet-plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			switch roleCalls[role] {
			case 1:
				return taskToolResult("packet-create", artifactCreateLocalName, map[string]any{
					"title": "Private Invoice Packet", "artifact_kind": "document", "filename": "invoice-packet.md",
					"media_type": "text/markdown", "versions": []any{map[string]any{"content": packet}},
				}), nil
			case 2:
				var artifactID, versionID string
				for _, messages := range [][]provider.GenerationMessage{request.Messages, request.ReplayMessages} {
					for _, message := range messages {
						if message.ToolResult == nil || message.ToolResult.Name != artifactCreateLocalName || !message.ToolResult.Success {
							continue
						}
						var value map[string]any
						if json.Unmarshal(message.ToolResult.Payload, &value) == nil {
							artifactID, _ = value["artifact_id"].(string)
							versionID, _ = value["artifact_version_id"].(string)
						}
					}
				}
				if artifactID == "" || versionID == "" {
					t.Fatalf("artifact result missing from Task replay: messages=%#v replay=%#v", request.Messages, request.ReplayMessages)
				}
				payload, _ := json.Marshal(map[string]any{"destination": "audit-packet-box", "artifact_id": artifactID,
					"artifact_version_id": versionID, "packet_markdown": packet})
				return provider.GenerationResult{ID: "response:packet-upload", Model: "model-a", FinishReason: "tool_calls",
					ToolCalls: []provider.GenerationToolCall{{ProviderItemID: "item:packet-upload", ProviderCallID: "call:packet-upload",
						ProviderName: binding.Name, Name: binding.Name, Payload: payload}}}, nil
			case 3:
				return taskToolResult("packet-progress", taskFilesWrite, map[string]any{"path": "TASK.md", "content": packet + "\nStatus: uploaded once.\n"}), nil
			case 4:
				return taskToolResult("packet-result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "Uploaded packet-receipt-42 once.\n"}), nil
			case 5:
				return taskToolResult("packet-finish", taskFinishExecution, map[string]any{}), nil
			}
		default:
			return taskToolResult("packet-review", taskFinishReview, map[string]any{"decision": "approve", "feedback": "The packet and receipt are exact.", "notify_human": false}), nil
		}
		return provider.GenerationResult{Text: "unexpected"}, nil
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home, mcpService)
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
	if action.TaskID != task.ID || remoteCalls != 0 || action.State != store.ActionAwaitingApproval {
		t.Fatalf("pending packet action = %#v; remote calls = %d", action, remoteCalls)
	}
	if action.Arguments["destination"] != "audit-packet-box" || action.Arguments["packet_markdown"] != packet ||
		action.Arguments["artifact_id"] == "" || action.Arguments["artifact_version_id"] == "" {
		t.Fatalf("review packet arguments = %#v", action.Arguments)
	}
	approved, err := runtime.ResolveActionRequest(t.Context(), action.ID, action.Revision, "human:local", "approve")
	if err != nil || approved.State != store.ActionSucceeded {
		t.Fatalf("approved packet action = %#v, %v", approved, err)
	}
	completed := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	if completed.State != store.TaskCompleted || remoteCalls != 1 {
		t.Fatalf("completed packet Task = %#v; remote calls = %d", completed, remoteCalls)
	}
	if received.Destination != "audit-packet-box" || received.ArtifactID != action.Arguments["artifact_id"] ||
		received.ArtifactVersionID != action.Arguments["artifact_version_id"] || received.Packet != packet {
		t.Fatalf("upload receipt = %#v; action = %#v", received, action.Arguments)
	}
	artifacts, err := database.ArtifactsForOwner(t.Context(), store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID}, 10)
	if err != nil || len(artifacts) != 1 {
		t.Fatalf("Task artifacts = %#v, %v", artifacts, err)
	}
	artifactService, err := artifact.New(chat.home, database, nil)
	if err != nil {
		t.Fatal(err)
	}
	file, err := artifactService.Read(artifacts[0].Artifact, artifacts[0].CurrentVersion)
	if err != nil || string(file.Bytes) != packet {
		t.Fatalf("published packet = %q, %v", file.Bytes, err)
	}
}

func TestTaskExecutionCompletesPlannerExecutorReviewerLineage(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "Complete the exact work.")
	var mu sync.Mutex
	roleCalls := map[string]int{}
	requestProblem := ""
	generation := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(request.Tools)
		mu.Lock()
		roleCalls[role]++
		call := roleCalls[role]
		if request.HostedWebSearch != (role == "executor") {
			requestProblem = "hosted web search did not match the Task role"
		}
		mu.Unlock()
		if call > 1 && (request.PreviousResponseID == "" || len(request.ReplayMessages) <= len(request.Messages)) {
			mu.Lock()
			requestProblem = "Task session did not keep incremental input with full replay"
			mu.Unlock()
		}
		switch role {
		case "planner":
			if call == 1 {
				if !taskRequestHasTool(request.Tools, fileParseName) ||
					len(request.Messages) < 3 || request.Messages[0].Role != "system" || !request.Messages[0].Instructions || request.Messages[1].Instructions || request.Messages[1].Role != "system" ||
					strings.Contains(request.Messages[0].Content, "Complete the exact work.") ||
					request.Messages[2].Role != "user" || !strings.Contains(request.Messages[2].Content, "Complete the exact work.") {
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
	generator := &sessionTestGenerator{generate: generation, closed: make(chan struct{}, 3)}
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	for range 3 {
		<-generator.closed
	}
	if current.State != store.TaskCompleted || current.CompletedAt == nil {
		t.Fatalf("completed Task = %#v", current)
	}
	if generator.opens != 3 || generator.closes != 3 || generator.direct != 0 {
		t.Fatalf("Task sessions = opened %d, closed %d, direct %d", generator.opens, generator.closes, generator.direct)
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

func TestTaskExecutionRunsQueuedTasksConcurrentlyAndKeepsFilesIsolated(t *testing.T) {
	chat, database, _ := chatFixture(t)
	tasks := []store.Task{
		createQueuedRuntimeTask(t, database, chat.home, "# Task\n\nCONCURRENT_ONE\n"),
		createQueuedRuntimeTask(t, database, chat.home, "# Task\n\nCONCURRENT_TWO\n"),
	}
	started := make(chan string, len(tasks))
	generationErrors := make(chan string, len(tasks))
	release := make(chan struct{})
	var mu sync.Mutex
	calls := map[string]map[string]int{}
	generation := generatorFunc(func(ctx context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		marker := ""
		for index, task := range tasks {
			runs, _ := database.TaskRuns(context.Background(), task.ID, 20)
			for _, run := range runs {
				if run.ID == request.ConversationID {
					marker = []string{"ONE", "TWO"}[index]
				}
			}
		}
		if marker == "" {
			select {
			case generationErrors <- fmt.Sprintf("marker missing for run %s", request.ConversationID):
			default:
			}
			return provider.GenerationResult{}, errors.New("concurrent Task marker missing")
		}
		role := taskRequestRole(request.Tools)
		mu.Lock()
		if calls[marker] == nil {
			calls[marker] = map[string]int{}
		}
		calls[marker][role]++
		call := calls[marker][role]
		mu.Unlock()
		if role == "planner" && call == 1 {
			started <- marker
			select {
			case <-release:
			case <-ctx.Done():
				return provider.GenerationResult{}, ctx.Err()
			}
			return taskToolResult(marker+"-plan-write", taskFilesWrite, map[string]any{
				"path": "TASK.md", "content": "# Task\n\n" + marker + " planned.\n",
			}), nil
		}
		switch role {
		case "planner":
			return taskToolResult(marker+"-plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			if call == 1 {
				return taskToolResult(marker+"-execute-progress", taskFilesWrite, map[string]any{
					"path": "TASK.md", "content": "# Task\n\n" + marker + " executing.\n",
				}), nil
			}
			if call == 2 {
				return taskToolResult(marker+"-execute-result", taskFilesWrite, map[string]any{
					"path": "RESULT.md", "content": "Result " + marker + "\n",
				}), nil
			}
			return taskToolResult(marker+"-execute-finish", taskFinishExecution, map[string]any{}), nil
		default:
			return taskToolResult(marker+"-review-finish", taskFinishReview, map[string]any{
				"decision": "approve", "feedback": "Complete.", "notify_human": false,
			}), nil
		}
	})
	runtime, err := NewTaskExecution(context.Background(), database, generation, generation, generation, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	defer runtime.Close()
	deadline := time.NewTimer(5 * time.Second)
	defer deadline.Stop()
	startedCount := 0
	for startedCount < len(tasks) {
		select {
		case <-started:
			startedCount++
		case message := <-generationErrors:
			t.Fatal(message)
		case <-deadline.C:
			states := make([]store.Task, 0, len(tasks))
			runStates := make([]any, 0, len(tasks))
			for _, task := range tasks {
				current, _ := database.Task(t.Context(), task.ID)
				states = append(states, current)
				runs, _ := database.TaskRuns(t.Context(), task.ID, 10)
				runStates = append(runStates, runs)
			}
			t.Fatalf("planner calls did not overlap: started=%d states=%#v runs=%#v", startedCount, states, runStates)
		}
	}
	close(release)
	for index, task := range tasks {
		current := waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
		if current.State != store.TaskCompleted {
			t.Fatalf("Task %s state = %s", task.ID, current.State)
		}
		result, readErr := home.ReadTaskFile(chat.home, task.ID, "RESULT.md")
		expected := []string{"ONE", "TWO"}[index]
		if readErr != nil || result != "Result "+expected+"\n" {
			t.Fatalf("Task %s result = %q, err = %v, expected = %s", task.ID, result, readErr, expected)
		}
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

func TestProgressFinalizationAllowsExecutorToSaveResult(t *testing.T) {
	chat, database, _ := chatFixture(t)
	policy, err := database.TaskExecutionPolicy(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	policy.ProgressAuditInterval = 1
	if _, err = database.UpdateTaskExecutionPolicy(t.Context(), policy); err != nil {
		t.Fatal(err)
	}
	task := createQueuedRuntimeTask(t, database, chat.home, "Save the preparation note and submit it for review.")
	planning, execution := 0, 0
	inExecutor := false
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if taskRequestHasTool(request.Tools, progressAuditToolName) {
			decision := "continue"
			if inExecutor {
				decision = "finalize"
			}
			return taskToolResult("progress", progressAuditToolName, map[string]any{"decision": decision, "user_summary": "Work is ready.", "next_goal": nil}), nil
		}
		if taskRequestHasTool(request.Tools, taskFinishPlanning) {
			planning++
			if planning == 1 {
				return taskToolResult("plan", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Plan\nSave the preparation note."}), nil
			}
			return taskToolResult("planned", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		}
		if taskRequestHasTool(request.Tools, taskFinishExecution) {
			inExecutor = true
			execution++
			if execution == 1 {
				return taskToolResult("progress-save", taskFilesWrite, map[string]any{"path": "TASK.md", "content": "# Progress\nThe preparation note is ready."}), nil
			}
			if execution == 2 {
				if !taskRequestHasTool(request.Tools, taskFilesWrite) {
					t.Fatal("progress finalization removed the required result write")
				}
				return taskToolResult("result-save", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "Preparation note saved. Missing evidence remains explicit."}), nil
			}
			return taskToolResult("finished", taskFinishExecution, map[string]any{}), nil
		}
		return taskToolResult("reviewed", taskFinishReview, map[string]any{"decision": "approve", "feedback": "The saved result matches the request.", "notify_human": false}), nil
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	defer runtime.Close()
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	result, err := home.ReadTaskFile(chat.home, task.ID, "RESULT.md")
	if err != nil || result != "Preparation note saved. Missing evidence remains explicit." {
		t.Fatalf("saved result = %q, %v", result, err)
	}
}

func TestTaskExecutionPolicyTriggersProgressAuditAndTerminalFinalization(t *testing.T) {
	chat, database, _ := chatFixture(t)
	policy, err := database.TaskExecutionPolicy(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	policy.MaxProviderContinuations, policy.MaxToolCalls, policy.ProgressAuditInterval = 3, 1, 1
	if _, err = database.UpdateTaskExecutionPolicy(t.Context(), policy); err != nil {
		t.Fatal(err)
	}
	task := createQueuedRuntimeTask(t, database, chat.home, "Finalize after the progress check.")
	calls := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if taskRequestHasTool(request.Tools, progressAuditToolName) {
			var digest progressAuditDigest
			if json.Unmarshal([]byte(request.Messages[1].Content), &digest) != nil || !strings.Contains(digest.UserGoal, "Finalize after the progress check.") {
				t.Fatalf("audit goal = %#v", digest)
			}
			return taskToolResult("audit", progressAuditToolName, map[string]any{"decision": "continue", "user_summary": "Continue.", "next_goal": nil}), nil
		}
		if request.ToolChoice == provider.ToolChoiceRequired {
			if !strings.Contains(fmt.Sprint(request.Messages), "task tool-call safety ceiling reached") {
				t.Fatalf("finalization reason is absent: %#v", request.Messages)
			}
			for _, tool := range request.Tools {
				if !taskTerminalTool(tool.Name) {
					t.Fatalf("nonterminal finalization tool %q", tool.Name)
				}
			}
			return taskToolResult("final", taskReportBlocked, map[string]any{"gate_kind": "clarification", "question": "Continue?"}), nil
		}
		return taskToolResult("work", taskFilesRead, map[string]any{"path": "TASK.md"}), nil
	})
	runtime, err := NewTaskExecution(context.Background(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	defer runtime.Close()
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	runs, err := database.TaskRuns(context.Background(), task.ID, 10)
	if err != nil || len(runs) != 1 {
		t.Fatalf("finalized run = %#v, %v", runs, err)
	}
	items, _ := database.TaskRunReplayItems(context.Background(), runs[0].ID)
	skipped := 0
	for _, item := range items {
		if item.Kind == "tool_call" && item.Status == "skipped" && item.Content != nil && *item.Content == taskFilesRead {
			skipped++
		}
	}
	if runs[0].ExecutionPolicy != policy || calls != 4 || skipped != 1 {
		t.Fatalf("finalized run = %#v, calls = %d, %v", runs, calls, err)
	}
}

func TestTaskExecutionPolicyRecoversDueAuditFromReplay(t *testing.T) {
	chat, database, _ := chatFixture(t)
	policy, _ := database.TaskExecutionPolicy(t.Context())
	policy.MaxProviderContinuations, policy.ProgressAuditInterval = 8, 5
	_, _ = database.UpdateTaskExecutionPolicy(t.Context(), policy)
	task := createQueuedRuntimeTask(t, database, chat.home, "Recover this exact TASK.md goal.")
	_, run, _, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || database.StartTaskExecution(t.Context(), run.ID, run.Generation, time.Now()) != nil {
		t.Fatalf("start seeded run: %v", err)
	}
	for round := int64(0); round < 5; round++ {
		call := store.TaskRunItemInput{Kind: "tool_call", Status: "running", Round: round, CorrelationID: fmt.Sprintf("call:seed:%d", round),
			Payload: map[string]any{"name": taskFilesRead, "arguments": json.RawMessage(`{"path":"TASK.md"}`)}}
		if err = database.AppendTaskRunItems(t.Context(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "assistant_output", Status: "completed", Round: round}, call}, store.TaskRunUsage{ProviderCalls: 1, ToolCalls: 1}, time.Now()); err != nil {
			t.Fatal(err)
		}
		items, _ := database.TaskRunReplayItems(t.Context(), run.ID)
		if err = database.AppendTaskRunItems(t.Context(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "tool_result", Status: "completed", Round: round, ParentID: items[len(items)-1].ID,
			Payload: map[string]any{"result": json.RawMessage(`{"content":"same"}`), "success": true, "side_effect": true}}}, store.TaskRunUsage{}, time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	calls := 0
	generator := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		calls++
		if taskRequestHasTool(request.Tools, progressAuditToolName) {
			var digest progressAuditDigest
			_ = json.Unmarshal([]byte(request.Messages[1].Content), &digest)
			if digest.WholeTurn.ToolCounts[taskFilesRead] != 5 || digest.WholeTurn.SideEffectCount != 5 || len(digest.Window.ToolCounts) != 0 || !strings.Contains(digest.UserGoal, "Recover this exact TASK.md goal.") {
				t.Fatalf("recovered audit digest = %#v", digest)
			}
			return taskToolResult("audit-recovery", progressAuditToolName, map[string]any{"decision": "ask_human", "user_summary": "Need input.", "next_goal": nil}), nil
		}
		if text := fmt.Sprint(request.Messages); !strings.Contains(text, "progress audit requires human input") || !strings.Contains(text, taskContinuationPrompt) {
			t.Fatalf("ask-human reason is absent: %#v", request.Messages)
		}
		return taskToolResult("final-recovery", taskReportBlocked, map[string]any{"gate_kind": "clarification", "question": "Continue?"}), nil
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
	if calls != 2 {
		t.Fatalf("recovered provider calls = %d", calls)
	}
	items, _ := database.TaskRunReplayItems(t.Context(), run.ID)
	notices := 0
	for _, item := range items {
		if item.CorrelationID != nil && *item.CorrelationID == "task:stall" {
			notices++
		}
	}
	if notices != 1 {
		t.Fatalf("stall notices = %d", notices)
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
	payload, success, _, _ := runtime.executeTaskTool(context.Background(), task, executorRun, webtool.FetchName, json.RawMessage(`{"url":"https://1.1.1.1/"}`), true)
	if success || !strings.Contains(string(payload), "unsupported_tool") {
		t.Fatalf("unreviewed Task fetch = %s, %t", payload, success)
	}
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

func TestTaskMessagesSeparateRequestAndCurrentClocks(t *testing.T) {
	for _, tc := range []struct{ name, sourceZone, scheduleZone string }{
		{"source_west", "America/Los_Angeles", ""},
		{"source_east", "Asia/Tokyo", ""},
		{"scheduled_occurrence", "America/Los_Angeles", "Asia/Tokyo"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			chat, database, conversation := chatFixture(t)
			requested := time.Date(2025, 1, 2, 1, 30, 0, 0, time.UTC)
			_, item, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Tomorrow at nine", nil, requested)
			if err != nil {
				t.Fatal(err)
			}
			task := createQueuedRuntimeTask(t, database, chat.home, "# Task\nTomorrow at nine. café 日本語\n")
			task.Source = store.ArtifactSource{ConversationID: conversation.ID, ItemID: item.ID}
			task.SourceClientTimeZone, task.ScheduleTimeZone = tc.sourceZone, tc.scheduleZone
			occurrence := requested.Add(48 * time.Hour)
			if tc.scheduleZone != "" {
				task.ScheduledFor, task.RecurrenceScheduledFor = &occurrence, &occurrence
			}
			if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "Current result"); err != nil {
				t.Fatal(err)
			}
			runtime := &TaskExecution{database: database, root: chat.home}
			for _, role := range []string{"planner", "executor", "reviewer"} {
				before := time.Now().Truncate(time.Second)
				messages, _, err := runtime.taskMessages(t.Context(), task, store.TaskRun{Kind: role})
				if err != nil {
					t.Fatal(err)
				}
				var system, data string
				for _, message := range messages {
					if message.Role == "system" {
						system += message.Content + "\n"
					} else {
						data += message.Content + "\n"
					}
				}
				zone := tc.sourceZone
				if tc.scheduleZone != "" {
					zone = tc.scheduleZone
				}
				location, _ := time.LoadLocation(zone)
				var current time.Time
				for _, line := range strings.Split(system, "\n") {
					if value, found := strings.CutPrefix(line, "- current_time: "); found {
						value, err = strconv.Unquote(value)
						if err == nil {
							current, err = time.Parse(time.RFC3339, value)
						}
						if err != nil {
							t.Fatal(err)
						}
					}
				}
				if current.Before(before) || current.After(time.Now()) || !strings.Contains(system, "- timezone: "+strconv.Quote(zone)) {
					t.Fatalf("%s current clock missing or stale: %s", role, system)
				}
				_, actualOffset := current.Zone()
				_, expectedOffset := current.In(location).Zone()
				if actualOffset != expectedOffset {
					t.Fatalf("clock offset = %d; want %d", actualOffset, expectedOffset)
				}
				sourceLocation, _ := time.LoadLocation(tc.sourceZone)
				if !strings.Contains(data, "date="+requested.In(sourceLocation).Format("2006-01-02")+", time="+requested.In(sourceLocation).Format("2006-01-02T15:04:05-07:00")) || !strings.Contains(data, "café 日本語") {
					t.Fatalf("original request time or Task text missing: %s", data)
				}
				if tc.scheduleZone != "" && role != "reviewer" && !strings.Contains(data, "Noema started this scheduled task for "+occurrence.UTC().Format(time.RFC3339)) {
					t.Fatalf("occurrence cutoff missing: %s", system)
				}
			}
		})
	}
}

func TestTaskReviewCorrectionUsesCurrentFiles(t *testing.T) {
	chat, database, _ := chatFixture(t)
	request := "# Correction audit\n\nInclude both alpha and café 日本語 in the result.\n"
	planned := request + "\nPlan: write both required values.\n"
	feedback := "The result omits café 日本語. Preserve alpha and add the missing value."
	task := createQueuedRuntimeTask(t, database, chat.home, request)
	calls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, input provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(input.Tools)
		calls[role]++
		call := calls[role]
		switch role {
		case "planner":
			if call == 1 {
				return taskToolResult("plan-write", taskFilesWrite, map[string]any{"path": "TASK.md", "content": planned}), nil
			}
			return taskToolResult("plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			if call == 4 {
				var data string
				for _, message := range input.Messages {
					if message.Role == "user" {
						data += message.Content
					}
				}
				for _, required := range []string{planned, "<RESULT_DOCUMENT>\nalpha\n", feedback} {
					if !strings.Contains(data, required) {
						t.Errorf("correction context omits %q: %s", required, data)
					}
				}
				current, err := database.Task(t.Context(), task.ID)
				if err != nil || current.StageKey == "done" || current.CompletedAt != nil {
					t.Errorf("rejected result completed Task: %#v, %v", current, err)
				}
			}
			switch call {
			case 1, 4:
				return taskToolResult(fmt.Sprintf("progress-%d", call), taskFilesWrite, map[string]any{"path": "TASK.md", "content": planned}), nil
			case 2:
				return taskToolResult("incomplete-result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "alpha\n"}), nil
			case 5:
				return taskToolResult("corrected-result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "alpha\ncafé 日本語\n"}), nil
			default:
				return taskToolResult(fmt.Sprintf("finish-%d", call), taskFinishExecution, map[string]any{}), nil
			}
		default:
			var data string
			for _, message := range input.Messages {
				if message.Role == "user" {
					data += message.Content
				}
			}
			expectedResult := "alpha\n"
			if call > 1 {
				expectedResult = "alpha\ncafé 日本語\n"
			}
			if !strings.Contains(data, planned) || !strings.Contains(data, "<RESULT_DOCUMENT>\n"+expectedResult+"\n</RESULT_DOCUMENT>") {
				t.Errorf("Reviewer did not receive current request and result: %s", data)
			}
			if call == 1 {
				return taskToolResult("request-correction", taskFinishReview, map[string]any{"decision": "request_changes", "feedback": feedback, "notify_human": false}), nil
			}
			return taskToolResult("approve-correction", taskFinishReview, map[string]any{"decision": "approve", "feedback": "Both required values are present.", "notify_human": false}), nil
		}
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	runs, err := database.TaskRuns(t.Context(), task.ID, 10)
	if err != nil || len(runs) != 5 {
		t.Fatalf("correction runs = %#v, %v", runs, err)
	}
	for i, kind := range []string{"reviewer", "executor", "reviewer", "executor", "planner"} {
		if runs[i].Kind != kind || runs[i].Status != "completed" {
			t.Fatalf("unexpected correction run: %#v", runs[i])
		}
		if i < 4 && runs[i].ParentRunID != runs[i+1].ID {
			t.Fatalf("correction parent is wrong: %#v", runs[i])
		}
	}
	if runs[0].ReviewRound != 2 || runs[1].ReviewRound != 2 || runs[2].ReviewRound != 1 {
		t.Fatalf("correction rounds = %#v", runs)
	}
	result, err := home.ReadTaskFile(chat.home, task.ID, "RESULT.md")
	if err != nil || result != "alpha\ncafé 日本語\n" {
		t.Fatalf("corrected result = %q, %v", result, err)
	}
}

func TestCancelledTaskKeepsLateMCPWriteUncertain(t *testing.T) {
	chat, database, _ := chatFixture(t)
	entered, release, remoteDone := make(chan struct{}), make(chan struct{}), make(chan struct{})
	var remoteCalls atomic.Int32
	var releaseOnce sync.Once
	unblock := func() { releaseOnce.Do(func() { close(release) }) }
	remote := mcpsdk.NewServer(&mcpsdk.Implementation{Name: "audit", Version: "1"}, nil)
	mcpsdk.AddTool(remote, &mcpsdk.Tool{Name: "write", Description: "Write a synthetic audit value", Annotations: &mcpsdk.ToolAnnotations{ReadOnlyHint: false, IdempotentHint: false, OpenWorldHint: runtimeBool(true)}},
		func(_ context.Context, _ *mcpsdk.CallToolRequest, args struct {
			Value string `json:"value"`
		}) (*mcpsdk.CallToolResult, map[string]any, error) {
			if args.Value != "café 日本語" {
				t.Errorf("remote arguments changed: %q", args.Value)
			}
			remoteCalls.Add(1)
			close(entered)
			<-release
			close(remoteDone)
			return nil, map[string]any{"written": args.Value}, nil
		})
	server := httptest.NewServer(mcpsdk.NewStreamableHTTPHandler(func(*http.Request) *mcpsdk.Server { return remote }, nil))
	t.Cleanup(server.Close)
	paths, _ := home.FromRoot(chat.home.Name())
	service, err := noemamcp.NewService(paths, database, false, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	setup, err := service.Create(t.Context(), noemamcp.SetupInput{DisplayName: "Cancellation audit", TransportKind: "streamable_http", URL: server.URL, AuthPreference: "USE_ANONYMOUS"})
	if err != nil || setup.Server == nil {
		t.Fatalf("MCP setup = %#v, %v", setup, err)
	}
	if _, err = service.SaveConnectionPolicy(t.Context(), setup.Server.ID, setup.Server.ConnectionRevision, 0, "allow_automatically", "always_ask"); err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings(t.Context())
	if err != nil || len(bindings) < 2 {
		t.Fatalf("bindings = %#v, %v", bindings, err)
	}
	binding := runtimeDiscoveredMCPBinding(t, bindings)
	task := createQueuedRuntimeTask(t, database, chat.home, "Write the audit value once.")
	_, planner, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || !found {
		t.Fatalf("Planner claim = %t, %v", found, err)
	}
	if err = database.StartTaskExecution(t.Context(), planner.ID, planner.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err = database.FinishTaskPlanning(t.Context(), planner.ID, planner.Generation, "simple", time.Now()); err != nil {
		t.Fatal(err)
	}
	task, run, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || !found {
		t.Fatalf("Executor claim = %t, %v", found, err)
	}
	if err = database.StartTaskExecution(t.Context(), run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	args := json.RawMessage(`{"value":"café 日本語"}`)
	if err = database.AppendTaskRunItems(t.Context(), run.ID, run.Generation, []store.TaskRunItemInput{{Kind: "tool_call", Status: "running", Payload: map[string]any{"name": binding.Name, "arguments": map[string]any{"value": "café 日本語"}}}}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	items, err := database.TaskRunReplayItems(t.Context(), run.ID)
	if err != nil || len(items) == 0 {
		t.Fatalf("call items = %#v, %v", items, err)
	}
	call := items[len(items)-1]
	action, err := database.CreateActionRequest(t.Context(), store.NewActionRequest{TaskID: task.ID, RunID: run.ID, RunItemID: call.ID, TaskGeneration: run.Generation, OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: binding.Name, OperationToken: binding.Name, ReviewRoute: store.ActionHumanReview, Behavior: binding.Behavior, Arguments: args, InputSchema: binding.InputSchema, AuthorizationContext: map[string]any{"mcp_binding": binding}, SafeSummary: "Write the synthetic audit value"}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	action, err = database.DecideActionRequest(t.Context(), action.ID, action.Revision, "human:local", "approve", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: database, mcp: service, root: chat.home}
	t.Cleanup(unblock)
	finished := make(chan error, 1)
	go func() {
		_, _, _, err := runtime.executeTaskMCPAction(t.Context(), task, run, call, action)
		finished <- err
	}()
	select {
	case <-entered:
	case <-time.After(5 * time.Second):
		t.Fatal("remote write did not start")
	}
	current, err := database.Task(t.Context(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := database.CancelTask(t.Context(), task.ID, current.Revision, current.Generation, "Stop", runtimeTaskCommand("cancel_task", "late-mcp"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	unblock()
	select {
	case <-remoteDone:
	case <-time.After(5 * time.Second):
		t.Fatal("remote write did not finish")
	}
	select {
	case err := <-finished:
		if err == nil {
			t.Fatal("late remote success was accepted")
		}
	case <-time.After(5 * time.Second):
		t.Fatal("runtime did not finish")
	}
	stored, err := database.ActionRequest(t.Context(), action.ID, action.Revision)
	if err != nil || stored.State != store.ActionOutcomeUncertain || stored.FailureCode != "outcome_uncertain" || stored.Arguments["value"] != "café 日本語" {
		t.Fatalf("late remote outcome = %#v, %v", stored, err)
	}
	current, err = database.Task(t.Context(), task.ID)
	if err != nil || current.StageKey != "cancelled" || current.Generation != cancelled.Task.Generation || current.CompletedAt != nil {
		t.Fatalf("late call changed Task = %#v, %v", current, err)
	}
	if remoteCalls.Load() != 1 {
		t.Fatalf("remote calls = %d", remoteCalls.Load())
	}
}

func TestTaskContinuationUsesCheckpointWithoutRepeatingWork(t *testing.T) {
	chat, database, _ := chatFixture(t)
	request := "# Continuation audit\n\nSave alpha once, then add café 日本語 to the result.\n"
	checkpoint := request + "\nCompleted: alpha is saved in step-one.md.\nRemaining: read step-one.md and write RESULT.md with alpha and café 日本語. Do not repeat the completed write.\n"
	task := createQueuedRuntimeTask(t, database, chat.home, request)
	calls := map[string]int{}
	generator := generatorFunc(func(_ context.Context, input provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		role := taskRequestRole(input.Tools)
		calls[role]++
		call := calls[role]
		switch role {
		case "planner":
			if call == 1 {
				return taskToolResult("plan", taskFilesWrite, map[string]any{"path": "TASK.md", "content": request}), nil
			}
			return taskToolResult("plan-finish", taskFinishPlanning, map[string]any{"complexity": "simple"}), nil
		case "executor":
			switch call {
			case 1:
				return taskToolResult("step-one", taskFilesWrite, map[string]any{"path": "step-one.md", "content": "alpha\n"}), nil
			case 2:
				return taskToolResult("checkpoint", taskFilesWrite, map[string]any{"path": "TASK.md", "content": checkpoint}), nil
			case 3:
				return taskToolResult("continue", taskContinueExecution, map[string]any{}), nil
			case 4:
				var data string
				for _, message := range input.Messages {
					if message.Role == "user" {
						data += message.Content
					}
				}
				if !strings.Contains(data, checkpoint) {
					t.Errorf("continuation lost current checkpoint: %s", data)
				}
				runs, err := database.TaskRuns(t.Context(), task.ID, 10)
				if err != nil || len(runs) != 3 || runs[0].Kind != "executor" || runs[1].Kind != "executor" || runs[0].ParentRunID != runs[1].ID || runs[1].Status != "completed" {
					t.Errorf("continuation did not create one child run: %#v, %v", runs, err)
				}
				return taskToolResult("read-completed", taskFilesRead, map[string]any{"path": "step-one.md"}), nil
			case 5:
				var results string
				for _, message := range input.Messages {
					if message.ToolResult != nil {
						results += string(message.ToolResult.Payload)
					}
				}
				if !strings.Contains(results, "alpha") {
					t.Errorf("continuation did not receive saved support file: %s", results)
				}
				return taskToolResult("complete-result", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "alpha\ncafé 日本語\n"}), nil
			case 6:
				return taskToolResult("complete-checkpoint", taskFilesWrite, map[string]any{"path": "TASK.md", "content": request + "\nCompleted both steps.\n"}), nil
			default:
				return taskToolResult("execution-finish", taskFinishExecution, map[string]any{}), nil
			}
		default:
			return taskToolResult("review", taskFinishReview, map[string]any{"decision": "approve", "feedback": "Both steps are complete.", "notify_human": false}), nil
		}
	})
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	waitRuntimeTask(t, database, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
	runs, err := database.TaskRuns(t.Context(), task.ID, 10)
	if err != nil || len(runs) != 4 {
		t.Fatalf("final runs = %#v, %v", runs, err)
	}
	for i, kind := range []string{"reviewer", "executor", "executor", "planner"} {
		if runs[i].Kind != kind || runs[i].Status != "completed" {
			t.Fatalf("unexpected run = %#v", runs[i])
		}
	}
	writes := 0
	for _, run := range runs {
		items, err := database.TaskRunReplayItems(t.Context(), run.ID)
		if err != nil {
			t.Fatal(err)
		}
		for _, item := range items {
			args, _ := item.Payload["arguments"].(map[string]any)
			if item.Kind == "tool_call" && item.Payload["name"] == taskFilesWrite && args["path"] == "step-one.md" {
				writes++
			}
		}
	}
	if writes != 1 {
		t.Fatalf("saved first-step write count = %d", writes)
	}
	for name, want := range map[string]string{"step-one.md": "alpha\n", "RESULT.md": "alpha\ncafé 日本語\n"} {
		content, err := home.ReadTaskFile(chat.home, task.ID, name)
		if err != nil || content != want {
			t.Fatalf("%s = %q, %v", name, content, err)
		}
	}
}

func TestReopenedTaskRejectsLateProviderFileWrite(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "# Reopen audit\nPreserve café 日本語.\n")
	if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "accepted café 日本語\n"); err != nil {
		t.Fatal(err)
	}
	started, release := make(chan struct{}), make(chan struct{})
	var once sync.Once
	unblock := func() { once.Do(func() { close(release) }) }
	var requests atomic.Int32
	generation := generatorFunc(func(ctx context.Context, _ provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		if requests.Add(1) == 1 {
			close(started)
			<-release
			return taskToolResult("late-overwrite", taskFilesWrite, map[string]any{"path": "RESULT.md", "content": "stale overwrite"}), nil
		}
		<-ctx.Done()
		return provider.GenerationResult{}, ctx.Err()
	})
	generator := &sessionTestGenerator{generate: generation, closed: make(chan struct{}, 4)}
	runtime, err := NewTaskExecution(t.Context(), database, generator, generator, generator, chat.home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(runtime.Close)
	t.Cleanup(unblock)
	select {
	case <-started:
	case <-time.After(5 * time.Second):
		t.Fatal("old provider did not start")
	}
	before, err := database.Task(t.Context(), task.ID)
	if err != nil {
		t.Fatal(err)
	}
	oldRunID := before.CurrentRunID
	cancelled, err := database.CancelTask(t.Context(), task.ID, before.Revision, before.Generation, "Replace old work", runtimeTaskCommand("cancel_task", "reopen-late"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	complexity := "simple"
	reopened, err := database.ReopenTask(t.Context(), task.ID, cancelled.Task.Revision, cancelled.Task.Generation, "Continue with accepted files", &complexity, "", runtimeTaskCommand("reopen_task", "reopen-late"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if reopened.Task.Generation <= before.Generation || reopened.Task.CurrentRunID == oldRunID {
		t.Fatalf("reopen did not replace authority: %#v", reopened.Task)
	}
	unblock()
	select {
	case <-generator.closed:
	case <-time.After(5 * time.Second):
		t.Fatal("old provider session did not close")
	}
	for path, want := range map[string]string{"TASK.md": "# Reopen audit\nPreserve café 日本語.\n", "RESULT.md": "accepted café 日本語\n"} {
		content, err := home.ReadTaskFile(chat.home, task.ID, path)
		if err != nil || content != want {
			t.Fatalf("late response changed %s: %q, %v", path, content, err)
		}
	}
	current, err := database.Task(t.Context(), task.ID)
	if err != nil || current.Generation != reopened.Task.Generation || current.CurrentRunID != reopened.Task.CurrentRunID || current.CompletedAt != nil {
		t.Fatalf("late response changed reopened Task: %#v, %v", current, err)
	}
	runs, err := database.TaskRuns(t.Context(), task.ID, 10)
	if err != nil || len(runs) != 2 {
		t.Fatalf("reopen runs = %#v, %v", runs, err)
	}
	for _, run := range runs {
		if run.ID == oldRunID && run.Status != "cancelled" {
			t.Fatalf("old run revived: %#v", run)
		}
	}
	items, err := database.TaskRunReplayItems(t.Context(), oldRunID)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range items {
		if item.Kind == "tool_call" {
			t.Fatalf("late tool call was accepted: %#v", item)
		}
	}
}

func TestCompletedTaskReopenRejectsLateReviewWithoutFileChanges(t *testing.T) {
	chat, database, _ := chatFixture(t)
	task := createQueuedRuntimeTask(t, database, chat.home, "# Completed reopen\nPreserve café 日本語.\n")
	if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", "accepted result\n"); err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: database, root: chat.home}
	var reviewer store.TaskRun
	for _, kind := range []string{"planner", "executor", "reviewer"} {
		_, run, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
		if err != nil || !found || run.Kind != kind {
			t.Fatalf("claim %s = %#v, %t, %v", kind, run, found, err)
		}
		if err = database.StartTaskExecution(t.Context(), run.ID, run.Generation, time.Now()); err != nil {
			t.Fatal(err)
		}
		name, raw := taskFinishPlanning, json.RawMessage(`{"complexity":"simple"}`)
		if kind == "executor" {
			name, raw = taskFinishExecution, json.RawMessage(`{}`)
		}
		if kind == "reviewer" {
			reviewer = run
			name, raw = taskFinishReview, json.RawMessage(`{"decision":"approve","feedback":"Accepted café 日本語.","notify_human":false}`)
		}
		if err = runtime.finishTaskTerminal(t.Context(), run, name, raw); err != nil {
			t.Fatal(err)
		}
	}
	completed, err := database.Task(t.Context(), task.ID)
	if err != nil || completed.StageKey != "done" {
		t.Fatalf("completed Task = %#v, %v", completed, err)
	}
	complexity := "simple"
	reopened, err := database.ReopenTask(t.Context(), task.ID, completed.Revision, completed.Generation, "Continue from accepted files", &complexity, "", runtimeTaskCommand("reopen_task", "completed-late"), time.Now())
	if err != nil {
		t.Fatal(err)
	}
	paths := map[string]string{"TASK.md": "# Completed reopen\nPreserve café 日本語.\n", "RESULT.md": "accepted result\n", "REVIEW.md": "Accepted café 日本語.\n"}
	fixed := time.Date(2025, 1, 1, 0, 0, 0, 0, time.UTC)
	for name := range paths {
		path := filepath.Join(chat.home.Name(), "tasks", strings.TrimPrefix(task.ID, "task:"), name)
		if err := os.Chtimes(path, fixed, fixed); err != nil {
			t.Fatal(err)
		}
	}
	err = runtime.finishTaskTerminal(t.Context(), reviewer, taskFinishReview, json.RawMessage(`{"decision":"request_changes","feedback":"Stale replacement.","notify_human":false}`))
	if !errors.Is(err, store.ErrStaleRun) {
		t.Fatalf("late review = %v", err)
	}
	for name, want := range paths {
		content, err := home.ReadTaskFile(chat.home, task.ID, name)
		if err != nil || content != want {
			t.Fatalf("late review changed %s: %q, %v", name, content, err)
		}
		info, err := os.Stat(filepath.Join(chat.home.Name(), "tasks", strings.TrimPrefix(task.ID, "task:"), name))
		if err != nil || !info.ModTime().Equal(fixed) {
			t.Fatalf("late review rewrote %s: %v, %v", name, info, err)
		}
	}
	current, err := database.Task(t.Context(), task.ID)
	if err != nil || current.Generation != reopened.Task.Generation || current.CurrentRunID != reopened.Task.CurrentRunID || current.StageKey != "queue" || current.CompletedAt != nil {
		t.Fatalf("late review changed reopened Task: %#v, %v", current, err)
	}
	runs, err := database.TaskRuns(t.Context(), task.ID, 10)
	if err != nil || len(runs) != 4 || runs[0].Kind != "executor" {
		t.Fatalf("reopened runs = %#v, %v", runs, err)
	}
}

func TestTaskRuntimeRestartResumesEachRole(t *testing.T) {
	for _, interruptedRole := range []string{"planner", "executor", "reviewer"} {
		t.Run(interruptedRole, func(t *testing.T) {
			ctx := t.Context()
			chat, database, _ := chatFixture(t)
			const requestText = "# Task\n\nPreserve café 日本語 through restart.\n"
			const resultText = "Accepted café 日本語\n"
			task := createQueuedRuntimeTask(t, database, chat.home, requestText)
			if err := home.WriteTaskFile(chat.home, task.ID, "RESULT.md", resultText); err != nil {
				t.Fatal(err)
			}
			paused := make(chan struct{})
			readRequested := false
			written := map[string]bool{}
			finish := func(role string) provider.GenerationResult {
				if role != "reviewer" && !written[role] {
					written[role] = true
					return taskToolResult(role+"-progress", taskFilesWrite, map[string]any{"path": "TASK.md", "content": requestText})
				}
				switch role {
				case "planner":
					return taskToolResult("plan", taskFinishPlanning, map[string]any{"complexity": "simple"})
				case "executor":
					return taskToolResult("execute", taskFinishExecution, map[string]any{})
				default:
					return taskToolResult("review", taskFinishReview, map[string]any{"decision": "approve", "feedback": "Accepted café 日本語", "notify_human": false})
				}
			}
			before := generatorFunc(func(ctx context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
				role := taskRequestRole(request.Tools)
				if role != interruptedRole || role != "reviewer" && !written[role] {
					return finish(role), nil
				}
				if !readRequested {
					readRequested = true
					return taskToolResult("saved-read", taskFilesRead, map[string]any{"path": "TASK.md"}), nil
				}
				close(paused)
				<-ctx.Done()
				return provider.GenerationResult{}, ctx.Err()
			})
			worker, err := NewTaskExecution(ctx, database, before, before, before, chat.home)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(worker.Close)
			select {
			case <-paused:
			case <-time.After(5 * time.Second):
				t.Fatal("role did not reach the restart boundary")
			}
			current, err := database.Task(ctx, task.ID)
			if err != nil {
				t.Fatal(err)
			}
			interruptedRun := current.CurrentRunID
			worker.Close()
			if err = chat.Close(); err != nil {
				t.Fatal(err)
			}
			if err = database.Close(); err != nil {
				t.Fatal(err)
			}
			reopened, err := store.Open(ctx, filepath.Join(chat.home.Name(), "noema.sqlite3"))
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(func() { _ = reopened.Close() })
			var resumed atomic.Bool
			after := generatorFunc(func(_ context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
				role := taskRequestRole(request.Tools)
				if !resumed.Load() {
					if role != interruptedRole {
						return provider.GenerationResult{}, fmt.Errorf("resumed %s instead of %s", role, interruptedRole)
					}
					found := false
					for _, message := range request.Messages {
						if message.ToolResult != nil {
							var payload struct{ Path, Content string }
							if json.Unmarshal(message.ToolResult.Payload, &payload) == nil && payload.Path == "TASK.md" && payload.Content == requestText {
								found = true
							}
						}
					}
					if !found {
						return provider.GenerationResult{}, errors.New("saved read result missing from resumed context")
					}
					resumed.Store(true)
				}
				return finish(role), nil
			})
			worker, err = NewTaskExecution(ctx, reopened, after, after, after, chat.home)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(worker.Close)
			completed := waitRuntimeTask(t, reopened, task.ID, func(value store.Task) bool { return value.StageKey == "done" })
			worker.Close()
			if !resumed.Load() || completed.Generation != current.Generation || completed.CompletedAt == nil {
				t.Fatalf("restarted Task = %#v, resumed=%t", completed, resumed.Load())
			}
			runs, err := reopened.TaskRuns(ctx, task.ID, 10)
			if err != nil || len(runs) != 3 {
				t.Fatalf("restart created extra runs = %#v, %v", runs, err)
			}
			for _, run := range runs {
				if run.Status != "completed" || run.Kind == interruptedRole && run.ID != interruptedRun {
					t.Fatalf("restart changed run identity or left it active: %#v", run)
				}
			}
			for name, want := range map[string]string{"TASK.md": requestText, "RESULT.md": resultText} {
				got, err := home.ReadTaskFile(chat.home, task.ID, name)
				if err != nil || got != want {
					t.Fatalf("%s changed after restart: %q, %v", name, got, err)
				}
			}
		})
	}
}
