package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskModelToolCatalogAndStrictArguments(t *testing.T) {
	want := []string{
		taskCaptureName, taskListName, taskInspectName, taskUpdateName, taskQueueName,
		taskScheduleName, taskRescheduleName, taskUnscheduleName, taskScheduleRunNowName,
		taskRecurrenceUpdateName, taskRecurrencePauseName, taskRecurrenceResumeName,
		taskRecurrenceSkipName, taskRecurrenceEndName, taskRecurrenceRunNowName,
		taskDelegateName, taskAnswerName, taskRetryName, taskCancelName, taskReopenName,
	}
	found := map[string]bool{}
	for _, tool := range localChatTools() {
		found[tool.Name] = true
	}
	for _, name := range want {
		if !found[name] || !supportsLocalChatTool(name) {
			t.Fatalf("primary Task tool %q is unavailable", name)
		}
	}
	roles := map[string][]string{}
	for _, kind := range []string{"planner", "executor", "reviewer"} {
		for _, tool := range taskExecutionTools(kind) {
			roles[kind] = append(roles[kind], tool.Name)
		}
	}
	for _, contract := range []struct {
		kind string
		name string
	}{{"planner", taskListArtifactsName}, {"executor", taskCaptureName}, {"executor", taskListName},
		{"executor", artifactCreateLocalName}, {"executor", taskReadArtifactName}, {"executor", taskParseArtifactName},
		{"reviewer", taskReadArtifactName}, {"reviewer", taskParseArtifactName}} {
		if !containsTaskTool(roles[contract.kind], contract.name) {
			t.Fatalf("%s lacks %s: %v", contract.kind, contract.name, roles[contract.kind])
		}
	}
	chat, _, conversation := chatFixture(t)
	for _, raw := range []json.RawMessage{json.RawMessage(`{"title":"One","extra":true}`),
		json.RawMessage(`{"title":"One","title":"Two"}`),
		json.RawMessage(`{"title":"One","task_document":"# One\n","schedule":{"scheduled_for":"2099-01-01T00:00:00Z","extra":true}}`),
		json.RawMessage{'{', '"', 't', 'i', 't', 'l', 'e', '"', ':', '"', 0xff, '"', '}'}} {
		payload, success := chat.executeChatTool(t.Context(), conversation, taskCaptureName, raw, "bad-task", "turn:bad")
		if success || !bytes.Contains(payload, []byte(`"code":"invalid_input"`)) {
			t.Fatalf("invalid Task arguments = %s, %t", payload, success)
		}
	}
}

