package store

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestTaskStateAndEventsCommitTogether(t *testing.T) {
	store := openTestStore(t)
	ctx := context.Background()
	createdAt := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	if _, err := store.CreateTask(ctx, "task:../outside", "Invalid identifier", createdAt); err == nil {
		t.Fatal("invalid Task ID must fail")
	}

	taskID, err := NewTaskID()
	if err != nil {
		t.Fatalf("create task ID: %v", err)
	}
	task, err := store.CreateTask(ctx, taskID, "Migrate the server", createdAt)
	if err != nil {
		t.Fatalf("create task: %v", err)
	}
	if task.State != TaskCaptured || task.Revision != 1 {
		t.Fatalf("created task = %#v", task)
	}

	started, err := store.StartTask(ctx, task.ID, "run:one", createdAt.Add(time.Second))
	if err != nil {
		t.Fatalf("start task: %v", err)
	}
	if started.State != TaskRunning || started.CurrentRunID != "run:one" || started.Revision != 2 {
		t.Fatalf("started task = %#v", started)
	}

	completed, err := store.FinishTask(
		ctx,
		task.ID,
		"run:one",
		TaskCompleted,
		createdAt.Add(2*time.Second),
	)
	if err != nil {
		t.Fatalf("finish task: %v", err)
	}
	if completed.State != TaskCompleted || completed.CurrentRunID != "" || completed.Revision != 3 {
		t.Fatalf("completed task = %#v", completed)
	}

	events, err := store.TaskEvents(ctx, task.ID, 0)
	if err != nil {
		t.Fatalf("read events: %v", err)
	}
	if len(events) != 3 {
		t.Fatalf("event count = %d, want 3", len(events))
	}
	for index, event := range events {
		wantRevision := int64(index + 1)
		if event.Revision != wantRevision {
			t.Fatalf("event %d revision = %d, want %d", index, event.Revision, wantRevision)
		}
	}
}

func TestStaleRunCannotFinishTask(t *testing.T) {
	store := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	taskID, err := NewTaskID()
	if err != nil {
		t.Fatalf("create task ID: %v", err)
	}
	task, err := store.CreateTask(ctx, taskID, "Keep current run authority", now)
	if err != nil {
		t.Fatalf("create task: %v", err)
	}
	if _, err := store.StartTask(ctx, task.ID, "run:current", now.Add(time.Second)); err != nil {
		t.Fatalf("start task: %v", err)
	}

	_, err = store.FinishTask(ctx, task.ID, "run:stale", TaskCompleted, now.Add(2*time.Second))
	if !errors.Is(err, ErrStaleRun) {
		t.Fatalf("stale finish error = %v, want %v", err, ErrStaleRun)
	}

	current, err := store.Task(ctx, task.ID)
	if err != nil {
		t.Fatalf("read current task: %v", err)
	}
	if current.State != TaskRunning || current.CurrentRunID != "run:current" || current.Revision != 2 {
		t.Fatalf("current task changed: %#v", current)
	}

	events, err := store.TaskEvents(ctx, task.ID, 0)
	if err != nil {
		t.Fatalf("read events: %v", err)
	}
	if len(events) != 2 {
		t.Fatalf("event count = %d, want 2", len(events))
	}
}

func TestStoreReopensFreshGoSchema(t *testing.T) {
	parent := t.TempDir()
	path := filepath.Join(parent, "home?variant#one", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	store, err := Open(context.Background(), path)
	if err != nil {
		t.Fatalf("open store: %v", err)
	}
	taskID, err := NewTaskID()
	if err != nil {
		t.Fatalf("create task ID: %v", err)
	}
	task, err := store.CreateTask(context.Background(), taskID, "Persist a task", time.Now())
	if err != nil {
		t.Fatalf("create task: %v", err)
	}
	if err := store.Close(); err != nil {
		t.Fatalf("close store: %v", err)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("database does not use the selected path: %v", err)
	}
	if _, err := os.Stat(filepath.Join(parent, "home")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("SQLite URI escaped the selected path: %v", err)
	}

	store, err = Open(context.Background(), path)
	if err != nil {
		t.Fatalf("reopen store: %v", err)
	}
	t.Cleanup(func() { _ = store.Close() })
	if _, err := store.Task(context.Background(), task.ID); err != nil {
		t.Fatalf("read reopened task: %v", err)
	}
	var version int
	if err := store.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("fresh schema version = %d, %v", version, err)
	}

	upgradePath := filepath.Join(t.TempDir(), "v1.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(upgradePath))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(`
CREATE TABLE tasks (task_id TEXT PRIMARY KEY) STRICT;
CREATE TABLE task_events (event_id INTEGER PRIMARY KEY) STRICT;
PRAGMA user_version = 1;`); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	upgraded, err := Open(context.Background(), upgradePath)
	if err != nil {
		t.Fatalf("upgrade version 1 schema: %v", err)
	}
	defer upgraded.Close()
	if err := upgraded.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("upgraded schema version = %d, %v", version, err)
	}
	for _, table := range []string{
		"human_passkeys", "browser_sessions", "clients", "native_oauth_codes",
		"native_oauth_families", "native_oauth_refresh_tokens", "native_oauth_access_tokens",
		"provider_accounts", "conversations", "local_human_state",
	} {
		var exists bool
		if err := upgraded.db.QueryRow(
			"SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?)",
			table,
		).Scan(&exists); err != nil || !exists {
			t.Fatalf("upgraded table %s = %v, %v", table, exists, err)
		}
	}

	v2Path := filepath.Join(t.TempDir(), "v2.sqlite3")
	v2, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(v2Path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := v2.Exec(schemaV2SQL + "\nPRAGMA user_version = 2;"); err != nil {
		t.Fatal(err)
	}
	if err := v2.Close(); err != nil {
		t.Fatal(err)
	}
	fromV2, err := Open(context.Background(), v2Path)
	if err != nil {
		t.Fatalf("upgrade version 2 schema: %v", err)
	}
	defer fromV2.Close()
	if err := fromV2.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("version 2 upgrade = %d, %v", version, err)
	}
}

func openTestStore(t *testing.T) *Store {
	t.Helper()
	store, err := Open(context.Background(), filepath.Join(t.TempDir(), "noema.sqlite3"))
	if err != nil {
		t.Fatalf("open test store: %v", err)
	}
	t.Cleanup(func() {
		if err := store.Close(); err != nil {
			t.Errorf("close test store: %v", err)
		}
	})
	return store
}
