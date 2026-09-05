package store

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"
)

func TestPrimaryNotificationsPersistCardsArtifactsAndCursorOnce(t *testing.T) {
	database := openTestStore(t)
	ctx, now := context.Background(), time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	taskID, _ := NewTaskID()
	task, err := database.CreateTask(ctx, taskID, "Notify Chat", "correlation:test", now)
	if err != nil {
		t.Fatal(err)
	}
	events, _ := database.WorkEvents(ctx, "workspace:personal", 0, 10)
	bad := PrimaryNotificationWrite{Event: events[0], Conversation: Conversation{ID: "conversation:00000000000000000000000000000000"}, Source: "work_notification", Task: &task}
	if _, err = database.CommitPrimaryNotification(ctx, bad, now); err == nil {
		t.Fatal("missing primary Chat accepted a notification")
	}
	if cursor, _ := database.PrimaryTaskNotificationCursor(ctx); cursor != 0 {
		t.Fatalf("missing Chat advanced cursor to %d", cursor)
	}
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Capture it.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if err = database.CancelConversationTurn(ctx, turn, now); err != nil {
		t.Fatal(err)
	}
	task.Source = ArtifactSource{ConversationID: conversation.ID, TurnID: turn.ID}
	items, err := database.CommitPrimaryNotification(ctx, PrimaryNotificationWrite{
		Event: events[0], Conversation: conversation, Source: "work_notification", Task: &task,
	}, now)
	if err != nil || len(items) != 1 || items[0].Kind != ConversationTaskReference || items[0].TurnID != turn.ID {
		t.Fatalf("capture notification = %#v, %v", items, err)
	}
	url, media := "https://example.test/result", "text/plain"
	artifact, err := database.CreateArtifact(ctx, Artifact{
		ID: "artifact:0123456789abcdef0123456789abcdef", Owner: ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: "Result", Kind: "report", StorageKind: ArtifactExternalURL, CreatedByActorID: "agent:task",
	}, ArtifactVersion{ID: "artifact_version:0123456789abcdef0123456789abcdef", ExternalURL: &url,
		MediaType: &media, CreatedByActorID: "agent:task"}, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = database.StartTask(ctx, task.ID, "run:one", now); err != nil {
		t.Fatal(err)
	}
	completed, err := database.FinishTask(ctx, task.ID, "run:one", TaskCompleted, now)
	if err != nil {
		t.Fatal(err)
	}
	events, _ = database.WorkEvents(ctx, "workspace:personal", events[0].ID, 10)
	completion := PrimaryNotificationWrite{Event: events[len(events)-1], Conversation: conversation,
		Source: "work_notification", Text: "The Task is complete.", Task: &completed,
		Artifacts: []ArtifactWithVersions{artifact}}
	items, err = database.CommitPrimaryNotification(ctx, completion, now)
	if err != nil || len(items) != 3 || items[0].Kind != ConversationAssistantText ||
		items[1].Kind != ConversationTaskReference || items[2].Kind != ConversationArtifactReference ||
		items[2].Payload["artifact_version_id"] != artifact.CurrentVersion.ID {
		t.Fatalf("completion notification = %#v, %v", items, err)
	}
	if repeated, err := database.CommitPrimaryNotification(ctx, completion, now); err != nil || len(repeated) != 0 {
		t.Fatalf("repeated notification = %#v, %v", repeated, err)
	}
	if cursor, _ := database.PrimaryTaskNotificationCursor(ctx); cursor != completion.Event.ID {
		t.Fatalf("completion cursor = %d", cursor)
	}
	for range 2 {
		if err := database.RecordCapabilityReady(ctx, "mcp", "Files", "connection:files", "revision:one", 3, now); err != nil {
			t.Fatal(err)
		}
	}
	readiness, err := database.WorkEvents(ctx, "workspace:personal", completion.Event.ID, 10)
	if err != nil || len(readiness) != 1 || readiness[0].Kind != "capability.ready" {
		t.Fatalf("readiness events = %#v, %v", readiness, err)
	}
}

func TestPrimaryNotificationSchemaBackfillsCurrentWorkEvent(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v29.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	_, err = legacy.Exec(schemaAtVersion(29) + `
INSERT INTO work_events(event_key,workspace_id,subject_revision,kind,actor_id,correlation_id,payload_json,occurred_at_ms)
VALUES('event:0123456789abcdef0123456789abcdef','workspace:personal',1,'test.event','actor:test','correlation:test','{}',1);
PRAGMA user_version=29;`)
	_ = legacy.Close()
	if err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	if cursor, err := database.PrimaryTaskNotificationCursor(context.Background()); err != nil || cursor != 1 {
		t.Fatalf("v29 cursor = %d, %v", cursor, err)
	}
}