func TestPrimaryTaskToolsCaptureUpdateListScheduleAndReplay(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, sourceItem, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Do the requested Task", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	created, success := chat.executeChatTool(ctx, conversation, taskCaptureName,
		mustToolJSON(t, map[string]any{"title": " First ", "task_document": "# First\n"}), "task-create", turn.ID,
		chatTaskToolDetails{SourceItemID: sourceItem.ID, TimeZone: "America/Los_Angeles"})
	if !success {
		t.Fatalf("capture = %s", created)
	}
	createdValue := mustToolValue(t, created)
	task := createdValue["task"].(map[string]any)
	taskID := task["task_id"].(string)
	storedSource, err := database.Task(ctx, taskID)
	if err != nil || storedSource.Source != (store.ArtifactSource{ConversationID: conversation.ID, TurnID: turn.ID, ItemID: sourceItem.ID}) ||
		storedSource.SourceToolCallID != "task-create" || storedSource.SourceClientTimeZone != "America/Los_Angeles" {
		t.Fatalf("Task source = %#v, %v", storedSource, err)
	}
	replayed, success := chat.executeChatTool(ctx, conversation, taskCaptureName,
		mustToolJSON(t, map[string]any{"title": " First ", "task_document": "# First\n"}), "task-create", turn.ID)
	if !success || mustToolValue(t, replayed)["event_sequence"] != createdValue["event_sequence"] {
		t.Fatalf("capture replay = %s, %t", replayed, success)
	}
	blank, success := chat.executeChatTool(ctx, conversation, taskUpdateName,
		mustToolJSON(t, map[string]any{"task_id": taskID, "expected_revision": 1, "expected_generation": 1,
			"project_id": "  "}), "task-blank-project", "turn:update")
	if success || !bytes.Contains(blank, []byte(`"code":"invalid_input"`)) {
		t.Fatalf("blank Task placement = %s, %t", blank, success)
	}
	updated, success := chat.executeChatTool(ctx, conversation, taskUpdateName,
		mustToolJSON(t, map[string]any{"task_id": taskID, "expected_revision": 1, "expected_generation": 1,
			"title": "Updated", "task_document": "# Updated\n"}), "task-update", "turn:update")
	if !success || mustToolValue(t, updated)["task"].(map[string]any)["revision"] != float64(2) {
		t.Fatalf("update = %s, %t", updated, success)
	}
	document, err := home.ReadTaskDocument(chat.home, taskID)
	if err != nil || document.Content != "# Updated\n" {
		t.Fatalf("document = %#v, %v", document, err)
	}
	listed, success := chat.executeChatTool(ctx, conversation, taskListName,
		mustToolJSON(t, map[string]any{"limit": 10}), "task-list", "turn:list")
	if !success || len(mustToolValue(t, listed)["tasks"].([]any)) != 1 {
		t.Fatalf("list = %s, %t", listed, success)
	}
	future := time.Now().Add(2 * time.Hour).UTC().Format(time.RFC3339Nano)
	scheduled, success := chat.executeChatTool(ctx, conversation, taskScheduleName,
		mustToolJSON(t, map[string]any{"task_id": taskID, "expected_revision": 2, "expected_generation": 1,
			"scheduled_for": future, "missed_run_policy": "run_once"}),
		"task-schedule", "turn:schedule", chatTaskToolDetails{TimeZone: "America/New_York"})
	stored, err := database.Task(ctx, taskID)
	if !success || err != nil || stored.ScheduledFor == nil || stored.Revision != 3 || stored.ScheduleTimeZone != "America/New_York" ||
		mustToolValue(t, scheduled)["task"].(map[string]any)["missed_run_policy"] != "run_once" {
		t.Fatalf("schedule = %s, %t; Task %#v, %v", scheduled, success, stored, err)
	}
	unscheduled, success := chat.executeChatTool(ctx, conversation, taskUnscheduleName,
		mustToolJSON(t, map[string]any{"task_id": taskID, "expected_revision": 3, "expected_generation": 1}),
		"task-unschedule", "turn:schedule")
	if !success || mustToolValue(t, unscheduled)["task"].(map[string]any)["scheduled_for"] != nil {
		t.Fatalf("unschedule = %s, %t", unscheduled, success)
	}
	starts := time.Now().Add(time.Hour).UTC()
	first, err := schedule.NextAtOrAfter("0 9 * * 1", "UTC", starts)
	if err != nil {
		t.Fatal(err)
	}
	recurring, success := chat.executeChatTool(ctx, conversation, taskCaptureName,
		mustToolJSON(t, map[string]any{"title": "Repeat", "task_document": "# Repeat\n",
			"schedule": map[string]any{"scheduled_for": first.Format(time.RFC3339Nano), "time_zone": "UTC", "missed_run_policy": "skip",
				"recurrence": map[string]any{"starts_at": starts.Format(time.RFC3339Nano), "cron_expression": "0 9 * * 1", "overlap_policy": "queue_one"}}}),
		"task-repeat", turn.ID, chatTaskToolDetails{SourceItemID: sourceItem.ID})
	if !success {
		t.Fatalf("recurring capture = %s", recurring)
	}
	recurrenceID := mustToolValue(t, recurring)["recurrence_id"].(string)
	if mustToolValue(t, recurring)["recurrence_authority"] == nil {
		t.Fatalf("recurring result lacks authority: %s", recurring)
	}
	eventsBefore, _ := database.WorkEventsForTask(ctx, mustToolValue(t, recurring)["task"].(map[string]any)["task_id"].(string), 0, 100)
	empty, success := chat.executeChatTool(ctx, conversation, taskRecurrenceUpdateName,
		mustToolJSON(t, map[string]any{"recurrence_id": recurrenceID, "expected_revision": 1}), "task-repeat-empty", "turn:repeat")
	afterEmpty, _ := database.TaskRecurrence(ctx, recurrenceID)
	eventsAfter, _ := database.WorkEventsForTask(ctx, mustToolValue(t, recurring)["task"].(map[string]any)["task_id"].(string), 0, 100)
	if success || !bytes.Contains(empty, []byte(`"code":"invalid_input"`)) || afterEmpty.Revision != 1 || len(eventsAfter) != len(eventsBefore) {
		t.Fatalf("empty recurrence update = %s, %t; revision %d; events %d -> %d", empty, success, afterEmpty.Revision, len(eventsBefore), len(eventsAfter))
	}
	changed, success := chat.executeChatTool(ctx, conversation, taskRecurrenceUpdateName,
		mustToolJSON(t, map[string]any{"recurrence_id": recurrenceID, "expected_revision": 1,
			"title": "Repeat updated", "project_id": "  "}), "task-repeat-update", "turn:repeat")
	if !success || mustToolValue(t, changed)["recurrence_authority"].(map[string]any)["project_id"] != nil {
		t.Fatalf("blank recurrence placement = %s, %t", changed, success)
	}
	paused, success := chat.executeChatTool(ctx, conversation, taskRecurrencePauseName,
		mustToolJSON(t, map[string]any{"recurrence_id": recurrenceID, "expected_revision": 2}),
		"task-repeat-pause", "turn:repeat")
	if !success {
		t.Fatalf("pause recurrence = %s", paused)
	}
	recurrence, err := database.TaskRecurrence(ctx, recurrenceID)
	if err != nil || recurrence.Lifecycle != store.RecurrencePaused || recurrence.Revision != 3 {
		t.Fatalf("paused recurrence = %#v, %v", recurrence, err)
	}
}

