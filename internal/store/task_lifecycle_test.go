package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
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
	if _, err = legacy.Exec(schemaAtVersion(14) + `PRAGMA user_version=14;`); err != nil {
		t.Fatal(err)
	}
	states := []string{"captured", "running", "completed", "failed", "cancelled"}
	for index, state := range states {
		id := fmt.Sprintf("task:%032x", index+1)
		run := ""
		if state == "running" {
			run = "run:legacy"
		}
		if _, err = legacy.Exec(`INSERT INTO tasks(task_id,title,state,current_run_id,revision,created_at_ms,updated_at_ms)
VALUES (?,?,?,NULLIF(?,''),1,1,?)`, id, state, state, run, index+2); err != nil {
			t.Fatal(err)
		}
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	wantStages := []string{"inbox", "doing", "done", "waiting", "cancelled"}
	for index, stage := range wantStages {
		task, err := database.Task(context.Background(), fmt.Sprintf("task:%032x", index+1))
		if err != nil || task.Generation != 1 || task.StageKey != stage || (stage == "done") != (task.CompletedAt != nil) || (stage == "cancelled") != (task.CancelledAt != nil) {
			t.Fatalf("upgraded %s Task = %#v, %v", states[index], task, err)
		}
	}
	for _, table := range []string{"task_runs", "task_gates", "task_messages", "task_run_items"} {
		var exists bool
		if err := database.db.QueryRow(`SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)`, table).Scan(&exists); err != nil || !exists {
			t.Fatalf("table %s = %t, %v", table, exists, err)
		}
	}
}

