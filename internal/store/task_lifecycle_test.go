package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"path/filepath"
	"testing"
	"time"
)

func TestTaskLifecycleSchemaConvergesFromVersionFourteen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v14.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(14) + `
INSERT INTO tasks(task_id,title,state,revision,created_at_ms,updated_at_ms)
VALUES ('task:0123456789abcdef0123456789abcdef','Existing','captured',1,1,1);
PRAGMA user_version=14;`); err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	task, err := database.Task(context.Background(), "task:0123456789abcdef0123456789abcdef")
	if err != nil || task.Generation != 1 || task.StageKey != "inbox" {
		t.Fatalf("upgraded Task = %#v, %v", task, err)
	}
	for _, table := range []string{"task_runs", "task_gates", "task_messages", "task_run_items"} {
		var exists bool
		if err := database.db.QueryRow(`SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)`, table).Scan(&exists); err != nil || !exists {
			t.Fatalf("table %s = %t, %v", table, exists, err)
		}
	}
}

func TestTaskInboxUpdateIsRepeatSafeAndFenced(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	task, err := database.CreateTask(ctx, id, "Original", "correlation:capture", now)
	if err != nil {
		t.Fatal(err)
	}
	title := "Changed"
	command := testTaskLifecycleCommand("update_inbox_task", "same")
	result, err := database.UpdateInboxTask(ctx, id, 1, 1, TaskUpdate{Title: &title, DocumentDigest: stringsOf('a', 64)}, command, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	replay, err := database.UpdateInboxTask(ctx, id, 1, 1, TaskUpdate{Title: &title, DocumentDigest: stringsOf('a', 64)}, command, now.Add(2*time.Second))
	if err != nil || !replay.Replayed || replay.Event.ID != result.Event.ID {
		t.Fatalf("replay = %#v, %v", replay, err)
	}
	other := testTaskLifecycleCommand("update_inbox_task", "stale")
	if _, err = database.UpdateInboxTask(ctx, id, 1, 1, TaskUpdate{Title: &title}, other, now); err == nil {
		t.Fatal("stale Task update succeeded")
	}
	current, _ := database.Task(ctx, task.ID)
	if current.Title != "Changed" || current.Revision != 2 || current.Generation != 1 {
		t.Fatalf("updated Task = %#v", current)
	}
}

func TestTaskQueueCancelAndReopenKeepRunAuthority(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if created, err := database.ConfirmHostedModelAssignments(context.Background(), account.ID, testModelAssignments(account, "model-a")); err != nil || !created {
		t.Fatalf("assignments = %t, %v", created, err)
	}
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	if _, err := database.CreateTask(ctx, id, "Lifecycle", "correlation:capture", now); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "queue"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if queued.Task.StageKey != "queue" || queued.Task.CurrentRunID == "" || queued.Task.Revision != 2 {
		t.Fatalf("queued Task = %#v", queued.Task)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) != 1 || runs[0].Kind != "planner" || runs[0].ModelProfile == nil {
		t.Fatalf("queued runs = %#v, %v", runs, err)
	}
	cancelled, err := database.CancelTask(ctx, id, 2, 1, testTaskLifecycleCommand("cancel_task", "cancel"), now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if cancelled.Task.StageKey != "cancelled" || cancelled.Task.Generation != 2 || cancelled.Task.CurrentRunID != "" {
		t.Fatalf("cancelled Task = %#v", cancelled.Task)
	}
	complexity := "simple"
	reopened, err := database.ReopenTask(ctx, id, 3, 2, "Try again", &complexity, "", testTaskLifecycleCommand("reopen_task", "reopen"), now.Add(3*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if reopened.Task.StageKey != "queue" || reopened.Task.Generation != 3 || reopened.Task.CurrentRunID == "" {
		t.Fatalf("reopened Task = %#v", reopened.Task)
	}
	runs, err = database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) != 2 || runs[0].Kind != "executor" || runs[1].Status != "cancelled" {
		t.Fatalf("final runs = %#v, %v", runs, err)
	}
}

func TestTaskGateResolutionAndRunItemPaging(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	_, _ = database.ConfirmHostedModelAssignments(context.Background(), account.ID, testModelAssignments(account, "model-a"))
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	id, _ := NewTaskID()
	_, _ = database.CreateTask(ctx, id, "Gate", "correlation:capture", now)
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "gate-queue"), now)
	if err != nil {
		t.Fatal(err)
	}
	gateID := "gate:recovery"
	if _, err = database.db.Exec(`INSERT INTO task_gates(gate_id,task_id,task_generation,gate_kind,gate_state,retry_run_kind,prompt,opened_by,originating_run_id,opened_at_ms) VALUES (?,?,1,'recovery','open','planner','Choose','actor:system',?,?)`, gateID, id, queued.Task.CurrentRunID, millis(now)); err != nil {
		t.Fatal(err)
	}
	if _, err = database.db.Exec(`UPDATE tasks SET stage_key='waiting',active_gate_id=? WHERE task_id=?`, gateID, id); err != nil {
		t.Fatal(err)
	}
	resolved, err := database.ResolveTaskGate(ctx, id, gateID, 2, 1, "Retry", "retry", testTaskLifecycleCommand("retry_task", "retry"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if resolved.Task.StageKey != "queue" || resolved.Task.ActiveGateID != "" || resolved.Task.CurrentRunID == queued.Task.CurrentRunID {
		t.Fatalf("resolved Task = %#v", resolved.Task)
	}
	gate, err := database.TaskGate(ctx, gateID)
	if err != nil || gate.State != "resolved" || gate.ResolutionMessageID == nil {
		t.Fatalf("resolved gate = %#v, %v", gate, err)
	}
	for index := range 3 {
		_, err = database.db.Exec(`INSERT INTO task_run_items(item_id,run_id,sequence_index,item_kind,status,content_text,created_at_ms,updated_at_ms) VALUES (?,?,?,'assistant_output','completed',?,?,?)`, "run_item:"+string(rune('a'+index)), resolved.Task.CurrentRunID, index, "item", millis(now), millis(now))
		if err != nil {
			t.Fatal(err)
		}
	}
	page, err := database.TaskRunItems(ctx, resolved.Task.CurrentRunID, 2, nil)
	if err != nil || len(page.Items) != 2 || !page.HasNextPage {
		t.Fatalf("first page = %#v, %v", page, err)
	}
	next, err := database.TaskRunItems(ctx, resolved.Task.CurrentRunID, 2, page.EndCursor)
	if err != nil || len(next.Items) != 1 || next.Items[0].Sequence != 0 {
		t.Fatalf("next page = %#v, %v", next, err)
	}
}

func testTaskLifecycleCommand(name, key string) TaskCommand {
	sum := sha256.Sum256([]byte(name + "\x00" + key))
	return TaskCommand{Name: name, ClientMutationID: key, RequestDigest: hex.EncodeToString(sum[:]), CorrelationID: "correlation:test:" + key}
}
func stringsOf(value byte, count int) string {
	bytes := make([]byte, count)
	for i := range bytes {
		bytes[i] = value
	}
	return string(bytes)
}