func TestTaskDelegateIsAtomicAndSelectsInitialRun(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, sourceItem, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Do the requested Task", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	projectPayload, projectSuccess := chat.executeChatTool(t.Context(), conversation, projectCreateName,
		projectTestArguments(t, map[string]any{"name": "Delegated"}), "delegate-project", turn.ID)
	if !projectSuccess {
		t.Fatalf("create Project = %s", projectPayload)
	}
	projectID := projectTestValue(t, projectPayload)["project"].(map[string]any)["project_id"].(string)
	raw := mustToolJSON(t, map[string]any{"title": "Delegate", "task_document": "# Delegate\n",
		"project":          map[string]any{"kind": "existing", "project_id": projectID},
		"execution_intent": map[string]any{"request_markdown": "Do it.", "complexity": "simple"}})
	var conflicting map[string]any
	if err := json.Unmarshal(raw, &conflicting); err != nil {
		t.Fatal(err)
	}
	conflicting["complexity_hint"] = "simple"
	invalid, accepted := chat.executeChatTool(t.Context(), conversation, taskDelegateName,
		mustToolJSON(t, conflicting), "delegate-call", turn.ID, chatTaskToolDetails{SourceItemID: sourceItem.ID})
	if accepted || !bytes.Contains(invalid, []byte("Omit complexity_hint when execution_intent is supplied")) {
		t.Fatalf("conflicting delegation did not explain correction: %s", invalid)
	}
	payload, success := chat.executeChatTool(t.Context(), conversation, taskDelegateName, raw, "delegate-call", turn.ID, chatTaskToolDetails{SourceItemID: sourceItem.ID})
	if !success {
		t.Fatalf("delegate = %s", payload)
	}
	value := mustToolValue(t, payload)
	taskID := value["task"].(map[string]any)["task_id"].(string)
	task, err := database.Task(t.Context(), taskID)
	runs, runErr := database.TaskRuns(t.Context(), taskID, 10)
	if err != nil || runErr != nil || task.StageKey != "queue" || task.Revision != 2 ||
		task.ExecutionComplexity != "simple" || len(runs) != 1 || runs[0].Kind != "executor" || task.CurrentRunID != runs[0].ID {
		t.Fatalf("delegated Task = %#v; runs %#v; errors %v, %v", task, runs, err, runErr)
	}
	if value["run_id"] != runs[0].ID || value["project"].(map[string]any)["project_id"] != projectID {
		t.Fatalf("delegated result = %s", payload)
	}
	replayed, success := chat.executeChatTool(t.Context(), conversation, taskDelegateName, raw, "delegate-call", turn.ID, chatTaskToolDetails{SourceItemID: sourceItem.ID})
	page, _ := database.ListTasks(t.Context(), store.TaskListFilter{Scope: "all"}, 10, nil)
	if !success || mustToolValue(t, replayed)["event_sequence"] != value["event_sequence"] || len(page.Tasks) != 1 {
		t.Fatalf("delegate replay = %s, %t; Tasks %d", replayed, success, len(page.Tasks))
	}
}

