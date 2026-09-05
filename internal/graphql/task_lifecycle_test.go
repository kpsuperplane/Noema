package graphql

import (
	"context"
	"testing"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
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
	id, digest := captured.Task.TaskID, captured.Task.TaskDocumentDigest
	updated, err := r.updateInboxTask(ctx, model.UpdateInboxTaskInput{
		TaskID: id, ExpectedRevision: 1, ExpectedGeneration: 1, Title: stringAddress("Ready"),
		TaskDocument: stringAddress("Second"), ExpectedTaskDocumentDigest: &digest, ClientMutationID: "update-lifecycle",
	})
	if err != nil {
		t.Fatal(err)
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
	_, err = r.cancelTask(ctx, model.CancelTaskInput{TaskID: id, ExpectedRevision: 3, ExpectedGeneration: 1, ClientMutationID: "cancel-lifecycle"})
	if err != nil {
		t.Fatal(err)
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
}

func stringAddress(value string) *string { return &value }
