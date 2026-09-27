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
		if index < 3 {
			continue
		}
		if finishErr := database.FinishRuntimeDebugSpan(ctx, id, "completed", RuntimeDebugMetadata{}, time.Millisecond, started.Add(time.Millisecond)); finishErr != nil {
			t.Fatal(finishErr)
		}
	}
	var spanCount int
	if err = database.db.QueryRow(`SELECT COUNT(*) FROM runtime_debug_spans WHERE conversation_turn_id=?`, turn.ID).Scan(&spanCount); err != nil || spanCount != runtimeDebugSpanLimit {
		t.Fatalf("stored span count = %d, %v", spanCount, err)
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

func TestRuntimeDebugChatTextMatchesRequestAndPreservesParagraphs(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Now().UTC().Truncate(time.Millisecond)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openai", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Inspect response", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	scope := RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}
	for index, phase := range []string{"initial", "continuation", "continuation"} {
		round := min(index, 1)
		start := now.Add(time.Duration(index*100) * time.Millisecond)
		id, err := database.BeginRuntimeDebugSpan(ctx, scope, "provider", phase, RuntimeDebugMetadata{RoundIndex: &round}, start)
		if err != nil {
			t.Fatal(err)
		}
		if index != 1 {
			for paragraph, text := range []string{"First 日本語 — https://example.com/authorization", "Second\nline"} {
				_, err = database.SaveConversationOutput(ctx, turn, round, 0, 0, paragraph, "message", "final_answer", "", text, "", "completed", false, nil, start.Add(10*time.Millisecond))
				if err != nil {
					t.Fatal(err)
				}
			}
		}
		status := "completed"
		if index == 1 {
			status = "failed"
		}
		if err := database.FinishRuntimeDebugSpan(ctx, id, status, RuntimeDebugMetadata{RoundIndex: &round}, 50*time.Millisecond, start.Add(50*time.Millisecond)); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := database.FailConversationTurn(ctx, turn, "Finished test turn", now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	otherTurn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Other turn", nil, now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.SaveConversationOutput(ctx, otherTurn, 0, 0, 0, 0, "message", "final_answer", "", "Other turn must stay separate", "", "completed", false, nil, now.Add(10*time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	profile, err := database.RuntimeDebugProfile(ctx, scope)
	if err != nil {
		t.Fatal(err)
	}
	want := "First 日本語 — https://example.com/authorization\n\nSecond\nline"
	if len(profile.Spans) != 3 || profile.Spans[0].ResponseText != want || profile.Spans[1].ResponseText != "" || profile.Spans[2].ResponseText != want {
		t.Fatalf("response matching = %#v", profile.Spans)
	}
	// Deleted transcript text must not reappear through the profile.
	if _, err := database.db.Exec(`UPDATE conversation_items SET deleted_at_ms=? WHERE turn_id=? AND kind='assistant_text'`, millis(now), turn.ID); err != nil {
		t.Fatal(err)
	}
	profile, err = database.RuntimeDebugProfile(ctx, scope)
	if err != nil {
		t.Fatal(err)
	}
	for _, span := range profile.Spans {
		if span.ResponseText != "" {
			t.Fatal("deleted text appeared in profile")
		}
	}
}

func TestRuntimeDebugTaskTextUsesOrderedOutputWithoutDuplicateSummary(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Now().UTC().Truncate(time.Millisecond)
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	taskID, _ := NewTaskID()
	if _, err := database.CreateTask(ctx, taskID, "Inspect Task response", "correlation:debug-text", now); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, taskID, 1, 1, testTaskLifecycleCommand("queue_task", "debug-text"), now)
	if err != nil {
		t.Fatal(err)
	}
	scope := RuntimeDebugScope{Kind: "task_run", ID: queued.Task.CurrentRunID}
	for index, payload := range []string{
		`{"output":[{"kind":"reasoning","text":"Check the saved facts."},{"kind":"message","text":"Full response\n日本語"}]}`,
		`{"output":[]}`,
	} {
		round := index + 1
		start := now.Add(time.Duration(index*100) * time.Millisecond)
		id, err := database.BeginRuntimeDebugSpan(ctx, scope, "provider", "Task provider request", RuntimeDebugMetadata{RoundIndex: &round}, start)
		if err != nil {
			t.Fatal(err)
		}
		_, err = database.db.Exec(`INSERT INTO task_run_items(item_id,run_id,sequence_index,round_index,item_kind,status,content_text,payload_json,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,'assistant_output','completed',?,?,?,?)`, "run_item:debug-text-"+string(rune('a'+index)), scope.ID, index, round, "Summary", payload, millis(start.Add(time.Millisecond)), millis(start.Add(time.Millisecond)))
		if err != nil {
			t.Fatal(err)
		}
		if err := database.FinishRuntimeDebugSpan(ctx, id, "completed", RuntimeDebugMetadata{RoundIndex: &round}, 50*time.Millisecond, start.Add(50*time.Millisecond)); err != nil {
			t.Fatal(err)
		}
	}
	profile, err := database.RuntimeDebugProfile(ctx, scope)
	if err != nil {
		t.Fatal(err)
	}
	if len(profile.Spans) != 2 || profile.Spans[0].ResponseText != "Check the saved facts.\n\nFull response\n日本語" || profile.Spans[1].ResponseText != "Summary" {
		t.Fatalf("Task response text = %#v", profile.Spans)
	}
	other, err := database.RuntimeDebugProfile(ctx, RuntimeDebugScope{Kind: "task_run", ID: "run:unrelated"})
	if err != nil || other != nil {
		t.Fatalf("unrelated profile = %#v, %v", other, err)
	}
}