func TestTaskArtifactToolsFenceOwnershipVersionsAndParse(t *testing.T) {
	chat, database, conversation := chatFixture(t)
	turn, sourceItem, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Do the requested Task", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	delegated, success := chat.executeChatTool(t.Context(), conversation, taskDelegateName,
		mustToolJSON(t, map[string]any{"title": "Artifacts", "task_document": "# Artifacts\n",
			"project":          map[string]any{"kind": "none"},
			"execution_intent": map[string]any{"request_markdown": "Create it.", "complexity": "simple"}}),
		"artifact-delegate", turn.ID, chatTaskToolDetails{SourceItemID: sourceItem.ID, TimeZone: "Europe/Paris"})
	if !success {
		t.Fatalf("delegate = %s", delegated)
	}
	taskID := mustToolValue(t, delegated)["task"].(map[string]any)["task_id"].(string)
	task, _ := database.Task(t.Context(), taskID)
	claimedTask, run, found, err := database.ClaimTaskExecution(t.Context(), time.Now())
	if err != nil || !found || claimedTask.ID != task.ID || database.StartTaskExecution(t.Context(), run.ID, run.Generation, time.Now()) != nil {
		t.Fatalf("claim = %#v, %#v, %t, %v", claimedTask, run, found, err)
	}
	service, err := artifact.New(chat.home, database, nil)
	if err != nil {
		t.Fatal(err)
	}
	runtime := &TaskExecution{database: database, root: chat.home, artifacts: service}
	run.Status = "running"
	created, success, _, _ := runtime.executeTaskTool(t.Context(), task, run, artifactCreateLocalName,
		mustToolJSON(t, map[string]any{"title": "Report", "artifact_kind": "document", "filename": "report.md",
			"media_type": "text/markdown", "versions": []any{map[string]any{"content": "# One\n"},
				map[string]any{"title": "Final", "content": "# Two\n"}}}), false)
	if !success {
		t.Fatalf("create Artifact = %s", created)
	}
	artifactID := mustToolValue(t, created)["artifact_id"].(string)
	createdArtifact, err := database.ArtifactWithVersionsByID(t.Context(), artifactID)
	if err != nil || createdArtifact.Artifact.Source != task.Source || createdArtifact.CurrentVersion.Source != task.Source {
		t.Fatalf("Artifact source = %#v, %v; Task source %#v", createdArtifact.Artifact.Source, err, task.Source)
	}
	childFuture := time.Now().Add(3 * time.Hour).UTC().Format(time.RFC3339Nano)
	childPayload, childSuccess := runtime.captureScopedTask(t.Context(), task, run,
		mustToolJSON(t, map[string]any{"title": "Child", "schedule": map[string]any{"scheduled_for": childFuture}}))
	child, childErr := database.Task(t.Context(), mustToolValue(t, childPayload)["task"].(map[string]any)["task_id"].(string))
	if !childSuccess || childErr != nil || child.ScheduleTimeZone != "Europe/Paris" || child.Source != task.Source {
		t.Fatalf("scoped capture = %s, %t; Task %#v, %v", childPayload, childSuccess, child, childErr)
	}
	listed, success, _, _ := runtime.executeTaskTool(t.Context(), task, run, taskListArtifactsName, json.RawMessage(`{}`), false)
	if !success || len(mustToolValue(t, listed)["artifacts"].([]any)) != 1 {
		t.Fatalf("list Artifacts = %s, %t", listed, success)
	}
	read, success, _, _ := runtime.executeTaskTool(t.Context(), task, run, taskReadArtifactName,
		mustToolJSON(t, map[string]any{"artifact_id": artifactID}), false)
	if !success || mustToolValue(t, read)["content"] != "# Two\n" {
		t.Fatalf("read Artifact = %s, %t", read, success)
	}
	parsed, success, _, _ := runtime.executeTaskTool(t.Context(), task, run, taskParseArtifactName,
		mustToolJSON(t, map[string]any{"artifact_id": artifactID, "max_chars": 1000}), false)
	parse := mustToolValue(t, parsed)["parse"].(map[string]any)
	if !success || parse["status"] != "converted" || !strings.Contains(parse["content"].(string), "# Two") {
		t.Fatalf("parse Artifact = %s, %t", parsed, success)
	}
	longContent := strings.Repeat("x", 70_000)
	longMediaType := "text/markdown"
	if _, err := service.AppendLocal(t.Context(), artifactID, "report.md", []byte(longContent), nil,
		&longMediaType, run.AgentID, store.ArtifactSource{}, nil); err != nil {
		t.Fatal(err)
	}
	read, success, _, _ = runtime.executeTaskTool(t.Context(), task, run, taskReadArtifactName,
		mustToolJSON(t, map[string]any{"artifact_id": artifactID}), false)
	if !success || mustToolValue(t, read)["content"] != longContent {
		t.Fatalf("long Artifact read = %d bytes, %t", len(read), success)
	}
	foreignTask := createQueuedRuntimeTask(t, database, chat.home, "# Foreign\n")
	foreignArtifact, err := service.CreateLocal(t.Context(), artifact.LocalInput{
		Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: foreignTask.ID}, Title: "Foreign",
		Kind: "document", Filename: "foreign.md", Bytes: []byte("private"), CreatedByActorID: run.AgentID,
	})
	if err != nil {
		t.Fatal(err)
	}
	foreign, success, _, _ := runtime.executeTaskTool(t.Context(), task, run, taskReadArtifactName,
		mustToolJSON(t, map[string]any{"artifact_id": foreignArtifact.Artifact.ID}), false)
	if success || !bytes.Contains(foreign, []byte(`"code":"unavailable"`)) {
		t.Fatalf("foreign Artifact = %s, %t", foreign, success)
	}
}

func containsTaskTool(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func mustToolJSON(t *testing.T, value any) json.RawMessage {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func mustToolValue(t *testing.T, payload json.RawMessage) map[string]any {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal(payload, &value); err != nil {
		t.Fatalf("decode %s: %v", payload, err)
	}
	return value
}
