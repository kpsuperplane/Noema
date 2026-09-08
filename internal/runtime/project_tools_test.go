package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/project"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestProjectToolsArePrimaryOnlyAndRejectInvalidArguments(t *testing.T) {
	primary := make(map[string]bool)
	for _, tool := range localChatTools() {
		primary[tool.Name] = true
	}
	for _, spec := range projectToolSpecs {
		if !primary[spec.Name] || !supportsLocalChatTool(spec.Name) {
			t.Fatalf("primary Project tool %q is unavailable", spec.Name)
		}
	}
	for _, kind := range []string{"planning", "execution", "review"} {
		for _, tool := range taskExecutionTools(kind) {
			if isProjectTool(tool.Name) {
				t.Fatalf("Task role %q received %q", kind, tool.Name)
			}
		}
	}
	chat, _, conversation := chatFixture(t)
	invalid := []json.RawMessage{
		json.RawMessage(`{"name":"One","extra":true}`),
		json.RawMessage(`{"name":"One","name":"Two"}`),
		json.RawMessage{'{', '"', 'n', 'a', 'm', 'e', '"', ':', '"', 0xff, '"', '}'},
	}
	for index, arguments := range invalid {
		payload, success := chat.executeChatTool(context.Background(), conversation,
			projectCreateName, arguments, "invalid-project-call", "turn:invalid")
		if success || !bytes.Contains(payload, []byte(`"code":"invalid_input"`)) {
			t.Fatalf("invalid arguments %d = %s, %t", index, payload, success)
		}
	}
}

func TestProjectToolsCreateReplayListAndReadExactDocument(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	arguments := projectTestArguments(t, map[string]any{
		"name": " Release ", "description": " Exact scope ", "project_document": "# Release\n\nExact.\n",
	})
	created, success := chat.executeChatTool(ctx, conversation, projectCreateName, arguments, "project-create-1", "turn:create")
	if !success {
		t.Fatalf("create Project = %s", created)
	}
	createdValue := projectTestValue(t, created)
	projectValue := createdValue["project"].(map[string]any)
	projectID := projectValue["project_id"].(string)
	events, err := database.WorkEvents(ctx, project.PersonalWorkspaceID, 0, 10)
	if err != nil || len(events) != 1 || events[0].ActorID != projectActorID ||
		events[0].CorrelationID != "correlation:turn:turn:create" {
		t.Fatalf("Project event authority = %#v, %v", events, err)
	}
	replayed, success := chat.executeChatTool(ctx, conversation, projectCreateName, arguments, "project-create-1", "turn:create")
	if !success || projectTestValue(t, replayed)["event_sequence"] != createdValue["event_sequence"] {
		t.Fatalf("replayed Project = %s, %t", replayed, success)
	}
	read, success := chat.executeChatTool(ctx, conversation, projectReadName,
		projectTestArguments(t, map[string]any{"project_id": projectID}), "project-read-1", "turn:read")
	readValue := projectTestValue(t, read)
	if !success || readValue["name"] != "Release" || readValue["description"] != "Exact scope" ||
		readValue["project_document"] != "# Release\n\nExact.\n" || readValue["digest"] == "" {
		t.Fatalf("read Project = %#v, %t", readValue, success)
	}
	if _, success := chat.executeChatTool(ctx, conversation, projectCreateName,
		projectTestArguments(t, map[string]any{"name": "Second"}), "project-create-2", "turn:create"); !success {
		t.Fatal("second Project creation failed")
	}
	listed, success := chat.executeChatTool(ctx, conversation, projectListName,
		projectTestArguments(t, map[string]any{"limit": 1}), "project-list-1", "turn:list")
	listValue := projectTestValue(t, listed)
	projects := listValue["projects"].([]any)
	if !success || len(projects) != 1 || listValue["has_next_page"] != true || listValue["end_cursor"] == nil {
		t.Fatalf("list Projects = %s, %t", listed, success)
	}
	next, success := chat.executeChatTool(ctx, conversation, projectListName,
		projectTestArguments(t, map[string]any{"limit": 1, "cursor": listValue["end_cursor"]}), "project-list-2", "turn:list")
	nextProjects := projectTestValue(t, next)["projects"].([]any)
	if !success || len(nextProjects) != 1 || nextProjects[0].(map[string]any)["project_id"] == projects[0].(map[string]any)["project_id"] {
		t.Fatalf("next Project page = %s, %t", next, success)
	}
}

