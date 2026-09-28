package graphql

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskLifecyclePublishesDocumentsAndReadModels(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	captured, err := r.captureTask(ctx, model.CaptureTaskInput{
		WorkspaceID: "workspace:personal", Title: "Draft", TaskDocument: "First",
		ExecutorAgentID: stringAddress("agent:task-executor"), ClientMutationID: "capture-lifecycle",
	})
	if err != nil {
		t.Fatal(err)
	}
	if captured.Task.Source.ConversationID != nil {
		t.Fatal("manual capture must not invent a conversation source")
	}
	id, digest := captured.Task.TaskID, captured.Task.TaskDocumentDigest
	initialAuthority, err := r.Store.Task(ctx, id)
	if err != nil || !strings.Contains(initialAuthority.AuthorizationContext, `"task_document_markdown":"First"`) {
		t.Fatalf("manual creation lost human authority: %s %v", initialAuthority.AuthorizationContext, err)
	}
	updated, err := r.updateInboxTask(ctx, model.UpdateInboxTaskInput{
		TaskID: id, ExpectedRevision: 1, ExpectedGeneration: 1, Title: stringAddress("Ready"),
		TaskDocument: stringAddress("Second"), ExpectedTaskDocumentDigest: &digest, ClientMutationID: "update-lifecycle",
	})
	if err != nil {
		t.Fatal(err)
	}
	savedAuthority, err := r.Store.Task(ctx, id)
	if err != nil || !strings.Contains(savedAuthority.AuthorizationContext, `"task_document_markdown":"Second"`) {
		t.Fatalf("manual edit lost human authority: %s %v", savedAuthority.AuthorizationContext, err)
	}
	replay, err := r.updateInboxTask(ctx, model.UpdateInboxTaskInput{
		TaskID: id, ExpectedRevision: 1, ExpectedGeneration: 1, Title: stringAddress("Ready"),
		TaskDocument: stringAddress("Second"), ExpectedTaskDocumentDigest: &digest, ClientMutationID: "update-lifecycle",
	})
	if err != nil || replay.EventCursor != updated.EventCursor || replay.Task.TaskDocument != "Second" {
		t.Fatalf("update replay = %#v, %v", replay, err)
	}
	queued, err := r.queueTask(ctx, model.QueueTaskInput{TaskID: id, ExpectedRevision: 2, ExpectedGeneration: 1, ClientMutationID: "queue-lifecycle"})
	if err != nil {
		t.Fatal(err)
	}
	if queued.Task.Stage.Key != "queue" || queued.Task.CurrentRun == nil || len(queued.Task.Runs) != 1 {
		t.Fatalf("queued Task = %#v", queued.Task)
	}
	cancelled, err := r.cancelTask(ctx, model.CancelTaskInput{TaskID: id, ExpectedRevision: 3, ExpectedGeneration: 1, Reason: stringAddress("   "), ClientMutationID: "cancel-lifecycle"})
	if err != nil {
		t.Fatal(err)
	}
	cancelReplay, err := r.cancelTask(ctx, model.CancelTaskInput{TaskID: id, ExpectedRevision: 3, ExpectedGeneration: 1, ClientMutationID: "cancel-lifecycle"})
	if err != nil || cancelReplay.EventCursor != cancelled.EventCursor {
		t.Fatalf("cancel replay = %#v, %v", cancelReplay, err)
	}
	history, err := r.taskHistory(ctx, personalWorkspaceID, nil, nil, nil, nil)
	if err != nil || len(history.Edges) != 1 || history.Edges[0].Node.Stage.Key != "cancelled" {
		t.Fatalf("Task history = %#v, %v", history, err)
	}
	reopened, err := r.reopenTask(ctx, model.ReopenTaskInput{
		TaskID: id, ExpectedRevision: 4, ExpectedGeneration: 2, FeedbackMarkdown: "Try again",
		RequestMarkdown: stringAddress("Third"), ClientMutationID: "reopen-lifecycle",
	})
	if err != nil {
		t.Fatal(err)
	}
	stored, readErr := home.ReadTaskDocument(r.home, id)
	if readErr != nil || stored.Content != "Third" || reopened.Task.Generation != 3 || len(reopened.Task.Messages) != 1 {
		t.Fatalf("reopened Task = %#v, document = %#v, %v", reopened.Task, stored, readErr)
	}
	overview, err := r.tasksOverview(ctx, personalWorkspaceID, nil)
	if err != nil || len(overview.RecentTasks.Edges) != 1 || overview.RecentTasks.Edges[0].Node.CurrentRun == nil {
		t.Fatalf("Tasks overview = %#v, %v", overview, err)
	}
	actions := validTaskActions(store.Task{StageKey: "waiting"}, &store.TaskGate{Kind: "approval"})
	if len(actions) != 2 || actions[0] != model.ValidTaskActionAnswer || actions[1] != model.ValidTaskActionCancel {
		t.Fatalf("approval actions = %#v", actions)
	}
}

