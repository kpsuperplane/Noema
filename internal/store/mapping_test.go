package store

import (
	"testing"
	"time"
)

func TestTaskMappingPreservesMillisecondsAndClearsNullableFields(t *testing.T) {
	database := openTestStore(t)
	id, _ := NewTaskID()
	now := time.Date(2026, 9, 5, 12, 0, 0, 123000000, time.FixedZone("offset", 3600))
	task, err := database.CreateTask(t.Context(), id, "Mapped Task", "correlation:mapping", now)
	if err != nil {
		t.Fatal(err)
	}
	for _, populated := range []bool{true, false} {
		task.ScheduledFor, task.CwdOverride = nil, nil
		if populated {
			task.ScheduledFor = &now
			cwd := "workspace with 'quotes'"
			task.CwdOverride = &cwd
		}
		if _, err := database.db.NewUpdate().Model(&task).
			Column("scheduled_for_ms", "cwd_override").WherePK().Exec(t.Context()); err != nil {
			t.Fatal(err)
		}
		loaded, err := database.Task(t.Context(), id)
		if err != nil || !loaded.CreatedAt.Equal(now) || loaded.CreatedAt.Location() != time.UTC {
			t.Fatalf("timestamp mapping: %#v, %v", loaded, err)
		}
		if populated {
			if loaded.ScheduledFor == nil || !loaded.ScheduledFor.Equal(now) || loaded.CwdOverride == nil || *loaded.CwdOverride != *task.CwdOverride {
				t.Fatalf("populated mapping: %#v", loaded)
			}
		} else if loaded.ScheduledFor != nil || loaded.CwdOverride != nil {
			t.Fatalf("cleared mapping: %#v", loaded)
		}
	}
	var raw int64
	if err := database.db.QueryRow("SELECT created_at_ms FROM tasks WHERE task_id=?", id).Scan(&raw); err != nil || raw != now.UnixMilli() {
		t.Fatalf("stored milliseconds = %d, %v", raw, err)
	}
}

func TestMappedTaskRunUpdateUsesTheExistingTransaction(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(t.Context(), account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 5, 12, 0, 0, 123000000, time.UTC)
	id, _ := NewTaskID()
	if _, err := database.CreateTask(t.Context(), id, "Run mapping", "correlation:run:mapping", now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.QueueTask(t.Context(), id, 1, 1, testTaskLifecycleCommand("queue_task", "mapping"), now); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(t.Context(), id, 10)
	if err != nil || len(runs) != 1 {
		t.Fatalf("queued runs: %#v, %v", runs, err)
	}
	run := runs[0]
	if !run.CreatedAt.Equal(now) || run.StartedAt != nil {
		t.Fatalf("run timestamps: %#v", run)
	}
	tx, err := database.db.BeginTx(t.Context(), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	for _, fast := range []bool{true, false} {
		run.FastMode, run.StartedAt = fast, &now
		if _, err := tx.NewUpdate().Model(&run).Column("fast_mode", "started_at_ms").WherePK().Exec(t.Context()); err != nil {
			t.Fatal(err)
		}
		loaded, err := taskRunTx(t.Context(), tx, run.ID)
		if err != nil || loaded.FastMode != fast || loaded.StartedAt == nil || !loaded.StartedAt.Equal(now) {
			t.Fatalf("transaction mapping: %#v, %v", loaded, err)
		}
	}
	if err := tx.Rollback(); err != nil {
		t.Fatal(err)
	}
	loaded, err := taskRunTx(t.Context(), database.db, run.ID)
	if err != nil || loaded.StartedAt != nil {
		t.Fatalf("mapped update survived rollback: %#v, %v", loaded, err)
	}
}