func TestProjectCreateAdoptsExistingExternalDocument(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	folder := t.TempDir()
	if err := os.WriteFile(filepath.Join(folder, "PROJECT.md"), []byte("# Existing\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	arguments := projectTestArguments(t, map[string]any{
		"name": "Adopt", "folder": folder, "project_document": "# Proposed\n",
	})
	payload, success := chat.executeChatTool(context.Background(), conversation,
		projectCreateName, arguments, "project-adopt-1", "turn:adopt")
	value := projectTestValue(t, payload)
	if !success || value["document_adopted"] != true {
		t.Fatalf("adopt Project = %#v, %t", value, success)
	}
	projectID := value["project"].(map[string]any)["project_id"].(string)
	stored, err := home.ReadProjectDocument(chat.home, projectID, stringPointer(folder))
	if err != nil || stored.Content != "# Existing\n" {
		t.Fatalf("adopted document = %#v, %v", stored, err)
	}
	replay, success := chat.executeChatTool(context.Background(), conversation,
		projectCreateName, arguments, "project-adopt-1", "turn:adopt")
	if !success || projectTestValue(t, replay)["document_adopted"] != true {
		t.Fatalf("adopt replay = %s, %t", replay, success)
	}
}

func TestProjectToolsMoveConflictAndLifecycleKeepAuthority(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	ctx := context.Background()
	created, success := chat.executeChatTool(ctx, conversation, projectCreateName,
		projectTestArguments(t, map[string]any{"name": "Move", "project_document": "# Exact\n"}), "project-move-create", "turn:move")
	if !success {
		t.Fatalf("create Project = %s", created)
	}
	projectID := projectTestValue(t, created)["project"].(map[string]any)["project_id"].(string)
	folder := t.TempDir()
	moved, success := chat.executeChatTool(ctx, conversation, projectUpdateName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 1, "folder": folder}), "project-move", "turn:move")
	if !success || projectTestValue(t, moved)["project"].(map[string]any)["revision"] != float64(2) {
		t.Fatalf("move Project = %s, %t", moved, success)
	}
	stale, success := chat.executeChatTool(ctx, conversation, projectUpdateName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 1,
			"description": "Stale"}), "project-stale", "turn:move")
	if success || !bytes.Contains(stale, []byte(`"code":"stale_revision"`)) {
		t.Fatalf("stale update = %s, %t", stale, success)
	}
	conflict := t.TempDir()
	if err := os.WriteFile(filepath.Join(conflict, "PROJECT.md"), []byte("# Other\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	failed, success := chat.executeChatTool(ctx, conversation, projectUpdateName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 2, "folder": conflict}), "project-conflict", "turn:move")
	current, document, err := chat.projects.Read(ctx, projectID)
	if success || err != nil || current.Revision != 2 || current.Folder == nil || *current.Folder != folder ||
		document.Content != "# Exact\n" || !bytes.Contains(failed, []byte(`"code":"invalid_input"`)) {
		t.Fatalf("conflicting move = %s, %t; current %#v, document %#v, error %v", failed, success, current, document, err)
	}
	taskID, _ := store.NewTaskID()
	if _, err := home.CreatePendingTaskDocument(chat.home, taskID, "# Linked\n"); err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTaskWithOptions(ctx, taskID, "Linked",
		store.TaskCommand{Name: "task.capture", ClientMutationID: "linked-task",
			RequestDigest: strings.Repeat("a", 64), CorrelationID: "correlation:test:linked-task"},
		store.TaskCreateOptions{ProjectID: projectID, ExecutorAgentID: store.TaskExecutorAgentID}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(chat.home, taskID); err != nil {
		t.Fatal(err)
	}
	inspected, success := chat.executeChatTool(ctx, conversation, taskInspectName,
		projectTestArguments(t, map[string]any{"task_id": task.Task.ID}), "task-inspect-linked", "turn:inspect")
	if !success || projectTestValue(t, inspected)["project_id"] != projectID {
		t.Fatalf("linked Task inspection = %s, %t", inspected, success)
	}
	archived, success := chat.executeChatTool(ctx, conversation, projectArchiveName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 2}), "project-archive", "turn:lifecycle")
	if !success || projectTestValue(t, archived)["project"].(map[string]any)["archived"] != true {
		t.Fatalf("archive Project = %s, %t", archived, success)
	}
	reopened, success := chat.executeChatTool(ctx, conversation, projectReopenName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 3}), "project-reopen", "turn:lifecycle")
	current, err = database.Project(ctx, projectID)
	linked, taskErr := database.Task(ctx, task.Task.ID)
	if !success || err != nil || current.Revision != 4 || current.ArchivedAt != nil ||
		projectTestValue(t, reopened)["project"].(map[string]any)["archived"] != false ||
		taskErr != nil || linked.State != store.TaskCaptured || linked.Revision != 1 || linked.ProjectID != projectID {
		t.Fatalf("reopen Project = %s, %t; current %#v, error %v", reopened, success, current, err)
	}
}

