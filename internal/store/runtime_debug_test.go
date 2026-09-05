package store

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"
)

func TestRuntimeDebugMigratesAndBoundsCompletedSpans(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "v31.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec(schemaAtVersion(31) + "; PRAGMA user_version=31"); err != nil {
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openai", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Profile this.", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	scope := RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}
	for index := 0; index < runtimeDebugSpanLimit+2; index++ {
		started := time.Now().Add(time.Duration(index) * time.Millisecond)
		id, beginErr := database.BeginRuntimeDebugSpan(ctx, scope, "provider", "Provider request", RuntimeDebugMetadata{}, started)
		if beginErr != nil {
			t.Fatal(beginErr)
		}
		if finishErr := database.FinishRuntimeDebugSpan(ctx, id, "completed", RuntimeDebugMetadata{}, time.Millisecond, started.Add(time.Millisecond)); finishErr != nil {
			t.Fatal(finishErr)
		}
	}
	profile, err := database.RuntimeDebugProfile(ctx, scope)
	if err != nil || profile == nil || len(profile.Spans) != runtimeDebugSpanLimit {
		t.Fatalf("bounded profile = %#v, %v", profile, err)
	}
	var version int
	if err = database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema version = %d, %v", version, err)
	}
	account := createReadyModelAccount(t, database)
	if _, err = database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	taskID, _ := NewTaskID()
	if _, err = database.CreateTask(ctx, taskID, "Profile Task", "correlation:debug", time.Now()); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, taskID, 1, 1, testTaskLifecycleCommand("queue_task", "debug"), time.Now())
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued Task = %#v, %v", queued, err)
	}
	runScope := RuntimeDebugScope{Kind: "task_run", ID: queued.Task.CurrentRunID}
	runSpan, err := database.BeginRuntimeDebugSpan(ctx, runScope, "runtime", "Prepare Task", RuntimeDebugMetadata{}, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	if err = database.FinishRuntimeDebugSpan(ctx, runSpan, "completed", RuntimeDebugMetadata{}, time.Millisecond, time.Now()); err != nil {
		t.Fatal(err)
	}
	runProfile, err := database.RuntimeDebugProfile(ctx, runScope)
	if err != nil || runProfile == nil || runProfile.Scope != runScope || len(runProfile.Spans) != 1 {
		t.Fatalf("Task profile = %#v, %v", runProfile, err)
	}
}