func TestTaskDetailPreservesConversationOrigin(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	id := "task:00000000000000000000000000000001"
	if _, err := home.CreatePendingTaskDocument(r.home, id, "Chat request"); err != nil {
		t.Fatal(err)
	}
	command, err := newTaskCommand("capture_task", "conversation-source", id)
	if err != nil {
		t.Fatal(err)
	}
	conversation, err := r.Store.EnsurePrimaryConversation(ctx, "openrouter", "", time.Now())
	if err != nil {
		t.Fatal(err)
	}
	turn, item, err := r.Store.BeginConversationTurn(ctx, conversation.ID, "Chat request", nil, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	options := store.TaskCreateOptions{Source: store.ArtifactSource{ConversationID: conversation.ID, TurnID: turn.ID, ItemID: item.ID}}
	if _, err := r.Store.CreateTaskWithOptions(ctx, id, "From Chat", command, options, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(r.home, id); err != nil {
		t.Fatal(err)
	}
	detail, err := r.task(ctx, id)
	if err != nil || detail.Source.ConversationID == nil || *detail.Source.ConversationID != options.Source.ConversationID {
		t.Fatalf("Task conversation source was not preserved: %#v, %v", detail, err)
	}
}

func TestTaskDocumentReplayDoesNotOverwriteNewerEdit(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	captured, err := r.captureTask(ctx, model.CaptureTaskInput{
		WorkspaceID: personalWorkspaceID, Title: "Draft", TaskDocument: "First",
		ExecutorAgentID: stringAddress("agent:task-executor"), ClientMutationID: "capture-stale-stage",
	})
	if err != nil {
		t.Fatal(err)
	}
	first := model.UpdateInboxTaskInput{
		TaskID: captured.Task.TaskID, ExpectedRevision: 1, ExpectedGeneration: 1,
		TaskDocument: stringAddress("Second"), ExpectedTaskDocumentDigest: &captured.Task.TaskDocumentDigest,
		ClientMutationID: "first-staged-edit",
	}
	command, err := newTaskCommand("update_inbox_task", first.ClientMutationID, first)
	if err != nil {
		t.Fatal(err)
	}
	stage, err := home.PrepareTaskDocumentReplace(r.home, first.TaskID, *first.ExpectedTaskDocumentDigest, *first.TaskDocument, command.RequestDigest)
	if err != nil {
		t.Fatal(err)
	}
	committed, err := r.Store.UpdateInboxTask(ctx, first.TaskID, 1, 1, store.TaskUpdate{DocumentDigest: stage.Document.Digest}, command, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	newer, err := r.updateInboxTask(ctx, model.UpdateInboxTaskInput{
		TaskID: first.TaskID, ExpectedRevision: 2, ExpectedGeneration: 1,
		TaskDocument: stringAddress("Third"), ExpectedTaskDocumentDigest: first.ExpectedTaskDocumentDigest,
		ClientMutationID: "newer-edit",
	})
	if err != nil {
		t.Fatal(err)
	}
	replay, err := r.updateInboxTask(ctx, first)
	current, readErr := home.ReadTaskDocument(r.home, first.TaskID)
	wantCursor, _ := store.EncodeWorkEventCursor(committed.Event.ID)
	if err != nil || readErr != nil || replay.EventCursor != wantCursor || current.Content != "Third" || newer.Task.Revision != 3 {
		t.Fatalf("stale replay = %#v, current = %#v, %v, %v", replay, current, err, readErr)
	}
}

func stringAddress(value string) *string { return &value }