func TestProjectToolStringContracts(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	ctx := context.Background()
	unicodeName := strings.Repeat("é", 200)
	created, success := chat.executeChatTool(ctx, conversation, projectCreateName,
		projectTestArguments(t, map[string]any{"name": unicodeName}), "project-unicode", "turn:unicode")
	if !success {
		t.Fatalf("Unicode Project creation = %s", created)
	}
	projectID := projectTestValue(t, created)["project"].(map[string]any)["project_id"].(string)
	longName, longDescription := strings.Repeat("n", 201), strings.Repeat("d", 20001)
	updated, success := chat.executeChatTool(ctx, conversation, projectUpdateName,
		projectTestArguments(t, map[string]any{"project_id": projectID, "expected_revision": 1,
			"name": longName, "description": longDescription}), "project-long-update", "turn:unicode")
	if !success {
		t.Fatalf("schema-valid Project update = %s", updated)
	}
}

func TestPrimaryChatRefreshesProjectContextForContinuation(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	var requests []provider.GenerateRequest
	chat.openRouter = generatorFunc(func(_ context.Context, request provider.GenerateRequest,
		_ func(provider.StreamEvent)) (provider.GenerationResult, error) {
		requests = append(requests, request)
		if len(requests) == 1 {
			return provider.GenerationResult{ID: "project-context-initial", ToolCalls: []provider.GenerationToolCall{{
				ProviderCallID: "project-context-create", ProviderName: projectCreateName,
				Name: projectCreateName, Payload: json.RawMessage(`{"name":"Context refresh"}`),
			}}}, nil
		}
		return provider.GenerationResult{ID: "project-context-final", Text: "Created."}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err = chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Create the project.",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	if len(requests) != 2 || !requestProjectContextContains(t, requests[0], `"projects":[]`) ||
		!requestProjectContextContains(t, requests[1], `"name":"Context refresh"`) {
		t.Fatalf("Project context requests = %#v", requests)
	}
}

func requestProjectContextContains(t *testing.T, request provider.GenerateRequest, text string) bool {
	return strings.Contains(modelContextSectionContent(t, request.Messages, "projects.catalog"), text)
}

func projectTestArguments(t *testing.T, value any) json.RawMessage {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func projectTestValue(t *testing.T, payload json.RawMessage) map[string]any {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal(payload, &value); err != nil {
		t.Fatal(err)
	}
	return value
}

func stringPointer(value string) *string { return &value }