func TestTaskLifecycleSchemaConvergesFromVersionSixteen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v16.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(16) + `
INSERT INTO tasks(task_id,title,state,stage_key,revision,created_at_ms,updated_at_ms)
VALUES ('task:00000000000000000000000000000001','cancelled','cancelled','inbox',1,1,9);
PRAGMA user_version=16;`); err != nil {
		t.Fatal(err)
	}
	_ = legacy.Close()
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	task, err := database.Task(context.Background(), "task:00000000000000000000000000000001")
	if err != nil || task.StageKey != "cancelled" || task.CancelledAt == nil {
		t.Fatalf("v16 Task = %#v, %v", task, err)
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
	cancelled, err := database.CancelTask(ctx, id, 2, 1, "reason", testTaskLifecycleCommand("cancel_task", "cancel"), now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if cancelled.Task.StageKey != "cancelled" || cancelled.Task.Generation != 2 || cancelled.Task.CurrentRunID != "" {
		t.Fatalf("cancelled Task = %#v", cancelled.Task)
	}
	if cancelled.Event.Payload["reason_present"] != true {
		t.Fatalf("cancel event payload = %#v", cancelled.Event.Payload)
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
	for _, test := range []struct {
		gate          TaskGate
		answer, retry bool
	}{
		{gate: TaskGate{Kind: "clarification"}, answer: true},
		{gate: TaskGate{Kind: "approval"}, answer: true},
		{gate: TaskGate{Kind: "recovery", RecoveryReason: stringAddressStore("infrastructure_retries_exhausted"), RetryRunKind: stringAddressStore("planner")}, answer: true, retry: true},
		{gate: TaskGate{Kind: "recovery", RecoveryReason: stringAddressStore("review_rounds_exhausted"), RetryRunKind: stringAddressStore("executor")}, retry: true},
		{gate: TaskGate{Kind: "recovery", RecoveryReason: stringAddressStore("unsafe_effect_uncertain"), RetryRunKind: stringAddressStore("executor")}, answer: true},
		{gate: TaskGate{Kind: "recovery", RecoveryReason: stringAddressStore("configuration_unavailable"), RetryRunKind: stringAddressStore("reviewer")}, retry: true},
		{gate: TaskGate{Kind: "recovery", RecoveryReason: stringAddressStore("invariant_fault")}},
	} {
		if test.gate.AllowsResolution("answer") != test.answer || test.gate.AllowsResolution("retry") != test.retry {
			t.Fatalf("gate policy = %#v", test)
		}
	}
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
	if _, err = database.db.Exec(`UPDATE task_runs SET status='waiting_for_approval',attempt_index=2,review_round=3 WHERE run_id=?`, queued.Task.CurrentRunID); err != nil {
		t.Fatal(err)
	}
	if _, err = database.db.Exec(`INSERT INTO task_gates(gate_id,task_id,task_generation,gate_kind,gate_state,recovery_reason,retry_run_kind,prompt,opened_by,originating_run_id,opened_at_ms) VALUES (?,?,1,'recovery','open','infrastructure_retries_exhausted','planner','Choose','actor:system',?,?)`, gateID, id, queued.Task.CurrentRunID, millis(now)); err != nil {
		t.Fatal(err)
	}
	if _, err = database.db.Exec(`UPDATE tasks SET stage_key='waiting',active_gate_id=? WHERE task_id=?`, gateID, id); err != nil {
		t.Fatal(err)
	}
	invalidDecision := "approved"
	if _, err = database.ResolveTaskGate(ctx, id, gateID, 2, 1, "Retry", "retry", &invalidDecision, testTaskLifecycleCommand("retry_task", "invalid-decision"), now); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("recovery approval decision error = %v", err)
	}
	resolved, err := database.ResolveTaskGate(ctx, id, gateID, 2, 1, "Retry", "retry", nil, testTaskLifecycleCommand("retry_task", "retry"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if resolved.Task.StageKey != "queue" || resolved.Task.ActiveGateID != "" || resolved.Task.CurrentRunID == queued.Task.CurrentRunID {
		t.Fatalf("resolved Task = %#v", resolved.Task)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) != 2 || runs[0].Kind != "planner" || runs[0].AttemptIndex != 3 || runs[0].ReviewRound != 3 || runs[1].Status != "completed" {
		t.Fatalf("gate run lineage = %#v, %v", runs, err)
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
	approvalTask, _ := NewTaskID()
	_, _ = database.CreateTask(ctx, approvalTask, "Approval", "correlation:approval", now)
	approvalRun, err := database.QueueTask(ctx, approvalTask, 1, 1, testTaskLifecycleCommand("queue_task", "approval-queue"), now)
	if err != nil {
		t.Fatal(err)
	}
	approvalGate, decision := "gate:approval", "approved"
	_, _ = database.db.Exec(`UPDATE task_runs SET status='waiting_for_approval' WHERE run_id=?`, approvalRun.Task.CurrentRunID)
	_, _ = database.db.Exec(`INSERT INTO task_gates(gate_id,task_id,task_generation,gate_kind,gate_state,prompt,opened_by,originating_run_id,opened_at_ms) VALUES (?,?,1,'approval','open','Approve','actor:system',?,?)`, approvalGate, approvalTask, approvalRun.Task.CurrentRunID, millis(now))
	_, _ = database.db.Exec(`UPDATE tasks SET stage_key='waiting',active_gate_id=? WHERE task_id=?`, approvalGate, approvalTask)
	if _, err = database.ResolveTaskGate(ctx, approvalTask, approvalGate, 2, 1, "Approved", "answer", nil, testTaskLifecycleCommand("answer_task", "missing-approval"), now); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("missing approval decision error = %v", err)
	}
	if _, err = database.ResolveTaskGate(ctx, approvalTask, approvalGate, 2, 1, "Approved", "answer", &decision, testTaskLifecycleCommand("answer_task", "approval"), now); err != nil {
		t.Fatal(err)
	}
	messages, err := database.TaskMessages(ctx, approvalTask, 1)
	if err != nil || len(messages) != 1 || messages[0].ApprovalDecision == nil || *messages[0].ApprovalDecision != "approved" {
		t.Fatalf("approval messages = %#v, %v", messages, err)
	}
}

func TestTaskOverviewCountsBeyondRecentPage(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	for index := range 101 {
		id := fmt.Sprintf("task:%032x", index+1)
		if _, err := database.CreateTask(ctx, id, "Task", fmt.Sprintf("correlation:overview:%d", index), now); err != nil {
			t.Fatal(err)
		}
	}
	page, counts, needs, err := database.TaskOverview(ctx, "", 50)
	if err != nil || len(page.Tasks) != 50 || counts["inbox"] != 101 || needs != 0 {
		t.Fatalf("overview = %d, %#v, %d, %v", len(page.Tasks), counts, needs, err)
	}
}

func stringAddressStore(value string) *string { return &value }

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
